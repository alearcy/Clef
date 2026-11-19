use crate::{Result, timeline::MAX_TICK};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

mod editing;
mod piano_roll;
pub use editing::{ClipEdit, ClipEditResult, ClipId, Snap, exposed};
pub use piano_roll::{EventChange, EventId, RollEvent};

const MAX_CLIPS: usize = 4096;
const MAX_TAKES: usize = 4096;
const MAX_EVENTS: usize = 65_536;
const MAX_SECTIONS: usize = 16_384;
const MAX_EXPOSED_EVENT_VISITS: usize = 262_144;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Merge,
    Replace,
    NewTake,
}

impl Mode {
    pub const ALL: [Self; 3] = [Self::Merge, Self::Replace, Self::NewTake];

    pub fn label(self) -> &'static str {
        match self {
            Self::Merge => "Merge",
            Self::Replace => "Replace",
            Self::NewTake => "New take",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Message {
    Note {
        channel: u8,
        key: u8,
        velocity: u8,
        end: u64,
        release: u8,
        release_order: u32,
    },
    Control([u8; 3]),
    CurvedControl {
        bytes: [u8; 3],
        bend: i8,
    },
}

pub fn supported_control([status, key, value]: [u8; 3]) -> bool {
    key < 128 && value < 128 && matches!(status & 0xf0, 0xb0 | 0xe0)
}

pub fn continuous_cc(cc: u8) -> bool {
    matches!(cc, 1..=31 | 33..=63 | 70..=95 | 102..=119) && !discrete_cc(cc)
}

pub fn discrete_cc(cc: u8) -> bool {
    matches!(cc, 0 | 6 | 32 | 38 | 84 | 88 | 96..=101 | 120..=127)
}

pub fn stateful_control([status, cc, _]: [u8; 3]) -> bool {
    status & 0xf0 == 0xe0 || status & 0xf0 == 0xb0 && !discrete_cc(cc)
}

pub fn carried_control([status, cc, _]: [u8; 3]) -> bool {
    status & 0xf0 == 0xe0 || status & 0xf0 == 0xb0 && cc < 120
}

impl Message {
    pub fn control(self) -> Option<[u8; 3]> {
        match self {
            Self::Control(bytes) | Self::CurvedControl { bytes, .. } => Some(bytes),
            Self::Note { .. } => None,
        }
    }

    pub fn interpolated(self, next: Self, fraction: f64) -> Option<u8> {
        let Self::CurvedControl { bytes, bend } = self else {
            return None;
        };
        let to = next.control()?;
        if bytes[..2] != to[..2] {
            return None;
        }
        let fraction = fraction.clamp(0.0, 1.0);
        let exponent = 2.0_f64.powf(f64::from(bend) / 32.0);
        let position = fraction.powf(exponent);
        Some(
            (f64::from(bytes[2]) + (f64::from(to[2]) - f64::from(bytes[2])) * position).round()
                as u8,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub tick: u64,
    pub order: u32,
    pub message: Message,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Take {
    pub id: u64,
    pub events: Vec<Event>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Section {
    pub start: u64,
    pub end: u64,
    pub take: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clip {
    pub start: u64,
    pub end: u64,
    pub takes: Vec<Take>,
    pub sections: Vec<Section>,
    #[serde(default, skip_serializing_if = "is_zero")]
    layer: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cycle: Option<Box<Cycle>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cycle {
    period: u64,
    phase: u64,
    extent: u64,
    takes: Vec<Take>,
    sections: Vec<Section>,
}

fn is_zero(value: &u64) -> bool {
    *value == 0
}

impl Clip {
    pub fn id(&self) -> ClipId {
        ClipId(self.takes[0].id)
    }

    pub fn layer(&self) -> u64 {
        self.layer
    }

    pub fn cycle(&self) -> Option<u64> {
        self.cycle.as_ref().map(|cycle| cycle.period)
    }

    pub fn repeat_phase(&self) -> u64 {
        self.cycle.as_ref().map_or(0, |cycle| cycle.phase)
    }

    pub fn repeat_extent(&self) -> Option<u64> {
        self.cycle.as_ref().map(|cycle| cycle.extent)
    }

    pub fn repeat_end(&self) -> Option<u64> {
        self.cycle
            .as_ref()
            .map(|cycle| self.end.min(self.start + cycle.extent))
    }

    pub fn playback_run(&self, tick: u64) -> [u64; 2] {
        let Some(cycle) = &self.cycle else {
            return [self.start, self.end];
        };
        let repeat_end = self.end.min(self.start + cycle.extent);
        if tick >= repeat_end {
            return [repeat_end, self.end];
        }
        let phase = (cycle.phase + tick - self.start) % cycle.period;
        [
            tick.saturating_sub(phase).max(self.start),
            (tick + (cycle.period - phase)).min(repeat_end),
        ]
    }

    pub fn repeat_source_bytes(&self) -> usize {
        self.cycle.as_ref().map_or(0, |cycle| {
            size_of::<Cycle>()
                + cycle.sections.capacity() * size_of::<Section>()
                + cycle.takes.capacity() * size_of::<Take>()
                + cycle
                    .takes
                    .iter()
                    .map(|take| take.events.capacity() * size_of::<Event>())
                    .sum::<usize>()
        })
    }

    pub fn source_end(&self) -> u64 {
        self.sections.last().map_or(self.end, |section| section.end)
    }

    pub fn active_events(&self) -> impl Iterator<Item = &Event> {
        self.takes.iter().flat_map(move |take| {
            take.events.iter().filter(move |event| {
                self.sections
                    .get(
                        self.sections
                            .partition_point(|section| section.end <= event.tick),
                    )
                    .is_some_and(|section| section.take == take.id)
            })
        })
    }

    fn section_at(&self, tick: u64) -> u64 {
        self.sections[self.sections.partition_point(|section| section.end <= tick)].take
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Arrangement {
    pub mode: Mode,
    pub tracks: BTreeMap<u64, Vec<Clip>>,
    next_take: u64,
}

impl Default for Arrangement {
    fn default() -> Self {
        Self {
            mode: Mode::Merge,
            tracks: BTreeMap::new(),
            next_take: 1,
        }
    }
}

impl Arrangement {
    pub(crate) fn is_default(&self) -> bool {
        self == &Self::default()
    }

    pub fn clips(&self, track: u64) -> &[Clip] {
        self.tracks.get(&track).map_or(&[], Vec::as_slice)
    }

    pub fn validate(&self, tracks: impl Iterator<Item = u64>) -> Result<()> {
        let tracks: HashSet<_> = tracks.collect();
        let mut ids = HashSet::new();
        let (mut clips_count, mut sections_count, mut events_count) = (0, 0, 0);
        if self.next_take == 0 {
            return Err("invalid next MIDI take identity".into());
        }
        for (&id, clips) in &self.tracks {
            if !tracks.contains(&id) || clips.is_empty() {
                return Err("invalid MIDI track identity".into());
            }
            let mut previous_start = 0;
            for clip in clips {
                let source_end = clip.source_end();
                clips_count += 1;
                sections_count += clip.sections.len();
                if clips_count > MAX_CLIPS
                    || sections_count > MAX_SECTIONS
                    || clip.start < previous_start
                    || clip.start >= clip.end
                    || clip.end > source_end
                    || source_end > MAX_TICK
                    || clip.takes.is_empty()
                    || clip.takes.len() > MAX_TAKES
                {
                    return Err("invalid MIDI clip or project clip limit exceeded".into());
                }
                previous_start = clip.start;
                let mut clip_ids = HashSet::new();
                for take in &clip.takes {
                    events_count += take.events.len();
                    if take.id == 0
                        || take.id >= self.next_take
                        || !ids.insert(take.id)
                        || ids.len() > MAX_TAKES
                        || events_count > MAX_EVENTS
                    {
                        return Err("invalid MIDI take or project MIDI event limit exceeded".into());
                    }
                    clip_ids.insert(take.id);
                    validate_events(take, clip.start, source_end)?;
                }
                validate_sections(&clip.sections, clip.start, source_end, &clip_ids)?;
                if let Some(cycle) = &clip.cycle {
                    sections_count += cycle.sections.len();
                    if cycle.period == 0
                        || cycle.period > MAX_TICK
                        || cycle.phase >= cycle.period
                        || cycle.extent > source_end - clip.start
                        || sections_count > MAX_SECTIONS
                        || cycle.takes.len() > clip.takes.len()
                    {
                        return Err("invalid MIDI loop source".into());
                    }
                    let mut cycle_ids = HashSet::new();
                    for take in &cycle.takes {
                        events_count += take.events.len();
                        if events_count > MAX_EVENTS
                            || !clip_ids.contains(&take.id)
                            || !cycle_ids.insert(take.id)
                        {
                            return Err(
                                "invalid MIDI loop take or project MIDI event limit exceeded"
                                    .into(),
                            );
                        }
                        validate_events(take, 0, cycle.period)?;
                    }
                    validate_sections(&cycle.sections, 0, cycle.period, &cycle_ids)?;
                }
            }
            let work: usize = exposed(clips)
                .into_iter()
                .filter_map(|(_, index)| index)
                .map(|index| {
                    clips[index]
                        .takes
                        .iter()
                        .map(|take| take.events.len())
                        .sum::<usize>()
                })
                .sum();
            if work > MAX_EXPOSED_EVENT_VISITS {
                return Err("Overlapping clips exceed the MIDI playback complexity limit.".into());
            }
        }
        Ok(())
    }

    pub fn record(
        &mut self,
        track: u64,
        mode: Mode,
        [start, end]: [u64; 2],
        mut passes: Vec<Vec<Event>>,
        recover: bool,
    ) -> Result<bool> {
        passes.retain(|pass| !pass.is_empty());
        if passes.is_empty() || start >= end {
            return Ok(false);
        }
        let mut candidate = self.clone();
        let mut clips = candidate.tracks.remove(&track).unwrap_or_default();
        if passes
            .iter()
            .flatten()
            .any(|event| !(start..end).contains(&event.tick))
        {
            return Err("recorded MIDI is outside the capture range".into());
        }
        let mut furthest_end = 0;
        let layered = clips.iter().any(|clip| {
            let overlaps = clip.start < furthest_end;
            furthest_end = furthest_end.max(clip.end);
            overlaps || clip.end != clip.source_end() || clip.cycle.is_some()
        });
        if layered {
            candidate.record_layered(&mut clips, mode, [start, end], passes, recover)?;
        } else if recover {
            candidate.recover(&mut clips, [start, end], passes)?;
        } else if mode == Mode::NewTake {
            let first = clips.partition_point(|clip| clip.end <= start);
            let last = clips.partition_point(|clip| clip.start < end);
            let mut joined = Clip {
                start,
                end,
                takes: Vec::new(),
                sections: Vec::new(),
                layer: 0,
                cycle: None,
            };
            if first != last {
                joined.start = start.min(clips[first].start);
                joined.end = end.max(clips[last - 1].end);
                let fallback = clips[first].sections[0].take;
                let mut boundary = joined.start;
                for clip in clips.drain(first..last) {
                    if boundary < clip.start {
                        push_section(&mut joined.sections, boundary, clip.start, fallback);
                    }
                    joined.sections.extend(clip.sections);
                    joined.takes.extend(clip.takes);
                    boundary = clip.end;
                }
                if boundary < joined.end {
                    push_section(&mut joined.sections, boundary, joined.end, fallback);
                }
            }
            for pass in passes {
                joined.takes.push(candidate.make_take(pass)?);
            }
            let newest = joined.takes.last().unwrap().id;
            if joined.sections.is_empty() {
                push_section(&mut joined.sections, joined.start, joined.end, newest);
            } else {
                let mut sections = Vec::new();
                for section in std::mem::take(&mut joined.sections) {
                    push_section(
                        &mut sections,
                        section.start,
                        section.end.min(start),
                        section.take,
                    );
                    push_section(
                        &mut sections,
                        section.start.max(start),
                        section.end.min(end),
                        newest,
                    );
                    push_section(
                        &mut sections,
                        section.start.max(end),
                        section.end,
                        section.take,
                    );
                }
                joined.sections = sections;
            }
            clips.insert(first, joined);
        } else {
            if mode == Mode::Replace {
                for clip in &mut clips {
                    let sections = &clip.sections;
                    for take in &mut clip.takes {
                        take.events.retain(|event| {
                            !(start..end).contains(&event.tick)
                                || sections
                                    [sections.partition_point(|section| section.end <= event.tick)]
                                .take
                                    != take.id
                        });
                    }
                }
                let last = passes.pop().unwrap();
                passes = vec![last];
            }
            let mut events: Vec<_> = passes.into_iter().flatten().collect();
            events.sort_by_key(|event| event.tick);
            let mut events = events.into_iter().peekable();
            let mut index = clips.partition_point(|clip| clip.end <= start);
            while let Some(event) = events.peek() {
                while clips.get(index).is_some_and(|clip| clip.end <= event.tick) {
                    index += 1;
                }
                if let Some(clip) = clips.get_mut(index).filter(|clip| clip.start <= event.tick) {
                    while let Some(event) = events.next_if(|event| event.tick < clip.end) {
                        let take_id = clip.section_at(event.tick);
                        let take = clip
                            .takes
                            .iter_mut()
                            .find(|take| take.id == take_id)
                            .unwrap();
                        take.events.push(event);
                    }
                } else {
                    let left = if index == 0 {
                        start
                    } else {
                        start.max(clips[index - 1].end)
                    };
                    let right = clips.get(index).map_or(end, |clip| end.min(clip.start));
                    if event.tick >= right {
                        return Err("recorded MIDI is outside the capture range".into());
                    }
                    let mut members = Vec::new();
                    while let Some(event) = events.next_if(|event| event.tick < right) {
                        members.push(event);
                    }
                    let take = candidate.make_take(members)?;
                    let section = Section {
                        start: left,
                        end: right,
                        take: take.id,
                    };
                    clips.insert(
                        index,
                        Clip {
                            start: left,
                            end: right,
                            takes: vec![take],
                            sections: vec![section],
                            layer: 0,
                            cycle: None,
                        },
                    );
                }
                index += 1;
            }
        }
        candidate.tracks.insert(track, clips);
        candidate.validate(candidate.tracks.keys().copied())?;
        *self = candidate;
        Ok(true)
    }

    fn record_layered(
        &mut self,
        clips: &mut Vec<Clip>,
        mode: Mode,
        [start, end]: [u64; 2],
        mut passes: Vec<Vec<Event>>,
        recover: bool,
    ) -> Result<()> {
        for pass in &mut passes {
            pass.sort_by_key(|event| event.tick);
        }
        let mut spans = exposed(clips);
        if let Some(&([first, _], _)) = spans.first()
            && first > start
        {
            spans.insert(0, ([start, first], None));
        }
        if let Some(&([_, last], _)) = spans.last()
            && last < end
        {
            spans.push(([last, end], None));
        }
        for ([left, right], clip) in spans {
            let left = start.max(left);
            let right = end.min(right);
            if left >= right {
                continue;
            }
            let mut chunks = passes.iter().map(|pass| {
                let first = pass.partition_point(|event| event.tick < left);
                let last = pass.partition_point(|event| event.tick < right);
                &pass[first..last]
            });
            let Some(clip) = clip.map(|index| &mut clips[index]) else {
                let selected: Vec<_> = if recover || mode == Mode::NewTake {
                    chunks.filter(|chunk| !chunk.is_empty()).collect()
                } else if mode == Mode::Replace {
                    chunks
                        .next_back()
                        .filter(|chunk| !chunk.is_empty())
                        .into_iter()
                        .collect()
                } else {
                    chunks.filter(|chunk| !chunk.is_empty()).collect()
                };
                if selected.is_empty() {
                    continue;
                }
                let takes = if recover || mode == Mode::NewTake {
                    selected
                        .iter()
                        .map(|events| self.make_take(events.to_vec()))
                        .collect::<Result<Vec<_>>>()?
                } else {
                    vec![self.make_take(selected.into_iter().flatten().copied().collect())?]
                };
                let take = takes.last().unwrap().id;
                clips.push(Clip {
                    start: left,
                    end: right,
                    takes,
                    sections: vec![Section {
                        start: left,
                        end: right,
                        take,
                    }],
                    layer: 0,
                    cycle: None,
                });
                continue;
            };
            if recover || mode == Mode::NewTake {
                if !recover && clip.takes.len().saturating_add(passes.len()) > MAX_TAKES {
                    return Err("MIDI take limit exceeded".into());
                }
                for events in chunks.filter(|events| !recover || !events.is_empty()) {
                    clip.takes.push(self.make_take(events.to_vec())?);
                }
                if !recover {
                    let take = clip.takes.last().unwrap().id;
                    let mut sections = Vec::new();
                    for section in std::mem::take(&mut clip.sections) {
                        push_section(
                            &mut sections,
                            section.start,
                            section.end.min(left),
                            section.take,
                        );
                        push_section(
                            &mut sections,
                            section.start.max(left),
                            section.end.min(right),
                            take,
                        );
                        push_section(
                            &mut sections,
                            section.start.max(right),
                            section.end,
                            section.take,
                        );
                    }
                    clip.sections = sections;
                }
            } else {
                let events: Vec<_> = if mode == Mode::Replace {
                    for take in &mut clip.takes {
                        take.events.retain(|event| {
                            let section = clip
                                .sections
                                .partition_point(|section| section.end <= event.tick);
                            !(left..right).contains(&event.tick)
                                || clip.sections[section].take != take.id
                        });
                    }
                    chunks.next_back().into_iter().flatten().copied().collect()
                } else {
                    chunks.flatten().copied().collect()
                };
                for event in events {
                    let take = clip.section_at(event.tick);
                    clip.takes
                        .iter_mut()
                        .find(|item| item.id == take)
                        .unwrap()
                        .events
                        .push(event);
                }
            }
        }
        clips.sort_by_key(|clip| (clip.start, clip.layer));
        Ok(())
    }

    fn recover(
        &mut self,
        clips: &mut Vec<Clip>,
        [start, end]: [u64; 2],
        mut passes: Vec<Vec<Event>>,
    ) -> Result<()> {
        for pass in &mut passes {
            pass.sort_by_key(|event| event.tick);
        }
        let mut position = start;
        let mut index = clips.partition_point(|clip| clip.end <= start);
        while position < end {
            let existing = clips.get(index).is_some_and(|clip| clip.start <= position);
            let boundary = clips
                .get(index)
                .map_or(end, |clip| if existing { clip.end } else { clip.start })
                .min(end);
            let mut takes = Vec::new();
            for pass in &passes {
                let first = pass.partition_point(|event| event.tick < position);
                let last = pass.partition_point(|event| event.tick < boundary);
                if first != last {
                    takes.push(self.make_take(pass[first..last].to_vec())?);
                }
            }
            if existing {
                clips[index].takes.extend(takes);
                index += 1;
            } else if let Some(take) = takes.last() {
                let section = Section {
                    start: position,
                    end: boundary,
                    take: take.id,
                };
                clips.insert(
                    index,
                    Clip {
                        start: position,
                        end: boundary,
                        takes,
                        sections: vec![section],
                        layer: 0,
                        cycle: None,
                    },
                );
                index += 1;
            }
            position = boundary;
        }
        Ok(())
    }

    fn make_take(&mut self, events: Vec<Event>) -> Result<Take> {
        let id = self.next_take;
        self.next_take = id.checked_add(1).ok_or("MIDI take identity exhausted")?;
        Ok(Take { id, events })
    }
}

fn validate_events(take: &Take, start: u64, end: u64) -> Result<()> {
    for event in &take.events {
        if !(start..end).contains(&event.tick)
            || match event.message {
                Message::Note {
                    channel,
                    key,
                    velocity,
                    end,
                    release,
                    ..
                } => {
                    channel >= 16
                        || key >= 128
                        || !(1..128).contains(&velocity)
                        || release >= 128
                        || end <= event.tick
                        || end > MAX_TICK
                }
                Message::Control(bytes) => !supported_control(bytes),
                Message::CurvedControl { bytes, bend } => {
                    !supported_control(bytes)
                        || bytes[0] & 0xf0 != 0xb0
                        || !continuous_cc(bytes[1])
                        || !(-100..=100).contains(&bend)
                }
            }
        {
            return Err("invalid MIDI event".into());
        }
    }
    Ok(())
}

fn validate_sections(
    sections: &[Section],
    start: u64,
    end: u64,
    take_ids: &HashSet<u64>,
) -> Result<()> {
    let mut boundary = start;
    for section in sections {
        if section.start != boundary
            || section.end <= boundary
            || section.end > end
            || !take_ids.contains(&section.take)
        {
            return Err("invalid MIDI take section".into());
        }
        boundary = section.end;
    }
    if boundary != end {
        return Err("MIDI take sections do not cover the clip".into());
    }
    Ok(())
}

fn push_section(sections: &mut Vec<Section>, start: u64, end: u64, take: u64) {
    if start >= end {
        return;
    }
    if let Some(previous) = sections.last_mut()
        && previous.end == start
        && previous.take == take
    {
        previous.end = end;
    } else {
        sections.push(Section { start, end, take });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(tick: u64, key: u8) -> Event {
        Event {
            tick,
            order: key.into(),
            message: Message::Note {
                channel: 0,
                key,
                velocity: 83,
                end: tick + 10,
                release: 12,
                release_order: u32::from(key) + 1,
            },
        }
    }

    fn audible(clip: &Clip) -> Vec<u64> {
        let mut ticks: Vec<_> = clip.active_events().map(|event| event.tick).collect();
        ticks.sort_unstable();
        ticks
    }

    #[test]
    fn replace_uses_the_last_nonempty_pass_and_preserves_inactive_takes() {
        let mut arrangement = Arrangement::default();
        arrangement
            .record(
                1,
                Mode::Merge,
                [0, 100],
                vec![vec![note(10, 50), note(60, 51)]],
                false,
            )
            .unwrap();
        arrangement
            .record(
                1,
                Mode::NewTake,
                [20, 100],
                vec![vec![note(30, 60), note(80, 61)]],
                false,
            )
            .unwrap();
        let before = arrangement.clone();
        assert!(
            !arrangement
                .record(1, Mode::Replace, [20, 100], vec![vec![]], false)
                .unwrap()
        );
        assert_eq!(arrangement, before);
        arrangement
            .record(
                1,
                Mode::Replace,
                [20, 100],
                vec![vec![note(55, 55)], vec![note(35, 55)], vec![]],
                false,
            )
            .unwrap();
        let clip = &arrangement.clips(1)[0];
        assert_eq!(audible(clip), vec![10, 35]);
        assert_eq!(clip.takes[0].events, before.clips(1)[0].takes[0].events);
        assert_eq!([clip.start, clip.end], [0, 100]);
    }

    #[test]
    fn new_take_joins_touched_clips_and_keeps_prior_sections_and_take_ids() {
        let mut arrangement = Arrangement::default();
        arrangement
            .record(
                1,
                Mode::Merge,
                [0, 40],
                vec![vec![note(10, 46), note(30, 45)]],
                false,
            )
            .unwrap();
        arrangement
            .record(
                1,
                Mode::Merge,
                [70, 110],
                vec![vec![note(80, 47), note(100, 48)]],
                false,
            )
            .unwrap();
        let originals = [
            arrangement.clips(1)[0].takes[0].id,
            arrangement.clips(1)[1].takes[0].id,
        ];
        arrangement
            .record(
                1,
                Mode::NewTake,
                [20, 90],
                vec![vec![note(45, 69)], vec![note(55, 70)]],
                false,
            )
            .unwrap();
        let [clip] = arrangement.clips(1) else {
            panic!("expected a unified clip")
        };
        assert_eq!([clip.start, clip.end], [0, 110]);
        assert_eq!(clip.takes.len(), 4);
        assert_eq!([clip.takes[0].id, clip.takes[1].id], originals);
        assert_eq!(audible(clip), vec![10, 55, 100]);
        arrangement.validate([1].into_iter()).unwrap();
    }

    #[test]
    fn merge_uses_the_active_take_and_interruption_keeps_the_existing_arrangement() {
        let mut arrangement = Arrangement::default();
        arrangement
            .record(
                1,
                Mode::NewTake,
                [0, 50],
                vec![vec![note(10, 51)], vec![note(20, 52)]],
                false,
            )
            .unwrap();
        arrangement
            .record(
                1,
                Mode::Merge,
                [0, 50],
                vec![vec![note(30, 53)], vec![note(40, 54)]],
                false,
            )
            .unwrap();
        assert_eq!(audible(&arrangement.clips(1)[0]), vec![20, 30, 40]);
        arrangement
            .record(1, Mode::Merge, [60, 90], vec![vec![note(75, 72)]], false)
            .unwrap();
        let before = arrangement.clips(1).to_vec();
        arrangement
            .record(
                1,
                Mode::Replace,
                [0, 90],
                vec![vec![note(31, 68), note(65, 74)]],
                true,
            )
            .unwrap();
        for (previous, recovered) in before.iter().zip(arrangement.clips(1)) {
            assert_eq!(audible(previous), audible(recovered));
            assert_eq!(previous.sections, recovered.sections);
            assert_eq!(previous.takes.len() + 1, recovered.takes.len());
        }
    }

    #[test]
    fn invalid_capture_keeps_the_previous_arrangement() {
        let mut arrangement = Arrangement::default();
        arrangement
            .record(1, Mode::Merge, [0, 100], vec![vec![note(10, 50)]], false)
            .unwrap();
        let before = arrangement.clone();
        assert!(
            arrangement
                .record(1, Mode::Replace, [0, 20], vec![vec![note(20, 60)]], false)
                .is_err()
        );
        assert_eq!(arrangement, before);
    }
}
