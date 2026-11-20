//! Instrument restarts served off the broker thread (T37 L2).
//!
//! A restart (Suspend, Stop, Start under 1 s limits each) can take about 3 s. The broker hands
//! the instrument's audio connection to a short-lived helper and keeps serving its queue; it
//! takes the connection back when the helper finishes.

use clef_plugin_host::{
    protocol::Result,
    rpc::{AudioConnection, ControlConnection},
};
use std::{
    sync::mpsc,
    thread::{self, JoinHandle},
};

type Restarted = Option<(AudioConnection, Result<u32>)>;

/// An instrument audio connection held by its broker or, while restarting, by a helper.
pub(crate) struct InstrumentAudio {
    audio: Option<AudioConnection>,
    helper: Option<JoinHandle<Restarted>>,
    finished: Option<Result<u32>>,
}

impl InstrumentAudio {
    pub(crate) fn new(audio: AudioConnection) -> Self {
        Self {
            audio: Some(audio),
            helper: None,
            finished: None,
        }
    }

    /// The connection, unless a restart holds it.
    pub(crate) fn ready(&mut self) -> Option<&mut AudioConnection> {
        self.audio.as_mut()
    }

    /// Start a restart on a helper thread. A request while one runs is ignored. If the helper
    /// cannot start, the restart runs here so the connection is never lost.
    pub(crate) fn begin_restart(&mut self, control: &ControlConnection) {
        let Some(audio) = self.audio.take() else {
            return;
        };
        let (send, receive) = mpsc::channel::<AudioConnection>();
        let control_for_helper = control.clone();
        let spawned = thread::Builder::new()
            .name("clef-instrument-reconfigure".into())
            .spawn(move || {
                let mut audio = receive.recv().ok()?;
                let result = audio.reconfigure(&control_for_helper);
                Some((audio, result))
            });
        match spawned {
            Ok(helper) => match send.send(audio) {
                Ok(()) => self.helper = Some(helper),
                Err(mpsc::SendError(returned)) => self.restart_here(returned, control),
            },
            Err(_) => self.restart_here(audio, control),
        }
    }

    fn restart_here(&mut self, mut audio: AudioConnection, control: &ControlConnection) {
        self.finished = Some(audio.reconfigure(control));
        self.audio = Some(audio);
    }

    /// The new latency, or the restart error, once a restart has finished.
    pub(crate) fn poll(&mut self) -> Option<Result<u32>> {
        if let Some(result) = self.finished.take() {
            return Some(result);
        }
        if !self.helper.as_ref().is_some_and(JoinHandle::is_finished) {
            return None;
        }
        Some(self.join())
    }

    fn join(&mut self) -> Result<u32> {
        match self.helper.take().map(JoinHandle::join) {
            Some(Ok(Some((audio, result)))) => {
                self.audio = Some(audio);
                result
            }
            _ => Err("instrument restart helper failed".into()),
        }
    }

    /// The connection for return to the worker, waiting for a running restart.
    pub(crate) fn into_inner(mut self) -> Option<AudioConnection> {
        if self.helper.is_some() {
            let _ = self.join();
        }
        self.audio
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clef_plugin_host::{
        protocol::{BLOCK, RATE, Request},
        rpc::Instrument,
    };
    use std::{
        path::Path,
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };

    #[test]
    #[ignore = "requires the built Clef worker and macOS WindowServer"]
    fn an_instrument_restart_runs_on_a_helper_while_the_broker_keeps_going() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut worker = Instrument::load_selected_cancellable(
            &root.join("target/debug/clef"),
            Path::new("@fixture"),
            "",
            RATE,
            None,
            &AtomicBool::new(false),
        )
        .unwrap();
        worker.start(false).unwrap();
        let control = worker.control_connection();
        let mut audio = InstrumentAudio::new(worker.take_audio().unwrap());
        worker.call(Request::FixtureRestart).unwrap();
        let reply = audio.ready().unwrap().process(0, &[], BLOCK).unwrap();
        assert!(reply.restart, "the fixture requests a restart");
        let started = Instant::now();
        audio.begin_restart(&control);
        assert!(
            started.elapsed() < Duration::from_millis(50),
            "the broker is not blocked"
        );
        let mut free_blocks = 0;
        let latency = loop {
            assert!(
                started.elapsed() < Duration::from_secs(4),
                "restart did not finish"
            );
            if let Some(result) = audio.poll() {
                break result.unwrap();
            }
            assert!(audio.ready().is_none(), "the helper holds the connection");
            audio.begin_restart(&control);
            free_blocks += 1;
            std::thread::sleep(Duration::from_millis(1));
        };
        assert!(free_blocks >= 1);
        assert_eq!(latency, worker.latency);
        let reply = audio
            .ready()
            .expect("the connection is back")
            .process(BLOCK as u64, &[], BLOCK)
            .unwrap();
        assert!(!reply.restart);
        worker.return_audio(audio.into_inner().unwrap());
        worker.stop().unwrap();
    }
}
