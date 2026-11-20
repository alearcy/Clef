//! Independent bounded worker paths sharing one device callback and sample clock.
use crate::inserts::{Chain, MAX_CHAIN_LATENCY, Processor};
use crate::mixer::{Matrix, MeterPeak, PreparedMix, Ramp, accumulate};
use crate::restart::InstrumentAudio;
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
use clef_model::{midi::Arrangement, mixer::Target, timeline::Timeline};
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
    meter: MeterPeak,
    pub midi_delivered: AtomicU64,
    pub midi_received: AtomicU64,
    pub midi_dropped: AtomicU64,
    pub playback_dropped: AtomicU64,
    pub playback_blocks: AtomicU64,
    pub error: std::sync::Mutex<Option<String>>,
    release: AtomicBool,
    audition: AtomicU32,
    mute_recording: AtomicBool,
    trim: AtomicU32,
    bypass: AtomicBool,
}
impl TrackStats {
    pub fn take_meter(&self) -> Frame {
        self.meter.take(self.token.load(Ordering::Acquire))
    }
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
    /// T38: bumped when a track joins the graph; the callback echoes the value it saw at the
    /// start of a callback in which every active track and the routing delivered packets of
    /// the current epoch, and stores that epoch in `aligned_epoch`.
    alignment_change: AtomicU64,
    aligned_change: AtomicU64,
    aligned_epoch: AtomicU64,
    pub latency: AtomicU32,
    pub clipped: AtomicU64,
    master_meter: MeterPeak,
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
    insert_chains: Mutex<HashMap<Target, Arc<Chain>>>,
    routing_mix: Mutex<Arc<PreparedMix>>,
    bus_ids: [AtomicU64; MAX_TRACKS],
    bus_meters: [MeterPeak; MAX_TRACKS],
    bus_count: AtomicU64,
    master_delay: AtomicU32,
    audition_activity: AtomicBool,
}
impl Clock {
    pub fn take_master_meter(&self) -> Frame {
        self.master_meter.take(0)
    }

    fn routing_mix(&self) -> Arc<PreparedMix> {
        self.routing_mix
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    fn set_routing_mix(&self, mix: PreparedMix) {
        let bus_count = mix.bus_len();
        for (index, slot) in self.bus_ids.iter().enumerate() {
            let id = mix.bus_id(index).unwrap_or(0);
            slot.store(id, Ordering::Release);
            if index < bus_count {
                let _ = self.bus_meters[index].take(0);
            }
        }
        self.bus_count.store(bus_count as u64, Ordering::Release);
        *self
            .routing_mix
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Arc::new(mix);
    }

    pub fn bus_meters(&self) -> Vec<(u64, Frame)> {
        let count = self.bus_count.load(Ordering::Acquire) as usize;
        self.bus_ids
            .iter()
            .take(count.min(MAX_TRACKS))
            .enumerate()
            .filter_map(|(index, id)| {
                let id = id.load(Ordering::Acquire);
                (id != 0).then(|| (id, self.bus_meters[index].take(0)))
            })
            .collect()
    }

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
            alignment_change: AtomicU64::new(0),
            aligned_change: AtomicU64::new(u64::MAX),
            aligned_epoch: AtomicU64::new(u64::MAX),
            latency: AtomicU32::new(0),
            clipped: AtomicU64::new(0),
            master_meter: MeterPeak::default(),
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
            insert_chains: Mutex::default(),
            routing_mix: Mutex::new(Arc::new(PreparedMix::default())),
            bus_ids: std::array::from_fn(|_| AtomicU64::new(0)),
            bus_meters: std::array::from_fn(|_| MeterPeak::default()),
            bus_count: AtomicU64::new(0),
            master_delay: AtomicU32::new(0),
            audition_activity: AtomicBool::new(false),
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
    fn aligned(&self) -> bool {
        let epoch = self.epoch.load(Ordering::Acquire);
        epoch.is_multiple_of(2)
            && self.aligned_epoch.load(Ordering::Acquire) == epoch
            && self.aligned_change.load(Ordering::Acquire)
                == self.alignment_change.load(Ordering::Acquire)
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

    fn chain(&self, target: Target) -> Option<Arc<Chain>> {
        self.insert_chains
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&target)
            .cloned()
    }

    fn total_latency(&self) -> u64 {
        u64::from(self.latency.load(Ordering::Acquire))
            + u64::from(self.master_delay.load(Ordering::Acquire))
    }
}
#[derive(Default)]
struct ChannelMix {
    token: Option<u64>,
    ramp: Ramp,
    peak: Frame,
}
#[derive(Default)]
struct DeviceMix {
    settings: PreparedMix,
    incoming: Option<Consumer<PreparedMix>>,
    master: Ramp,
    ramp_samples: u32,
}

#[derive(Clone, Copy, Default)]
struct DrySample {
    frame: Frame,
    position: u64,
    epoch: u64,
    transport: u64,
}

const MAX_ROUTING_LATENCY: u32 = MAX_CHAIN_LATENCY.saturating_mul(2);

type BusFrames = Vec<[Frame; BLOCK]>;

struct RoutingPacket {
    at: u64,
    epoch: u64,
    transport: u64,
    direct: [Frame; BLOCK],
    buses: BusFrames,
}

struct RoutingInput {
    lead: u64,
    bound: usize,
    input: Producer<RoutingPacket>,
    output: Consumer<Packet>,
    free: Consumer<BusFrames>,
    next: Option<Packet>,
    pending: RoutingPacket,
    spare: Option<BusFrames>,
    dry: Vec<DrySample>,
    /// False when the last `process` call substituted the delayed dry signal.
    hit: bool,
}

impl RoutingInput {
    fn begin(&mut self, position: u64, epoch: u64, transport: u64, bus_len: usize) {
        if !(position as usize).is_multiple_of(BLOCK) {
            return;
        }
        self.pending.at = position;
        self.pending.epoch = epoch;
        self.pending.transport = transport;
        self.pending.direct = [[0.0; 2]; BLOCK];
        self.pending.buses[..bus_len.min(MAX_TRACKS)].fill([[0.0; 2]; BLOCK]);
    }

    fn finish(&mut self, position: u64) {
        if position as usize % BLOCK != BLOCK - 1 {
            return;
        }
        // Move the bus buffers into the packet instead of copying them through
        // the audio thread's small stack. The broker returns each buffer to the
        // free ring after use.
        let Some(spare) = self.spare.take().or_else(|| self.free.pop().ok()) else {
            return;
        };
        let buses = std::mem::replace(&mut self.pending.buses, spare);
        let packet = RoutingPacket {
            at: self.pending.at,
            epoch: self.pending.epoch,
            transport: self.pending.transport,
            direct: self.pending.direct,
            buses,
        };
        match self.input.push(packet) {
            Ok(()) => self.spare = self.free.pop().ok(),
            Err(rtrb::PushError::Full(packet)) => self.spare = Some(packet.buses),
        }
    }

    fn process(
        &mut self,
        source: Frame,
        position: u64,
        epoch: u64,
        transport: u64,
        clock: &Clock,
    ) -> Frame {
        let delay = u64::from(clock.master_delay.load(Ordering::Acquire));
        self.hit = true;
        if delay == 0 {
            return source;
        }
        let capacity = self.dry.len() as u64;
        self.dry[(position % capacity) as usize] = DrySample {
            frame: source,
            position,
            epoch,
            transport,
        };
        let offset = position as usize % BLOCK;
        if offset == 0 {
            for _ in 0..=self.bound {
                if self.next.is_none() {
                    self.next = self.output.pop().ok();
                }
                match &self.next {
                    Some(packet)
                        if packet.epoch != epoch
                            || packet.transport != transport
                            || packet.at < position =>
                    {
                        self.next = None
                    }
                    _ => break,
                }
            }
        }
        if let Some(packet) = &self.next
            && packet.epoch == epoch
            && packet.transport == transport
            && packet.at == position - offset as u64
        {
            return packet.frames[offset];
        }
        self.hit = false;

        if position >= self.lead && offset == 0 {
            clock.transitions.fetch_add(BLOCK as u64, Ordering::Relaxed);
        }
        position.checked_sub(delay).map_or([0.0; 2], |at| {
            let dry = self.dry[(at % capacity) as usize];
            if dry.position == at && dry.epoch == epoch && dry.transport == transport {
                dry.frame
            } else {
                [0.0; 2]
            }
        })
    }
}

struct BusWorker {
    processor: Processor,
    latency: u32,
    delay: crate::graph::Delay,
    ramp: Ramp,
}

impl Default for BusWorker {
    fn default() -> Self {
        Self {
            processor: Processor::default(),
            latency: 0,
            delay: crate::graph::Delay::new(0),
            ramp: Ramp::default(),
        }
    }
}

struct RoutingBroker {
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl RoutingBroker {
    fn start(
        clock: Arc<Clock>,
        bound: usize,
        lead: u64,
        rate: u32,
    ) -> Result<(Self, RoutingInput)> {
        let (input, mut receive) = RingBuffer::<RoutingPacket>::new(bound);
        let (mut send, output) = RingBuffer::new(bound);
        let (mut free_send, free_receive) = RingBuffer::<BusFrames>::new(4);
        let bus_buffer = || {
            let mut frames = vec![[[0.0; 2]; BLOCK]; MAX_TRACKS];
            frames.shrink_to_fit();
            frames
        };
        for _ in 0..3 {
            free_send.push(bus_buffer())?;
        }
        let mut dry = Vec::new();
        let capacity = MAX_ROUTING_LATENCY as usize + lead as usize + 1;
        dry.try_reserve_exact(capacity)?;
        dry.resize(capacity, DrySample::default());
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let worker = std::thread::Builder::new()
            .name("Clef routing".into())
            .spawn(move || {
                let _ = crate::threading::audio_priority(true);
                let idle = idle_wait(rate);
                let mut master = Processor::default();
                let mut direct_delay = crate::graph::Delay::new(0);
                let mut buses: Vec<BusWorker> = Vec::with_capacity(MAX_TRACKS);
                let mut free_send = free_send;
                let budget =
                    Duration::from_secs_f64((lead - BLOCK as u64) as f64 / f64::from(rate));
                let mut structural = true;
                while !stopping.load(Ordering::Acquire) {
                    let mix = clock.routing_mix();
                    let mut changed = false;
                    if buses.len() != mix.bus_len() {
                        buses.resize_with(mix.bus_len(), BusWorker::default);
                        changed = true;
                    }
                    let mut max_bus_latency = 0_u32;
                    let mut any_chain = false;
                    for (index, worker) in buses.iter_mut().enumerate() {
                        let id = mix.bus_id(index).unwrap_or(0);
                        let chain = clock.chain(Target::Bus(id));
                        any_chain |= chain.is_some();
                        if worker.processor.set_chain(chain) {
                            changed = true;
                        }
                        worker.latency = worker.processor.latency();
                        max_bus_latency = max_bus_latency.max(worker.latency);
                    }
                    let master_chain = clock.chain(Target::Master);
                    any_chain |= master_chain.is_some();
                    if master.set_chain(master_chain) {
                        changed = true;
                    }
                    let delay = if any_chain {
                        (lead as u32)
                            .saturating_add(max_bus_latency)
                            .saturating_add(master.latency())
                    } else {
                        0
                    };
                    if changed || structural {
                        for worker in &mut buses {
                            worker.delay = crate::graph::Delay::new(
                                max_bus_latency.saturating_sub(worker.latency),
                            );
                        }
                        direct_delay = crate::graph::Delay::new(max_bus_latency);
                        clock.master_delay.store(delay, Ordering::Release);
                        clock.epoch.fetch_add(2, Ordering::AcqRel);
                        structural = false;
                    }
                    let Ok(packet) = receive.pop() else {
                        // Without chains the callback sends nothing; poll for new chains
                        // slowly. With chains, a packet arrives once per block.
                        std::thread::sleep(if delay == 0 { idle * 8 } else { idle });
                        continue;
                    };
                    let RoutingPacket {
                        at,
                        epoch: packet_epoch,
                        transport,
                        direct,
                        buses: mut bus_frames,
                    } = packet;
                    let epoch = clock.epoch.load(Ordering::Acquire);
                    if packet_epoch != epoch || !epoch.is_multiple_of(2) || delay == 0 {
                        let _ = free_send.push(bus_frames);
                        continue;
                    }
                    let now = clock.played.load(Ordering::Acquire);
                    if at + lead + BLOCK as u64 <= now {
                        let _ = free_send.push(bus_frames);
                        continue;
                    }
                    let position = at;
                    let deadline = Instant::now() + budget;
                    let mut restart = false;
                    let mut sum = direct.map(|frame| direct_delay.apply(frame));
                    for (index, worker) in buses.iter_mut().enumerate() {
                        let trim = Matrix::trim(mix.buses()[index].trim);
                        for frame in &mut bus_frames[index] {
                            *frame = trim.apply(*frame);
                        }
                        if worker
                            .processor
                            .process(position, &mut bus_frames[index], deadline)
                        {
                            worker.latency = worker.processor.latency();
                            restart = true;
                            break;
                        }
                        for frame in &mut bus_frames[index] {
                            *frame = worker.delay.apply(*frame);
                        }
                        worker
                            .ramp
                            .set(mix.buses()[index].matrix, (rate / 200).max(1));
                        let mut peak = [0.0; 2];
                        for (out, frame) in sum.iter_mut().zip(bus_frames[index]) {
                            let wet = worker.ramp.apply(frame);
                            accumulate(&mut peak, wet);
                            out[0] += wet[0];
                            out[1] += wet[1];
                        }
                        clock.bus_meters[index].publish(0, peak);
                    }
                    if restart {
                        let _ = free_send.push(bus_frames);
                        structural = true;
                        continue;
                    }
                    if master.process(position, &mut sum, deadline) {
                        let _ = free_send.push(bus_frames);
                        structural = true;
                        continue;
                    }
                    let _ = send.push(Packet {
                        at: at.saturating_add(lead),
                        epoch,
                        token: 0,
                        transport,
                        frames: sum,
                    });
                    let _ = free_send.push(bus_frames);
                }
            })?;
        Ok((
            Self {
                stop,
                worker: Some(worker),
            },
            RoutingInput {
                lead,
                bound,
                input,
                output,
                free: free_receive,
                next: None,
                pending: RoutingPacket {
                    at: 0,
                    epoch: 0,
                    transport: 0,
                    direct: [[0.0; 2]; BLOCK],
                    buses: bus_buffer(),
                },
                spare: Some(bus_buffer()),
                dry,
                hit: false,
            },
        ))
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for RoutingBroker {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Reader {
    mix: ChannelMix,
    used: bool,
    epoch: u64,
    token: u64,
    transitioning: bool,
    queue: Consumer<Packet>,
    next: Option<Packet>,
    stats: Arc<TrackStats>,
}
struct Mix {
    mixer: DeviceMix,
    readers: Vec<Reader>,
    clock: Arc<Clock>,
    position: u64,
    channels: usize,
    pair: [usize; 2],
    bound: usize,
    lead: u64,
    transport: Option<Renderer>,
    routing: Option<RoutingInput>,
    /// Per-sample bus sums; cleared before every sample is accumulated.
    bus_frame: [Frame; MAX_TRACKS],
    bus_peaks: [[f32; 2]; MAX_TRACKS],
    /// Declicks bus gain/fader changes on the callback path, keyed by bus id.
    bus_ramps: [(u64, Ramp); MAX_TRACKS],
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
        if let Some(incoming) = self.mixer.incoming.as_mut() {
            // The publisher and callback each do bounded work, even if the sender is faster.
            for _ in 0..2 {
                if let Ok(settings) = incoming.pop() {
                    self.mixer.settings = settings;
                }
            }
        }
        let ramp_samples = self.mixer.ramp_samples.max(1);
        self.mixer
            .master
            .set(self.mixer.settings.master, ramp_samples);
        let mut master_peak = [0.0; 2];
        for reader in &mut self.readers {
            let token = reader.stats.token.load(Ordering::Acquire);
            let id = reader.stats.id.load(Ordering::Acquire);
            let target = self.mixer.settings.matrix(id);
            if reader.mix.token == Some(token) {
                reader.mix.ramp.set(target, ramp_samples);
            } else {
                reader.mix.token = Some(token);
                reader.mix.ramp.reset(target);
            }
            reader.mix.peak = [0.0; 2];
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
            if self.clock.audition_activity.swap(false, Ordering::AcqRel) {
                transport.resume_gate();
            }
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
        let bus_len = self.mixer.settings.bus_len();
        for (bus, (id, ramp)) in self.mixer.settings.buses().iter().zip(&mut self.bus_ramps) {
            if *id == bus.id {
                ramp.set(bus.direct, ramp_samples);
            } else {
                *id = bus.id;
                ramp.reset(bus.direct);
            }
        }
        let routing_active =
            self.routing.is_some() && self.clock.master_delay.load(Ordering::Acquire) != 0;
        self.bus_peaks[..bus_len].fill([0.0; 2]);
        let alignment_change = self.clock.alignment_change.load(Ordering::Acquire);
        let start_epoch = self.clock.epoch.load(Ordering::Acquire);
        let mut aligned = true;
        for frame in output.chunks_exact_mut(self.channels) {
            let epoch = self.clock.epoch.load(Ordering::Acquire);
            let (click, gain) = self.transport.as_mut().map_or((0.0, 1.0), |transport| {
                let click = transport.sample();
                (click, transport.gain())
            });
            let offset = self.position as usize % BLOCK;
            // A callback can span several blocks: every block gets its own routing
            // packet header, and the bus sums restart at every sample.
            if routing_active
                && offset == 0
                && let Some(routing) = &mut self.routing
            {
                routing.begin(self.position, epoch, playback_generation, bus_len);
            }
            self.bus_frame[..bus_len].fill([0.0; 2]);
            let mut direct = [click; 2];
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
                    aligned &= epoch.is_multiple_of(2);
                    continue;
                }
                aligned &= !reader.transitioning
                    || reader
                        .next
                        .as_ref()
                        .is_some_and(|packet| packet.at <= self.position);
                if let Some(packet) = &reader.next
                    && packet.at <= self.position
                {
                    reader.transitioning = false;
                    reader.used = true;
                    if reader.mix.token == Some(token) {
                        let source = packet.frames[(self.position - packet.at) as usize];
                        let processed = reader.mix.ramp.apply(source);
                        accumulate(&mut reader.mix.peak, processed);
                        let id = reader.stats.id.load(Ordering::Acquire);
                        if let Some(route) = self.mixer.settings.channel(id) {
                            match route.output {
                                crate::mixer::Output::Master => {
                                    for (sum, sample) in direct.iter_mut().zip(processed) {
                                        if sample.is_finite() {
                                            *sum += sample;
                                        }
                                    }
                                }
                                crate::mixer::Output::Bus(bus) => {
                                    if bus < bus_len {
                                        for (sum, sample) in
                                            self.bus_frame[bus].iter_mut().zip(processed)
                                        {
                                            if sample.is_finite() {
                                                *sum += sample;
                                            }
                                        }
                                    }
                                }
                            }
                            let gate = route.gate;
                            let pre = [source[0] * gate, source[1] * gate];
                            let post_send = route.send_matrix.apply(source);
                            for send in self.mixer.settings.sends(route) {
                                if send.bus >= bus_len || send.gain == 0.0 {
                                    continue;
                                }
                                let tap = if send.pre {
                                    pre
                                } else if gate == 0.0 {
                                    post_send
                                } else {
                                    processed
                                };
                                for (sum, sample) in self.bus_frame[send.bus].iter_mut().zip(tap) {
                                    if sample.is_finite() {
                                        *sum += sample * send.gain;
                                    }
                                }
                            }
                        } else {
                            for (sum, sample) in direct.iter_mut().zip(processed) {
                                if sample.is_finite() {
                                    *sum += sample;
                                }
                            }
                        }
                    }
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
            let sum = if routing_active {
                let mut dry = direct;
                for bus in 0..bus_len {
                    let wet = self.bus_ramps[bus].1.apply(self.bus_frame[bus]);
                    for (sum, sample) in dry.iter_mut().zip(wet) {
                        if sample.is_finite() {
                            *sum += sample;
                        }
                    }
                }
                if let Some(routing) = &mut self.routing {
                    routing.pending.direct[offset] = direct;
                    for bus in 0..bus_len {
                        routing.pending.buses[bus][offset] = self.bus_frame[bus];
                    }
                    let wet = routing.process(
                        dry,
                        self.position,
                        epoch,
                        playback_generation,
                        &self.clock,
                    );
                    aligned &= routing.hit;
                    routing.finish(self.position);
                    wet
                } else {
                    dry
                }
            } else {
                let mut sum = direct;
                for bus in 0..bus_len {
                    let wet = self.bus_ramps[bus].1.apply(self.bus_frame[bus]);
                    accumulate(&mut self.bus_peaks[bus], wet);
                    for (sum, sample) in sum.iter_mut().zip(wet) {
                        if sample.is_finite() {
                            *sum += sample;
                        }
                    }
                }
                if !epoch.is_multiple_of(2) {
                    [0.0; 2]
                } else {
                    sum
                }
            };
            let sum = self.mixer.master.apply(sum);
            let sum = [sum[0] * gain, sum[1] * gain];
            accumulate(&mut master_peak, sum);
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
        if routing_active {
            // The routing worker publishes the bus meters.
        } else {
            for bus in 0..bus_len {
                self.clock.bus_meters[bus].publish(0, self.bus_peaks[bus]);
            }
        }
        if let Some(transport) = &mut self.transport {
            if transport.take_record_stop() {
                self.clock.record_end.store(
                    self.position.saturating_sub(output_latency),
                    Ordering::Release,
                );
            }
            transport.publish(
                output_latency
                    .saturating_add(count)
                    .saturating_add(u64::from(self.clock.master_delay.load(Ordering::Acquire))),
            );
        }
        self.clock.master_meter.publish(0, master_peak);
        for reader in &self.readers {
            if let Some(token) = reader.mix.token {
                reader.stats.meter.publish(token, reader.mix.peak);
            }
        }
        let epoch = self.clock.epoch.load(Ordering::Acquire);
        if aligned && epoch == start_epoch && epoch.is_multiple_of(2) {
            self.clock.aligned_epoch.store(epoch, Ordering::Release);
            self.clock
                .aligned_change
                .store(alignment_change, Ordering::Release);
        }
        self.clock.played.store(self.position, Ordering::Release);
    }
}
/// Broker wait while its queue is full or empty. Half a block keeps the refill
/// well inside the lead (at least four blocks) without spinning at realtime priority.
pub(crate) fn idle_wait(rate: u32) -> Duration {
    Duration::from_secs_f64(BLOCK as f64 / f64::from(rate.max(1)) / 2.0)
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
    audio: Option<AudioConnection>,
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
    mixer: Producer<PreparedMix>,
    desired_mix: PreparedMix,
    mix_pending: bool,
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
    routing: RoutingBroker,
    published: Mutex<PublishedArrangement>,
}
/// The inputs of the plans last published, so an edit rebuilds only the tracks it changed.
#[derive(Default)]
struct PublishedArrangement {
    map: Option<SampleMap>,
    clips: std::collections::BTreeMap<u64, Vec<clef_model::midi::Clip>>,
}
impl PublishedArrangement {
    /// Build the plans for `tracks`, reusing the previous plan of every track whose
    /// clips and sample map are unchanged so its broker keeps its held notes.
    fn prepare(
        &mut self,
        previous: &HashMap<u64, Arc<Plan>>,
        tracks: &std::collections::BTreeMap<u64, Vec<clef_model::midi::Clip>>,
        map: SampleMap,
    ) -> Result<HashMap<u64, Arc<Plan>>> {
        let same_map = self
            .map
            .as_ref()
            .is_some_and(|published| published.same_mapping(&map));
        let plans = tracks
            .iter()
            .map(|(&id, clips)| {
                if same_map
                    && self.clips.get(&id) == Some(clips)
                    && let Some(plan) = previous.get(&id)
                {
                    return Ok((id, plan.clone()));
                }
                Ok((id, Arc::new(Plan::new(clips, &map)?)))
            })
            .collect::<Result<_>>()?;
        self.map = Some(map);
        self.clips.clone_from(tracks);
        Ok(plans)
    }
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
                mix: ChannelMix::default(),
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
        // T39: MIDI input is optional; the session reports a missing port.
        let midi_id = midi_id.filter(|id| !id.is_empty());
        let midi = midi_id
            .as_deref()
            .map(|midi_id| {
                audio_io::midi_input(
                    "Clef session",
                    "Clef selected track",
                    Some(midi_id),
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
                        if let Some((slot, stats)) =
                            state.tracks.iter().enumerate().find(|(_, s)| {
                                s.id.load(Ordering::Acquire) == selected
                                    && s.active.load(Ordering::Acquire)
                            })
                        {
                            let token = stats.token.load(Ordering::Acquire);
                            if revision != state.revision.load(Ordering::Acquire) {
                                return;
                            }
                            let now =
                                state.origin.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
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
                )
            })
            .transpose()?;
        let (transport, renderer) = Renderer::new(timing, tick);
        let (mixer, incoming) = RingBuffer::new(2);
        let (routing, routing_input) = RoutingBroker::start(clock.clone(), bound, lead, rate)?;
        let mut mix = Mix {
            mixer: DeviceMix {
                incoming: Some(incoming),
                ramp_samples: rate / 200,
                ..Default::default()
            },
            readers,
            clock: clock.clone(),
            position: 0,
            channels,
            pair,
            bound,
            lead,
            transport: Some(renderer),
            routing: Some(routing_input),
            bus_frame: [[0.0; 2]; MAX_TRACKS],
            bus_ramps: std::array::from_fn(|_| (0, Ramp::default())),
            bus_peaks: [[0.0; 2]; MAX_TRACKS],
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
            mixer,
            desired_mix: PreparedMix::default(),
            mix_pending: false,
            stream,
            midi,
            outputs,
            inputs,
            clock,
            rate,
            buffer,
            lead,
            transport,
            capture,
            routing,
            published: Mutex::default(),
        })
    }
    pub fn set_mix(
        &mut self,
        project: &clef_model::project::Project,
        preview: Option<(clef_model::mixer::Target, clef_model::mixer::Control, i16)>,
    ) {
        let settings = PreparedMix::new(project, preview);
        if self.desired_mix != settings {
            self.clock.set_routing_mix(settings.clone());
            self.desired_mix = settings;
            for stats in &self.clock.tracks {
                let trim = self.desired_mix.trim(stats.id.load(Ordering::Acquire));
                stats.trim.store(trim.to_bits(), Ordering::Release);
            }
            self.mix_pending = true;
        }
        self.flush_mix();
    }

    pub fn bus_meters(&self) -> Vec<(u64, Frame)> {
        self.clock.bus_meters()
    }

    pub fn flush_mix(&mut self) {
        if self.mix_pending && self.mixer.push(self.desired_mix.clone()).is_ok() {
            self.mix_pending = false;
        }
    }

    pub fn set_instrument_bypass(&self, id: u64, bypass: bool) {
        if let Some(stats) = self
            .clock
            .tracks
            .iter()
            .find(|stats| stats.id.load(Ordering::Acquire) == id)
        {
            stats.bypass.store(bypass, Ordering::Release);
        }
    }

    pub fn set_insert_chain(&mut self, target: Target, chain: Option<Chain>) {
        let mut chains = self
            .clock
            .insert_chains
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let structural = match (chains.get(&target), &chain) {
            (Some(current), Some(next)) => !current.same_effects(next),
            (None, None) => return,
            _ => true,
        };
        if let Some(chain) = chain {
            chains.insert(target, Arc::new(chain));
        } else {
            chains.remove(&target);
        }
        // A bypass-only replacement keeps the same stage topology, latency and
        // compensation delay; the worker crossfades it without an epoch change.
        if structural {
            self.clock.epoch.fetch_add(2, Ordering::AcqRel);
        }
    }

    pub fn set_arrangement(&self, arrangement: &Arrangement, timeline: &Timeline) -> Result<()> {
        if arrangement.tracks.is_empty() && self.clock.plan_revision.load(Ordering::Acquire) == 0 {
            return Ok(());
        }
        let map = SampleMap::new(timeline, self.rate)?;
        let previous = self
            .clock
            .plans
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let plans = self
            .published
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .prepare(&previous, &arrangement.tracks, map)?;
        let retired = std::mem::replace(
            &mut *self
                .clock
                .plans
                .lock()
                .unwrap_or_else(|error| error.into_inner()),
            plans,
        );
        drop(retired);
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
        self.clock.audition_activity.store(true, Ordering::Release);
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
        let mut audio = InstrumentAudio::new(instrument.take_audio()?);
        let initial_latency = instrument.latency;
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
        *stats
            .error
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = None;
        stats.bypass.store(false, Ordering::Release);
        stats.id.store(id, Ordering::Release);
        let initial_trim = self.desired_mix.trim(id);
        stats.trim.store(initial_trim.to_bits(), Ordering::Release);
        // T38: count the track's insert chain now, so the broker's first chain check does not
        // publish a second latency change after the transport may already have started.
        let chain_latency = self
            .clock
            .chain(Target::Track(id))
            .map_or(0, |chain| chain.latency());
        stats
            .latency
            .store(instrument.latency + chain_latency, Ordering::Release);
        let token = stats.token.fetch_add(1, Ordering::AcqRel) + 1;
        stats.meter.take(token);
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
        self.clock.alignment_change.fetch_add(1, Ordering::AcqRel);
        let clock = self.clock.clone();
        let lead = self.lead;
        let rate = self.rate;
        let audition_frames = u64::from(self.rate) / 6;
        let idle = idle_wait(self.rate);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let report = stats.clone();
        let thread = std::thread::spawn(move || {
            let mut elapsed =
                clock.played.load(Ordering::Acquire).div_ceil(BLOCK as u64) * BLOCK as u64;
            let mut revision = clock.revision.load(Ordering::Acquire);
            let mut delay = crate::graph::Delay::new(0);
            let mut inserts = Processor::default();
            let mut trim = Ramp::default();
            trim.reset(Matrix::trim(initial_trim));
            let initial_bypass = report.bypass.load(Ordering::Acquire);
            let mut bypass = Ramp::default();
            bypass.reset(Matrix::trim(if initial_bypass { 0.0 } else { 1.0 }));
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
            let mut instrument_latency = initial_latency;
            let result: Result<()> = (|| {
                let _ = crate::threading::audio_priority(true);
                while !stopping.load(Ordering::Acquire) {
                    if output.slots() == 0 {
                        std::thread::sleep(idle);
                        continue;
                    }
                    elapsed = catch_up(elapsed, clock.played.load(Ordering::Acquire), lead);
                    let epoch = clock.epoch.load(Ordering::Acquire);
                    if !epoch.is_multiple_of(2) {
                        std::thread::yield_now();
                        continue;
                    }
                    if inserts.set_chain(clock.chain(Target::Track(id))) {
                        report
                            .latency
                            .store(instrument_latency + inserts.latency(), Ordering::Release);
                        clock.refresh_latency();
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
                        let next = plans.get(&id).cloned();
                        drop(plans);
                        plan_revision = next_plan;
                        // Another track's edit republishes this track's plan unchanged.
                        let same = match (&plan, &next) {
                            (Some(current), Some(next)) => Arc::ptr_eq(current, next),
                            (None, None) => true,
                            _ => false,
                        };
                        if !same {
                            plan = next;
                            scheduler.replace_plan();
                        }
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
                    if let Some(restarted) = audio.poll() {
                        let restarted = restarted?;
                        // T38 R3: a restart that keeps the latency needs no realignment.
                        if restarted == instrument_latency {
                            continue;
                        }
                        instrument_latency = restarted;
                        report
                            .latency
                            .store(instrument_latency + inserts.latency(), Ordering::Release);
                        clock.refresh_latency();
                        clock.epoch.fetch_add(2, Ordering::AcqRel);
                        elapsed += BLOCK as u64;
                        continue;
                    }
                    // While a restart runs the instrument contributes silence; the insert
                    // chain, meters and the queue keep running (T37 L2).
                    let mut frames: [Frame; BLOCK] = [[0.0; 2]; BLOCK];
                    let mut restart = false;
                    if let Some(connection) = audio.ready() {
                        let reply =
                            connection.process(crate::sequence::START + elapsed, &events, BLOCK)?;
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
                        if reply.restart || reply.latency != instrument_latency {
                            restart = true;
                        } else {
                            frames = reply.frames.try_into().map_err(|_| "wrong block size")?;
                        }
                    }
                    if restart {
                        audio.begin_restart(&control);
                    }
                    if epoch != clock.epoch.load(Ordering::Acquire) {
                        elapsed += BLOCK as u64;
                        continue;
                    }
                    let bypassed = report.bypass.load(Ordering::Acquire);
                    bypass.set(
                        Matrix::trim(if bypassed { 0.0 } else { 1.0 }),
                        (rate / 200).max(1),
                    );
                    for frame in &mut frames {
                        *frame = bypass.apply(*frame);
                    }
                    trim.set(
                        Matrix::trim(f32::from_bits(report.trim.load(Ordering::Acquire))),
                        (rate / 200).max(1),
                    );
                    for frame in &mut frames {
                        *frame = trim.apply(*frame);
                    }
                    let budget = Duration::from_secs_f64(lead as f64 / f64::from(rate));
                    if inserts.process(
                        crate::sequence::START + elapsed,
                        &mut frames,
                        started + budget,
                    ) {
                        report
                            .latency
                            .store(instrument_latency + inserts.latency(), Ordering::Release);
                        clock.refresh_latency();
                        clock.epoch.fetch_add(2, Ordering::AcqRel);
                        elapsed += BLOCK as u64;
                        continue;
                    }
                    for frame in &mut frames {
                        *frame = delay.apply(*frame);
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
                // A restart still running is waited for when the connection is returned.
                if let Some(connection) = audio.ready() {
                    let _ = connection.process(crate::sequence::START + elapsed, &releases, BLOCK);
                    connection.suspend()?;
                }
                Ok(())
            })();
            if let Err(error) = &result {
                *report
                    .error
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(error.to_string());
                report.failed.store(true, Ordering::Release);
            }
            Returned {
                audio: audio.into_inner(),
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
        if let Some(audio) = returned.audio {
            instrument.return_audio(audio);
        }
        self.outputs[broker.slot] = Some(returned.output);
        self.inputs[broker.slot] = Some(returned.input);
        if let Some(error) = returned.error {
            return Err(error.into());
        }
        instrument.stop()
    }
    /// T38: true once a callback has delivered the current epoch to every active track and
    /// the routing since the last track joined, so a transport start plays an aligned graph.
    pub fn aligned(&self) -> bool {
        self.clock.aligned()
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
            self.clock.total_latency(),
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
        self.routing.stop();
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
                mix: ChannelMix::default(),
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
            mixer: DeviceMix::default(),
            routing: None,
            bus_frame: [[0.0; 2]; MAX_TRACKS],
            bus_ramps: std::array::from_fn(|_| (0, Ramp::default())),
            bus_peaks: [[0.0; 2]; MAX_TRACKS],
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
            mixer: DeviceMix::default(),
            routing: None,
            bus_frame: [[0.0; 2]; MAX_TRACKS],
            bus_ramps: std::array::from_fn(|_| (0, Ramp::default())),
            bus_peaks: [[0.0; 2]; MAX_TRACKS],
            readers: vec![Reader {
                mix: ChannelMix::default(),
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
        assert_eq!(clock.tracks[0].take_meter(), [2.0; 2]);
        assert_eq!(clock.take_master_meter(), [2.0; 2]);
        assert_eq!(clock.take_master_meter(), [0.0; 2]);
        clock.epoch.store(1, Ordering::Release);
        mix.fill(&mut out, 0);
        assert_eq!(out, [0.0; 2]);
    }
    #[test]
    fn alignment_waits_for_every_active_track_to_deliver_the_current_epoch() {
        let clock = Arc::new(Clock::new());
        let (mut tx, rx) = RingBuffer::new(4);
        let stats = clock.tracks[0].clone();
        stats.active.store(true, Ordering::Relaxed);
        let mut mix = Mix {
            mixer: DeviceMix::default(),
            routing: None,
            bus_frame: [[0.0; 2]; MAX_TRACKS],
            bus_ramps: std::array::from_fn(|_| (0, Ramp::default())),
            bus_peaks: [[0.0; 2]; MAX_TRACKS],
            readers: vec![Reader {
                mix: ChannelMix::default(),
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
        let mut out = [0.0; 2 * BLOCK];
        mix.fill(&mut out, 0);
        assert!(
            !clock.aligned(),
            "a joined track without packets is not aligned"
        );
        let packet = |at| Packet {
            at,
            epoch: 0,
            token: 0,
            transport: 0,
            frames: [[0.0; 2]; BLOCK],
        };
        tx.push(packet(BLOCK as u64)).ok().unwrap();
        mix.fill(&mut out, 0);
        assert!(clock.aligned(), "every active track delivered the epoch");
        clock.alignment_change.fetch_add(1, Ordering::AcqRel);
        assert!(!clock.aligned(), "a newly joined track clears alignment");
        tx.push(packet(2 * BLOCK as u64)).ok().unwrap();
        mix.fill(&mut out, 0);
        assert!(clock.aligned());
        clock.epoch.store(2, Ordering::Release);
        assert!(!clock.aligned(), "an epoch change clears alignment");
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
                mix: ChannelMix::default(),
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
            mixer: DeviceMix::default(),
            routing: None,
            bus_frame: [[0.0; 2]; MAX_TRACKS],
            bus_ramps: std::array::from_fn(|_| (0, Ramp::default())),
            bus_peaks: [[0.0; 2]; MAX_TRACKS],
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
    fn bus_output_and_send_sum_in_the_callback_without_allocation() {
        use clef_model::project::{Project, Send, Track, TrackKind};
        let track = |id, output, sends| Track {
            id,
            name: format!("Track {id}"),
            plugin: None,
            instrument_bypassed: false,
            instrument_offline: false,
            record_mode: Default::default(),
            kind: TrackKind::Instrument,
            output,
            sends,
            mix: Default::default(),
            inserts: Vec::new(),
            take_lanes: Default::default(),
            state: Default::default(),
        };
        let bus = Track {
            id: 3,
            name: "Reverb".into(),
            plugin: None,
            instrument_bypassed: false,
            instrument_offline: false,
            record_mode: Default::default(),
            kind: TrackKind::Bus,
            output: None,
            sends: Vec::new(),
            mix: Default::default(),
            inserts: Vec::new(),
            take_lanes: Default::default(),
            state: Default::default(),
        };
        let mut project = Project::default();
        project.tracks.push(track(1, Some(3), Vec::new()));
        project.tracks.push(bus);
        project.tracks[1].mix.level.fader = -60;
        project.next_id = 4;

        let mut mix = prepared(&[0]);
        mix.clock.tracks[0].id.store(1, Ordering::Release);
        mix.mixer.settings = PreparedMix::new(&project, None);
        mix.mixer.ramp_samples = 1;
        let mut output = vec![0.0; 400 * 4];
        COUNT.with(|counter| counter.set(Some(0)));
        mix.fill(&mut output, 0);
        assert_eq!(COUNT.with(|counter| counter.replace(None)), Some(0));
        let gain = 10.0_f32.powf(-0.3);
        let expected = 0.125 * gain;
        assert!((output[37 * 4 + 2] - expected).abs() < 1e-6);
        assert!((output[37 * 4 + 3] - expected).abs() < 1e-6);

        project.tracks[0].output = None;
        project.tracks[0].sends = vec![Send {
            id: 5,
            target: 3,
            level: 0,
            pre: false,
            enabled: true,
        }];
        project.next_id = 6;
        let mut mix = prepared(&[0]);
        mix.clock.tracks[0].id.store(1, Ordering::Release);
        mix.mixer.settings = PreparedMix::new(&project, None);
        mix.mixer.ramp_samples = 1;
        let mut output = vec![0.0; 400 * 4];
        mix.fill(&mut output, 0);
        let expected = 0.125 + 0.125 * gain;
        assert!((output[37 * 4 + 2] - expected).abs() < 1e-6);
        assert!((output[37 * 4 + 3] - expected).abs() < 1e-6);
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

    fn bus_project(trim: i16) -> clef_model::project::Project {
        use clef_model::project::{Project, Track, TrackKind};
        let track = |id, kind, output| Track {
            id,
            name: format!("Track {id}"),
            plugin: None,
            instrument_bypassed: false,
            instrument_offline: false,
            record_mode: Default::default(),
            kind,
            output,
            sends: Vec::new(),
            mix: Default::default(),
            inserts: Vec::new(),
            take_lanes: Default::default(),
            state: Default::default(),
        };
        let mut project = Project::default();
        project
            .tracks
            .push(track(1, TrackKind::Instrument, Some(3)));
        project.tracks.push(track(3, TrackKind::Bus, None));
        project.tracks[1].mix.trim = trim;
        project.next_id = 4;
        project
    }

    #[test]
    fn bus_sum_restarts_every_block_when_a_callback_spans_several_blocks() {
        for partition in [400, 256, 73] {
            let mut mix = prepared(&[0]);
            mix.clock.tracks[0].id.store(1, Ordering::Release);
            mix.mixer.settings = PreparedMix::new(&bus_project(0), None);
            mix.mixer.ramp_samples = 1;
            let mut output = vec![0.0; 768 * 4];
            COUNT.with(|counter| counter.set(Some(0)));
            for chunk in output.chunks_mut(partition * 4) {
                mix.fill(chunk, 0);
            }
            assert_eq!(COUNT.with(|counter| counter.replace(None)), Some(0));
            let heard: Vec<usize> = (0..768).filter(|i| output[i * 4 + 2] != 0.0).collect();
            assert_eq!(heard, vec![37], "partition {partition}");
            assert_eq!(output[37 * 4 + 2], 0.125);
        }
    }

    #[test]
    fn bus_gain_applies_without_bus_inserts() {
        let mut mix = prepared(&[0]);
        mix.clock.tracks[0].id.store(1, Ordering::Release);
        mix.mixer.settings = PreparedMix::new(&bus_project(-60), None);
        mix.mixer.ramp_samples = 1;
        let mut output = vec![0.0; 256 * 4];
        mix.fill(&mut output, 0);
        let expected = 0.125 * 10.0_f32.powf(-0.3);
        assert!((output[37 * 4 + 2] - expected).abs() < 1e-6);
        assert!((output[37 * 4 + 3] - expected).abs() < 1e-6);
    }

    #[test]
    fn every_block_of_a_long_callback_gets_its_own_routing_packet() {
        let (input, mut read) = RingBuffer::<RoutingPacket>::new(8);
        let (_, output) = RingBuffer::<Packet>::new(1);
        let (mut free_send, free) = RingBuffer::<BusFrames>::new(4);
        for _ in 0..3 {
            free_send
                .push(vec![[[0.0; 2]; BLOCK]; MAX_TRACKS])
                .ok()
                .unwrap();
        }
        let mut mix = prepared(&[0]);
        mix.clock.tracks[0].id.store(1, Ordering::Release);
        mix.mixer.settings = PreparedMix::new(&bus_project(0), None);
        mix.mixer.ramp_samples = 1;
        mix.clock.master_delay.store(1, Ordering::Release);
        mix.routing = Some(RoutingInput {
            lead: 0,
            bound: 8,
            input,
            output,
            free,
            next: None,
            pending: RoutingPacket {
                at: u64::MAX,
                epoch: 0,
                transport: 0,
                direct: [[0.0; 2]; BLOCK],
                buses: vec![[[0.0; 2]; BLOCK]; MAX_TRACKS],
            },
            spare: Some(vec![[[0.0; 2]; BLOCK]; MAX_TRACKS]),
            dry: vec![DrySample::default(); 4 * BLOCK],
            hit: false,
        });
        let mut frames = vec![0.0; 3 * BLOCK * 4];
        COUNT.with(|counter| counter.set(Some(0)));
        mix.fill(&mut frames, 0);
        assert_eq!(COUNT.with(|counter| counter.replace(None)), Some(0));
        let packets: Vec<_> = std::iter::from_fn(|| read.pop().ok()).collect();
        assert_eq!(
            packets.iter().map(|packet| packet.at).collect::<Vec<_>>(),
            vec![0, BLOCK as u64, 2 * BLOCK as u64]
        );
        // The impulse at frame 37 reaches the bus once, in the first block only.
        assert_eq!(packets[0].buses[0][37], [0.125; 2]);
        assert!(
            packets[1..]
                .iter()
                .all(|packet| packet.buses[0].iter().all(|frame| *frame == [0.0; 2]))
        );
    }

    #[test]
    fn republishing_an_arrangement_keeps_the_plans_of_unchanged_tracks() {
        use clef_model::{midi::Clip, timeline::Timeline};
        use std::collections::BTreeMap;
        let clip: Clip =
            serde_json::from_str(r#"{"start":0,"end":960,"takes":[],"sections":[]}"#).unwrap();
        let map = || SampleMap::new(&Timeline::default(), 48_000).unwrap();
        let mut published = PublishedArrangement::default();
        let mut tracks = BTreeMap::from([(1, Vec::new()), (2, Vec::new())]);
        let first = published.prepare(&HashMap::new(), &tracks, map()).unwrap();
        tracks.insert(2, vec![clip]);
        let second = published.prepare(&first, &tracks, map()).unwrap();
        assert!(Arc::ptr_eq(&first[&1], &second[&1]));
        assert!(!Arc::ptr_eq(&first[&2], &second[&2]));
        let mut tempo = Timeline::default();
        tempo.changes[0].bpm = 90;
        let third = published
            .prepare(&second, &tracks, SampleMap::new(&tempo, 48_000).unwrap())
            .unwrap();
        assert!(!Arc::ptr_eq(&second[&1], &third[&1]));
    }

    #[test]
    fn routing_packet_moves_bus_buffers_through_the_handoff() {
        let (input, mut read) = RingBuffer::<RoutingPacket>::new(2);
        let (_, output) = RingBuffer::<Packet>::new(1);
        let (mut free_send, free) = RingBuffer::<BusFrames>::new(2);
        free_send.push(vec![[[0.0; 2]; BLOCK]; 2]).ok().unwrap();
        let mut routing = RoutingInput {
            lead: 0,
            bound: 2,
            input,
            output,
            free,
            next: None,
            pending: RoutingPacket {
                at: 0,
                epoch: 0,
                transport: 0,
                direct: [[0.0; 2]; BLOCK],
                buses: vec![[[0.0; 2]; BLOCK]; 2],
            },
            spare: Some(vec![[[0.0; 2]; BLOCK]; 2]),
            dry: Vec::new(),
            hit: false,
        };
        routing.begin(0, 1, 2, 2);
        routing.pending.direct[0] = [0.25, -0.25];
        routing.pending.buses[1][0] = [0.5, 0.5];
        routing.finish(BLOCK as u64 - 1);
        let packet = read.pop().unwrap();
        assert_eq!((packet.at, packet.epoch, packet.transport), (0, 1, 2));
        assert_eq!(packet.direct[0], [0.25, -0.25]);
        assert_eq!(packet.buses[1][0], [0.5, 0.5]);
        assert!(routing.spare.is_some() || routing.free.pop().is_ok());
    }

    #[test]
    fn routing_handoff_fits_on_a_small_audio_thread_stack() {
        let result = std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(|| {
                let (input, mut read) = RingBuffer::<RoutingPacket>::new(2);
                let (_, output) = RingBuffer::<Packet>::new(1);
                let (mut free_send, free) = RingBuffer::<BusFrames>::new(2);
                free_send
                    .push(vec![[[0.0; 2]; BLOCK]; MAX_TRACKS])
                    .ok()
                    .unwrap();
                let mut routing = RoutingInput {
                    lead: 0,
                    bound: 2,
                    input,
                    output,
                    free,
                    next: None,
                    pending: RoutingPacket {
                        at: 0,
                        epoch: 0,
                        transport: 0,
                        direct: [[0.0; 2]; BLOCK],
                        buses: vec![[[0.0; 2]; BLOCK]; MAX_TRACKS],
                    },
                    spare: Some(vec![[[0.0; 2]; BLOCK]; MAX_TRACKS]),
                    dry: Vec::new(),
                    hit: false,
                };
                routing.begin(0, 1, 2, 1);
                routing.finish(BLOCK as u64 - 1);
                read.pop().is_ok()
            })
            .unwrap()
            .join()
            .unwrap();
        assert!(result);
    }

    #[test]
    fn master_handoff_keeps_the_dry_signal_aligned_and_drops_late_wet_blocks() {
        let clock = Clock::new();
        let (input, _read) = RingBuffer::new(8);
        let (mut write, output) = RingBuffer::new(8);
        let (_, free) = RingBuffer::<BusFrames>::new(1);
        let lead = 2 * BLOCK as u64;
        clock.master_delay.store(lead as u32 + 1, Ordering::Release);
        let mut master = RoutingInput {
            lead,
            bound: 8,
            input,
            output,
            free,
            next: None,
            pending: RoutingPacket {
                at: 0,
                epoch: 0,
                transport: 0,
                direct: [[0.0; 2]; BLOCK],
                buses: Vec::new(),
            },
            spare: Some(Vec::new()),
            dry: vec![DrySample::default(); lead as usize + BLOCK],
            hit: false,
        };
        let mut heard = [[0.0; 2]; 4 * BLOCK];
        COUNT.with(|counter| counter.set(Some(0)));
        for (i, out) in heard.iter_mut().enumerate() {
            if i as u64 == lead + 1 {
                assert!(
                    write
                        .push(Packet {
                            at: lead,
                            epoch: 2,
                            token: 0,
                            transport: 1,
                            frames: [[1.0; 2]; BLOCK],
                        })
                        .is_ok()
                );
            }
            let source = if i == 0 { [0.5, -0.25] } else { [0.0; 2] };
            *out = master.process(source, i as u64, 2, 1, &clock);
        }
        let allocations = COUNT.with(|counter| counter.replace(None).unwrap());
        assert_eq!(allocations, 0);
        assert_eq!(heard[lead as usize + 1], [0.5, -0.25]);
        assert_eq!(heard.iter().filter(|&&frame| frame != [0.0; 2]).count(), 1);
    }

    #[test]
    fn mixed_aligned_tracks_publish_post_fader_peaks_without_allocating() {
        use clef_model::project::{Project, Track};
        let mut mix = prepared(&[0, 257]);
        let mut project = Project::default();
        for id in 1..=2 {
            mix.clock.tracks[(id - 1) as usize]
                .id
                .store(id, Ordering::Release);
            project.tracks.push(Track {
                id,
                name: format!("Instrument {id}"),
                plugin: None,
                instrument_bypassed: false,
                instrument_offline: false,
                record_mode: Default::default(),
                inserts: Vec::new(),
                take_lanes: Default::default(),
                kind: Default::default(),
                output: None,
                sends: Vec::new(),
                mix: Default::default(),
                state: Default::default(),
            });
        }
        project.tracks[0].mix.solo = true;
        project.tracks[0].mix.level.pan = -100;
        project.master.fader = -60;
        let (mut send, receive) = RingBuffer::new(2);
        assert!(send.push(PreparedMix::new(&project, None)).is_ok());
        mix.mixer.incoming = Some(receive);
        let mut frames = [0.0; 384 * 4];
        COUNT.with(|c| c.set(Some(0)));
        mix.fill(&mut frames, 0);
        assert_eq!(COUNT.with(|c| c.replace(None)), Some(0));
        assert_eq!(mix.clock.tracks[0].take_meter(), [0.125, 0.0]);
        assert_eq!(mix.clock.tracks[1].take_meter(), [0.0; 2]);
        let output = 0.125 * 10.0_f32.powf(-0.3);
        assert_eq!(mix.clock.take_master_meter(), [output, 0.0]);
        assert_eq!(&frames[(37 + 257) * 4..][..4], &[0.0, 0.0, output, 0.0]);
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
        for master_pan in [0, -100] {
            let (mut control, renderer) = Renderer::new(Prepared::new(&timing, 48_000).unwrap(), 0);
            let mut mix = prepared(&[0]);
            mix.readers.clear();
            let project = clef_model::project::Project {
                master: clef_model::mixer::Level {
                    pan: master_pan,
                    ..Default::default()
                },
                ..Default::default()
            };
            mix.mixer.settings = PreparedMix::new(&project, None);
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
                assert_eq!(frame[3], if master_pan == 0 { frame[2] } else { 0.0 });
            }
            for wrapped in [0, 225, 450] {
                assert_eq!(output[wrapped * 4 + 2], 0.20);
            }
            control.send(Command::Stop).unwrap();
            let mut fade = vec![0.0; 612 * 4];
            mix.fill(&mut fade, 0);
            assert!(fade[240 * 4..].iter().all(|sample| *sample == 0.0));
        }
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
            mix: ChannelMix::default(),
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
        let mut fade = vec![0.0; 1024 * 4];
        mix.fill(&mut fade, 17);
        let mut settle = vec![0.0; 256 * 4];
        mix.fill(&mut settle, 17);
        assert!(mix.clock.playback_generation.load(Ordering::Acquire) > old_generation);
        assert!(settle.iter().all(|sample| *sample == 0.0));
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
