use crate::recording::Projection;
use clef_model::{
    Result,
    timeline::{MAX_CHANGES, MAX_TICK, QUARTER, StopMode, Tempo, Timeline},
};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::{
    Arc,
    atomic::{AtomicU32, AtomicU64, Ordering},
};

const FRACTION: u128 = 1 << 32;
const HALF: u128 = FRACTION / 2;
const COMMAND_CAPACITY: usize = 8;
const TICK_BITS: u32 = 30;
const TICK_MASK: u64 = (1 << TICK_BITS) - 1;
const _: () = assert!(MAX_TICK <= TICK_MASK);

#[derive(Clone, Copy, PartialEq, Eq)]
struct Segment {
    tick: u64,
    frame: u64,
    exact: u128,
    tempo: Tempo,
}

/// Prepared output-clock mapping. Use half-open sample spans to assign events
/// at a boundary to exactly one block, across tempo changes and loop wraps.
pub struct SampleMap {
    segments: [Segment; MAX_CHANGES],
    len: usize,
    rate: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SampleSpan {
    pub start: u64,
    pub frames: u64,
    pub next: u64,
}

impl SampleMap {
    /// True when both maps place every tick at the same sample.
    pub(crate) fn same_mapping(&self, other: &Self) -> bool {
        self.rate == other.rate && self.segments[..self.len] == other.segments[..other.len]
    }

    pub fn new(timeline: &Timeline, rate: u32) -> Result<Self> {
        timeline.validate()?;
        if !(8000..=384_000).contains(&rate) {
            return Err("unsupported timeline sample rate".into());
        }
        let mut previous = Segment {
            tick: 0,
            frame: 0,
            exact: 0,
            tempo: timeline.changes[0],
        };
        let mut segments = [previous; MAX_CHANGES];
        for (slot, (tick, tempo)) in segments.iter_mut().zip(timeline.sections()) {
            let exact = previous.exact + duration(tick - previous.tick, previous.tempo, rate);
            previous = Segment {
                tick,
                frame: rounded(exact),
                exact,
                tempo,
            };
            *slot = previous;
        }
        Ok(Self {
            segments,
            len: timeline.changes.len(),
            rate,
        })
    }

    fn at_sample(&self, frame: u64) -> usize {
        self.segments[..self.len].partition_point(|segment| segment.frame <= frame) - 1
    }

    pub fn sample_at(&self, tick: u64) -> u64 {
        let tick = tick.min(MAX_TICK);
        let index = self.segments[..self.len].partition_point(|segment| segment.tick <= tick) - 1;
        let segment = self.segments[index];
        rounded(segment.exact + duration(tick - segment.tick, segment.tempo, self.rate))
    }

    pub fn tick_at(&self, frame: u64) -> u64 {
        let segment = self.segments[self.at_sample(frame)];
        let relative = u128::from(frame) * FRACTION + HALF - 1 - segment.exact;
        let numerator = relative * u128::from(segment.tempo.bpm) * u128::from(QUARTER);
        let ticks = numerator / (u128::from(self.rate) * 60 * FRACTION);
        let tick = (u128::from(segment.tick) + ticks).min(u128::from(MAX_TICK)) as u64;
        if tick < MAX_TICK && self.sample_at(tick + 1) <= frame {
            tick + 1
        } else {
            tick
        }
    }

    pub fn span(&self, start: u64, limit: u64, range: Option<[u64; 2]>) -> SampleSpan {
        let index = self.at_sample(start);
        let mut end = self.segments[..self.len]
            .get(index + 1)
            .map_or(start.saturating_add(limit), |segment| segment.frame)
            .min(start.saturating_add(limit));
        let mut next = end;
        if let Some([left, right]) = range
            && left < right
            && start < right
            && right <= end
        {
            end = right;
            next = left;
        }
        SampleSpan {
            start,
            frames: end - start,
            next,
        }
    }

    fn beat_at_or_after(&self, frame: u64) -> (u64, bool) {
        let segment = self.segments[self.at_sample(frame)];
        let beat = segment.tempo.beat_ticks();
        let mut tick = segment.tick + (self.tick_at(frame) - segment.tick).div_ceil(beat) * beat;
        if self.sample_at(tick) < frame {
            tick += beat;
        }
        if tick >= MAX_TICK {
            return (u64::MAX, false);
        }
        let sample = self.sample_at(tick);
        let meter = self.segments[self.at_sample(sample)];
        (
            sample,
            (tick - meter.tick).is_multiple_of(meter.tempo.bar_ticks()),
        )
    }
}

fn duration(ticks: u64, tempo: Tempo, rate: u32) -> u128 {
    u128::from(ticks) * 60 * u128::from(rate) * FRACTION
        / (u128::from(tempo.bpm) * u128::from(QUARTER))
}

fn rounded(frames: u128) -> u64 {
    ((frames + HALF) / FRACTION) as u64
}

pub struct Prepared {
    map: SampleMap,
    range: Option<[u64; 2]>,
    click: bool,
    count_in: bool,
    stop_mode: StopMode,
}

impl Prepared {
    pub fn new(timeline: &Timeline, rate: u32) -> Result<Self> {
        let map = SampleMap::new(timeline, rate)?;
        let range = timeline
            .looping
            .then(|| timeline.loop_range.map(|tick| map.sample_at(tick)));
        Ok(Self {
            map,
            range,
            click: timeline.click,
            count_in: timeline.count_in,
            stop_mode: timeline.stop_mode,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Phase {
    #[default]
    Stopped,
    CountingIn,
    Playing,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start(timeline: &Timeline, rate: u32, tick: u64) -> (Control, Renderer) {
        Renderer::new(Prepared::new(timeline, rate).unwrap(), tick)
    }

    fn advance(renderer: &mut Renderer, frames: u64) {
        renderer.begin_callback();
        for _ in 0..frames {
            renderer.sample();
        }
        renderer.publish(0);
    }

    #[test]
    fn tempo_meter_offsets_and_spans_use_exact_sample_boundaries() {
        for rate in [44_100, 48_000] {
            let mut timeline = Timeline::default();
            timeline.changes.push(Tempo {
                bar: 2,
                bpm: 137,
                beats: 7,
                unit: 8,
            });
            timeline.changes.push(Tempo {
                bar: 6,
                bpm: 83,
                beats: 3,
                unit: 4,
            });
            let map = SampleMap::new(&timeline, rate).unwrap();
            let boundary = u64::from(rate) * 2;
            let end_tick = timeline.bar_start(6);
            let end = boundary + ((14.0 * 60.0 * f64::from(rate) / 137.0).round() as u64);
            assert_eq!(timeline.position(3840).bar, 2);
            assert_eq!(timeline.position(4320).beat, 2);
            assert_eq!(map.sample_at(3840), boundary);
            assert_eq!(map.sample_at(end_tick), end);
            assert_eq!(
                map.span(boundary - 5, 32, None),
                SampleSpan {
                    start: boundary - 5,
                    frames: 5,
                    next: boundary
                }
            );
            let loop_frames = [boundary - 101, boundary + 13];
            assert_eq!(
                map.span(boundary, 27, Some(loop_frames)),
                SampleSpan {
                    start: boundary,
                    frames: 13,
                    next: boundary - 101
                }
            );
            assert_eq!(map.span(boundary - 101, 14, Some(loop_frames)).frames, 14);
            for tick in 0..=end_tick + 3000 {
                let frame = map.sample_at(tick);
                assert_eq!(map.tick_at(frame), tick);
                if tick > 0 {
                    assert_eq!(map.tick_at(frame - 1), tick - 1);
                }
            }
        }
    }

    #[test]
    fn only_an_unacknowledged_play_request_marks_play_pending() {
        let timeline = Timeline::default();
        let (mut control, mut renderer) = start(&timeline, 48_000, 0);
        control
            .settings(Prepared::new(&timeline, 48_000).unwrap())
            .unwrap();
        assert!(control.view().pending);
        assert!(!control.view().play_pending);
        advance(&mut renderer, 0);

        control.send(Command::Play).unwrap();
        assert!(control.view().play_pending);
        control.send(Command::Stop).unwrap();
        assert!(control.view().pending);
        assert!(!control.view().play_pending);
        advance(&mut renderer, 0);
        assert_eq!(control.view().phase, Phase::Stopped);

        control.send(Command::Play).unwrap();
        advance(&mut renderer, 1);
        assert_eq!(control.view().phase, Phase::Playing);
        assert!(!control.view().play_pending);
    }

    #[test]
    fn stop_keep_resumes_the_exact_frame_and_return_uses_the_last_play_start() {
        for stop_mode in [StopMode::KeepPosition, StopMode::ReturnToPlayStart] {
            let timeline = Timeline {
                looping: true,
                loop_range: [960, 1920],
                stop_mode,
                ..Default::default()
            };
            let (mut control, mut renderer) = start(&timeline, 48_000, 0);
            control.send(Command::Play).unwrap();
            assert!(control.view().pending);
            advance(&mut renderer, 24_017);
            assert_eq!(renderer.frame, 24_017);
            assert!(!control.view().pending);
            control.send(Command::Locate(1440)).unwrap();
            advance(&mut renderer, 17);
            control.send(Command::Stop).unwrap();
            advance(&mut renderer, 500);
            let expected = if stop_mode == StopMode::KeepPosition {
                36_017
            } else {
                24_000
            };
            assert_eq!(control.view().phase, Phase::Stopped);
            assert_eq!(renderer.frame, expected);
            control.send(Command::Stop).unwrap();
            advance(&mut renderer, 0);
            assert_eq!(renderer.frame, expected);
            control.send(Command::Play).unwrap();
            advance(&mut renderer, 1);
            assert_eq!(renderer.frame, expected + 1);
        }
    }

    #[test]
    fn stop_keeps_the_output_open_and_a_second_stop_is_a_panic() {
        let timeline = Timeline {
            looping: true,
            loop_range: [960, 1920],
            ..Default::default()
        };
        let (mut control, mut renderer) = start(&timeline, 48_000, 0);
        control.send(Command::Play).unwrap();
        advance(&mut renderer, 2_000);
        let before = renderer.frame;
        control.send(Command::Stop).unwrap();
        advance(&mut renderer, 0);
        assert_eq!(control.view().phase, Phase::Stopped);
        assert_eq!(renderer.frame, before);
        // Tails ring out: Stop neither fades nor closes the output.
        for _ in 0..480 {
            renderer.sample();
            assert_eq!(renderer.gain(), 1.0);
        }
        assert_eq!(renderer.panics(), 0);

        control.send(Command::Stop).unwrap();
        advance(&mut renderer, 0);
        assert_eq!(renderer.panics(), 1);
        let mut previous = renderer.gain();
        for _ in 0..240 {
            renderer.sample();
            let gain = renderer.gain();
            assert!(gain <= previous + f32::EPSILON, "{gain} > {previous}");
            previous = gain;
        }
        assert!(renderer.gain() <= f32::EPSILON);
        for _ in 0..240 {
            renderer.sample();
        }
        assert_eq!(renderer.gain(), 0.0, "the panic keeps the output closed");
        assert_eq!(renderer.frame, before);
        renderer.resume_gate();
        for _ in 0..240 {
            renderer.sample();
        }
        assert_eq!(renderer.gain(), 1.0);

        control.send(Command::Stop).unwrap();
        advance(&mut renderer, 0);
        control.send(Command::Play).unwrap();
        advance(&mut renderer, 0);
        assert_eq!(
            renderer.gain(),
            1.0,
            "Play reopens the output after a panic"
        );
    }

    #[test]
    fn count_in_clicks_use_the_current_meter_and_seek_cancels_the_count() {
        for rate in [44_100, 48_000] {
            let mut timeline = Timeline {
                count_in: true,
                ..Default::default()
            };
            timeline.changes[0] = Tempo {
                bar: 1,
                bpm: 137,
                beats: 3,
                unit: 8,
            };
            let (mut control, mut renderer) = start(&timeline, rate, 720);
            control.send(Command::Play).unwrap();
            renderer.begin_callback();
            let count_frames = (3.0 * 30.0 * f64::from(rate) / 137.0).round() as u64;
            let mut click_frames = Vec::new();
            for i in 0..count_frames {
                let sample = renderer.sample();
                if sample == 0.20 || sample == 0.12 {
                    click_frames.push(i);
                }
                assert_eq!(renderer.frame, renderer.play_start);
            }
            assert_eq!(
                click_frames,
                (0..3)
                    .map(|beat| (f64::from(beat) * 30.0 * f64::from(rate) / 137.0).round() as u64)
                    .collect::<Vec<_>>()
            );
            assert_eq!(renderer.phase, Phase::Playing);
            assert_eq!(renderer.sample(), 0.0);
            control.send(Command::Stop).unwrap();
            advance(&mut renderer, 0);
            control.send(Command::Play).unwrap();
            advance(&mut renderer, 1);
            assert_eq!(control.view().phase, Phase::CountingIn);
            control.send(Command::Locate(960)).unwrap();
            advance(&mut renderer, 100);
            assert_eq!(
                control.view(),
                View {
                    tick: 960,
                    audible_tick: 960,
                    phase: Phase::Stopped,
                    pending: false,
                    play_pending: false,
                }
            );
        }
    }

    #[test]
    fn audible_playhead_projects_through_a_loop_and_resets_on_seek() {
        let timeline = Timeline {
            looping: true,
            loop_range: [0, 960],
            ..Default::default()
        };
        let (mut control, mut renderer) = start(&timeline, 48_000, 0);
        control.send(Command::Play).unwrap();
        advance(&mut renderer, 24_500);
        renderer.publish(1000);
        assert_eq!(control.view().tick, 20);
        assert_eq!(control.view().audible_tick, 940);
        control.send(Command::Locate(480)).unwrap();
        advance(&mut renderer, 50);
        renderer.publish(1000);
        assert_eq!(control.view().audible_tick, 480);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct View {
    pub tick: u64,
    pub audible_tick: u64,
    pub phase: Phase,
    pub pending: bool,
    pub play_pending: bool,
}

#[derive(Clone, Copy)]
pub enum Command {
    Play,
    Record,
    Stop,
    Locate(u64),
    Settings,
}

struct Update {
    revision: u32,
    command: Command,
    settings: Option<Prepared>,
}

#[derive(Default)]
struct Shared {
    position: AtomicU64,
    revision: AtomicU32,
}

pub struct Control {
    updates: Producer<Update>,
    shared: Arc<Shared>,
    revision: u32,
    play_queued: bool,
}

impl Control {
    pub fn send(&mut self, command: Command) -> Result<()> {
        self.push(command, None)
    }

    pub fn settings(&mut self, settings: Prepared) -> Result<()> {
        self.push(Command::Settings, Some(settings))
    }

    fn push(&mut self, command: Command, settings: Option<Prepared>) -> Result<()> {
        let revision = self.revision.wrapping_add(1);
        self.updates
            .push(Update {
                revision,
                command,
                settings,
            })
            .map_err(|_| "transport is busy")?;
        self.revision = revision;
        self.play_queued = matches!(command, Command::Play | Command::Record);
        Ok(())
    }

    pub fn view(&self) -> View {
        let revision = self.shared.revision.load(Ordering::Acquire);
        let value = self.shared.position.load(Ordering::Acquire);
        let pending = revision != self.revision;
        View {
            tick: value & TICK_MASK,
            audible_tick: (value >> TICK_BITS) & TICK_MASK,
            phase: match value >> (TICK_BITS * 2) {
                1 => Phase::CountingIn,
                2 => Phase::Playing,
                _ => Phase::Stopped,
            },
            pending,
            play_pending: pending && self.play_queued,
        }
    }
}

#[derive(Default)]
struct Click {
    remaining: u32,
    length: u32,
    phase: u32,
    step: u32,
    strength: f32,
}

impl Click {
    fn start(&mut self, rate: u32, accent: bool) {
        self.length = (rate / 100).max(1);
        self.remaining = self.length;
        self.phase = 0;
        let frequency = if accent { 1760 } else { 1174 };
        self.step = ((1_u64 << 32) * frequency / u64::from(rate)) as u32;
        self.strength = if accent { 0.20 } else { 0.12 };
    }

    fn sample(&mut self) -> f32 {
        if self.remaining == 0 {
            return 0.0;
        }
        let cycle = self.phase as f32 / (1_u64 << 32) as f32;
        self.phase = self.phase.wrapping_add(self.step);
        let envelope = self.remaining as f32 / self.length as f32;
        self.remaining -= 1;
        ((cycle - 0.5).abs() * 4.0 - 1.0) * envelope * envelope * self.strength
    }
}

struct CountIn {
    tempo: Tempo,
    elapsed: u64,
    beat: u8,
    next: u64,
    length: u64,
}

pub(crate) struct Renderer {
    settings: Prepared,
    updates: Consumer<Update>,
    shared: Arc<Shared>,
    revision: u32,
    phase: Phase,
    frame: u64,
    play_start: u64,
    origin: u64,
    elapsed: u64,
    last_frame: u64,
    span: SampleSpan,
    beat: (u64, bool),
    count: Option<CountIn>,
    click: Click,
    hold: u64,
    generation: u64,
    recording: bool,
    stopped_recording: bool,
    /// Counts panics (Stop while stopped); the engine releases every voice on a change.
    panics: u64,
    declick_gain: f32,
    declick_target: f32,
    declick_step: f32,
    declick_remaining: u32,
}

impl Renderer {
    pub(crate) fn new(settings: Prepared, tick: u64) -> (Control, Self) {
        let (tx, rx) = RingBuffer::new(COMMAND_CAPACITY);
        let shared = Arc::new(Shared::default());
        let frame = settings.map.sample_at(tick);
        let mut renderer = Self {
            settings,
            updates: rx,
            shared: shared.clone(),
            revision: 0,
            phase: Phase::Stopped,
            frame,
            play_start: frame,
            origin: frame,
            elapsed: 0,
            last_frame: 0,
            span: SampleSpan {
                start: frame,
                frames: 0,
                next: frame,
            },
            beat: (u64::MAX, false),
            count: None,
            click: Click::default(),
            hold: 0,
            generation: 0,
            recording: false,
            stopped_recording: false,
            panics: 0,
            declick_gain: 1.0,
            declick_target: 1.0,
            declick_step: 0.0,
            declick_remaining: 0,
        };
        renderer.prepare_position();
        renderer.publish(0);
        (
            Control {
                updates: tx,
                shared,
                revision: 0,
                play_queued: false,
            },
            renderer,
        )
    }

    fn prepare_position(&mut self) {
        self.origin = self.frame;
        self.elapsed = 0;
        self.last_frame = self.settings.map.sample_at(MAX_TICK);
        self.click.remaining = 0;
        self.prepare_span();
    }

    fn prepare_span(&mut self) {
        self.span = self.settings.map.span(
            self.frame,
            self.last_frame.saturating_sub(self.frame),
            self.settings.range,
        );
        self.beat = self.settings.map.beat_at_or_after(self.frame);
    }

    fn stop(&mut self) {
        self.stopped_recording |= self.recording;
        self.recording = false;
        self.hold = 0;
        self.generation = self.generation.wrapping_add(1);
        if self.phase != Phase::Stopped && self.settings.stop_mode == StopMode::ReturnToPlayStart {
            self.frame = self.play_start;
        }
        self.phase = Phase::Stopped;
        self.count = None;
        // The output stays open so instrument and effect tails ring out (T15 Stop and Tails).
        self.prepare_position();
    }

    fn fade_to(&mut self, target: f32, frames: u32) {
        self.declick_target = target;
        self.declick_remaining = frames.max(1);
        self.declick_step = (target - self.declick_gain) / self.declick_remaining as f32;
    }

    fn advance_declick(&mut self) {
        if self.declick_remaining != 0 {
            self.declick_remaining -= 1;
            if self.declick_remaining == 0 {
                self.declick_gain = self.declick_target;
            } else {
                self.declick_gain += self.declick_step;
            }
        }
    }

    fn request_stop(&mut self) {
        if self.phase != Phase::Stopped {
            self.stop();
            return;
        }
        // A second Stop is a panic: fade the output out and keep it closed until Play or
        // live audition; the engine releases every voice.
        self.panics = self.panics.wrapping_add(1);
        self.fade_to(0.0, (self.settings.map.rate / 200).max(1));
    }

    pub(crate) fn panics(&self) -> u64 {
        self.panics
    }

    pub(crate) fn gain(&self) -> f32 {
        self.declick_gain
    }

    /// Reopen the output gate for live audition after a panic closed it.
    pub(crate) fn resume_gate(&mut self) {
        if self.phase == Phase::Stopped && self.declick_gain < 1.0 {
            self.fade_to(1.0, (self.settings.map.rate / 200).max(1));
        }
    }

    fn play(&mut self, record: bool, hold: u64) {
        if self.phase != Phase::Stopped {
            return;
        }
        if let Some([left, right]) = self.settings.range
            && !(left..right).contains(&self.frame)
        {
            self.frame = left;
        }
        if self.frame >= self.last_frame {
            return;
        }
        self.play_start = self.frame;
        self.hold = hold;
        self.recording = record;
        self.generation = self.generation.wrapping_add(1);
        self.prepare_position();
        self.phase = Phase::Playing;
        self.declick_gain = 1.0;
        self.declick_target = 1.0;
        self.declick_step = 0.0;
        self.declick_remaining = 0;
        if self.settings.count_in {
            let map = &self.settings.map;
            let tempo = map.segments[map.at_sample(self.frame)].tempo;
            self.count = Some(CountIn {
                tempo,
                elapsed: 0,
                beat: 0,
                next: 0,
                length: rounded(duration(tempo.bar_ticks(), tempo, map.rate)),
            });
            self.phase = Phase::CountingIn;
        }
    }

    #[cfg(test)]
    pub(crate) fn begin_callback(&mut self) {
        self.begin_device_callback(0);
    }

    pub(crate) fn begin_device_callback(&mut self, hold: u64) {
        for _ in 0..COMMAND_CAPACITY {
            let Ok(update) = self.updates.pop() else {
                break;
            };
            match update.command {
                Command::Play => self.play(false, hold),
                Command::Record => self.play(true, hold),
                Command::Stop => self.request_stop(),
                Command::Locate(tick) => {
                    if self.phase == Phase::CountingIn {
                        self.stop();
                    }
                    if self.phase == Phase::Playing {
                        self.hold = hold;
                        self.generation = self.generation.wrapping_add(1);
                    }
                    self.frame = self.settings.map.sample_at(tick);
                    self.prepare_position();
                }
                Command::Settings => {
                    if let Some(settings) = update.settings {
                        if self.phase == Phase::Stopped {
                            let before = &self.settings.map;
                            let after = &settings.map;
                            if before.segments[..before.len] != after.segments[..after.len] {
                                self.frame = after.sample_at(before.tick_at(self.frame));
                            }
                            self.settings = settings;
                            self.prepare_position();
                        } else {
                            let changed_range = self.settings.range != settings.range;
                            self.settings = settings;
                            if changed_range {
                                self.hold = hold;
                                self.generation = self.generation.wrapping_add(1);
                                self.prepare_position();
                            }
                            if !self.settings.click && self.phase == Phase::Playing {
                                self.click.remaining = 0;
                            }
                        }
                    }
                }
            }
            self.revision = update.revision;
        }
    }

    pub(crate) fn projection(&self, position: u64) -> Projection {
        let generation = self.generation;
        if self.phase == Phase::Stopped {
            return Projection {
                generation,
                start: u64::MAX,
                ..Default::default()
            };
        }
        let remaining_count = self
            .count
            .as_ref()
            .map_or(0, |count| count.length - count.elapsed);
        let start = position + self.hold + remaining_count;
        let [left, right] = self.settings.range.unwrap_or([0, 0]);
        Projection {
            generation,
            start: start.saturating_sub(self.elapsed),
            origin: self.origin,
            left,
            right,
            limit: self.last_frame,
        }
    }

    pub(crate) fn recording(&self) -> bool {
        self.recording
    }

    pub(crate) fn take_record_stop(&mut self) -> bool {
        std::mem::take(&mut self.stopped_recording)
    }

    pub(crate) fn sample(&mut self) -> f32 {
        if self.hold > 0 {
            self.hold -= 1;
            return 0.0;
        }
        if self.phase == Phase::Stopped {
            self.advance_declick();
            return 0.0;
        }
        let rate = self.settings.map.rate;
        if let Some(count) = &mut self.count {
            if count.elapsed == count.next && count.beat < count.tempo.beats {
                self.click.start(rate, count.beat == 0);
                count.beat += 1;
                count.next = rounded(duration(
                    u64::from(count.beat) * count.tempo.beat_ticks(),
                    count.tempo,
                    rate,
                ));
            }
            count.elapsed += 1;
            let sample = self.click.sample();
            if count.elapsed == count.length {
                self.count = None;
                self.phase = Phase::Playing;
                self.click.remaining = 0;
            }
            return sample;
        }
        if self.frame >= self.last_frame {
            self.stop();
            return 0.0;
        }
        if self.frame == self.beat.0 {
            if self.settings.click {
                self.click.start(rate, self.beat.1);
            }
            self.beat = self.settings.map.beat_at_or_after(self.frame + 1);
        }
        let sample = self.click.sample();
        self.frame += 1;
        self.elapsed += 1;
        if self.frame == self.span.start + self.span.frames {
            if self.frame != self.span.next {
                self.click.remaining = 0;
            }
            self.frame = self.span.next;
            self.prepare_span();
        }
        self.advance_declick();
        sample
    }

    pub(crate) fn publish(&self, latency_frames: u64) {
        let phase = self.phase;
        let frame = self.frame;
        let audible = if phase == Phase::Playing {
            let projected = self.origin + self.elapsed.saturating_sub(latency_frames);
            match self.settings.range {
                Some([left, right]) if self.origin < right && projected >= right => {
                    left + (projected - right) % (right - left)
                }
                _ => projected,
            }
        } else {
            frame
        };
        let map = &self.settings.map;
        let packed = map.tick_at(frame)
            | (map.tick_at(audible) << TICK_BITS)
            | ((phase as u64) << (TICK_BITS * 2));
        self.shared.position.store(packed, Ordering::Release);
        self.shared.revision.store(self.revision, Ordering::Release);
    }
}
