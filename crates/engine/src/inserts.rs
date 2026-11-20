use crate::graph::Delay;
use clef_plugin_host::{
    protocol::{BLOCK, Frame, RATE, Result},
    rpc::{AudioConnection, ControlConnection, Instrument},
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

/// Bound the per-chain compensation memory, independently of the number of slots.
pub(crate) const MAX_CHAIN_LATENCY: u32 = RATE * 10;

pub struct Effect {
    audio: Mutex<Option<AudioConnection>>,
    control: ControlConnection,
    latency: AtomicU32,
    error: Mutex<Option<String>>,
    failed: AtomicBool,
    // A latency or restart request is served by a helper thread holding `audio`; the
    // stage outputs its delay-aligned dry signal until the helper publishes the result.
    reconfiguring: AtomicBool,
    reconfigured: AtomicBool,
    late: LateBlocks,
}

/// Blocks in one late-share window: about 2 s of processed audio.
const LATE_WINDOW: u32 = (2 * RATE).div_ceil(BLOCK as u32);
/// A window with more than this share of late blocks (per mille) marks the effect Late.
const LATE_LIMIT: u32 = 100;

/// Late-block share of the last completed window. The counters are written only by the
/// processing thread; the share is published once per window for the session to read.
#[derive(Default)]
struct LateBlocks {
    blocks: AtomicU32,
    late: AtomicU32,
    share: AtomicU32,
}

impl LateBlocks {
    fn record(&self, late: bool) {
        let blocks = self.blocks.load(Ordering::Relaxed) + 1;
        let count = self.late.load(Ordering::Relaxed) + u32::from(late);
        if blocks < LATE_WINDOW {
            self.blocks.store(blocks, Ordering::Relaxed);
            self.late.store(count, Ordering::Relaxed);
            return;
        }
        self.share.store(count * 1000 / blocks, Ordering::Relaxed);
        self.blocks.store(0, Ordering::Relaxed);
        self.late.store(0, Ordering::Relaxed);
    }

    /// The last window's late share in percent, when it is above the limit.
    fn percent(&self) -> Option<u8> {
        let share = self.share.load(Ordering::Relaxed);
        (share > LATE_LIMIT).then(|| share.div_ceil(10).min(100) as u8)
    }
}

impl Effect {
    pub fn activate(worker: &mut Instrument) -> Result<Arc<Self>> {
        worker.start(false)?;
        Ok(Arc::new(Self {
            audio: Mutex::new(Some(worker.take_audio()?)),
            control: worker.control_connection(),
            latency: AtomicU32::new(worker.latency),
            error: Mutex::new(None),
            failed: AtomicBool::new(false),
            reconfiguring: AtomicBool::new(false),
            reconfigured: AtomicBool::new(false),
            late: LateBlocks::default(),
        }))
    }

    pub fn error(&self) -> Option<String> {
        self.error.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// The share of late blocks in percent while the effect is Late: more than 10 % of the
    /// blocks in the last window of about 2 s missed their deadline. Late is not a failure.
    pub fn late_percent(&self) -> Option<u8> {
        self.late.percent()
    }

    fn fail(&self, error: String) {
        *self.error.lock().unwrap_or_else(|e| e.into_inner()) = Some(error);
        self.failed.store(true, Ordering::Release);
    }

    /// Restart the plugin off the processing thread. Lifecycle limits are 1 s per step.
    fn begin_reconfigure(self: &Arc<Self>) {
        if self.reconfiguring.swap(true, Ordering::AcqRel) {
            return;
        }
        let effect = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name("clef-insert-reconfigure".into())
            .spawn(move || {
                let result = {
                    let mut audio = effect.audio.lock().unwrap_or_else(|e| e.into_inner());
                    audio
                        .as_mut()
                        .map(|audio| audio.reconfigure(&effect.control))
                };
                match result {
                    Some(Ok(latency)) => {
                        effect.latency.store(latency, Ordering::Release);
                        effect.reconfigured.store(true, Ordering::Release);
                    }
                    Some(Err(error)) => effect.fail(error.to_string()),
                    // Deactivated meanwhile; the next activation starts fresh.
                    None => {}
                }
                effect.reconfiguring.store(false, Ordering::Release);
            });
        if spawned.is_err() {
            self.fail("could not start the insert reconfiguration thread".into());
            self.reconfiguring.store(false, Ordering::Release);
        }
    }

    pub fn deactivate(&self, worker: &mut Instrument) -> Result<()> {
        let mut audio = self.audio.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(connection) = audio.take() {
            worker.return_audio(connection);
            worker.stop()?;
        }
        Ok(())
    }

    /// Capture state while the effect stays attached to its processing thread. The stage
    /// outputs dry while the plugin is briefly stopped for `Save`.
    pub fn capture(&self, worker: &mut Instrument) -> Result<Vec<u8>> {
        let mut audio = self.audio.lock().unwrap_or_else(|e| e.into_inner());
        match audio.as_mut() {
            Some(audio) => audio.with_stopped(&self.control, || self.control.save()),
            None => worker.save(),
        }
    }
}

pub struct Slot {
    pub effect: Arc<Effect>,
    pub bypassed: bool,
}

#[derive(Default)]
pub struct Chain(Vec<Slot>);

impl Chain {
    pub fn prepare(slots: Vec<Slot>) -> Result<Self> {
        let chain = Self(slots);
        if chain.latency() > MAX_CHAIN_LATENCY {
            return Err("Insert chain exceeds available delay compensation.".into());
        }
        Ok(chain)
    }

    pub(crate) fn latency(&self) -> u32 {
        self.0.iter().fold(0_u32, |total, slot| {
            total.saturating_add(slot.effect.latency.load(Ordering::Acquire))
        })
    }

    pub(crate) fn same_effects(&self, other: &Self) -> bool {
        self.0.len() == other.0.len()
            && self
                .0
                .iter()
                .zip(&other.0)
                .all(|(a, b)| Arc::ptr_eq(&a.effect, &b.effect))
    }
}

struct Stage {
    latency: u32,
    dry: Delay,
    failed: bool,
    bypassed: bool,
    mix: f32,
    target: f32,
    step: f32,
    remaining: u32,
}

impl Stage {
    // A bypass toggle crossfades over 5 ms at 48 kHz. Keeping the dry delay
    // running and preserving the stages avoids the click from rebuilding the
    // compensation delay and switching wet/dry in one sample.
    const RAMP: u32 = 240;

    fn set_bypassed(&mut self, bypassed: bool) {
        self.bypassed = bypassed;
        let target = if bypassed { 0.0 } else { 1.0 };
        if target != self.target {
            self.target = target;
            self.remaining = Self::RAMP;
            self.step = (target - self.mix) / Self::RAMP as f32;
        }
    }

    fn next_mix(&mut self) -> f32 {
        let mix = self.mix;
        if self.remaining != 0 {
            self.remaining -= 1;
            if self.remaining == 0 {
                self.mix = self.target;
            } else {
                self.mix += self.step;
            }
        }
        mix
    }
}

#[derive(Default)]
pub(crate) struct Processor {
    chain: Option<Arc<Chain>>,
    stages: Vec<Stage>,
    latency: u32,
}

impl Processor {
    pub(crate) fn set_chain(&mut self, chain: Option<Arc<Chain>>) -> bool {
        if self
            .chain
            .as_ref()
            .zip(chain.as_ref())
            .is_some_and(|(a, b)| Arc::ptr_eq(a, b))
            || (self.chain.is_none() && chain.is_none())
        {
            return false;
        }
        if let (Some(current), Some(next)) = (&self.chain, &chain)
            && current.same_effects(next)
        {
            for (stage, slot) in self.stages.iter_mut().zip(&next.0) {
                stage.set_bypassed(slot.bypassed);
            }
            self.chain = chain;
            return false;
        }
        self.latency = 0;
        self.stages.clear();
        if let Some(chain) = &chain {
            for slot in &chain.0 {
                let latency = slot.effect.latency.load(Ordering::Acquire);
                let bypassed = slot.bypassed;
                self.latency += latency;
                self.stages.push(Stage {
                    latency,
                    dry: Delay::new(latency),
                    failed: slot.effect.error().is_some(),
                    bypassed,
                    mix: if bypassed { 0.0 } else { 1.0 },
                    target: if bypassed { 0.0 } else { 1.0 },
                    step: 0.0,
                    remaining: 0,
                });
            }
        }
        self.chain = chain;
        true
    }

    pub(crate) fn latency(&self) -> u32 {
        self.latency
    }

    /// Return true when the caller must retire the current compensation generation.
    pub(crate) fn process(
        &mut self,
        position: u64,
        frames: &mut [Frame; BLOCK],
        deadline: Instant,
    ) -> bool {
        let Some(chain) = &self.chain else {
            return false;
        };
        for (slot, stage) in chain.0.iter().zip(&mut self.stages) {
            let dry = frames.map(|frame| stage.dry.apply(frame));
            stage.set_bypassed(slot.bypassed);
            stage.failed |= slot.effect.failed.load(Ordering::Acquire);
            if !stage.failed && slot.effect.reconfigured.swap(false, Ordering::AcqRel) {
                let latency = slot.effect.latency.load(Ordering::Acquire);
                let total = self.latency - stage.latency + latency;
                if total > MAX_CHAIN_LATENCY {
                    slot.effect
                        .fail("Effect latency exceeds available delay compensation.".to_owned());
                    stage.failed = true;
                } else {
                    stage.latency = latency;
                    stage.dry = Delay::new(latency);
                    self.latency = total;
                    return true;
                }
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if stage.failed
                || remaining.is_zero()
                || stage.bypassed && stage.mix == 0.0
                || slot.effect.reconfiguring.load(Ordering::Acquire)
            {
                *frames = dry;
                continue;
            }
            let Ok(mut audio) = slot.effect.audio.try_lock() else {
                *frames = dry;
                continue;
            };
            let Some(audio) = audio.as_mut() else {
                *frames = dry;
                continue;
            };
            let result = audio
                .process_effect(position, frames, remaining.max(Duration::from_micros(100)))
                .map(|reply| {
                    slot.effect.late.record(reply.is_none());
                    // A late block keeps the connection; its reply is discarded on arrival.
                    let Some(reply) = reply else {
                        *frames = dry;
                        return;
                    };
                    if !reply.restart && reply.latency == stage.latency {
                        if stage.mix == 1.0 && stage.target == 1.0 {
                            frames.copy_from_slice(&reply.frames);
                        } else {
                            for (index, wet) in reply.frames.iter().enumerate() {
                                let mix = stage.next_mix();
                                frames[index] = [
                                    wet[0] * mix + dry[index][0] * (1.0 - mix),
                                    wet[1] * mix + dry[index][1] * (1.0 - mix),
                                ];
                            }
                        }
                        return;
                    }
                    // Output dry while a helper restarts the plugin; the structural
                    // restart follows when the helper publishes the new latency.
                    *frames = dry;
                    slot.effect.begin_reconfigure();
                });
            match result {
                Ok(()) => {}
                Err(error) => {
                    slot.effect.fail(error.to_string());
                    stage.failed = true;
                    *frames = dry;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clef_plugin_host::protocol::Request;
    use std::{path::Path, sync::atomic::AtomicBool};

    fn window(late: &LateBlocks, late_blocks: u32) {
        for block in 0..LATE_WINDOW {
            late.record(block < late_blocks);
        }
    }

    #[test]
    fn often_late_blocks_mark_the_effect_late_until_a_clean_window() {
        let late = LateBlocks::default();
        window(&late, 1);
        assert_eq!(late.percent(), None, "a single late block is tolerated");
        window(&late, LATE_WINDOW / 10);
        assert_eq!(late.percent(), None, "exactly 10 % is not Late");
        window(&late, LATE_WINDOW / 4);
        assert_eq!(late.percent(), Some(25));
        for _ in 0..LATE_WINDOW - 1 {
            late.record(false);
        }
        assert_eq!(
            late.percent(),
            Some(25),
            "the state holds until the window ends"
        );
        late.record(false);
        assert_eq!(late.percent(), None, "a clean window clears Late");
    }

    #[test]
    #[ignore = "requires the built Clef worker, CLAP insert fixture and macOS WindowServer"]
    fn serial_effects_run_in_order_and_bypass_skips_only_its_own_stage() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let executable = root.join("target/debug/clef");
        let load = || {
            Instrument::load_effect(
                &executable,
                Path::new("@fixture"),
                "",
                RATE,
                None,
                &AtomicBool::new(false),
            )
            .unwrap()
        };
        let mut first_worker = load();
        let mut second_worker = load();
        let first = Effect::activate(&mut first_worker).unwrap();
        let second = Effect::activate(&mut second_worker).unwrap();
        let mut processor = Processor::default();
        processor.set_chain(Some(Arc::new(
            Chain::prepare(vec![
                Slot {
                    effect: first.clone(),
                    bypassed: false,
                },
                Slot {
                    effect: second.clone(),
                    bypassed: false,
                },
            ])
            .unwrap(),
        )));
        let mut frames = [[0.0; 2]; BLOCK];
        frames[0] = [0.5, -0.25];
        assert!(!processor.process(0, &mut frames, Instant::now() + Duration::from_secs(2)));
        assert_eq!(frames[0], [0.125, -0.0625]);

        assert!(
            !processor.set_chain(Some(Arc::new(
                Chain::prepare(vec![
                    Slot {
                        effect: first,
                        bypassed: true,
                    },
                    Slot {
                        effect: second,
                        bypassed: false,
                    },
                ])
                .unwrap(),
            )))
        );
        for _ in 0..3 {
            let mut frames = [[0.0; 2]; BLOCK];
            assert!(!processor.process(0, &mut frames, Instant::now() + Duration::from_secs(2)));
        }
        let mut frames = [[0.0; 2]; BLOCK];
        frames[0] = [0.5, -0.25];
        assert!(!processor.process(0, &mut frames, Instant::now() + Duration::from_secs(2)));
        assert_eq!(frames[0], [0.25, -0.125]);
    }

    #[test]
    #[ignore = "requires the built Clef worker and macOS WindowServer"]
    fn bypass_crossfades_instead_of_stepping_between_wet_and_dry() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let executable = root.join("target/debug/clef");
        let mut worker = Instrument::load_effect(
            &executable,
            Path::new("@fixture"),
            "",
            RATE,
            None,
            &AtomicBool::new(false),
        )
        .unwrap();
        let effect = Effect::activate(&mut worker).unwrap();
        let active = Arc::new(
            Chain::prepare(vec![Slot {
                effect: effect.clone(),
                bypassed: false,
            }])
            .unwrap(),
        );
        let mut processor = Processor::default();
        assert!(processor.set_chain(Some(active)));
        let mut output = Vec::new();
        let mut phase = 0.0_f32;
        for block in 0..16 {
            if block == 8 {
                assert!(
                    !processor.set_chain(Some(Arc::new(
                        Chain::prepare(vec![Slot {
                            effect: effect.clone(),
                            bypassed: true,
                        }])
                        .unwrap(),
                    )))
                );
            }
            let mut frames = [[0.0; 2]; BLOCK];
            for frame in &mut frames {
                let sample = 0.5 * phase.sin();
                *frame = [sample, sample];
                phase += std::f32::consts::TAU * 1_000.0 / RATE as f32;
            }
            assert!(!processor.process(
                block as u64 * BLOCK as u64,
                &mut frames,
                Instant::now() + Duration::from_secs(2)
            ));
            output.extend_from_slice(&frames);
        }
        let peak = |block: usize| {
            output[block * BLOCK..(block + 1) * BLOCK]
                .iter()
                .map(|frame| frame[0].abs())
                .fold(0.0, f32::max)
        };
        assert!(peak(6) < 0.3, "wet peak {}", peak(6));
        assert!(peak(15) > 0.45, "dry peak {}", peak(15));
        let step = output
            .windows(2)
            .map(|pair| (pair[1][0] - pair[0][0]).abs())
            .fold(0.0, f32::max);
        assert!(step < 0.09, "largest sample step {step}");
    }

    #[test]
    #[ignore = "requires the built Clef worker, CLAP insert fixture and macOS WindowServer"]
    fn bypass_and_an_effect_crash_keep_the_dry_path_aligned_and_later_effects_running() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let executable = root.join("target/debug/clef");
        let effect_path = root.join("validation-output/fixtures/ClefInsertCLAP.clap");
        let load = |location: &Path, id, state: Option<&[u8]>| {
            Instrument::load_effect(
                &executable,
                location,
                id,
                RATE,
                state,
                &AtomicBool::new(false),
            )
            .unwrap()
        };
        let mut killer = load(Path::new("@fixture"), "", None);
        let mut delayed = load(
            &effect_path,
            "dev.clef.insert-fixture",
            Some(&257_u32.to_le_bytes()),
        );
        let mut last = load(Path::new("@fixture"), "", None);
        let killer_fx = Effect::activate(&mut killer).unwrap();
        let delayed_fx = Effect::activate(&mut delayed).unwrap();
        let last_fx = Effect::activate(&mut last).unwrap();
        killer.call(Request::FixtureFault { hang: false }).unwrap();

        let mut processor = Processor::default();
        processor.set_chain(Some(Arc::new(
            Chain::prepare(vec![
                Slot {
                    effect: killer_fx.clone(),
                    bypassed: false,
                },
                Slot {
                    effect: delayed_fx,
                    bypassed: true,
                },
                Slot {
                    effect: last_fx.clone(),
                    bypassed: false,
                },
            ])
            .unwrap(),
        )));
        assert_eq!(processor.latency(), 257);
        let mut output = Vec::new();
        for block in 0..3 {
            let mut frames = [[0.0; 2]; BLOCK];
            if block == 0 {
                frames[0] = [0.5, -0.25];
            }
            assert!(!processor.process(
                block * BLOCK as u64,
                &mut frames,
                Instant::now() + Duration::from_secs(2),
            ));
            output.extend(frames);
        }
        assert!(killer_fx.error().is_some());
        assert!(last_fx.error().is_none());
        assert_eq!(output[257], [0.25, -0.125]);
        assert_eq!(output.iter().filter(|&&frame| frame != [0.0; 2]).count(), 1);
    }

    fn fixture_effect() -> (Instrument, Arc<Effect>, Processor) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut worker = Instrument::load_effect(
            &root.join("target/debug/clef"),
            Path::new("@fixture"),
            "",
            RATE,
            None,
            &AtomicBool::new(false),
        )
        .unwrap();
        let effect = Effect::activate(&mut worker).unwrap();
        let mut processor = Processor::default();
        processor.set_chain(Some(Arc::new(
            Chain::prepare(vec![Slot {
                effect: effect.clone(),
                bypassed: false,
            }])
            .unwrap(),
        )));
        (worker, effect, processor)
    }

    fn impulse() -> [Frame; BLOCK] {
        let mut frames = [[0.0; 2]; BLOCK];
        frames[0] = [0.5, -0.25];
        frames
    }

    #[test]
    #[ignore = "requires the built Clef worker and macOS WindowServer"]
    fn a_late_block_outputs_dry_and_keeps_the_effect_running() {
        let (mut worker, effect, mut processor) = fixture_effect();
        worker
            .call(Request::FixtureDelay { micros: 30_000 })
            .unwrap();
        let mut frames = impulse();
        let started = Instant::now();
        assert!(!processor.process(0, &mut frames, started + Duration::from_millis(2)));
        assert!(started.elapsed() < Duration::from_millis(20));
        assert_eq!(frames[0], [0.5, -0.25], "a late block is dry");
        worker.call(Request::FixtureDelay { micros: 0 }).unwrap();
        std::thread::sleep(Duration::from_millis(60));
        let mut frames = impulse();
        assert!(!processor.process(
            BLOCK as u64,
            &mut frames,
            Instant::now() + Duration::from_secs(2)
        ));
        assert_eq!(frames[0], [0.25, -0.125], "the next block is processed");
        assert!(effect.error().is_none());
        assert!(!worker.exited());
    }

    #[test]
    #[ignore = "requires the built Clef worker and macOS WindowServer"]
    fn a_restart_request_is_served_off_the_processing_thread() {
        let (mut worker, effect, mut processor) = fixture_effect();
        worker.call(Request::FixtureRestart).unwrap();
        let started = Instant::now();
        let mut position = 0;
        let mut dry_blocks = 0;
        loop {
            assert!(
                started.elapsed() < Duration::from_secs(4),
                "reconfiguration did not finish"
            );
            let mut frames = impulse();
            let block_started = Instant::now();
            let restart = processor.process(
                position,
                &mut frames,
                block_started + Duration::from_millis(5),
            );
            assert!(block_started.elapsed() < Duration::from_millis(50));
            position += BLOCK as u64;
            if restart {
                break;
            }
            assert_eq!(frames[0], [0.5, -0.25], "dry while reconfiguring");
            dry_blocks += 1;
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(dry_blocks >= 1);
        let mut frames = impulse();
        assert!(!processor.process(
            position,
            &mut frames,
            Instant::now() + Duration::from_secs(2)
        ));
        assert_eq!(frames[0], [0.25, -0.125]);
        assert!(effect.error().is_none());
    }

    #[test]
    #[ignore = "requires the built Clef worker and macOS WindowServer"]
    fn capture_saves_state_while_the_effect_stays_attached() {
        let (mut worker, effect, mut processor) = fixture_effect();
        worker.call(Request::FixtureEdit { value: 42 }).unwrap();
        assert_eq!(effect.capture(&mut worker).unwrap(), vec![42]);
        let mut frames = impulse();
        assert!(!processor.process(0, &mut frames, Instant::now() + Duration::from_secs(2)));
        assert_eq!(frames[0], [0.25, -0.125]);
        assert!(effect.error().is_none());
    }
}
