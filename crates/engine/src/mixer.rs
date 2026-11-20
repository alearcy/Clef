use crate::protocol::Frame;
use clef_model::{
    mixer::{Channel, Control, Target},
    project::{MAX_TRACKS, Project},
};
use std::sync::atomic::{AtomicU64, Ordering};

/// A stereo matrix in the order LL, LR, RR, RL.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Matrix([f32; 4]);

impl Default for Matrix {
    fn default() -> Self {
        Self([1.0, 0.0, 1.0, 0.0])
    }
}

impl Matrix {
    pub(crate) fn trim(trim: f32) -> Self {
        Self([trim, 0.0, trim, 0.0])
    }

    fn from_channel(channel: Channel, any_solo: bool) -> Self {
        if channel.mute
            || (any_solo && !channel.solo)
            || channel.level.fader == Control::Fader.bounds().0
        {
            return Self([0.0; 4]);
        }
        let gain =
            10.0_f32.powf((f32::from(channel.trim) + f32::from(channel.level.fader)) / 200.0);
        let pan = f32::from(channel.level.pan) / 100.0;
        let left = gain * (1.0 - pan.max(0.0));
        let right = gain * (1.0 + pan.min(0.0));
        let side = f32::from(channel.width) / 100.0;
        let same = 0.5 * (1.0 + side);
        let cross = 0.5 * (1.0 - side);
        Self([left * same, left * cross, right * same, right * cross])
    }

    fn apply(self, source: Frame) -> Frame {
        let [ll, lr, rr, rl] = self.0;
        if lr == 0.0 && rl == 0.0 {
            return [source[0] * ll, source[1] * rr];
        }
        [
            source[0] * ll + source[1] * lr,
            source[1] * rr + source[0] * rl,
        ]
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct PreparedMix {
    channels: [(u64, Matrix, f32); MAX_TRACKS],
    len: usize,
    pub(crate) master: Matrix,
}

impl Default for PreparedMix {
    fn default() -> Self {
        Self {
            channels: [(0, Matrix::default(), 1.0); MAX_TRACKS],
            len: 0,
            master: Matrix::default(),
        }
    }
}

impl PreparedMix {
    pub(crate) fn new(project: &Project, preview: Option<(Target, Control, i16)>) -> Self {
        let mut result = Self::default();
        let apply_preview = |target, mut channel: Channel| {
            if let Some((owner, control, value)) = preview
                && target == owner
            {
                let _ = channel.set(control, value);
            }
            channel
        };
        let any_solo = project.tracks.iter().any(|track| track.mix.solo);
        for (entry, track) in result.channels.iter_mut().zip(&project.tracks) {
            let mut channel = apply_preview(Target::Track(track.id), track.mix);
            let trim = 10.0_f32.powf(f32::from(channel.trim) / 200.0);
            channel.trim = 0;
            *entry = (track.id, Matrix::from_channel(channel, any_solo), trim);
            result.len += 1;
        }
        result.channels[..result.len].sort_unstable_by_key(|entry| entry.0);
        result.master = Matrix::from_channel(
            apply_preview(
                Target::Master,
                Channel {
                    level: project.master,
                    ..Channel::default()
                },
            ),
            false,
        );
        result
    }

    pub(crate) fn channel(&self, id: u64) -> Matrix {
        self.channels[..self.len]
            .binary_search_by_key(&id, |entry| entry.0)
            .map_or_else(|_| Matrix::default(), |index| self.channels[index].1)
    }

    pub(crate) fn trim(&self, id: u64) -> f32 {
        self.channels[..self.len]
            .binary_search_by_key(&id, |entry| entry.0)
            .map_or(1.0, |index| self.channels[index].2)
    }
}

#[derive(Default)]
pub(crate) struct Ramp {
    current: Matrix,
    target: Matrix,
    step: [f32; 4],
    remaining: u32,
}

impl Ramp {
    pub(crate) fn reset(&mut self, target: Matrix) {
        self.current = target;
        self.target = target;
        self.remaining = 0;
    }

    pub(crate) fn set(&mut self, target: Matrix, samples: u32) {
        if self.target != target {
            self.target = target;
            self.remaining = samples;
            self.step = std::array::from_fn(|i| (target.0[i] - self.current.0[i]) / samples as f32);
        }
    }

    pub(crate) fn apply(&mut self, frame: Frame) -> Frame {
        if self.remaining != 0 {
            self.remaining -= 1;
            if self.remaining == 0 {
                self.current = self.target;
            } else {
                for (value, delta) in self.current.0.iter_mut().zip(self.step) {
                    *value += delta;
                }
            }
        }
        self.current.apply(frame)
    }
}

#[derive(Default)]
pub(crate) struct MeterPeak([AtomicU64; 2]);

impl MeterPeak {
    pub(crate) fn publish(&self, generation: u64, peak: Frame) {
        let tag = generation << 32;
        for (shared, value) in self.0.iter().zip(peak) {
            if value > 0.0 {
                // A late callback may finish after its reader slot has been reused.
                shared.fetch_max(tag | u64::from(value.to_bits()), Ordering::Relaxed);
            }
        }
    }

    pub(crate) fn take(&self, generation: u64) -> Frame {
        let tag = generation << 32;
        self.0.each_ref().map(|peak| {
            let packed = peak.swap(tag, Ordering::Relaxed);
            if packed >> 32 == tag >> 32 {
                f32::from_bits(packed as u32)
            } else {
                0.0
            }
        })
    }
}

pub(crate) fn accumulate(peak: &mut Frame, frame: Frame) {
    for (peak, sample) in peak.iter_mut().zip(frame) {
        let level = if sample.is_finite() {
            sample.abs()
        } else {
            f32::MAX
        };
        *peak = peak.max(level);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_reader_cannot_publish_into_a_reused_meter_slot() {
        let meter = MeterPeak::default();
        meter.publish(1, [1.0, 2.0]);
        assert_eq!(meter.take(2), [0.0; 2]);
        meter.publish(2, [0.5, 0.25]);
        meter.publish(1, [2.0, 3.0]);
        assert_eq!(meter.take(2), [0.5, 0.25]);
    }

    #[test]
    fn trim_width_balance_and_fader_compose_in_stereo() {
        let mut channel = Channel::default();
        let source = [0.75, -0.25];
        assert_eq!(Matrix::from_channel(channel, false).apply(source), source);
        channel.width = 0;
        assert_eq!(
            Matrix::from_channel(channel, false).apply(source),
            [0.25; 2]
        );
        channel.width = 200;
        assert_eq!(
            Matrix::from_channel(channel, false).apply(source),
            [1.25, -0.75]
        );
        channel.level.pan = 100;
        channel.trim = 60;
        channel.level.fader = -60;
        assert_eq!(
            Matrix::from_channel(channel, false).apply(source),
            [0.0, -0.75]
        );
        channel.level.fader = Control::Fader.bounds().0;
        assert_eq!(Matrix::from_channel(channel, false).apply(source), [0.0; 2]);
    }

    #[test]
    fn mute_has_priority_with_or_without_any_active_solo() {
        for any_solo in [false, true] {
            for solo in [false, true] {
                for mute in [false, true] {
                    let channel = Channel {
                        solo,
                        mute,
                        ..Channel::default()
                    };
                    let audible = !mute && (!any_solo || solo);
                    assert_eq!(
                        Matrix::from_channel(channel, any_solo).apply([1.0; 2]),
                        [if audible { 1.0 } else { 0.0 }; 2]
                    );
                }
            }
        }
    }

    #[test]
    fn ramp_finishes_at_exact_target_and_retargets_from_current_value() {
        let mut ramp = Ramp::default();
        let silent = Matrix([0.0; 4]);
        ramp.set(silent, 4);
        assert_eq!(ramp.apply([1.0; 2]), [0.75; 2]);
        ramp.set(Matrix::default(), 2);
        assert_eq!(ramp.apply([1.0; 2]), [0.875; 2]);
        assert_eq!(ramp.apply([1.0; 2]), [1.0; 2]);
        ramp.set(silent, 2);
        assert_eq!(ramp.apply([1.0; 2]), [0.5; 2]);
        assert_eq!(ramp.apply([1.0; 2]), [0.0; 2]);
    }
}
