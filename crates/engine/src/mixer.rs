use crate::protocol::Frame;
use clef_model::{
    mixer::{Channel, Control, Target},
    project::{MAX_SEND_EDGES, MAX_TRACKS, Project},
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

    fn from_channel(channel: Channel, audible: bool) -> Self {
        if !audible || channel.level.fader == Control::Fader.bounds().0 {
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

    pub(crate) fn apply(self, source: Frame) -> Frame {
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

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Output {
    Master,
    Bus(usize),
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct PreparedChannel {
    pub id: u64,
    pub matrix: Matrix,
    pub send_matrix: Matrix,
    pub trim: f32,
    pub gate: f32,
    pub output: Output,
    send_start: u16,
    send_len: u16,
}

impl Default for PreparedChannel {
    fn default() -> Self {
        Self {
            id: 0,
            matrix: Matrix::default(),
            send_matrix: Matrix::default(),
            trim: 1.0,
            gate: 0.0,
            output: Output::Master,
            send_start: 0,
            send_len: 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct SendMix {
    pub bus: usize,
    pub gain: f32,
    pub pre: bool,
}

impl Default for SendMix {
    fn default() -> Self {
        Self {
            bus: 0,
            gain: 0.0,
            pre: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct BusMix {
    pub id: u64,
    /// Width, pan, fader and gate; the routing worker applies `trim` before inserts.
    pub matrix: Matrix,
    /// `matrix` with Gain folded in, for the callback path without bus inserts.
    pub direct: Matrix,
    pub trim: f32,
}

impl Default for BusMix {
    fn default() -> Self {
        Self {
            id: 0,
            matrix: Matrix::default(),
            direct: Matrix::default(),
            trim: 1.0,
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct PreparedMix {
    channels: [PreparedChannel; MAX_TRACKS],
    channel_len: usize,
    buses: [BusMix; MAX_TRACKS],
    bus_len: usize,
    sends: [SendMix; MAX_SEND_EDGES],
    send_len: usize,
    pub(crate) master: Matrix,
}

impl Default for PreparedMix {
    fn default() -> Self {
        Self {
            channels: std::array::from_fn(|_| PreparedChannel::default()),
            channel_len: 0,
            buses: std::array::from_fn(|_| BusMix::default()),
            bus_len: 0,
            sends: std::array::from_fn(|_| SendMix::default()),
            send_len: 0,
            master: Matrix::default(),
        }
    }
}

fn gain(level: i16) -> f32 {
    if level == Control::Fader.bounds().0 {
        0.0
    } else {
        10.0_f32.powf(f32::from(level) / 200.0)
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

        // Buses are sorted by id so the index stored in a send/output is stable for
        // the lifetime of this snapshot.
        let mut buses: Vec<_> = project.tracks.iter().filter(|t| t.is_bus()).collect();
        buses.sort_unstable_by_key(|t| t.id);
        for (index, bus) in buses.iter().enumerate() {
            let channel = apply_preview(Target::Bus(bus.id), bus.mix);
            result.buses[index] = BusMix {
                id: bus.id,
                matrix: Matrix::default(),
                direct: Matrix::default(),
                trim: gain(channel.trim),
            };
        }
        result.bus_len = buses.len();

        let any_solo = project
            .tracks
            .iter()
            .any(|track| track.mix.solo && !track.mix.mute);
        // A channel is solo-effective only when it is both soloed and unmuted.
        let effective: Vec<bool> = project
            .tracks
            .iter()
            .map(|track| track.mix.solo && !track.mix.mute)
            .collect();
        let bus_index = |id: u64| buses.iter().position(|bus| bus.id == id);
        let bus_effective: Vec<bool> = buses
            .iter()
            .map(|bus| bus.mix.solo && !bus.mix.mute)
            .collect();
        // Direct-output propagation: a track feeding a soloed bus is in the solo set.
        let track_in_set: Vec<bool> = project
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| {
                track.is_instrument()
                    && (effective[index]
                        || track
                            .output
                            .and_then(bus_index)
                            .is_some_and(|bus| bus_effective[bus]))
            })
            .collect();
        // A send is audible when its source is in the set or its destination bus is
        // soloed. A non-soloed source can feed a soloed FX return without making its
        // own dry output audible.
        let send_audible = |project_index: usize, bus: usize| {
            !project.tracks[project_index].mix.mute
                && !buses[bus].mix.mute
                && (!any_solo || track_in_set[project_index] || bus_effective[bus])
        };
        let mut bus_audible = bus_effective.clone();
        for (index, track) in project.tracks.iter().enumerate() {
            if track.is_instrument()
                && track_in_set[index]
                && let Some(bus) = track.output.and_then(bus_index)
            {
                bus_audible[bus] = true;
            }
        }
        for (index, track) in project.tracks.iter().enumerate() {
            if !track.is_instrument() {
                continue;
            }
            for send in &track.sends {
                if let Some(bus) = bus_index(send.target)
                    && send.enabled
                    && send_audible(index, bus)
                {
                    bus_audible[bus] = true;
                }
            }
        }

        for (index, bus) in buses.iter().enumerate() {
            let channel = apply_preview(Target::Bus(bus.id), bus.mix);
            let audible = !channel.mute && (!any_solo || bus_audible[index]);
            result.buses[index].matrix =
                Matrix::from_channel(Channel { trim: 0, ..channel }, audible);
            result.buses[index].direct = Matrix::from_channel(channel, audible);
        }

        // Instrument channels, sorted by id for the callback's binary search.
        let mut tracks: Vec<_> = project
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| track.is_instrument())
            .collect();
        tracks.sort_unstable_by_key(|(_, track)| track.id);
        for (channel_index, (project_index, track)) in tracks.iter().enumerate() {
            let channel = apply_preview(Target::Track(track.id), track.mix);
            let audible = !channel.mute && (!any_solo || track_in_set[*project_index]);
            let matrix = Matrix::from_channel(Channel { trim: 0, ..channel }, audible);
            result.channels[channel_index] = PreparedChannel {
                id: track.id,
                matrix,
                send_matrix: Matrix::from_channel(Channel { trim: 0, ..channel }, !channel.mute),
                trim: gain(channel.trim),
                gate: if channel.mute { 0.0 } else { 1.0 },
                output: track
                    .output
                    .and_then(bus_index)
                    .map_or(Output::Master, Output::Bus),
                send_start: result.send_len as u16,
                send_len: 0,
            };
            for send in &track.sends {
                if result.send_len == MAX_SEND_EDGES {
                    break;
                }
                let Some(bus) = bus_index(send.target) else {
                    continue;
                };
                let audible_send = send.enabled && send_audible(*project_index, bus);
                result.sends[result.send_len] = SendMix {
                    bus,
                    gain: if audible_send { gain(send.level) } else { 0.0 },
                    pre: send.pre,
                };
                result.send_len += 1;
                result.channels[channel_index].send_len += 1;
            }
        }
        result.channel_len = tracks.len();
        result.master = Matrix::from_channel(
            apply_preview(
                Target::Master,
                Channel {
                    level: project.master,
                    ..Channel::default()
                },
            ),
            true,
        );
        result
    }

    pub(crate) fn channel(&self, id: u64) -> Option<&PreparedChannel> {
        self.channels[..self.channel_len]
            .binary_search_by_key(&id, |entry| entry.id)
            .ok()
            .map(|index| &self.channels[index])
    }

    pub(crate) fn matrix(&self, id: u64) -> Matrix {
        self.channel(id)
            .map_or_else(Matrix::default, |entry| entry.matrix)
    }

    pub(crate) fn trim(&self, id: u64) -> f32 {
        self.channel(id).map_or(1.0, |entry| entry.trim)
    }

    pub(crate) fn sends(&self, channel: &PreparedChannel) -> &[SendMix] {
        let start = usize::from(channel.send_start);
        let end = start + usize::from(channel.send_len);
        &self.sends[start..end]
    }

    pub(crate) fn buses(&self) -> &[BusMix] {
        &self.buses[..self.bus_len]
    }

    pub(crate) fn bus_id(&self, index: usize) -> Option<u64> {
        self.buses.get(index).map(|bus| bus.id)
    }

    pub(crate) fn bus_len(&self) -> usize {
        self.bus_len
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
        assert_eq!(Matrix::from_channel(channel, true).apply(source), source);
        channel.width = 0;
        assert_eq!(Matrix::from_channel(channel, true).apply(source), [0.25; 2]);
        channel.width = 200;
        assert_eq!(
            Matrix::from_channel(channel, true).apply(source),
            [1.25, -0.75]
        );
        channel.level.pan = 100;
        channel.trim = 60;
        channel.level.fader = -60;
        assert_eq!(
            Matrix::from_channel(channel, true).apply(source),
            [0.0, -0.75]
        );
        channel.level.fader = Control::Fader.bounds().0;
        assert_eq!(Matrix::from_channel(channel, true).apply(source), [0.0; 2]);
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
                        Matrix::from_channel(channel, audible).apply([1.0; 2]),
                        [if audible { 1.0 } else { 0.0 }; 2]
                    );
                }
            }
        }
    }

    #[test]
    fn bus_solo_propagates_upstream_and_send_returns_stay_audible() {
        use clef_model::project::{Project, Send, Track, TrackKind};
        let channel = |id: u64, bus: Option<u64>| Track {
            colour: None,
            id,
            name: format!("Track {id}"),
            plugin: None,
            instrument_bypassed: false,
            instrument_offline: false,
            record_mode: Default::default(),
            kind: TrackKind::Instrument,
            output: bus,
            sends: Vec::new(),
            mix: Channel::default(),
            inserts: Vec::new(),
            take_lanes: Default::default(),
            state: Default::default(),
        };
        let bus = Track {
            colour: None,
            id: 3,
            name: "Reverb".into(),
            plugin: None,
            instrument_bypassed: false,
            instrument_offline: false,
            record_mode: Default::default(),
            kind: TrackKind::Bus,
            output: None,
            sends: Vec::new(),
            mix: Channel::default(),
            inserts: Vec::new(),
            take_lanes: Default::default(),
            state: Default::default(),
        };
        let mut project = Project::default();
        project.tracks.push(channel(1, Some(3)));
        project.tracks.push(channel(2, None));
        project.tracks.push(bus);
        project.next_id = 5;
        project.tracks[1].sends.push(Send {
            id: 4,
            target: 3,
            level: -60,
            pre: false,
            enabled: true,
        });
        let mix = PreparedMix::new(&project, None);
        let track1 = mix.channel(1).unwrap();
        let track2 = mix.channel(2).unwrap();
        assert_eq!(track1.matrix.apply([1.0; 2]), [1.0; 2]);
        assert_eq!(track2.matrix.apply([1.0; 2]), [1.0; 2]);
        assert!((mix.sends(track2)[0].gain - 10.0_f32.powf(-0.3)).abs() < 1e-6);
        assert_eq!(mix.buses()[0].matrix.apply([1.0; 2]), [1.0; 2]);

        project.tracks[2].mix.solo = true;
        let mix = PreparedMix::new(&project, None);
        let track1 = mix.channel(1).unwrap();
        let track2 = mix.channel(2).unwrap();
        let bus = mix.buses()[0];
        // Track 1 feeds the soloed bus and is pulled in; track 2's dry path is
        // excluded but its send into the soloed return remains audible.
        assert_eq!(track1.matrix.apply([1.0; 2]), [1.0; 2]);
        assert_eq!(track2.matrix.apply([1.0; 2]), [0.0; 2]);
        assert!((mix.sends(track2)[0].gain - 10.0_f32.powf(-0.3)).abs() < 1e-6);
        assert_eq!(bus.matrix.apply([1.0; 2]), [1.0; 2]);

        project.tracks[2].mix.solo = false;
        project.tracks[0].mix.solo = true;
        let mix = PreparedMix::new(&project, None);
        assert_eq!(mix.channel(1).unwrap().matrix.apply([1.0; 2]), [1.0; 2]);
        assert_eq!(mix.channel(2).unwrap().matrix.apply([1.0; 2]), [0.0; 2]);
        assert_eq!(mix.sends(mix.channel(2).unwrap())[0].gain, 0.0);
        assert_eq!(mix.buses()[0].matrix.apply([1.0; 2]), [1.0; 2]);

        project.tracks[2].mix.mute = true;
        let mix = PreparedMix::new(&project, None);
        assert_eq!(mix.buses()[0].matrix.apply([1.0; 2]), [0.0; 2]);
    }

    #[test]
    fn send_matrix_keeps_pan_and_fader_without_the_solo_gate() {
        use clef_model::project::{Project, Send, Track, TrackKind};
        let mut project = Project::default();
        project.tracks.push(Track {
            colour: None,
            id: 1,
            name: "Strings".into(),
            plugin: None,
            instrument_bypassed: false,
            instrument_offline: false,
            record_mode: Default::default(),
            kind: TrackKind::Instrument,
            output: None,
            sends: vec![Send {
                id: 3,
                target: 2,
                level: 0,
                pre: false,
                enabled: true,
            }],
            mix: Channel::default(),
            inserts: Vec::new(),
            take_lanes: Default::default(),
            state: Default::default(),
        });
        project.tracks.push(Track {
            colour: None,
            id: 2,
            name: "Reverb".into(),
            plugin: None,
            instrument_bypassed: false,
            instrument_offline: false,
            record_mode: Default::default(),
            kind: TrackKind::Bus,
            output: None,
            sends: Vec::new(),
            mix: Channel::default(),
            inserts: Vec::new(),
            take_lanes: Default::default(),
            state: Default::default(),
        });
        project.next_id = 4;
        project.tracks[0].mix.level.fader = -60;
        project.tracks[0].mix.solo = false;
        project.tracks[1].mix.solo = true;
        let mix = PreparedMix::new(&project, None);
        let channel = mix.channel(1).unwrap();
        // The dry path is outside the solo set; the send into the soloed return
        // still carries pan and fader without the solo gate.
        assert_eq!(channel.matrix.apply([1.0; 2]), [0.0; 2]);
        let sent = channel.send_matrix.apply([1.0; 2]);
        let expected = 10.0_f32.powf(-0.3);
        assert!((sent[0] - expected).abs() < 1e-6 && (sent[1] - expected).abs() < 1e-6);
        assert_eq!(mix.sends(channel)[0].gain, 1.0);
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
