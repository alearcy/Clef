use crate::protocol::{Frame, MAX_STATE, Midi, Reply, Request, Result, validate_block};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, TryLockError},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const MAX_PACKET: usize = MAX_STATE * 4 + 8192;

pub fn send<T: Serialize>(stream: &mut impl Write, value: &T) -> Result<()> {
    let data = serde_json::to_vec(value)?;
    if data.len() > MAX_PACKET {
        return Err("IPC packet exceeds limit".into());
    }
    stream.write_all(&(data.len() as u32).to_le_bytes())?;
    stream.write_all(&data)?;
    Ok(())
}

pub fn receive<T: DeserializeOwned>(stream: &mut impl Read) -> Result<T> {
    receive_limited(stream, MAX_PACKET)
}

pub fn receive_audio<T: DeserializeOwned>(stream: &mut impl Read) -> Result<T> {
    receive_limited(stream, 128 * 1024)
}

fn receive_limited<T: DeserializeOwned>(stream: &mut impl Read, limit: usize) -> Result<T> {
    let mut header = [0; 4];
    stream.read_exact(&mut header)?;
    let length = u32::from_le_bytes(header) as usize;
    if length == 0 || length > limit {
        return Err("invalid IPC packet length".into());
    }
    let mut data = vec![0; length];
    stream.read_exact(&mut data)?;
    Ok(serde_json::from_slice(&data)?)
}

/// Recomputes the remaining timeout on every read/write, including short transfers.
struct DeadlineStream<'a> {
    stream: &'a mut UnixStream,
    deadline: Instant,
}

impl DeadlineStream<'_> {
    fn remaining(&self) -> std::io::Result<Duration> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "worker exchange deadline exceeded",
                )
            })
    }
}
impl<'a> DeadlineStream<'a> {
    fn new(stream: &'a mut UnixStream, deadline: Instant) -> Result<Self> {
        // Darwin can reject SO_RCVTIMEO after a peer sends its failure reply and closes.
        stream.set_nonblocking(true)?;
        Ok(Self { stream, deadline })
    }
    fn wait(&self, writable: bool) -> std::io::Result<()> {
        let mut descriptor = libc::pollfd {
            fd: self.stream.as_raw_fd(),
            events: if writable {
                libc::POLLOUT
            } else {
                libc::POLLIN
            },
            revents: 0,
        };
        let timeout = self
            .remaining()?
            .as_millis()
            .saturating_add(1)
            .min(i32::MAX as u128) as i32;
        // poll wakes on socket readiness; a periodic sleep can miss the audio budget.
        let result = unsafe { libc::poll(&mut descriptor, 1, timeout) };
        if result < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
        self.remaining()?;
        Ok(())
    }
}
impl Read for DeadlineStream<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        loop {
            self.remaining()?;
            match self.stream.read(bytes) {
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => self.wait(false)?,
                result => return result,
            }
        }
    }
}
impl Write for DeadlineStream<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        loop {
            self.remaining()?;
            match self.stream.write(bytes) {
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => self.wait(true)?,
                result => return result,
            }
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn exchange(stream: &mut UnixStream, request: &Request, timeout: Duration) -> Result<Reply> {
    let mut stream = DeadlineStream::new(stream, Instant::now() + timeout)?;
    send(&mut stream, request)?;
    if matches!(request, Request::Process { .. } | Request::SuspendAudio) {
        receive_audio(&mut stream)
    } else {
        receive(&mut stream)
    }
}

#[derive(Clone)]
pub struct ControlConnection(Arc<Mutex<UnixStream>>);

impl ControlConnection {
    fn call_until(&self, request: &Request, deadline: Instant) -> Result<Reply> {
        let mut stream = loop {
            match self.0.try_lock() {
                Ok(stream) => break stream,
                Err(TryLockError::Poisoned(_)) => return Err("control connection poisoned".into()),
                Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_micros(100))
                }
                Err(_) => return Err("control connection deadline exceeded".into()),
            }
        };
        let mut stream = DeadlineStream::new(&mut stream, deadline)?;
        send(&mut stream, request)?;
        let reply: Reply = receive(&mut stream)?;
        if let Some(error) = &reply.error {
            return Err(error.clone().into());
        }
        if reply.latency > crate::protocol::RATE * 10 {
            return Err("worker latency exceeds prototype capacity".into());
        }
        Ok(reply)
    }
}

pub struct Instrument {
    processing_mode: Option<bool>,
    child: Child,
    control: ControlConnection,
    audio: Option<AudioConnection>,
    pub name: String,
    pub latency: u32,
}

struct SocketPath(PathBuf);
impl Drop for SocketPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        if let Some(parent) = self.0.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
}

impl Instrument {
    pub fn load(executable: &Path, path: &Path, state: Option<&[u8]>) -> Result<Self> {
        Self::load_selected(executable, path, "", crate::protocol::RATE, state)
    }

    pub fn load_selected(
        executable: &Path,
        path: &Path,
        selected: &str,
        rate: u32,
        state: Option<&[u8]>,
    ) -> Result<Self> {
        Self::load_selected_cancellable(
            executable,
            path,
            selected,
            rate,
            state,
            &std::sync::atomic::AtomicBool::new(false),
        )
    }

    pub fn load_selected_cancellable(
        executable: &Path,
        path: &Path,
        selected: &str,
        rate: u32,
        state: Option<&[u8]>,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<Self> {
        if !(8000..=384000).contains(&rate) {
            return Err("unsupported sample rate".into());
        }
        let path = if path == Path::new("@fixture") {
            path.to_owned()
        } else {
            path.canonicalize()?
        };
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        use std::os::unix::fs::DirBuilderExt;
        // macOS sockaddr_un cannot hold the long per-user TMPDIR plus this private name.
        let directory =
            Path::new("/private/tmp").join(format!("clef-{}-{nonce}-{serial}", std::process::id()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(|e| {
                format!(
                    "create worker socket directory {}: {e}",
                    directory.display()
                )
            })?;
        let socket = SocketPath(directory.join("rpc"));
        let listener = UnixListener::bind(&socket.0)
            .map_err(|error| format!("bind worker socket {}: {error}", socket.0.display()))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("configure worker listener: {error}"))?;
        let mut child = Command::new(executable)
            .arg("worker")
            .arg(&socket.0)
            .arg(path)
            .arg(selected)
            .arg(rate.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()?;
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut accept = || -> Result<UnixStream> {
            loop {
                if cancel.load(std::sync::atomic::Ordering::Acquire) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("load cancelled".into());
                }
                match listener.accept() {
                    Ok((stream, _)) => {
                        stream.set_nonblocking(false)?;
                        return Ok(stream);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if let Some(status) = child.try_wait()? {
                            return Err(format!("plugin worker exited: {status}").into());
                        }
                        if Instant::now() >= deadline {
                            let _ = child.kill();
                            let _ = child.wait();
                            return Err("plugin worker startup timed out".into());
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(e.into());
                    }
                }
            }
        };
        let streams = (|| -> Result<_> { Ok((accept()?, accept()?)) })();
        let (mut stream, audio) = match streams {
            Ok(streams) => streams,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        let hello: Reply = match DeadlineStream::new(&mut stream, deadline)
            .and_then(|mut stream| receive(&mut stream))
        {
            Ok(reply) => reply,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("receive worker startup reply: {error}").into());
            }
        };
        let mut instrument = Self {
            processing_mode: None,
            child,
            control: ControlConnection(Arc::new(Mutex::new(stream))),
            audio: Some(AudioConnection {
                stream: audio,
                timeout: Duration::from_secs(1),
                latency: 0,
                failed: false,
            }),
            name: String::new(),
            latency: 0,
        };
        if let Some(e) = hello.error {
            return Err(e.into());
        }
        instrument.name = hello.name;
        instrument.latency = hello.latency;
        if let Some(state) = state {
            instrument.restore(state)?;
        }
        Ok(instrument)
    }

    pub fn exited(&mut self) -> bool {
        self.child
            .try_wait()
            .map_or(true, |status| status.is_some())
    }

    pub fn call(&mut self, request: Request) -> Result<Reply> {
        if self.audio.as_ref().is_some_and(|audio| audio.failed) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            return Err("audio connection failed; recreate worker".into());
        }
        if self.audio.is_none()
            && !matches!(
                request,
                Request::Editor { .. } | Request::FixtureFault { .. } | Request::FixtureEdit { .. }
            )
        {
            return Err("stop live playback before this operation".into());
        }
        let result = (|| {
            if matches!(
                request,
                Request::Start { .. } | Request::Stop | Request::Restore { .. } | Request::Quit
            ) {
                let audio = self
                    .audio
                    .as_mut()
                    .ok_or("stop live playback before lifecycle changes")?;
                let reply = exchange(&mut audio.stream, &Request::SuspendAudio, audio.timeout)?;
                if let Some(error) = reply.error {
                    return Err(error.into());
                }
            }
            if matches!(request, Request::Process { .. }) {
                let audio = self
                    .audio
                    .as_mut()
                    .ok_or("audio connection belongs to live playback")?;
                exchange(&mut audio.stream, &request, audio.timeout)
            } else {
                self.control
                    .call_until(&request, Instant::now() + Duration::from_secs(60))
            }
        })();
        let reply = match result {
            Ok(reply) => reply,
            Err(error) => {
                let _ = self.child.kill();
                let _ = self.child.wait();
                return Err(error);
            }
        };
        if let Some(error) = reply.error.as_ref() {
            return Err(error.clone().into());
        }
        if reply.latency > crate::protocol::RATE * 10 {
            let _ = self.child.kill();
            let _ = self.child.wait();
            return Err("worker latency exceeds prototype capacity".into());
        }
        self.latency = reply.latency;
        Ok(reply)
    }

    pub fn start(&mut self, offline: bool) -> Result<()> {
        if self.processing_mode == Some(offline) {
            if self.exited() {
                return Err("plugin worker exited".into());
            }
            return Ok(());
        }
        self.processing_mode = None;
        self.call(Request::Start { offline })?;
        self.processing_mode = Some(offline);
        self.audio
            .as_mut()
            .ok_or("audio connection unavailable")?
            .timeout = Duration::from_secs(if offline { 60 } else { 1 });
        Ok(())
    }

    pub fn process(
        &mut self,
        position: u64,
        events: &[Midi],
        frames: usize,
    ) -> Result<(Vec<Frame>, bool)> {
        validate_block(frames, events)?;
        let reply = self.call(Request::Process {
            position,
            events: events.to_vec(),
            frames,
        })?;
        if reply.frames.len() != frames || reply.frames.iter().flatten().any(|v| !v.is_finite()) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            return Err("invalid or non-finite worker audio reply".into());
        }
        Ok((reply.frames, reply.restart))
    }

    pub fn control_connection(&self) -> ControlConnection {
        self.control.clone()
    }

    pub fn take_audio(&mut self) -> Result<AudioConnection> {
        self.audio
            .take()
            .ok_or_else(|| "audio connection already in use".into())
    }

    pub fn return_audio(&mut self, audio: AudioConnection) {
        self.latency = audio.latency;
        self.audio = Some(audio);
    }

    pub fn stop(&mut self) -> Result<()> {
        self.processing_mode = None;
        self.call(Request::Stop)?;
        Ok(())
    }
    pub fn editor(&mut self, open: bool) -> Result<()> {
        self.call(Request::Editor { open })?;
        Ok(())
    }
    pub fn save(&mut self) -> Result<Vec<u8>> {
        let state = self.call(Request::Save)?.state;
        if state.len() > MAX_STATE {
            return Err("plugin state exceeds limit".into());
        }
        Ok(state)
    }
    pub fn restore(&mut self, state: &[u8]) -> Result<()> {
        if state.len() > MAX_STATE {
            return Err("plugin state exceeds limit".into());
        }
        self.processing_mode = None;
        self.call(Request::Restore {
            state: state.to_vec(),
        })?;
        Ok(())
    }
}

pub struct AudioConnection {
    stream: UnixStream,
    timeout: Duration,
    pub latency: u32,
    failed: bool,
}

impl AudioConnection {
    pub fn process(&mut self, position: u64, events: &[Midi], frames: usize) -> Result<Reply> {
        if self.failed {
            return Err("audio connection failed; recreate worker".into());
        }
        validate_block(frames, events)?;
        let result = exchange(
            &mut self.stream,
            &Request::Process {
                position,
                events: events.to_vec(),
                frames,
            },
            self.timeout,
        );
        let reply = match result {
            Ok(reply) => reply,
            Err(error) => {
                self.failed = true;
                let _ = self.stream.shutdown(std::net::Shutdown::Both);
                return Err(error);
            }
        };
        if let Some(error) = &reply.error {
            return Err(error.clone().into());
        }
        if reply.frames.len() != frames
            || reply.frames.iter().flatten().any(|v| !v.is_finite())
            || reply.latency > crate::protocol::RATE * 10
        {
            return Err("invalid worker audio reply".into());
        }
        self.latency = reply.latency;
        Ok(reply)
    }

    pub fn reconfigure(&mut self, control: &ControlConnection) -> Result<u32> {
        let result = (|| {
            let deadline = Instant::now() + Duration::from_secs(1);
            self.suspend()?;
            control.call_until(&Request::Stop, deadline)?;
            let reply = control.call_until(&Request::Start { offline: false }, deadline)?;
            self.latency = reply.latency;
            Ok(reply.latency)
        })();
        if result.is_err() {
            self.failed = true;
            let _ = self.stream.shutdown(std::net::Shutdown::Both);
        }
        result
    }

    pub fn suspend(&mut self) -> Result<()> {
        if self.failed {
            return Err("audio connection failed; recreate worker".into());
        }
        let reply = exchange(&mut self.stream, &Request::SuspendAudio, self.timeout)?;
        if let Some(error) = reply.error {
            return Err(error.into());
        }
        Ok(())
    }
}

impl Drop for Instrument {
    fn drop(&mut self) {
        // Never wait on a plugin during teardown; the owned Child prevents PID-reuse races.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queued_failure_reply_survives_the_peer_closing() {
        let (mut tx, mut rx) = UnixStream::pair().unwrap();
        send(&mut tx, &Reply::failed("deliberate load failure")).unwrap();
        drop(tx);
        let mut deadline =
            DeadlineStream::new(&mut rx, Instant::now() + Duration::from_secs(60)).unwrap();
        let reply: Reply = receive(&mut deadline).unwrap();
        assert_eq!(reply.error.as_deref(), Some("deliberate load failure"));
    }

    #[test]
    fn blocked_writes_cannot_extend_the_whole_exchange_deadline() {
        let (mut tx, _rx) = UnixStream::pair().unwrap();
        let started = Instant::now();
        let mut stream =
            DeadlineStream::new(&mut tx, started + Duration::from_millis(100)).unwrap();
        let error = stream.write_all(&vec![0; 4 * 1024 * 1024]).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_millis(250));
    }

    #[test]
    fn trickling_data_cannot_extend_the_whole_exchange_deadline() {
        let (mut tx, mut rx) = UnixStream::pair().unwrap();
        let writer = std::thread::spawn(move || {
            for byte in [10, 0, 0, 0, 123, 32, 32, 32] {
                if tx.write_all(&[byte]).is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(30));
            }
        });
        let started = Instant::now();
        let mut stream =
            DeadlineStream::new(&mut rx, started + Duration::from_millis(100)).unwrap();
        assert!(receive::<Reply>(&mut stream).is_err());
        assert!(started.elapsed() < Duration::from_millis(220));
        drop(rx);
        writer.join().unwrap();
    }

    #[test]
    fn malformed_packet_is_rejected_before_allocation() {
        let (mut tx, mut rx) = UnixStream::pair().unwrap();
        tx.write_all(&u32::MAX.to_le_bytes()).unwrap();
        assert!(receive::<Request>(&mut rx).is_err());
    }
}
