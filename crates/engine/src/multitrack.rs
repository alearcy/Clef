//! Independent bounded worker paths sharing one device callback and sample clock.
use crate::transport::SampleMap;
use crate::{
    live::{Configuration, channel_message},
    project::MAX_TRACKS,
    protocol::{BLOCK, Frame, MAX_EVENTS, Midi, Result},
    recording::{
        CONTROLS, Captured, ControllerBaseline, MAX_CAPTURE_MESSAGES, Plan, Projection, Published,
        Scheduler, capture_message, control_index,
    },
    rpc::{AudioConnection, Instrument},
    transport::{Control, Prepared, Renderer},
};
use clef_audio_io::{self as audio_io, MidiInputConnection, Stream, StreamTrait};
use clef_model::{midi::Arrangement, timeline::Timeline};
use rtrb::{Consumer, Producer, RingBuffer};
use std::time::{Duration, Instant};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
};

struct Packet {
    at: u64,
    epoch: u64,
    token: u64,
    transport: u64,
    frames: [Frame; BLOCK],
}
#[derive(Clone, Copy)]
struct Input {
    at: u64,
    revision: u64,
    token: u64,
    bytes: [u8; 3],
}
#[derive(Default)]
pub struct TrackStats {
    pub id: AtomicU64,
    active: AtomicBool,
    ready_at: AtomicU64,
    token: AtomicU64,
    pub latency: AtomicU32,
    pub misses: AtomicU64,
    pub transition_frames: AtomicU64,
    pub stale: AtomicU64,
    pub failed: AtomicBool,
    pub max_rpc_us: AtomicU64,
    pub max_worker_us: AtomicU64,
    pub processed: AtomicU64,
    pub peak: AtomicU32,
    pub midi_delivered: AtomicU64,
    pub midi_received: AtomicU64,
    pub midi_dropped: AtomicU64,
    pub playback_dropped: AtomicU64,
    pub playback_blocks: AtomicU64,
    release: AtomicBool,
    audition: AtomicU32,
    mute_recording: AtomicBool,
}
pub struct Clock {
    origin: Instant,
    anchor_sequence: AtomicU64,
    anchor_frame: AtomicU64,
    anchor_ns: AtomicU64,
    pub silence: Arc<AtomicBool>,
    pub tracks: Vec<Arc<TrackStats>>,
    pub played: AtomicU64,
    selected: AtomicU64,
    revision: AtomicU64,
    epoch: AtomicU64,
    pub latency: AtomicU32,
    pub clipped: AtomicU64,
    pub errors: AtomicU64,
    pub transitions: AtomicU64,
    latency_update: std::sync::Mutex<()>,
    pub callbacks: AtomicU64,
    pub min_callback: AtomicU64,
    pub max_callback: AtomicU64,
    timeline: Published,
    record_window: Published,
    record_end: AtomicU64,
    capture_target: AtomicU64,
    capture_epoch: AtomicU64,
    capture_overflow: AtomicBool,
    capture_writing: AtomicBool,
    output_latency: AtomicU64,
    playback_generation: AtomicU64,
    plan_revision: AtomicU64,
    has_plans: AtomicBool,
    plans: Mutex<HashMap<u64, Arc<Plan>>>,
    controls: [AtomicU32; CONTROLS],
}
impl Clock {
    fn new() -> Self {
        Self {
            origin: Instant::now(),
            anchor_sequence: AtomicU64::new(0),
            anchor_frame: AtomicU64::new(0),
            anchor_ns: AtomicU64::new(0),
            silence: Arc::new(AtomicBool::new(false)),
            tracks: (0..MAX_TRACKS)
                .map(|_| Arc::new(TrackStats::default()))
                .collect(),
            played: AtomicU64::new(0),
            selected: AtomicU64::new(0),
            revision: AtomicU64::new(0),
            epoch: AtomicU64::new(0),
            latency: AtomicU32::new(0),
            clipped: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            transitions: AtomicU64::new(0),
            latency_update: std::sync::Mutex::new(()),
            callbacks: AtomicU64::new(0),
            min_callback: AtomicU64::new(u64::MAX),
            max_callback: AtomicU64::new(0),
            timeline: Published::default(),
            record_window: Published::default(),
            record_end: AtomicU64::new(u64::MAX),
            capture_target: AtomicU64::new(0),
            capture_epoch: AtomicU64::new(0),
            capture_overflow: AtomicBool::new(false),
            capture_writing: AtomicBool::new(false),
            output_latency: AtomicU64::new(0),
            playback_generation: AtomicU64::new(0),
            plan_revision: AtomicU64::new(0),
            has_plans: AtomicBool::new(false),
            plans: Mutex::new(HashMap::new()),
            controls: std::array::from_fn(|_| AtomicU32::new(0)),
        }
    }
    fn event_position(&self, now_ns: u64, rate: u32) -> Option<(u64, u64)> {
        // One writer (the device callback); bounded retry on the MIDI thread.
        for _ in 0..3 {
            let version = self.anchor_sequence.load(Ordering::Acquire);
            if version == 0 {
                return None;
            }
            if !version.is_multiple_of(2) {
                continue;
            }
            let frame = self.anchor_frame.load(Ordering::Relaxed);
            let at = self.anchor_ns.load(Ordering::Relaxed);
            let latency = self.output_latency.load(Ordering::Relaxed);
            std::sync::atomic::fence(Ordering::Acquire);
            if version == self.anchor_sequence.load(Ordering::Acquire) {
                let delta =
                    u128::from(now_ns.saturating_sub(at)) * u128::from(rate) / 1_000_000_000;
                return Some((
                    frame.saturating_add(delta.min(u128::from(u64::MAX)) as u64),
                    latency,
                ));
            }
        }
        Some((
            self.played.load(Ordering::Acquire),
            self.output_latency.load(Ordering::Relaxed),
        ))
    }
    pub fn select(&self, id: u64) {
        if self.selected.load(Ordering::Acquire) == id {
            return;
        }
        self.selected.store(0, Ordering::Release);
        self.revision.fetch_add(1, Ordering::AcqRel);
        self.selected.store(id, Ordering::Release);
    }
    fn refresh_latency(&self) {
        let _update = self
            .latency_update
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let max = self
            .tracks
            .iter()
            .filter(|s| s.active.load(Ordering::Acquire))
            .map(|s| s.latency.load(Ordering::Acquire))
            .max()
            .unwrap_or(0);
        if self.latency.load(Ordering::Acquire) != max {
            self.epoch.fetch_add(1, Ordering::AcqRel);
            self.latency.store(max, Ordering::Release);
            self.epoch.fetch_add(1, Ordering::AcqRel);
        }
    }
}
struct Reader {
    used: bool,
    epoch: u64,
    token: u64,
    transitioning: bool,
    queue: Consumer<Packet>,
    next: Option<Packet>,
    stats: Arc<TrackStats>,
}
struct Mix {
    readers: Vec<Reader>,
    clock: Arc<Clock>,
    position: u64,
    channels: usize,
    pair: [usize; 2],
    bound: usize,
    lead: u64,
    transport: Option<Renderer>,
}
impl Mix {
    fn fill(&mut self, output: &mut [f32], output_latency: u64) {
        output.fill(0.0);
        let count = (output.len() / self.channels) as u64;
        let now = self
            .clock
            .origin
            .elapsed()
            .as_nanos()
            .min(u128::from(u64::MAX)) as u64;
        self.clock.anchor_sequence.fetch_add(1, Ordering::AcqRel);
        self.clock
            .anchor_frame
            .store(self.position, Ordering::Relaxed);
        self.clock.anchor_ns.store(now, Ordering::Relaxed);
        self.clock.anchor_sequence.fetch_add(1, Ordering::Release);
        self.clock.callbacks.fetch_add(1, Ordering::Relaxed);
        self.clock.min_callback.fetch_min(count, Ordering::Relaxed);
        self.clock.max_callback.fetch_max(count, Ordering::Relaxed);
        if self.clock.silence.load(Ordering::Acquire) {
            self.position += count;
            self.clock.played.store(self.position, Ordering::Release);
            return;
        }
        let mut playback_generation = self.clock.playback_generation.load(Ordering::Relaxed);
        if let Some(transport) = &mut self.transport {
            let scheduled = self.clock.has_plans.load(Ordering::Acquire)
                || self.clock.capture_target.load(Ordering::Acquire) != 0;
            let hold = if scheduled {
                self.lead + u64::from(self.clock.latency.load(Ordering::Acquire)) + 2 * BLOCK as u64
            } else {
                0
            };
            transport.begin_device_callback(hold);
            let timeline = transport.projection(self.position);
            if let Some(previous) = self
                .clock
                .timeline
                .get()
                .filter(|previous| *previous != timeline)
            {
                if scheduled
                    && previous.start != u64::MAX
                    && previous.limit > 0
                    && previous.generation != timeline.generation
                {
                    playback_generation = timeline.generation;
                    self.clock
                        .playback_generation
                        .store(playback_generation, Ordering::Release);
                }
                self.clock.timeline.set(timeline);
                if transport.recording() {
                    self.clock.record_window.set(timeline);
                }
            }
            if transport.take_record_stop() {
                self.clock.record_end.store(
                    self.position.saturating_sub(output_latency),
                    Ordering::Release,
                );
            }
        }
        for frame in output.chunks_exact_mut(self.channels) {
            let epoch = self.clock.epoch.load(Ordering::Acquire);
            let click = self.transport.as_mut().map_or(0.0, Renderer::sample);
            let mut sum = [click; 2];
            for reader in &mut self.readers {
                if !reader.stats.active.load(Ordering::Acquire)
                    || reader.stats.failed.load(Ordering::Acquire)
                {
                    continue;
                }
                let token = reader.stats.token.load(Ordering::Acquire);
                if reader.epoch != epoch || reader.token != token {
                    reader.epoch = epoch;
                    reader.token = token;
                    reader.transitioning = true;
                }
                // Every pop discards at most a fixed queue capacity, even after a stall.
                for _ in 0..=self.bound {
                    if reader.next.is_none() {
                        reader.next = reader.queue.pop().ok();
                        reader.used = false;
                    }
                    if reader.next.as_ref().is_some_and(|p| {
                        p.epoch != epoch
                            || p.token != token
                            || p.transport < playback_generation
                            || p.at + BLOCK as u64 <= self.position
                    }) {
                        if reader
                            .next
                            .as_ref()
                            .is_some_and(|p| !reader.used || p.at + BLOCK as u64 > self.position)
                        {
                            reader.stats.stale.fetch_add(1, Ordering::Relaxed);
                        }
                        reader.next = None;
                        reader.used = false;
                    } else {
                        break;
                    }
                }
                if !epoch.is_multiple_of(2)
                    || !reader.stats.active.load(Ordering::Acquire)
                    || reader.stats.failed.load(Ordering::Acquire)
                {
                    continue;
                }
                if let Some(packet) = &reader.next
                    && packet.at <= self.position
                {
                    reader.transitioning = false;
                    reader.used = true;
                    let source = packet.frames[(self.position - packet.at) as usize];
                    sum[0] += source[0];
                    sum[1] += source[1];
                } else if self.position >= self.lead {
                    if reader.transitioning {
                        reader
                            .stats
                            .transition_frames
                            .fetch_add(1, Ordering::Relaxed);
                        self.clock.transitions.fetch_add(1, Ordering::Relaxed);
                    } else if self.position >= reader.stats.ready_at.load(Ordering::Acquire)
                        && self.position.is_multiple_of(BLOCK as u64)
                    {
                        reader.stats.misses.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            for channel in 0..2 {
                if !sum[channel].is_finite() || sum[channel].abs() > 1.0 {
                    self.clock.clipped.fetch_add(1, Ordering::Relaxed);
                }
                frame[self.pair[channel]] = if sum[channel].is_finite() {
                    sum[channel].clamp(-1.0, 1.0)
                } else {
                    0.0
                };
            }
            self.position += 1;
        }
        if let Some(transport) = &mut self.transport {
            if transport.take_record_stop() {
                self.clock.record_end.store(
                    self.position.saturating_sub(output_latency),
                    Ordering::Release,
                );
            }
            transport.publish(output_latency.saturating_add(count));
        }
        self.clock.played.store(self.position, Ordering::Release);
    }
}
// Source time is behind presentation time by the transport lead. Only skip
// source blocks whose presentation deadline has actually passed.
fn catch_up(cursor: u64, played: u64, lead: u64) -> u64 {
    cursor.max(played.saturating_sub(lead).div_ceil(BLOCK as u64) * BLOCK as u64)
}
fn belongs_to_selection(owners: &mut [u64; 16 * 128], bytes: [u8; 3], revision: u64) -> bool {
    let kind = bytes[0] & 0xf0;
    let key = usize::from(bytes[0] & 15) * 128 + usize::from(bytes[1]);
    if kind == 0x80 || kind == 0x90 && bytes[2] == 0 {
        if owners[key] != revision {
            return false;
        }
        owners[key] = u64::MAX;
    } else if kind == 0x90 {
        owners[key] = revision;
    }
    true
}
struct Returned {
    audio: AudioConnection,
    output: Producer<Packet>,
    input: Consumer<Input>,
    error: Option<String>,
}
pub struct Broker {
    slot: usize,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<Returned>>,
    pub stats: Arc<TrackStats>,
}
pub struct Engine {
    stream: Stream,
    midi: Option<MidiInputConnection<()>>,
    outputs: Vec<Option<Producer<Packet>>>,
    inputs: Vec<Option<Consumer<Input>>>,
    pub clock: Arc<Clock>,
    pub rate: u32,
    pub buffer: u32,
    pub lead: u64,
    pub transport: Control,
    capture: Consumer<Captured>,
}
impl Engine {
    pub fn start(
        config: Configuration,
        silence: Arc<AtomicBool>,
        timing: Prepared,
        tick: u64,
    ) -> Result<Self> {
        let Configuration {
            device,
            pair,
            midi_id,
            midi_index: _,
            rate,
            buffer,
        } = config;
        let channel_count = audio_io::output_channels(&device, pair, rate, buffer)?
            .ok_or("unsupported device configuration")?;
        let channels = usize::from(channel_count);
        let bound = (buffer as usize).div_ceil(BLOCK).max(1) * 4;
        let lead = (bound * BLOCK) as u64;
        let mut state = Clock::new();
        state.silence = silence;
        let clock = Arc::new(state);
        let mut readers = Vec::new();
        let mut outputs = Vec::new();
        let mut senders = Vec::new();
        let mut inputs = Vec::new();
        for stats in &clock.tracks {
            let (tx, rx) = RingBuffer::new(bound);
            outputs.push(Some(tx));
            readers.push(Reader {
                used: false,
                epoch: 0,
                token: 0,
                transitioning: true,
                queue: rx,
                next: None,
                stats: stats.clone(),
            });
            let (tx, rx) = RingBuffer::new(MAX_EVENTS);
            senders.push(tx);
            inputs.push(Some(rx));
        }
        let (mut capture_tx, capture) = RingBuffer::new(MAX_CAPTURE_MESSAGES);
        let state = clock.clone();
        let mut owners = [u64::MAX; 16 * 128];
        let midi = audio_io::midi_input(
            "Clef session",
            "Clef selected track",
            midi_id.as_deref(),
            None,
            false,
            move |bytes| {
                if state.silence.load(Ordering::Acquire) {
                    return;
                }
                let Some(bytes) = channel_message(bytes) else {
                    return;
                };
                let revision = state.revision.load(Ordering::Acquire);
                let selected = state.selected.load(Ordering::Acquire);
                if let Some(index) = control_index(bytes) {
                    state.controls[index].store(
                        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]),
                        Ordering::Release,
                    );
                }
                if !belongs_to_selection(&mut owners, bytes, revision) {
                    return;
                }
                if selected == 0 {
                    return;
                }
                if let Some((slot, stats)) = state.tracks.iter().enumerate().find(|(_, s)| {
                    s.id.load(Ordering::Acquire) == selected && s.active.load(Ordering::Acquire)
                }) {
                    let token = stats.token.load(Ordering::Acquire);
                    if revision != state.revision.load(Ordering::Acquire) {
                        return;
                    }
                    let now = state.origin.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
                    let Some((at, output_latency)) = state.event_position(now, rate) else {
                        return;
                    };
                    let capture_epoch = state.capture_epoch.load(Ordering::SeqCst);
                    if capture_epoch % 2 == 1
                        && state.capture_target.load(Ordering::Acquire) == selected
                        && capture_message(bytes)
                    {
                        state.capture_writing.store(true, Ordering::SeqCst);
                        if state.capture_epoch.load(Ordering::SeqCst) == capture_epoch
                            && !state.capture_overflow.load(Ordering::Acquire)
                        {
                            let heard = at.saturating_sub(output_latency);
                            if capture_tx.push(Captured { at: heard, bytes }).is_err() {
                                state.capture_overflow.store(true, Ordering::Release);
                            }
                        }
                        state.capture_writing.store(false, Ordering::SeqCst);
                    }
                    if senders[slot]
                        .push(Input {
                            at,
                            revision,
                            token,
                            bytes,
                        })
                        .is_err()
                    {
                        stats.midi_dropped.fetch_add(1, Ordering::Relaxed);
                        stats.release.store(true, Ordering::Release);
                    } else {
                        stats.midi_received.fetch_add(1, Ordering::Relaxed);
                    }
                }
            },
        )?;
        let (transport, renderer) = Renderer::new(timing, tick);
        let mut mix = Mix {
            readers,
            clock: clock.clone(),
            position: 0,
            channels,
            pair,
            bound,
            lead,
            transport: Some(renderer),
        };
        let errors = clock.clone();
        let stream = audio_io::timed_output_stream(
            &device,
            channel_count,
            rate,
            buffer,
            move |out, latency| mix.fill(out, latency),
            move || {
                errors.errors.fetch_add(1, Ordering::Relaxed);
            },
        )?;
        stream.play()?;
        Ok(Self {
            stream,
            midi: Some(midi),
            outputs,
            inputs,
            clock,
            rate,
            buffer,
            lead,
            transport,
            capture,
        })
    }
    pub fn set_arrangement(&self, arrangement: &Arrangement, timeline: &Timeline) -> Result<()> {
        if arrangement.tracks.is_empty() && self.clock.plan_revision.load(Ordering::Acquire) == 0 {
            return Ok(());
        }
        let map = SampleMap::new(timeline, self.rate)?;
        let plans = arrangement
            .tracks
            .iter()
            .map(|(&id, clips)| Ok((id, Arc::new(Plan::new(clips, &map)?))))
            .collect::<Result<_>>()?;
        *self
            .clock
            .plans
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = plans;
        self.clock
            .has_plans
            .store(!arrangement.tracks.is_empty(), Ordering::Release);
        self.clock.plan_revision.fetch_add(1, Ordering::Release);
        Ok(())
    }

    pub fn begin_capture(&mut self, target: u64) -> Result<ControllerBaseline> {
        for _ in 0..MAX_CAPTURE_MESSAGES {
            if self.capture.pop().is_err() {
                break;
            }
        }
        self.clock.record_window.set(Projection::default());
        self.clock.record_end.store(u64::MAX, Ordering::Release);
        self.clock.capture_overflow.store(false, Ordering::Release);
        let baseline = self.clock.controls.each_ref().map(|value| {
            let [status, key, data, _] = value.load(Ordering::Acquire).to_le_bytes();
            [status, key, data]
        });
        self.clock.capture_target.store(target, Ordering::Release);
        self.clock.capture_epoch.fetch_add(1, Ordering::SeqCst);
        if let Err(error) = self.transport.send(crate::transport::Command::Record) {
            self.end_capture();
            return Err(error);
        }
        Ok(baseline)
    }

    pub fn recording_window(&self) -> Option<Projection> {
        self.clock
            .record_window
            .get()
            .filter(|window| window.limit > 0)
    }

    pub fn recording_end(&self) -> u64 {
        let end = self.clock.record_end.load(Ordering::Acquire);
        if end != u64::MAX {
            return end;
        }
        let now = self
            .clock
            .origin
            .elapsed()
            .as_nanos()
            .min(u128::from(u64::MAX)) as u64;
        let played = self.clock.played.load(Ordering::Acquire);
        self.clock
            .event_position(now, self.rate)
            .map_or(0, |(position, latency)| {
                position.min(played).saturating_sub(latency)
            })
    }

    pub fn drain_capture(&mut self, into: &mut Vec<Captured>) -> bool {
        for _ in 0..MAX_CAPTURE_MESSAGES {
            let Ok(event) = self.capture.pop() else { break };
            if into.len() == MAX_CAPTURE_MESSAGES {
                self.clock.capture_overflow.store(true, Ordering::Release);
                break;
            }
            into.push(event);
        }
        self.clock.capture_overflow.load(Ordering::Acquire)
    }

    pub fn end_capture(&self) {
        self.clock.capture_epoch.fetch_add(1, Ordering::SeqCst);
        self.clock.capture_target.store(0, Ordering::Release);
        // Only the CoreMIDI producer can hold this flag, across a single ring push.
        while self.clock.capture_writing.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }
    }

    pub fn mute_recording(&self, id: u64, muted: bool) {
        if let Some(stats) = self
            .clock
            .tracks
            .iter()
            .find(|stats| stats.id.load(Ordering::Acquire) == id)
        {
            stats.mute_recording.store(muted, Ordering::Release);
        }
    }

    pub fn release_live(&self, id: u64) {
        if let Some(stats) = self
            .clock
            .tracks
            .iter()
            .find(|stats| stats.id.load(Ordering::Acquire) == id)
        {
            stats.release.store(true, Ordering::Release);
        }
    }

    pub fn audition(&self, id: u64, channel: u8, key: u8, velocity: u8) {
        if channel < 16
            && key < 128
            && (1..128).contains(&velocity)
            && let Some(stats) = self.clock.tracks.iter().find(|stats| {
                stats.id.load(Ordering::Acquire) == id && stats.active.load(Ordering::Acquire)
            })
        {
            stats.audition.store(
                u32::from_le_bytes([0x90 | channel, key, velocity, 0]),
                Ordering::Release,
            );
        }
    }

    pub fn attach(&mut self, id: u64, instrument: &mut Instrument) -> Result<Broker> {
        let slot = self
            .outputs
            .iter()
            .position(Option::is_some)
            .ok_or("no free audio slots")?;
        instrument.start(false)?;
        let control = instrument.control_connection();
        let mut audio = instrument.take_audio()?;
        let mut output = self.outputs[slot].take().ok_or("audio slot busy")?;
        let mut input = self.inputs[slot].take().ok_or("MIDI slot busy")?;
        let stats = self.clock.tracks[slot].clone();
        for counter in [
            &stats.misses,
            &stats.stale,
            &stats.transition_frames,
            &stats.processed,
            &stats.max_rpc_us,
            &stats.max_worker_us,
            &stats.midi_delivered,
            &stats.midi_received,
            &stats.midi_dropped,
            &stats.playback_dropped,
            &stats.playback_blocks,
        ] {
            counter.store(0, Ordering::Relaxed);
        }
        stats.peak.store(0, Ordering::Relaxed);
        stats.release.store(false, Ordering::Release);
        stats.audition.store(0, Ordering::Release);
        stats.mute_recording.store(false, Ordering::Release);
        stats.failed.store(false, Ordering::Release);
        stats.id.store(id, Ordering::Release);
        stats.latency.store(instrument.latency, Ordering::Release);
        let token = stats.token.fetch_add(1, Ordering::AcqRel) + 1;
        stats.ready_at.store(
            self.clock
                .played
                .load(Ordering::Acquire)
                .div_ceil(BLOCK as u64)
                * BLOCK as u64
                + self.lead,
            Ordering::Release,
        );
        stats.active.store(true, Ordering::Release);
        self.clock.refresh_latency();
        let clock = self.clock.clone();
        let lead = self.lead;
        let audition_frames = u64::from(self.rate) / 6;
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let report = stats.clone();
        let thread = std::thread::spawn(move || {
            let mut elapsed =
                clock.played.load(Ordering::Acquire).div_ceil(BLOCK as u64) * BLOCK as u64;
            let mut revision = clock.revision.load(Ordering::Acquire);
            let mut delay = crate::graph::Delay::new(0);
            let mut delay_epoch = u64::MAX;
            let mut pending = None;
            let mut events = Vec::with_capacity(MAX_EVENTS);
            let releases: Vec<_> = crate::sequence::panic_events()
                .into_iter()
                .filter(|e| e.bytes[1] != 120)
                .collect();
            let mut scheduler = Scheduler::new();
            let mut projection = Projection::default();
            let mut plan_revision = 0;
            let mut plan = None;
            let mut audition: Option<(u64, [u8; 3])> = None;
            let result: Result<()> = (|| {
                let _ = crate::threading::audio_priority(true);
                while !stopping.load(Ordering::Acquire) {
                    if output.slots() == 0 {
                        std::thread::sleep(Duration::from_micros(100));
                        continue;
                    }
                    elapsed = catch_up(elapsed, clock.played.load(Ordering::Acquire), lead);
                    let epoch = clock.epoch.load(Ordering::Acquire);
                    if !epoch.is_multiple_of(2) {
                        std::thread::yield_now();
                        continue;
                    }
                    events.clear();
                    let next_revision = clock.revision.load(Ordering::Acquire);
                    let release_live = report.release.swap(false, Ordering::AcqRel);
                    if next_revision != revision || release_live {
                        revision = next_revision;
                        scheduler.release_live();
                        audition = None;
                    }
                    let next_plan = clock.plan_revision.load(Ordering::Acquire);
                    if next_plan != plan_revision
                        && let Ok(plans) = clock.plans.try_lock()
                    {
                        plan = plans.get(&id).cloned();
                        plan_revision = next_plan;
                        scheduler.replace_plan();
                    }
                    if let Some(next) = clock.timeline.get() {
                        projection = next;
                    }
                    let [status, key, velocity, _] =
                        report.audition.swap(0, Ordering::AcqRel).to_le_bytes();
                    let stopped = projection.limit == 0;
                    if let Some((end, bytes)) = audition
                        && (end <= elapsed || status != 0 || !stopped)
                    {
                        scheduler.live(Midi { offset: 0, bytes });
                        audition = None;
                    }
                    if stopped && status != 0 {
                        scheduler.live(Midi {
                            offset: 0,
                            bytes: [status, key, velocity],
                        });
                        audition = Some((elapsed + audition_frames, [status - 0x10, key, 0]));
                    }
                    for _ in 0..MAX_EVENTS {
                        if pending.is_none() {
                            pending = input.pop().ok();
                        }
                        let Some(event) = pending else {
                            break;
                        };
                        if event.token != token
                            || event.revision != revision
                            || clock.selected.load(Ordering::Acquire) != id
                        {
                            pending = None;
                            continue;
                        }
                        if event.at >= elapsed + BLOCK as u64 {
                            break;
                        }
                        report.midi_delivered.fetch_add(1, Ordering::Relaxed);
                        scheduler.live(Midi {
                            offset: event.at.saturating_sub(elapsed) as u32,
                            bytes: event.bytes,
                        });
                        pending = None;
                    }
                    if scheduler.schedule(
                        plan.as_deref(),
                        projection,
                        elapsed + lead + u64::from(clock.latency.load(Ordering::Acquire)),
                        report.mute_recording.load(Ordering::Acquire),
                    ) {
                        report.playback_blocks.fetch_add(1, Ordering::Relaxed);
                    }
                    if scheduler.render(&mut events) {
                        report.playback_dropped.fetch_add(1, Ordering::Relaxed);
                    }
                    if delay_epoch != epoch {
                        delay = crate::graph::Delay::new(
                            clock
                                .latency
                                .load(Ordering::Acquire)
                                .saturating_sub(report.latency.load(Ordering::Acquire)),
                        );
                        delay_epoch = epoch;
                    }
                    let started = Instant::now();
                    let reply = audio.process(crate::sequence::START + elapsed, &events, BLOCK)?;
                    report
                        .max_rpc_us
                        .fetch_max(started.elapsed().as_micros() as u64, Ordering::Relaxed);
                    report
                        .max_worker_us
                        .fetch_max(reply.processing_nanos / 1000, Ordering::Relaxed);
                    report.processed.fetch_add(1, Ordering::Relaxed);
                    let peak = reply
                        .frames
                        .iter()
                        .flatten()
                        .fold(0.0_f32, |p, s| p.max(s.abs()));
                    report.peak.fetch_max(peak.to_bits(), Ordering::Relaxed);
                    if reply.restart || reply.latency != report.latency.load(Ordering::Acquire) {
                        let latency = audio.reconfigure(&control)?;
                        report.latency.store(latency, Ordering::Release);
                        clock.refresh_latency();
                        clock.epoch.fetch_add(2, Ordering::AcqRel);
                        elapsed += BLOCK as u64;
                        continue;
                    }
                    if epoch != clock.epoch.load(Ordering::Acquire) {
                        elapsed += BLOCK as u64;
                        continue;
                    }
                    let mut frames = [[0.0; 2]; BLOCK];
                    for (out, source) in frames.iter_mut().zip(reply.frames) {
                        *out = delay.apply(source);
                    }
                    output
                        .push(Packet {
                            at: elapsed + lead,
                            epoch,
                            token,
                            transport: projection.generation,
                            frames,
                        })
                        .map_err(|_| "audio queue full")?;
                    elapsed += BLOCK as u64;
                }
                let _ = audio.process(crate::sequence::START + elapsed, &releases, BLOCK);
                audio.suspend()?;
                Ok(())
            })();
            if result.is_err() {
                report.failed.store(true, Ordering::Release);
            }
            Returned {
                audio,
                output,
                input,
                error: result.err().map(|e| e.to_string()),
            }
        });
        Ok(Broker {
            slot,
            stop,
            thread: Some(thread),
            stats,
        })
    }
    pub fn detach(&mut self, mut broker: Broker, instrument: &mut Instrument) -> Result<()> {
        broker.stats.active.store(false, Ordering::Release);
        broker.stats.id.store(0, Ordering::Release);
        broker.stop.store(true, Ordering::Release);
        self.clock.refresh_latency();
        let returned = broker
            .thread
            .take()
            .ok_or("broker already stopped")?
            .join()
            .map_err(|_| "broker panicked")?;
        instrument.return_audio(returned.audio);
        self.outputs[broker.slot] = Some(returned.output);
        self.inputs[broker.slot] = Some(returned.input);
        if let Some(error) = returned.error {
            return Err(error.into());
        }
        instrument.stop()
    }
    pub fn summary(&self) -> String {
        format!(
            "{} Hz · requested {} frames · actual {}–{} frames · lead {} samples · plugin latency {} samples · clipped samples {}",
            self.rate,
            self.buffer,
            if self.clock.callbacks.load(Ordering::Relaxed) == 0 {
                0
            } else {
                self.clock.min_callback.load(Ordering::Relaxed)
            },
            self.clock.max_callback.load(Ordering::Relaxed),
            self.lead,
            self.clock.latency.load(Ordering::Relaxed),
            self.clock.clipped.load(Ordering::Relaxed)
        )
    }
}
impl Drop for Broker {
    fn drop(&mut self) {
        self.stats.active.store(false, Ordering::Release);
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.stream.pause();
        self.midi.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn midi_time_uses_the_device_anchor_instead_of_stream_startup_wall_time() {
        let clock = Clock::new();
        assert_eq!(clock.event_position(1_000_000_000, 48000), None);
        clock.anchor_sequence.store(2, Ordering::Release);
        clock.anchor_frame.store(1000, Ordering::Relaxed);
        clock.anchor_ns.store(5_000_000_000, Ordering::Relaxed);
        assert_eq!(clock.event_position(5_002_000_000, 48000), Some((1096, 0)));
        clock.anchor_sequence.store(3, Ordering::Release);
        clock.played.store(1280, Ordering::Release);
        assert_eq!(clock.event_position(5_002_000_000, 48000), Some((1280, 0)));
    }
    #[test]
    fn held_note_releases_cannot_migrate_to_a_new_selection_or_channel() {
        let mut owners = [u64::MAX; 16 * 128];
        assert!(belongs_to_selection(&mut owners, [0x90, 60, 80], 1));
        assert!(!belongs_to_selection(&mut owners, [0x80, 60, 0], 2));
        assert!(belongs_to_selection(&mut owners, [0x90, 60, 80], 2));
        assert!(!belongs_to_selection(&mut owners, [0x81, 60, 0], 2));
        assert!(belongs_to_selection(&mut owners, [0x80, 60, 0], 2));
        assert!(!belongs_to_selection(&mut owners, [0x80, 60, 0], 3));
        assert!(belongs_to_selection(&mut owners, [0xb0, 64, 0], 3));
    }
    #[test]
    fn buffered_source_blocks_are_not_skipped_at_driver_callback_boundaries() {
        assert_eq!(catch_up(1152, 1280, 1024), 1152);
        assert_eq!(catch_up(0, 1024, 1024), 0);
        assert_eq!(catch_up(128, 4096, 1024), 3072);
        assert_eq!(catch_up(128, 4097, 1024), 3200);
    }
    #[test]
    fn independent_paths_sum_align_and_failed_path_cannot_replay_queued_audio() {
        let clock = Arc::new(Clock::new());
        let mut readers = Vec::new();
        for slot in 0..3 {
            let (mut tx, rx) = RingBuffer::new(4);
            let stats = clock.tracks[slot].clone();
            stats.active.store(true, Ordering::Relaxed);
            stats.token.store(1, Ordering::Relaxed);
            let mut frames = [[0.0; 2]; BLOCK];
            frames[37] = [0.2; 2];
            // Different plugin latencies produce the same compensated arrival.
            tx.push(Packet {
                at: 257,
                epoch: 0,
                token: 1,
                transport: 0,
                frames,
            })
            .ok()
            .unwrap();
            readers.push(Reader {
                used: false,
                epoch: 0,
                token: 0,
                transitioning: true,
                queue: rx,
                next: None,
                stats,
            });
        }
        let mut mix = Mix {
            readers,
            clock: clock.clone(),
            position: 0,
            channels: 2,
            pair: [0, 1],
            bound: 4,
            lead: 0,
            transport: None,
        };
        let mut out = vec![0.0; 800];
        mix.fill(&mut out, 0);
        assert!((out[(257 + 37) * 2] - 0.6).abs() < 1e-6);
        assert!(
            out.iter()
                .enumerate()
                .all(|(i, v)| i / 2 == 294 || *v == 0.0)
        );
        clock.tracks[0].failed.store(true, Ordering::Relaxed);
        let mut out = vec![1.0; BLOCK * 2];
        mix.fill(&mut out, 0);
        assert!(out.iter().all(|v| *v == 0.0));
    }
    #[test]
    fn callback_discards_old_generations_and_clips_only_the_final_sum() {
        let clock = Arc::new(Clock::new());
        let (mut tx, rx) = RingBuffer::new(4);
        let stats = clock.tracks[0].clone();
        stats.active.store(true, Ordering::Relaxed);
        tx.push(Packet {
            at: 0,
            epoch: 0,
            token: 0,
            transport: 0,
            frames: [[2.0; 2]; BLOCK],
        })
        .ok()
        .unwrap();
        let mut mix = Mix {
            readers: vec![Reader {
                used: false,
                epoch: 0,
                token: 0,
                transitioning: true,
                queue: rx,
                next: None,
                stats,
            }],
            clock: clock.clone(),
            position: 0,
            channels: 2,
            pair: [0, 1],
            bound: 4,
            lead: 0,
            transport: None,
        };
        let mut out = [0.0; 2];
        mix.fill(&mut out, 0);
        assert_eq!(out, [1.0; 2]);
        assert_eq!(clock.clipped.load(Ordering::Relaxed), 2);
        clock.epoch.store(1, Ordering::Release);
        mix.fill(&mut out, 0);
        assert_eq!(out, [0.0; 2]);
    }
}

#[cfg(test)]
mod callback_proof {
    use super::*;
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    thread_local! { static COUNT: Cell<Option<usize>> = const {Cell::new(None)}; }
    struct CountingAllocator;
    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let _ = COUNT.try_with(|c| {
                if let Some(n) = c.get() {
                    c.set(Some(n + 1));
                }
            });
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
            let _ = COUNT.try_with(|c| {
                if let Some(n) = c.get() {
                    c.set(Some(n + 1));
                }
            });
            unsafe { System.dealloc(p, layout) }
        }
        unsafe fn realloc(&self, p: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            let _ = COUNT.try_with(|c| {
                if let Some(n) = c.get() {
                    c.set(Some(n + 1));
                }
            });
            unsafe { System.realloc(p, layout, size) }
        }
    }
    fn prepared(latencies: &[u32]) -> Mix {
        let clock = Arc::new(Clock::new());
        let mut readers = Vec::new();
        let maximum = *latencies.iter().max().unwrap();
        for (index, latency) in latencies.iter().enumerate() {
            let (mut tx, rx) = RingBuffer::new(8);
            let stats = clock.tracks[index].clone();
            stats.active.store(true, Ordering::Relaxed);
            let mut plugin = crate::graph::Delay::new(*latency);
            let mut compensate = crate::graph::Delay::new(maximum - latency);
            for block in 0..6 {
                let mut frames = [[0.0; 2]; BLOCK];
                for (offset, frame) in frames.iter_mut().enumerate() {
                    let source = if block * BLOCK + offset == 37 {
                        [0.125; 2]
                    } else {
                        [0.0; 2]
                    };
                    *frame = compensate.apply(plugin.apply(source));
                }
                tx.push(Packet {
                    at: (block * BLOCK) as u64,
                    epoch: 0,
                    token: 0,
                    transport: 0,
                    frames,
                })
                .ok()
                .unwrap();
            }
            readers.push(Reader {
                used: false,
                epoch: 0,
                token: 0,
                transitioning: true,
                queue: rx,
                next: None,
                stats,
            });
        }
        Mix {
            readers,
            clock,
            position: 0,
            channels: 4,
            pair: [2, 3],
            bound: 8,
            lead: 0,
            transport: None,
        }
    }
    #[test]
    fn independent_latency_paths_align_across_driver_partitions_without_callback_allocation() {
        for latencies in [&[0, 257][..], &[17, 0, 257][..]] {
            let mut expected = None;
            for partition in [1, 73, 128, 511] {
                let mut mix = prepared(latencies);
                let mut output = vec![0.0; 768 * 4];
                COUNT.with(|c| c.set(Some(0)));
                for chunk in output.chunks_mut(partition * 4) {
                    mix.fill(chunk, 0);
                }
                let count = COUNT.with(|c| c.replace(None).unwrap());
                assert_eq!(count, 0);
                assert!(
                    mix.readers
                        .iter()
                        .all(|r| r.stats.stale.load(Ordering::Relaxed) == 0)
                );
                assert_eq!(output[(37 + 257) * 4 + 2], 0.125 * latencies.len() as f32);
                assert!(
                    output
                        .iter()
                        .enumerate()
                        .all(|(i, v)| i / 4 == 294 && i % 4 >= 2 || *v == 0.0)
                );
                if let Some(expected) = &expected {
                    assert_eq!(&output, expected);
                } else {
                    expected = Some(output);
                }
            }
        }
    }
    #[test]
    fn click_wraps_on_sample_and_only_feeds_the_selected_pair_without_allocating() {
        use crate::transport::Command;
        use clef_model::timeline::Timeline;
        let timing = Timeline {
            click: true,
            looping: true,
            loop_range: [0, 9],
            ..Default::default()
        };
        let (mut control, renderer) = Renderer::new(Prepared::new(&timing, 48_000).unwrap(), 0);
        let mut mix = prepared(&[0]);
        mix.readers.clear();
        mix.transport = Some(renderer);
        control.send(Command::Play).unwrap();
        let mut output = [0.0; 612 * 4];
        COUNT.with(|c| c.set(Some(0)));
        for chunk in output.chunks_mut(83 * 4) {
            mix.fill(chunk, 0);
        }
        let allocations = COUNT.with(|c| c.replace(None).unwrap());
        assert_eq!(allocations, 0);
        for frame in output.as_chunks::<4>().0 {
            assert_eq!(&frame[..2], &[0.0; 2]);
            assert_eq!(frame[2], frame[3]);
        }
        for wrapped in [0, 225, 450] {
            assert_eq!(output[wrapped * 4 + 2], 0.20);
        }
        control.send(Command::Stop).unwrap();
        mix.fill(&mut output, 0);
        assert!(output.iter().all(|sample| *sample == 0.0));
    }
    #[test]
    fn recording_clock_and_stop_flush_old_packets_without_allocating() {
        use crate::transport::Command;
        let timing = Timeline::default();
        let (mut control, renderer) = Renderer::new(Prepared::new(&timing, 48_000).unwrap(), 0);
        let mut mix = prepared(&[0]);
        mix.lead = 512;
        mix.readers.clear();
        mix.transport = Some(renderer);
        mix.clock.capture_target.store(1, Ordering::Release);
        let mut output = [0.0; 64 * 4];
        control.send(Command::Record).unwrap();
        COUNT.with(|counter| counter.set(Some(0)));
        mix.fill(&mut output, 17);
        let recording = mix.clock.record_window.get().unwrap();
        for _ in 0..15 {
            mix.fill(&mut output, 17);
        }
        let allocations = COUNT.with(|counter| counter.replace(None).unwrap());
        assert_eq!(allocations, 0);
        control.send(Command::Stop).unwrap();
        let old_generation = recording.generation;
        let (mut audio, pending) = RingBuffer::new(2);
        audio
            .push(Packet {
                at: mix.position,
                epoch: 0,
                token: 1,
                transport: old_generation,
                frames: [[0.5; 2]; BLOCK],
            })
            .ok()
            .unwrap();
        let track = mix.clock.tracks[0].clone();
        track.active.store(true, Ordering::Relaxed);
        track.token.store(1, Ordering::Relaxed);
        // The queue fixture is prepared outside the measured callback.
        mix.readers.push(Reader {
            used: false,
            epoch: 0,
            token: 0,
            transitioning: true,
            queue: pending,
            next: None,
            stats: track,
        });
        COUNT.with(|counter| counter.set(Some(0)));
        let stop = mix.position;
        mix.fill(&mut output, 17);
        let allocations = COUNT.with(|counter| counter.replace(None).unwrap());
        assert_eq!(allocations, 0);
        assert_eq!(recording.start, 512 + 2 * BLOCK as u64);
        assert_eq!(mix.clock.record_end.load(Ordering::Acquire), stop - 17);
        assert!(mix.clock.playback_generation.load(Ordering::Acquire) > old_generation);
        assert!(output.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn a_failed_path_contributes_silence_while_healthy_paths_keep_their_samples() {
        let mut mix = prepared(&[0, 257, 17]);
        let mut prefix = vec![0.0; 257 * 4];
        mix.fill(&mut prefix, 0);
        mix.clock.tracks[0].failed.store(true, Ordering::Release);
        let mut output = vec![0.0; BLOCK * 4];
        COUNT.with(|c| c.set(Some(0)));
        mix.fill(&mut output, 0);
        let allocations = COUNT.with(|c| c.replace(None).unwrap());
        assert_eq!(allocations, 0);
        assert_eq!(output[37 * 4 + 2], 0.25);
        assert!(mix.clock.tracks[0].misses.load(Ordering::Relaxed) == 0);
    }

    #[test]
    fn scheduler_overflow_emits_the_prepared_panic_without_allocating() {
        use crate::protocol::{MAX_EVENTS, Midi};
        use crate::recording::Scheduler;
        let mut scheduler = Scheduler::new();
        for _ in 0..=MAX_EVENTS {
            scheduler.live(Midi {
                offset: 0,
                bytes: [0x90, 60, 100],
            });
        }
        let mut events = Vec::with_capacity(MAX_EVENTS);
        COUNT.with(|counter| counter.set(Some(0)));
        assert!(scheduler.render(&mut events));
        let allocations = COUNT.with(|counter| counter.replace(None).unwrap());
        assert_eq!(allocations, 0);
        assert_eq!(events.len(), 48);
        assert!(events.iter().any(|event| event.bytes == [0xb0, 123, 0]));
        assert!(
            events
                .iter()
                .all(|event| event.bytes[0] & 0xf0 == 0xb0 && event.bytes[2] == 0)
        );
    }
}
