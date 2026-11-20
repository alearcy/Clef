use crate::graph::Delay;
use clef_plugin_host::{
    protocol::{BLOCK, Frame, RATE, Result},
    rpc::{AudioConnection, ControlConnection, Instrument},
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
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
}

impl Effect {
    pub fn activate(worker: &mut Instrument) -> Result<Arc<Self>> {
        worker.start(false)?;
        Ok(Arc::new(Self {
            audio: Mutex::new(Some(worker.take_audio()?)),
            control: worker.control_connection(),
            latency: AtomicU32::new(worker.latency),
            error: Mutex::new(None),
        }))
    }

    pub fn error(&self) -> Option<String> {
        self.error.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn fail(&self, error: String) {
        *self.error.lock().unwrap_or_else(|e| e.into_inner()) = Some(error);
    }

    pub fn deactivate(&self, worker: &mut Instrument) -> Result<()> {
        let mut audio = self.audio.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(connection) = audio.take() {
            worker.return_audio(connection);
            worker.stop()?;
        }
        Ok(())
    }

    pub fn capture(&self, worker: &mut Instrument) -> Result<Vec<u8>> {
        let mut audio = self.audio.lock().unwrap_or_else(|e| e.into_inner());
        match audio.as_mut() {
            Some(audio) => audio.with_stopped(&self.control, || worker.save()),
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
            let remaining = deadline.saturating_duration_since(Instant::now());
            if stage.failed || remaining.is_zero() || stage.bypassed && stage.mix == 0.0 {
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
                .and_then(|reply| {
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
                        return Ok(false);
                    }
                    let latency = audio.reconfigure(&slot.effect.control)?;
                    let total = self.latency - stage.latency + latency;
                    if total > MAX_CHAIN_LATENCY {
                        return Err("Effect latency exceeds available delay compensation.".into());
                    }
                    slot.effect.latency.store(latency, Ordering::Release);
                    stage.latency = latency;
                    stage.dry = Delay::new(latency);
                    self.latency = total;
                    Ok(true)
                });
            match result {
                Ok(false) => {}
                Ok(true) => return true,
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
}
