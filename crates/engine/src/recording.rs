use crate::{
    protocol::{BLOCK, MAX_EVENTS, Midi},
    transport::SampleMap,
};
use clef_model::{
    Result,
    midi::{Clip, Event, Message, exposed, stateful_control, supported_control},
    timeline::MAX_TICK,
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::atomic::{AtomicU64, Ordering},
};

pub const MAX_CAPTURE_MESSAGES: usize = 16_384;
const KEYS: usize = 16 * 128;
const LEGACY_CONTROLS: usize = 16 * 3;
pub(crate) const CONTROLS: usize = LEGACY_CONTROLS + 16 * 128;
pub type ControllerBaseline = [[u8; 3]; CONTROLS];
const MAX_CURVE_EDGES: usize = 262_144;

pub(crate) fn control_index(bytes: [u8; 3]) -> Option<usize> {
    legacy_control_index(bytes).or_else(|| {
        stateful_control(bytes)
            .then(|| LEGACY_CONTROLS + usize::from(bytes[0] & 15) * 128 + usize::from(bytes[1]))
    })
}

fn legacy_control_index(bytes: [u8; 3]) -> Option<usize> {
    let kind = match (bytes[0] & 0xf0, bytes[1]) {
        (0xb0, 64) => 0,
        (0xb0, 1) => 1,
        (0xe0, _) => 2,
        _ => return None,
    };
    Some(usize::from(bytes[0] & 15) * 3 + kind)
}

fn neutral(index: usize) -> [u8; 3] {
    if index >= LEGACY_CONTROLS {
        return [0; 3];
    }
    let channel = (index / 3) as u8;
    match index % 3 {
        0 => [0xb0 | channel, 64, 0],
        1 => [0xb0 | channel, 1, 0],
        _ => [0xe0 | channel, 0, 64],
    }
}

pub(crate) fn capture_message(bytes: [u8; 3]) -> bool {
    matches!(bytes[0] & 0xf0, 0x80 | 0x90) || supported_control(bytes)
}

/// Mapping from a device presentation sample to the running musical timeline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Projection {
    pub generation: u64,
    pub start: u64,
    pub origin: u64,
    pub left: u64,
    pub right: u64,
    pub limit: u64,
}

impl Projection {
    pub fn position(self, at: u64) -> Option<(u64, u64)> {
        let offset = at.checked_sub(self.start)?;
        let frame = self.origin.saturating_add(offset);
        if self.right > self.left && self.origin < self.right && frame >= self.right {
            let len = self.right - self.left;
            let distance = frame - self.right;
            Some((1 + distance / len, self.left + distance % len))
        } else if frame < self.limit {
            Some((0, frame))
        } else {
            None
        }
    }

    fn before_end(self, frame: u64) -> u64 {
        if self.right > self.left && self.origin < self.right && frame < self.right {
            self.right - frame
        } else {
            self.limit - frame
        }
    }
}

/// One callback writer and non-blocking readers. Values contain no owned
/// resources, so publishing a transition never frees memory on the callback.
#[derive(Default)]
pub(crate) struct Published {
    stamp: AtomicU64,
    fields: [AtomicU64; 6],
}

impl Published {
    pub(crate) fn set(&self, value: Projection) {
        self.stamp.fetch_add(1, Ordering::AcqRel);
        for (field, value) in self.fields.iter().zip([
            value.generation,
            value.start,
            value.origin,
            value.left,
            value.right,
            value.limit,
        ]) {
            field.store(value, Ordering::Relaxed);
        }
        self.stamp.fetch_add(1, Ordering::Release);
    }

    pub(crate) fn get(&self) -> Option<Projection> {
        let before = self.stamp.load(Ordering::Acquire);
        if !before.is_multiple_of(2) {
            return None;
        }
        let [generation, start, origin, left, right, limit] = self
            .fields
            .each_ref()
            .map(|field| field.load(Ordering::Relaxed));
        std::sync::atomic::fence(Ordering::Acquire);
        (before == self.stamp.load(Ordering::Acquire)).then_some(Projection {
            generation,
            start,
            origin,
            left,
            right,
            limit,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Captured {
    pub at: u64,
    pub bytes: [u8; 3],
}

pub struct Recording {
    pub range: [u64; 2],
    pub passes: Vec<Vec<Event>>,
    pub truncated: bool,
}

struct OpenNote {
    tick: u64,
    order: u32,
    velocity: u8,
}

struct Passes {
    controls: [[u8; 3]; CONTROLS],
    held: BTreeMap<(u8, u8), VecDeque<OpenNote>>,
    current: Vec<Event>,
    passes: Vec<Vec<Event>>,
    number: u64,
    produced: usize,
    order: u32,
    has_input: bool,
}

impl Passes {
    fn seed_controls(&mut self, tick: u64) {
        for (index, bytes) in self.controls.into_iter().enumerate() {
            if bytes != neutral(index) {
                self.control(tick, bytes);
            }
        }
    }

    fn control(&mut self, tick: u64, bytes: [u8; 3]) {
        self.current.push(Event {
            tick,
            order: self.order,
            message: Message::Control(bytes),
        });
        self.order += 1;
    }

    fn note(&mut self, tick: u64, bytes: [u8; 3]) -> bool {
        let [status, key, velocity] = bytes;
        let channel = status & 15;
        let notes = self.held.entry((channel, key)).or_default();
        if status & 0xf0 == 0x90 && velocity != 0 {
            notes.push_back(OpenNote {
                tick,
                order: self.order,
                velocity,
            });
            self.order += 1;
        } else if let Some(note) = notes.pop_front() {
            self.end_note(channel, key, note, tick, velocity);
        } else {
            return false;
        }
        true
    }

    fn end_note(&mut self, channel: u8, key: u8, note: OpenNote, tick: u64, release: u8) {
        self.current.push(Event {
            tick: note.tick,
            order: note.order,
            message: Message::Note {
                channel,
                key,
                velocity: note.velocity,
                end: tick.max(note.tick + 1).min(MAX_TICK),
                release,
                release_order: self.order,
            },
        });
        self.order += 1;
    }

    fn finish(&mut self, end: u64, next: Option<u64>) {
        let mut held = std::mem::take(&mut self.held);
        for (&(channel, key), notes) in &held {
            for note in notes {
                let ending = OpenNote {
                    tick: note.tick,
                    order: note.order,
                    velocity: note.velocity,
                };
                self.end_note(channel, key, ending, end, 0);
            }
        }
        if self.has_input {
            self.current.sort_by_key(|event| (event.tick, event.order));
            self.passes.push(std::mem::take(&mut self.current));
        } else {
            self.current.clear();
        }
        self.has_input = false;
        if let Some(start) = next {
            self.seed_controls(start);
            for note in held.values_mut().flatten() {
                note.tick = start;
                note.order = self.order;
                self.order += 1;
                self.has_input = true;
            }
            self.held = held;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoteMark {
    pub start: u64,
    pub end: Option<u64>,
    pub key: u8,
}

impl NoteMark {
    pub fn from_event(event: &Event) -> Option<Self> {
        let Message::Note { key, end, .. } = event.message else {
            return None;
        };
        Some(Self {
            start: event.tick,
            end: Some(end),
            key,
        })
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct NotePreview {
    pub current: Vec<NoteMark>,
    pub previous: Vec<NoteMark>,
}

impl Passes {
    fn advance(&mut self, number: u64, [left, right]: [u64; 2]) -> bool {
        while self.number < number {
            if self.held.values().all(VecDeque::is_empty) && !self.has_input {
                self.number = number;
                self.current.clear();
                self.seed_controls(left);
                break;
            }
            let previous_len = self.passes.len();
            self.finish(right, Some(left));
            self.number += 1;
            if let Some(added) = self.passes.get(previous_len) {
                self.produced += added.len();
            }
            if self.produced >= MAX_CAPTURE_MESSAGES || self.passes.len() >= 1024 {
                return false;
            }
        }
        true
    }
}

/// Reuse the capture's timestamp, held-note, and wrap rules for the live view.
/// `input` is sorted and contains only packets through the requested position.
fn passes_through(
    map: &SampleMap,
    projection: Projection,
    number: u64,
    baseline: ControllerBaseline,
    input: &[Captured],
) -> (Passes, bool) {
    let mut passes = Passes {
        controls: std::array::from_fn(|index| {
            let bytes = baseline[index];
            if control_index(bytes) == Some(index) {
                bytes
            } else {
                neutral(index)
            }
        }),
        held: BTreeMap::new(),
        current: Vec::new(),
        passes: Vec::new(),
        number: 0,
        produced: 0,
        order: 0,
        has_input: false,
    };
    let before = input.partition_point(|packet| packet.at < projection.start);
    for packet in &input[..before] {
        if let Some(index) = control_index(packet.bytes) {
            passes.controls[index] = packet.bytes;
        }
    }
    passes.seed_controls(map.tick_at(projection.origin));
    let loop_ticks = [projection.left, projection.right].map(|frame| map.tick_at(frame));
    for packet in &input[before..] {
        let Some((packet_pass, frame)) = projection.position(packet.at) else {
            continue;
        };
        if !passes.advance(packet_pass, loop_ticks) {
            return (passes, false);
        }
        let tick = map.tick_at(frame);
        if supported_control(packet.bytes) {
            if let Some(index) = control_index(packet.bytes) {
                passes.controls[index] = packet.bytes;
            }
            passes.control(tick, packet.bytes);
            passes.has_input = true;
        } else {
            passes.has_input |= passes.note(tick, packet.bytes);
        }
    }
    let complete = passes.advance(number, loop_ticks);
    (passes, complete)
}

pub fn note_preview(
    map: &SampleMap,
    projection: Projection,
    pass: u64,
    baseline: ControllerBaseline,
    input: &[Captured],
) -> NotePreview {
    let (passes, complete) = passes_through(map, projection, pass, baseline, input);
    let mut current = Vec::new();
    if complete {
        current.extend(passes.current.iter().filter_map(NoteMark::from_event));
        for (&(_, key), notes) in &passes.held {
            current.extend(notes.iter().map(|note| NoteMark {
                start: note.tick,
                end: None,
                key,
            }));
        }
    }
    let previous = passes
        .passes
        .iter()
        .rev()
        .find(|pass| {
            pass.iter()
                .any(|event| matches!(event.message, Message::Note { .. }))
        })
        .into_iter()
        .flatten()
        .filter_map(NoteMark::from_event)
        .collect();
    NotePreview { current, previous }
}

/// Assemble the capture against its half-open presentation window on the session thread.
pub fn assemble(
    map: &SampleMap,
    projection: Projection,
    end: u64,
    baseline: ControllerBaseline,
    input: &[Captured],
) -> Result<Recording> {
    let loops = projection.right > projection.left;
    let start_tick = map.tick_at(projection.origin);
    let end = if loops {
        end
    } else {
        end.min(projection.start + projection.limit.saturating_sub(projection.origin))
    };
    if end <= projection.start {
        return Ok(Recording {
            range: [start_tick, start_tick],
            passes: Vec::new(),
            truncated: false,
        });
    }
    let (number, finish_frame) = projection
        .position(end - 1)
        .ok_or("invalid recording end")?;
    let last_tick = map.tick_at(finish_frame).saturating_add(1).min(MAX_TICK);
    let range = if loops {
        [projection.left, projection.right].map(|frame| map.tick_at(frame))
    } else {
        [start_tick, last_tick]
    };
    let due = input.partition_point(|packet| packet.at < end);
    let (mut passes, complete) = passes_through(map, projection, number, baseline, &input[..due]);
    if complete {
        passes.finish(last_tick.min(range[1]), None);
    }
    Ok(Recording {
        range,
        passes: passes.passes,
        truncated: !complete,
    })
}

#[derive(Clone, Copy)]
struct Edge {
    frame: u64,
    bytes: [u8; 3],
    order: u64,
}

struct Note {
    attack: Edge,
    release: Edge,
    sounding_until: u64,
}

pub(crate) struct Plan {
    edges: Vec<Edge>,
    notes: Vec<Note>,
    controls: [Vec<Edge>; LEGACY_CONTROLS],
    extra_controls: BTreeMap<(u8, u8), Vec<Edge>>,
}

impl Plan {
    pub(crate) fn new(clips: &[Clip], map: &SampleMap) -> Result<Self> {
        let mut plan = Self {
            edges: Vec::new(),
            notes: Vec::new(),
            controls: std::array::from_fn(|_| Vec::new()),
            extra_controls: BTreeMap::new(),
        };
        let mut curve_edges = 0;
        let mut spans = exposed(clips).into_iter().peekable();
        while let Some(([left, right], index)) = spans.next() {
            let Some(index) = index else { continue };
            let clip = &clips[index];
            let limit = right < clip.end
                || clip.end < clip.source_end()
                || clip.cycle().is_some()
                || spans
                    .peek()
                    .is_some_and(|&(_, next)| next.is_some_and(|next| clips[next].start < right));
            let mut resets = BTreeSet::new();
            let mut chase: [Option<Event>; LEGACY_CONTROLS] = [None; LEGACY_CONTROLS];
            let mut sustain: [Vec<(u64, u32, u8)>; 16] = std::array::from_fn(|_| Vec::new());
            let [scan_from, reentry_end] = clip.playback_run(left);
            if left > scan_from {
                for event in clip
                    .active_events()
                    .filter(|event| (scan_from..left).contains(&event.tick))
                {
                    if let Message::Control([status, 64, value]) = event.message
                        && status & 0xf0 == 0xb0
                    {
                        sustain[usize::from(status & 15)].push((event.tick, event.order, value));
                    }
                }
                for channel in &mut sustain {
                    channel.sort_unstable();
                }
            }
            let last_pedal_up: [_; 16] = std::array::from_fn(|channel| {
                sustain[channel]
                    .iter()
                    .rfind(|&&(_, _, value)| value < 64)
                    .map(|&(tick, order, _)| (tick, order))
            });
            let mut lanes = BTreeMap::<(u8, u8), Vec<Event>>::new();
            for event in clip.active_events() {
                if let Some(bytes) = event.message.control()
                    && stateful_control(bytes)
                    && bytes[0] & 0xf0 == 0xb0
                {
                    lanes.entry((bytes[0], bytes[1])).or_default().push(*event);
                }
                if !(scan_from..right).contains(&event.tick) {
                    continue;
                }
                let frame = map.sample_at(event.tick.max(left));
                // A boundary reset and chase precede any attacks at the same sample.
                let order = u64::from(event.order) + 2;
                match event.message {
                    Message::Note {
                        channel,
                        key,
                        velocity,
                        end,
                        release,
                        release_order,
                    } => {
                        let boundary = if clip.cycle().is_some() {
                            clip.playback_run(event.tick)[1].min(right)
                        } else if limit {
                            right
                        } else {
                            MAX_TICK
                        };
                        let end = if end <= left {
                            let controls = &sustain[usize::from(channel)];
                            let release_index = controls.partition_point(|&(tick, order, _)| {
                                (tick, order) <= (end, release_order)
                            });
                            if release_index == 0
                                || controls[release_index - 1].2 < 64
                                || last_pedal_up[usize::from(channel)]
                                    .is_some_and(|up| up > (end, release_order))
                            {
                                continue;
                            }
                            left
                        } else {
                            end.min(boundary)
                        };
                        let attack = Edge {
                            frame,
                            bytes: [0x90 | channel, key, velocity],
                            order,
                        };
                        let release_frame = map.sample_at(end).max(frame + 1);
                        let release = Edge {
                            frame: release_frame,
                            bytes: [0x80 | channel, key, release],
                            order: if end == right || end == boundary {
                                1
                            } else {
                                u64::from(release_order) + 2
                            },
                        };
                        plan.notes.push(Note {
                            attack,
                            release,
                            sounding_until: release_frame,
                        });
                        plan.edges.extend([attack, release]);
                    }
                    message => {
                        let bytes = message.control().unwrap();
                        if event.tick < left {
                            if let Some(index) = legacy_control_index(bytes)
                                && chase[index].is_none_or(|old| {
                                    (event.tick, event.order) >= (old.tick, old.order)
                                })
                            {
                                chase[index] = Some(Event {
                                    message: Message::Control(bytes),
                                    ..*event
                                });
                            }
                            continue;
                        }
                        let edge = Edge {
                            frame,
                            bytes,
                            order,
                        };
                        plan.edges.push(edge);
                        if let Some(index) = legacy_control_index(bytes) {
                            let seam = clip.playback_run(event.tick)[1];
                            if !(matches!(message, Message::CurvedControl { .. })
                                && seam < clip.end)
                            {
                                resets.insert((seam.min(right), index));
                            }
                            plan.controls[index].push(edge);
                        } else if stateful_control(bytes) {
                            plan.extra_controls
                                .entry((bytes[0], bytes[1]))
                                .or_default()
                                .push(edge);
                        }
                    }
                }
            }
            for (index, event) in chase.into_iter().enumerate() {
                if let Some(Event {
                    message: Message::Control(bytes),
                    ..
                }) = event
                {
                    let edge = Edge {
                        frame: map.sample_at(left),
                        bytes,
                        order: 1,
                    };
                    resets.insert((reentry_end.min(right), index));
                    plan.edges.push(edge);
                    plan.controls[index].push(edge);
                }
            }
            for (tick, index) in resets {
                let edge = Edge {
                    frame: map.sample_at(tick),
                    bytes: neutral(index),
                    order: 0,
                };
                plan.edges.push(edge);
                plan.controls[index].push(edge);
            }
            for ((status, cc), events) in &mut lanes {
                events.sort_by_key(|event| (event.tick, event.order));
                let emit = |plan: &mut Self, tick, value, order| {
                    let edge = Edge {
                        frame: map.sample_at(tick),
                        bytes: [*status, *cc, value],
                        order,
                    };
                    plan.edges.push(edge);
                    if let Some(index) = legacy_control_index(edge.bytes) {
                        plan.controls[index].push(edge);
                    } else {
                        plan.extra_controls
                            .entry((*status, *cc))
                            .or_default()
                            .push(edge);
                    }
                };
                if legacy_control_index([*status, *cc, 0]).is_none()
                    && let Some(previous) = events
                        .iter()
                        .rev()
                        .find(|event| (scan_from..left).contains(&event.tick))
                {
                    emit(&mut plan, left, previous.message.control().unwrap()[2], 1);
                }
                for pair in events.windows(2) {
                    let [from, to] = [pair[0], pair[1]];
                    let run_end = clip.playback_run(from.tick)[1];
                    if from.tick >= to.tick
                        || (clip.cycle().is_some() && to.tick >= run_end)
                        || to.tick <= left
                        || from.tick >= right
                    {
                        continue;
                    }
                    let value_at = |tick| {
                        from.message.interpolated(
                            to.message,
                            (tick - from.tick) as f64 / (to.tick - from.tick) as f64,
                        )
                    };
                    let mut tick = (from.tick + 1).max(left);
                    let end = to.tick.min(right);
                    let Some(mut value) = value_at(tick) else {
                        continue;
                    };
                    let mut previous = value_at(tick - 1).unwrap();
                    if tick == left && from.tick < left {
                        previous = from.message.control().unwrap()[2];
                    }
                    while tick < end {
                        if value != previous {
                            if curve_edges == MAX_CURVE_EDGES {
                                return Err("MIDI controller interpolation exceeds the playback event limit.".into());
                            }
                            curve_edges += 1;
                            emit(&mut plan, tick, value, 1);
                        }
                        let mut low = tick + 1;
                        let mut high = end;
                        while low < high {
                            let mid = low + (high - low) / 2;
                            if value_at(mid) == Some(value) {
                                low = mid + 1;
                            } else {
                                high = mid;
                            }
                        }
                        tick = low;
                        previous = value;
                        value = value_at(tick).unwrap();
                    }
                }
            }
        }
        let sort = |edge: &Edge| (edge.frame, edge.order);
        plan.edges.sort_by_key(sort);
        plan.notes.sort_by_key(|note| sort(&note.attack));
        for control in plan
            .controls
            .iter_mut()
            .chain(plan.extra_controls.values_mut())
        {
            control.sort_by_key(sort);
        }
        let pedal_releases: [Vec<_>; 16] = std::array::from_fn(|channel| {
            plan.controls[channel * 3]
                .iter()
                .filter(|edge| edge.bytes[2] < 64)
                .collect()
        });
        for note in &mut plan.notes {
            let channel = usize::from(note.attack.bytes[0] & 15);
            let pedal = &plan.controls[channel * 3];
            let key = sort(&note.release);
            let before = pedal.partition_point(|edge| sort(edge) <= key);
            if before > 0 && pedal[before - 1].bytes[2] >= 64 {
                let releases = &pedal_releases[channel];
                if let Some(edge) = releases.get(releases.partition_point(|edge| sort(edge) <= key))
                {
                    note.sounding_until = edge.frame;
                }
            }
        }
        Ok(plan)
    }

    fn chase(&self, frame: u64, offset: u32, events: &mut TimedQueue) {
        for edges in self.controls.iter().chain(self.extra_controls.values()) {
            let index = edges.partition_point(|edge| edge.frame < frame);
            if let Some(edge) = index.checked_sub(1).map(|index| edges[index]) {
                events.push(offset, Kind::Clip(edge.bytes));
            }
            if events.overflow {
                return;
            }
        }
        for note in self
            .notes
            .iter()
            .take_while(|note| note.attack.frame < frame)
        {
            if note.sounding_until > frame {
                events.push(offset, Kind::Clip(note.attack.bytes));
                if note.release.frame < frame {
                    events.push(offset, Kind::Clip(note.release.bytes));
                }
            }
            if events.overflow {
                break;
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Kind {
    ClearClip,
    ClearLive,
    Clip([u8; 3]),
    Live([u8; 3]),
}

struct Timed {
    offset: u32,
    order: usize,
    kind: Kind,
}

struct TimedQueue {
    items: Vec<Timed>,
    overflow: bool,
}

impl TimedQueue {
    fn push(&mut self, offset: u32, kind: Kind) {
        if self.items.len() < MAX_EVENTS {
            self.items.push(Timed {
                offset,
                order: self.items.len(),
                kind,
            });
        } else {
            self.overflow = true;
        }
    }
}

#[derive(Default)]
struct Voices {
    held: Vec<u16>,
    controls: Vec<Option<[u8; 3]>>,
}

impl Voices {
    fn new() -> Self {
        Self {
            held: vec![0; KEYS],
            controls: vec![None; CONTROLS],
        }
    }
}

pub(crate) struct Scheduler {
    live: Voices,
    clip: Voices,
    timed: TimedQueue,
    generation: u64,
    active_keys: usize,
    cursor: Option<(u64, u64, u64)>,
    panic: Vec<Midi>,
    mute: bool,
}

impl Scheduler {
    pub(crate) fn new() -> Self {
        Self {
            live: Voices::new(),
            clip: Voices::new(),
            timed: TimedQueue {
                items: Vec::with_capacity(MAX_EVENTS),
                overflow: false,
            },
            generation: 0,
            active_keys: 0,
            cursor: None,
            panic: crate::sequence::panic_events(),
            mute: false,
        }
    }

    pub(crate) fn live(&mut self, event: Midi) {
        self.timed.push(event.offset, Kind::Live(event.bytes));
    }

    pub(crate) fn replace_plan(&mut self) {
        self.cursor = None;
        self.timed.push(0, Kind::ClearClip);
    }

    pub(crate) fn release_live(&mut self) {
        self.timed.push(0, Kind::ClearLive);
    }

    pub(crate) fn schedule(
        &mut self,
        plan: Option<&Plan>,
        projection: Projection,
        at: u64,
        muted: bool,
    ) -> bool {
        if self.generation != projection.generation || self.mute != muted {
            self.timed.push(0, Kind::ClearClip);
            self.generation = projection.generation;
            self.mute = muted;
            self.cursor = None;
        }
        let Some(plan) = plan.filter(|_| !muted) else {
            return false;
        };
        let initial = self.timed.items.len();
        let end = at + BLOCK as u64;
        let mut at_frame = at.max(projection.start);
        while at_frame < end && !self.timed.overflow {
            let offset = (at_frame - at) as u32;
            let Some((pass, frame)) = projection.position(at_frame) else {
                if self.cursor.take().is_some() {
                    self.timed.push(offset, Kind::ClearClip);
                }
                break;
            };
            if self.cursor != Some((at_frame, pass, frame)) {
                if self.cursor.is_some() {
                    self.timed.push(offset, Kind::ClearClip);
                }
                plan.chase(frame, offset, &mut self.timed);
            }
            let frames = projection.before_end(frame).min(end - at_frame);
            let start = plan.edges.partition_point(|edge| edge.frame < frame);
            for edge in plan.edges[start..]
                .iter()
                .take_while(|edge| edge.frame < frame + frames)
            {
                self.timed
                    .push(offset + (edge.frame - frame) as u32, Kind::Clip(edge.bytes));
                if self.timed.overflow {
                    break;
                }
            }
            at_frame += frames;
            self.cursor = Some((at_frame, pass, frame + frames));
        }
        initial < self.timed.items.len()
    }

    pub(crate) fn render(&mut self, events: &mut Vec<Midi>) -> bool {
        self.timed
            .items
            .sort_unstable_by_key(|event| (event.offset, event.order));
        let mut overflow = std::mem::take(&mut self.timed.overflow);
        for timed in self.timed.items.drain(..) {
            let (owner, other) = match timed.kind {
                Kind::Live(_) | Kind::ClearLive => (&mut self.live, &self.clip),
                Kind::Clip(_) | Kind::ClearClip => (&mut self.clip, &self.live),
            };
            let mut emit = |bytes| {
                if events.len() == MAX_EVENTS {
                    overflow = true;
                } else {
                    events.push(Midi {
                        offset: timed.offset,
                        bytes,
                    });
                }
            };
            match timed.kind {
                Kind::ClearClip | Kind::ClearLive => {
                    for (index, held) in owner.held.iter_mut().enumerate() {
                        if *held != 0 && other.held[index] == 0 {
                            self.active_keys -= 1;
                            emit([0x80 | (index / 128) as u8, (index % 128) as u8, 0]);
                        }
                        *held = 0;
                    }
                    for (index, bytes) in owner.controls.iter_mut().enumerate() {
                        if bytes.take().is_some()
                            && let Some(bytes) = other.controls[index]
                                .or_else(|| (index < LEGACY_CONTROLS).then(|| neutral(index)))
                        {
                            emit(bytes);
                        }
                    }
                }
                Kind::Clip(bytes) | Kind::Live(bytes) => {
                    let [status, key, velocity] = bytes;
                    let index = usize::from(status & 15) * 128 + usize::from(key);
                    match status & 0xf0 {
                        0x90 if velocity > 0 => {
                            if owner.held[index] != 0 || other.held[index] != 0 {
                                emit([0x80 | (status & 15), key, 0]);
                            } else if self.active_keys == MAX_EVENTS {
                                overflow = true;
                                continue;
                            } else {
                                self.active_keys += 1;
                            }
                            owner.held[index] = owner.held[index].saturating_add(1);
                            emit(bytes);
                        }
                        0x80 | 0x90 => {
                            let held = &mut owner.held[index];
                            if *held > 0 {
                                *held -= 1;
                                if *held == 0 && other.held[index] == 0 {
                                    self.active_keys -= 1;
                                    emit(bytes);
                                }
                            }
                        }
                        0xb0 if matches!(key, 120 | 123) => {
                            let first = usize::from(status & 15) * 128;
                            let range = first..first + 128;
                            let shared = other.held[range.clone()].iter().any(|held| *held != 0)
                                || other.controls[usize::from(status & 15) * 3]
                                    .is_some_and(|state| state[2] >= 64);
                            for index in range {
                                if owner.held[index] != 0 && other.held[index] == 0 {
                                    self.active_keys -= 1;
                                    if shared {
                                        emit([0x80 | (status & 15), (index - first) as u8, 0]);
                                    }
                                }
                                owner.held[index] = 0;
                            }
                            if !shared {
                                emit(bytes);
                            }
                        }
                        _ => {
                            if let Some(control) = control_index(bytes) {
                                owner.controls[control] = Some(bytes);
                                if control < LEGACY_CONTROLS
                                    && control % 3 == 0
                                    && velocity < 64
                                    && let Some(sustain) =
                                        other.controls[control].filter(|state| state[2] >= 64)
                                {
                                    emit(sustain);
                                    continue;
                                }
                            }
                            emit(bytes);
                        }
                    }
                }
            }
        }
        if overflow {
            events.clear();
            events.extend_from_slice(&self.panic);
            self.live.held.fill(0);
            self.clip.held.fill(0);
            self.live.controls.fill(None);
            self.clip.controls.fill(None);
            self.cursor = None;
            self.active_keys = 0;
        }
        overflow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clef_model::{
        midi::{Arrangement, ClipEdit, Mode},
        timeline::{Tempo, Timeline},
    };

    #[test]
    fn all_ccs_capture_and_play_with_order_and_channel_intact() {
        let map = SampleMap::new(&Timeline::default(), 48_000).unwrap();
        let projection = Projection {
            origin: 0,
            limit: map.sample_at(256),
            ..Default::default()
        };
        let input = (0_u8..128)
            .map(|cc| Captured {
                at: map.sample_at(12),
                bytes: [0xb0 | (cc % 16), cc, 75],
            })
            .collect::<Vec<_>>();
        assert!(input.iter().all(|event| capture_message(event.bytes)));
        let recording = assemble(
            &map,
            projection,
            map.sample_at(240),
            [[0; 3]; CONTROLS],
            &input,
        )
        .unwrap();
        let mut arrangement = Arrangement::default();
        arrangement
            .record(1, Mode::Merge, recording.range, recording.passes, false)
            .unwrap();
        let plan = Plan::new(arrangement.clips(1), &map).unwrap();
        assert_eq!(
            plan.edges
                .iter()
                .filter(|event| event.frame == map.sample_at(12))
                .map(|event| event.bytes)
                .collect::<Vec<_>>(),
            input.iter().map(|event| event.bytes).collect::<Vec<_>>()
        );
        let mut queue = TimedQueue {
            items: Vec::with_capacity(MAX_EVENTS),
            overflow: false,
        };
        plan.chase(map.sample_at(14), 0, &mut queue);
        assert!(
            queue
                .items
                .iter()
                .all(|event| matches!(event.kind, Kind::Clip(bytes) if stateful_control(bytes)))
        );
    }

    #[test]
    fn authored_curve_interpolates_and_chases_without_changing_legacy_lanes() {
        let map = SampleMap::new(&Timeline::default(), 48_000).unwrap();
        let control = |tick, order, bytes| Event {
            tick,
            order,
            message: Message::Control(bytes),
        };
        let mut model = Arrangement::default();
        model
            .record(
                1,
                Mode::Merge,
                [0, 300],
                vec![vec![
                    Event {
                        tick: 30,
                        order: 1,
                        message: Message::CurvedControl {
                            bytes: [0xb0, 11, 0],
                            bend: 32,
                        },
                    },
                    control(30, 2, [0xb3, 11, 13]),
                    control(230, 3, [0xb0, 11, 100]),
                    control(230, 4, [0xb3, 11, 89]),
                ]],
                false,
            )
            .unwrap();
        let plan = Plan::new(model.clips(1), &map).unwrap();
        let mut queue = TimedQueue {
            items: Vec::with_capacity(MAX_EVENTS),
            overflow: false,
        };
        plan.chase(map.sample_at(131), 0, &mut queue);
        let bytes = queue
            .items
            .iter()
            .filter_map(|item| match item.kind {
                Kind::Clip(bytes) => Some(bytes),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(bytes, [[0xb0, 11, 25], [0xb3, 11, 13]]);
        assert!(
            plan.edges
                .iter()
                .any(|event| event.frame == map.sample_at(230) && event.bytes == [0xb0, 11, 100])
        );
    }

    #[test]
    fn a_curved_legacy_lane_keeps_its_value_across_repeat_seams() {
        let map = SampleMap::new(&Timeline::default(), 48_000).unwrap();
        let curve = |tick, order, value| Event {
            tick,
            order,
            message: Message::CurvedControl {
                bytes: [0xb0, 1, value],
                bend: 0,
            },
        };
        let legacy = |tick, order, value| Event {
            tick,
            order,
            message: Message::Control([0xb0, 1, value]),
        };
        let looped = |events: Vec<Event>| {
            let mut model = Arrangement::default();
            model
                .record(1, Mode::Merge, [0, 480], vec![events], false)
                .unwrap();
            let clip = model.clips(1)[0].id();
            model
                .edit(
                    &ClipEdit::Resize {
                        clip,
                        end: 1_440,
                        repeat: true,
                    },
                    [1].into_iter(),
                )
                .unwrap();
            Plan::new(model.clips(1), &map).unwrap()
        };
        let seam = map.sample_at(480);
        let plan = looped(vec![curve(0, 0, 30), curve(240, 1, 90)]);
        assert!(
            !plan
                .edges
                .iter()
                .any(|edge| edge.frame < map.sample_at(1_440) && edge.bytes == [0xb0, 1, 0]),
            "an authored curve must not be reset to neutral inside the repeated region"
        );
        assert!(
            plan.edges
                .iter()
                .any(|edge| edge.frame >= seam && edge.bytes == [0xb0, 1, 30]),
            "the repeated curve restarts at its first point value"
        );
        let plan = looped(vec![legacy(0, 0, 30), legacy(240, 1, 90)]);
        assert!(
            plan.edges
                .iter()
                .any(|edge| edge.frame >= seam && edge.bytes == [0xb0, 1, 0]),
            "a legacy recorded lane keeps its held reset at the repeat seam"
        );
    }

    #[test]
    fn foreground_clip_cuts_covered_notes_and_chases_exposed_notes_and_controls() {
        let map = SampleMap::new(&Timeline::default(), 48_000).unwrap();
        let mut arrangement = Arrangement::default();
        let note = |tick, end, key| Event {
            tick,
            order: 1,
            message: Message::Note {
                channel: 0,
                key,
                velocity: 90,
                end,
                release: 0,
                release_order: 2,
            },
        };
        arrangement
            .record(
                1,
                Mode::Merge,
                [0, 400],
                vec![vec![
                    Event {
                        tick: 0,
                        order: 0,
                        message: Message::Control([0xb0, 64, 127]),
                    },
                    note(10, 130, 60),
                    note(160, 300, 63),
                ]],
                false,
            )
            .unwrap();
        arrangement
            .record(
                2,
                Mode::Merge,
                [100, 200],
                vec![vec![note(100, 230, 67)]],
                false,
            )
            .unwrap();
        let top = arrangement.clips(2)[0].id();
        arrangement
            .edit(
                &ClipEdit::Move {
                    destinations: vec![(top, 1)],
                    delta: 0,
                },
                [1, 2].into_iter(),
            )
            .unwrap();
        let plan = Plan::new(arrangement.clips(1), &map).unwrap();
        let at = |tick| {
            plan.edges
                .iter()
                .filter(|edge| edge.frame == map.sample_at(tick))
                .map(|edge| edge.bytes)
                .collect::<Vec<_>>()
        };
        assert_eq!(at(100), vec![[0xb0, 64, 0], [0x80, 60, 0], [0x90, 67, 90]]);
        assert!(at(160).is_empty());
        assert_eq!(
            at(200),
            vec![
                [0x80, 67, 0],
                [0xb0, 64, 127],
                [0x90, 60, 90],
                [0x90, 63, 90]
            ]
        );
    }

    fn window(map: &SampleMap, start: u64, origin: u64, range: Option<[u64; 2]>) -> Projection {
        let [left, right] = range.unwrap_or_default().map(|tick| map.sample_at(tick));
        Projection {
            generation: 1,
            start,
            origin: map.sample_at(origin),
            left,
            right,
            limit: map.sample_at(MAX_TICK),
        }
    }

    #[test]
    fn preview_shows_held_notes_and_keeps_the_last_pass_with_notes_visible() {
        let map = SampleMap::new(&Timeline::default(), 48_000).unwrap();
        let start = 8_000;
        let projection = window(&map, start, 100, Some([100, 200]));
        let span = map.sample_at(200) - map.sample_at(100);
        let at = |pass, tick| start + pass * span + map.sample_at(tick) - map.sample_at(100);
        let input = [
            Captured {
                at: at(0, 110),
                bytes: [0x91, 60, 90],
            },
            Captured {
                at: at(0, 140),
                bytes: [0x81, 60, 0],
            },
            Captured {
                at: at(1, 120),
                bytes: [0xb1, 1, 32],
            },
            Captured {
                at: at(2, 130),
                bytes: [0x91, 65, 80],
            },
            Captured {
                at: at(2, 170),
                bytes: [0x81, 65, 0],
            },
        ];
        let preview =
            |pass, due| note_preview(&map, projection, pass, [[0; 3]; CONTROLS], &input[..due]);
        let held = NoteMark {
            start: 110,
            end: None,
            key: 60,
        };
        assert_eq!(preview(0, 1).current, vec![held]);
        let first = NoteMark {
            end: Some(140),
            ..held
        };
        assert_eq!(preview(0, 2).current, vec![first]);
        let wrap = preview(1, 2);
        assert!(wrap.current.is_empty());
        assert_eq!(wrap.previous, vec![first]);
        let after_controllers = preview(2, 4);
        assert_eq!(after_controllers.previous, vec![first]);
        assert_eq!(
            after_controllers.current,
            vec![NoteMark {
                start: 130,
                end: None,
                key: 65
            }]
        );
        assert_eq!(
            preview(3, 5).previous,
            vec![NoteMark {
                start: 130,
                end: Some(170),
                key: 65
            }]
        );
        let linear = window(&map, start, 100, None);
        let linear_preview = note_preview(&map, linear, 0, [[0; 3]; CONTROLS], &input[..2]);
        assert!(linear_preview.previous.is_empty());
        assert_eq!(linear_preview.current, vec![first]);
    }

    #[test]
    fn captures_the_audible_tempo_map_and_schedules_the_same_offsets() {
        let mut timeline = Timeline::default();
        timeline.changes.push(Tempo {
            bar: 2,
            bpm: 90,
            beats: 3,
            unit: 4,
        });
        for rate in [44_100, 48_000] {
            let map = SampleMap::new(&timeline, rate).unwrap();
            let start = 8_000;
            let origin = 3835;
            let projection = window(&map, start, origin, None);
            let samples = |tick| start + map.sample_at(tick) - map.sample_at(origin);
            let latency = 347;
            // The driver position at each arrival is ahead by its output latency.
            let packet = |tick, bytes| {
                let driver = samples(tick) + latency;
                Captured {
                    at: driver - latency,
                    bytes,
                }
            };
            let input = [
                Captured {
                    at: start - 1,
                    bytes: [0x92, 70, 100],
                },
                packet(3835, [0xb2, 64, 127]),
                packet(3840, [0xe2, 19, 77]),
                packet(3840, [0xb2, 1, 85]),
                packet(3840, [0x92, 60, 91]),
                packet(3842, [0x82, 60, 37]),
                packet(3850, [0x92, 61, 100]),
            ];
            let recording =
                assemble(&map, projection, samples(3850), [[0; 3]; CONTROLS], &input).unwrap();
            assert_eq!(recording.range, [3835, 3850]);
            assert_eq!(recording.passes[0].len(), 4);
            assert_eq!(
                recording.passes[0][1].message,
                Message::Control([0xe2, 19, 77])
            );
            assert_eq!(
                recording.passes[0][2].message,
                Message::Control([0xb2, 1, 85])
            );
            assert!(matches!(
                recording.passes[0][3].message,
                Message::Note {
                    channel: 2,
                    key: 60,
                    velocity: 91,
                    end: 3842,
                    release: 37,
                    ..
                }
            ));
            let mut arrangement = Arrangement::default();
            arrangement
                .record(1, Mode::Merge, recording.range, recording.passes, false)
                .unwrap();
            let plan = Plan::new(arrangement.clips(1), &map).unwrap();
            let mut scheduler = Scheduler::new();
            let mut events = Vec::new();
            let base = samples(3840) - 16;
            scheduler.schedule(Some(&plan), projection, base, false);
            assert!(!scheduler.render(&mut events));
            assert_eq!(
                events
                    .iter()
                    .filter(|event| event.offset == 16)
                    .map(|event| event.bytes)
                    .collect::<Vec<_>>(),
                vec![[0xe2, 19, 77], [0xb2, 1, 85], [0x92, 60, 91]]
            );
            assert!(
                events
                    .iter()
                    .any(|event| event.offset == (samples(3842) - base) as u32
                        && event.bytes == [0x82, 60, 37])
            );
        }
    }

    #[test]
    fn held_keys_and_controller_state_survive_a_recording_wrap() {
        let map = SampleMap::new(&Timeline::default(), 48_000).unwrap();
        let projection = window(&map, 4_000, 102, Some([100, 104]));
        let samples = |tick| map.sample_at(tick);
        let wrap = 4_000 + samples(104) - samples(102);
        let input = [
            Captured {
                at: 4_000,
                bytes: [0xb3, 1, 32],
            },
            Captured {
                at: 4_001,
                bytes: [0x93, 60, 96],
            },
            Captured {
                at: wrap + 27,
                bytes: [0x83, 60, 21],
            },
        ];
        let recording = assemble(&map, projection, wrap + 45, [[0; 3]; CONTROLS], &input).unwrap();
        assert_eq!(recording.range, [100, 104]);
        assert_eq!(recording.passes.len(), 2);
        assert!(matches!(
            recording.passes[0][1].message,
            Message::Note { end: 104, .. }
        ));
        assert_eq!(recording.passes[1][1].tick, 100);
        assert!(matches!(
            recording.passes[1][1].message,
            Message::Note {
                end: 101,
                release: 21,
                ..
            }
        ));
        assert!(
            recording.passes[1]
                .iter()
                .any(|event| event.message == Message::Control([0xb3, 1, 32]))
        );
        let cancelled = assemble(&map, projection, 3999, [[0; 3]; CONTROLS], &input).unwrap();
        assert!(cancelled.passes.is_empty());
    }

    #[test]
    fn seek_chases_sustained_notes_and_stop_resets_clip_without_releasing_live() {
        let map = SampleMap::new(&Timeline::default(), 48_000).unwrap();
        let mut arrangement = Arrangement::default();
        let events = vec![
            Event {
                tick: 0,
                order: 0,
                message: Message::Control([0xb0, 64, 127]),
            },
            Event {
                tick: 1,
                order: 1,
                message: Message::Note {
                    channel: 0,
                    key: 60,
                    velocity: 95,
                    end: 2,
                    release: 4,
                    release_order: 2,
                },
            },
        ];
        arrangement
            .record(1, Mode::Merge, [0, 20], vec![events], false)
            .unwrap();
        let plan = Plan::new(arrangement.clips(1), &map).unwrap();
        let mut scheduler = Scheduler::new();
        let mut events = Vec::new();
        let projection = window(&map, 512, 4, None);
        scheduler.schedule(Some(&plan), projection, 512, false);
        assert!(!scheduler.render(&mut events));
        assert_eq!(
            events.iter().map(|event| event.bytes).collect::<Vec<_>>(),
            vec![[0xb0, 64, 127], [0x90, 60, 95], [0x80, 60, 4]]
        );
        events.clear();
        scheduler.live(Midi {
            offset: 0,
            bytes: [0x90, 61, 92],
        });
        let stopped = Projection {
            generation: 2,
            start: u64::MAX,
            ..Default::default()
        };
        scheduler.schedule(Some(&plan), stopped, 640, false);
        assert!(!scheduler.render(&mut events));
        assert!(events.iter().any(|event| event.bytes == [0xb0, 64, 0]));
        assert!(
            !events
                .iter()
                .any(|event| event.bytes[0] == 0x80 && event.bytes[1] == 61)
        );
        events.clear();
        scheduler.release_live();
        scheduler.render(&mut events);
        assert!(events.iter().any(|event| event.bytes == [0x80, 61, 0]));
    }

    #[test]
    fn clip_end_resets_controller_state_and_loop_wrap_releases_previous_voices() {
        let map = SampleMap::new(&Timeline::default(), 48_000).unwrap();
        let mut arrangement = Arrangement::default();
        let events = vec![
            Event {
                tick: 1,
                order: 0,
                message: Message::Control([0xb0, 64, 127]),
            },
            Event {
                tick: 2,
                order: 1,
                message: Message::Note {
                    channel: 0,
                    key: 60,
                    velocity: 95,
                    end: 12,
                    release: 0,
                    release_order: 2,
                },
            },
        ];
        arrangement
            .record(1, Mode::Merge, [1, 3], vec![events], false)
            .unwrap();
        let plan = Plan::new(arrangement.clips(1), &map).unwrap();
        let mut scheduler = Scheduler::new();
        let mut events = Vec::new();
        scheduler.schedule(Some(&plan), window(&map, 0, 0, Some([0, 4])), 0, false);
        assert!(!scheduler.render(&mut events));
        assert!(
            events
                .iter()
                .any(|event| event.offset == 75 && event.bytes == [0xb0, 64, 0])
        );
        assert!(
            events
                .iter()
                .any(|event| event.offset == 100 && event.bytes == [0x80, 60, 0])
        );
        assert!(
            events
                .iter()
                .any(|event| event.offset == 125 && event.bytes == [0xb0, 64, 127])
        );
    }
}
