use super::{
    Arrangement, Clip, Cycle, Event, MAX_EVENTS, MAX_SECTIONS, MAX_TICK, Message, Section, Take,
    push_section,
};
use crate::{
    Result,
    timeline::{QUARTER, Timeline},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClipId(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Snap {
    Bar,
    #[default]
    Beat,
    Eighth,
    Sixteenth,
    Off,
}

impl Snap {
    pub const ALL: [Self; 5] = [
        Self::Bar,
        Self::Beat,
        Self::Eighth,
        Self::Sixteenth,
        Self::Off,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Bar => "Bar",
            Self::Beat => "Beat",
            Self::Eighth => "1/8",
            Self::Sixteenth => "1/16",
            Self::Off => "Off",
        }
    }

    pub fn step(self, timeline: &Timeline, tick: u64) -> u64 {
        let (_, tempo) = timeline.section_at(tick);
        match self {
            Self::Bar => tempo.bar_ticks(),
            Self::Beat => tempo.beat_ticks(),
            Self::Eighth => QUARTER / 2,
            Self::Sixteenth => QUARTER / 4,
            Self::Off => 1,
        }
    }

    pub fn nearest(self, timeline: &Timeline, tick: u64) -> u64 {
        let (origin, _) = timeline.section_at(tick);
        let step = self.step(timeline, tick);
        let next = timeline
            .sections()
            .map(|(start, _)| start)
            .find(|&start| start > tick)
            .unwrap_or(MAX_TICK);
        (origin + ((tick - origin + step / 2) / step) * step).min(next)
    }
}

#[derive(Clone, Debug)]
pub enum ClipEdit {
    Create {
        track: u64,
        start: u64,
        end: u64,
    },
    Move {
        destinations: Vec<(ClipId, u64)>,
        delta: i64,
    },
    Resize {
        clip: ClipId,
        end: u64,
        repeat: bool,
    },
    Split {
        clips: Vec<ClipId>,
        tick: u64,
        confirmed: bool,
    },
    Duplicate(Vec<ClipId>),
    Delete(Vec<ClipId>),
    EditEvents {
        clip: ClipId,
        label: &'static str,
        take: Option<u64>,
        changes: Vec<super::EventChange>,
    },
}

impl ClipEdit {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Create { .. } => "Create Clip",
            Self::Move { .. } => "Move Clips",
            Self::Resize { repeat: true, .. } => "Loop Clip",
            Self::Resize { .. } => "Resize Clip",
            Self::Split { .. } => "Split at Playhead",
            Self::Duplicate(_) => "Duplicate",
            Self::Delete(_) => "Delete Clips",
            Self::EditEvents { label, .. } => label,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ClipEditResult {
    Applied(Vec<ClipId>),
    Unchanged,
    NeedsSplitConfirmation,
}

impl Arrangement {
    pub fn find_clip(&self, id: ClipId) -> Option<(u64, &Clip)> {
        self.tracks.iter().find_map(|(&track, clips)| {
            clips
                .iter()
                .find(|clip| clip.id() == id)
                .map(|clip| (track, clip))
        })
    }

    pub fn split_crosses_notes(&self, ids: &[ClipId], tick: u64) -> bool {
        ids.iter().any(|&id| {
            self.find_clip(id).is_some_and(|(_, clip)| {
                clip.start < tick && tick < clip.end && clip.crosses_notes(tick)
            })
        })
    }

    pub fn edit(
        &mut self,
        edit: &ClipEdit,
        track_ids: impl Iterator<Item = u64>,
    ) -> Result<ClipEditResult> {
        let mut candidate = self.clone();
        let result = candidate.apply_edit(edit)?;
        if matches!(result, ClipEditResult::Applied(_)) {
            candidate.tracks.retain(|_, clips| !clips.is_empty());
            for clips in candidate.tracks.values_mut() {
                clips.sort_by_key(|clip| (clip.start, clip.layer));
            }
            candidate.validate(track_ids)?;
            *self = candidate;
        }
        Ok(result)
    }

    fn selected(&self, ids: impl Iterator<Item = ClipId>) -> Result<Vec<(u64, Clip)>> {
        let mut seen = HashSet::new();
        ids.filter(|id| seen.insert(*id))
            .map(|id| {
                self.find_clip(id)
                    .map(|(track, clip)| (track, clip.clone()))
                    .ok_or_else(|| "A selected clip has changed or was removed.".into())
            })
            .collect()
    }

    fn next_layer(&self) -> Result<u64> {
        self.tracks
            .values()
            .flatten()
            .map(|clip| clip.layer)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| "Clip stacking limit exceeded.".into())
    }

    fn remove(&mut self, track: u64, id: ClipId) {
        self.tracks
            .get_mut(&track)
            .unwrap()
            .retain(|clip| clip.id() != id);
    }

    fn renew_take_ids(&mut self, clip: &mut Clip) -> Result<()> {
        for take in &mut clip.takes {
            let previous = take.id;
            *take = self.make_take(std::mem::take(&mut take.events))?;
            for section in &mut clip.sections {
                if section.take == previous {
                    section.take = take.id;
                }
            }
            if let Some(cycle) = &mut clip.cycle {
                if let Some(source) = cycle.takes.iter_mut().find(|source| source.id == previous) {
                    source.id = take.id;
                }
                for section in &mut cycle.sections {
                    if section.take == previous {
                        section.take = take.id;
                    }
                }
            }
        }
        Ok(())
    }

    fn apply_edit(&mut self, edit: &ClipEdit) -> Result<ClipEditResult> {
        let mut selection = Vec::new();
        match edit {
            ClipEdit::EditEvents {
                clip,
                take,
                changes,
                ..
            } => {
                let (track, previous) =
                    self.find_clip(*clip).ok_or("The MIDI clip was removed.")?;
                let mut updated = previous.clone();
                updated.edit_events(changes, *take)?;
                if &updated == previous {
                    return Ok(ClipEditResult::Unchanged);
                }
                let slot = self
                    .tracks
                    .get_mut(&track)
                    .unwrap()
                    .iter_mut()
                    .find(|item| item.id() == *clip)
                    .unwrap();
                *slot = updated;
                selection.push(*clip);
            }
            ClipEdit::Create { track, start, end } => {
                let take = self.make_take(Vec::new())?;
                let layer = self.next_layer()?;
                selection.push(ClipId(take.id));
                self.tracks.entry(*track).or_default().push(Clip {
                    start: *start,
                    end: *end,
                    sections: vec![Section {
                        start: *start,
                        end: *end,
                        take: take.id,
                    }],
                    takes: vec![take],
                    layer,
                    cycle: None,
                });
            }
            ClipEdit::Move {
                destinations,
                delta,
            } => {
                let mut clips = self.selected(destinations.iter().map(|&(id, _)| id))?;
                let destination = |id| {
                    destinations
                        .iter()
                        .find(|&&(candidate, _)| candidate == id)
                        .unwrap()
                        .1
                };
                if *delta == 0
                    && clips
                        .iter()
                        .all(|(track, clip)| *track == destination(clip.id()))
                {
                    return Ok(ClipEditResult::Unchanged);
                }
                let mut layer = self.next_layer()?;
                clips.sort_by_key(|(_, clip)| (clip.layer, clip.start, clip.id()));
                for (track, mut clip) in clips {
                    let id = clip.id();
                    self.remove(track, id);
                    clip.shift(*delta)?;
                    clip.layer = layer;
                    layer = layer
                        .checked_add(1)
                        .ok_or("Clip stacking limit exceeded.")?;
                    selection.push(id);
                    self.tracks.entry(destination(id)).or_default().push(clip);
                }
            }
            ClipEdit::Resize {
                clip: id,
                end,
                repeat,
            } => {
                let (track, mut clip) = self
                    .find_clip(*id)
                    .map(|(track, clip)| (track, clip.clone()))
                    .ok_or("The clip was removed.")?;
                let end = if *repeat { clip.loop_end(*end) } else { *end };
                if clip.end == end {
                    return Ok(ClipEditResult::Unchanged);
                }
                clip.resize(end, *repeat)?;
                self.remove(track, *id);
                self.tracks.entry(track).or_default().push(clip);
                selection.push(*id);
            }
            ClipEdit::Split {
                clips,
                tick,
                confirmed,
            } => {
                let clips: Vec<_> = self
                    .selected(clips.iter().copied())?
                    .into_iter()
                    .filter(|(_, clip)| clip.start < *tick && *tick < clip.end)
                    .collect();
                if clips.is_empty() {
                    return Ok(ClipEditResult::Unchanged);
                }
                if !confirmed && clips.iter().any(|(_, clip)| clip.crosses_notes(*tick)) {
                    return Ok(ClipEditResult::NeedsSplitConfirmation);
                }
                for (track, mut left) in clips {
                    let boundary = left.split_boundary(*tick);
                    let mut right = left.split(*tick);
                    self.renew_take_ids(&mut right)?;
                    if let Some(events) = boundary {
                        let take = self.make_take(events)?;
                        let first = &mut right.sections[0];
                        let splice = Section {
                            start: *tick,
                            end: *tick + 1,
                            take: take.id,
                        };
                        if first.end == splice.end {
                            *first = splice;
                        } else {
                            first.start = splice.end;
                            right.sections.insert(0, splice);
                        }
                        right.takes.push(take);
                    }
                    self.remove(track, left.id());
                    selection.extend([left.id(), right.id()]);
                    self.tracks.entry(track).or_default().extend([left, right]);
                }
            }
            ClipEdit::Duplicate(ids) => {
                let mut clips = self.selected(ids.iter().copied())?;
                if clips.is_empty() {
                    return Ok(ClipEditResult::Unchanged);
                }
                let min = clips.iter().map(|(_, clip)| clip.start).min().unwrap();
                let max = clips.iter().map(|(_, clip)| clip.end).max().unwrap();
                let mut layer = self.next_layer()?;
                clips.sort_by_key(|(_, clip)| (clip.layer, clip.start, clip.id()));
                for (track, mut clip) in clips {
                    clip.shift((max - min) as i64)?;
                    self.renew_take_ids(&mut clip)?;
                    clip.layer = layer;
                    layer = layer
                        .checked_add(1)
                        .ok_or("Clip stacking limit exceeded.")?;
                    selection.push(clip.id());
                    self.tracks.entry(track).or_default().push(clip);
                }
            }
            ClipEdit::Delete(ids) => {
                let clips = self.selected(ids.iter().copied())?;
                if clips.is_empty() {
                    return Ok(ClipEditResult::Unchanged);
                }
                for (track, clip) in clips {
                    self.remove(track, clip.id());
                }
            }
        }
        Ok(ClipEditResult::Applied(selection))
    }
}

impl Clip {
    pub fn loop_end(&self, requested: u64) -> u64 {
        let requested = requested.min(MAX_TICK);
        let (first, period) = self
            .cycle
            .as_ref()
            .map_or((self.end, self.end - self.start), |cycle| {
                (self.start + cycle.period - cycle.phase, cycle.period)
            });
        if requested == self.end || first > MAX_TICK {
            return self.end;
        }
        let distance = requested.saturating_sub(first);
        if requested > self.end {
            let cycles = distance.div_ceil(period).min((MAX_TICK - first) / period);
            (first + cycles * period).max(self.end)
        } else {
            (first + distance / period * period).min(self.end)
        }
    }

    fn split_boundary(&self, tick: u64) -> Option<Vec<Event>> {
        let current = self.section_at(tick);
        let changed_take = self.takes.iter().any(|take| {
            take.events.iter().any(|event| {
                self.crossing_note(event, tick)
                    && (self.section_at(event.tick) == take.id) != (current == take.id)
            })
        });
        let control_key =
            |[status, key, _]: [u8; 3]| (status, if status & 0xf0 == 0xe0 { 0 } else { key });
        let mut controls = BTreeMap::<_, Event>::new();
        let mut at_cut = HashSet::new();
        let mut boundary = Vec::new();
        let [run_start, _] = self.playback_run(tick);
        for event in self
            .active_events()
            .filter(|event| (run_start..=tick).contains(&event.tick))
        {
            if event.tick == tick {
                boundary.push(*event);
                if let Some(bytes) = event.message.control() {
                    at_cut.insert(control_key(bytes));
                }
                continue;
            }
            match event.message {
                Message::Note { .. } if self.crossing_note(event, tick) => {
                    boundary.push(Event { tick, ..*event })
                }
                message if message.control().is_some_and(super::carried_control) => {
                    let bytes = message.control().unwrap();
                    let key = control_key(bytes);
                    if controls
                        .get(&key)
                        .is_none_or(|old| (old.tick, old.order) <= (event.tick, event.order))
                    {
                        controls.insert(key, *event);
                    }
                }
                _ => {}
            }
        }
        let mut carry: Vec<_> = controls
            .into_iter()
            .filter_map(|(key, event)| {
                let [status, value1, value2] = event.message.control().unwrap();
                let neutral = status & 0xf0 == 0xe0 && value1 == 0 && value2 == 64;
                (!neutral && !at_cut.contains(&key)).then_some(Event {
                    tick,
                    order: 0,
                    message: Message::Control([status, value1, value2]),
                })
            })
            .collect();
        if !changed_take && carry.is_empty() {
            return None;
        }
        carry.extend(boundary);
        Some(carry)
    }

    fn crosses_notes(&self, tick: u64) -> bool {
        self.takes
            .iter()
            .flat_map(|take| &take.events)
            .any(|event| self.crossing_note(event, tick))
    }

    fn crossing_note(&self, event: &Event, tick: u64) -> bool {
        if event.tick >= tick {
            return false;
        }
        let boundary = self.playback_run(event.tick)[1];
        matches!(event.message, Message::Note { end, .. } if end.min(boundary) > tick)
    }

    fn shift(&mut self, delta: i64) -> Result<()> {
        let shift = |tick: &mut u64| -> Result<()> {
            *tick = tick
                .checked_add_signed(delta)
                .filter(|&tick| tick <= MAX_TICK)
                .ok_or("The clip would extend beyond the timeline.")?;
            Ok(())
        };
        shift(&mut self.start)?;
        shift(&mut self.end)?;
        for section in &mut self.sections {
            shift(&mut section.start)?;
            shift(&mut section.end)?;
        }
        for event in self.takes.iter_mut().flat_map(|take| &mut take.events) {
            shift(&mut event.tick)?;
            if let Message::Note { end, .. } = &mut event.message {
                shift(end)?;
            }
        }
        Ok(())
    }

    fn resize(&mut self, mut end: u64, mut repeat: bool) -> Result<()> {
        if end <= self.start || end > MAX_TICK {
            return Err("The clip must have a positive length within the timeline.".into());
        }
        if self
            .cycle
            .as_ref()
            .is_some_and(|cycle| end > self.start + cycle.extent)
        {
            end = self.loop_end(end);
            repeat = true;
        }
        let source_end = self.source_end();
        if repeat && end > self.end {
            let mut cycle = self
                .cycle
                .take()
                .unwrap_or_else(|| Box::new(self.make_cycle(self.end)));
            let from = self.end.max(self.start + cycle.extent);
            if end > from {
                let mut tail = Vec::new();
                self.sections.retain_mut(|section| {
                    if section.end > end {
                        tail.push(Section {
                            start: section.start.max(end),
                            ..*section
                        });
                    }
                    section.end = section.end.min(from);
                    section.start < section.end
                });
                for take in &mut self.takes {
                    take.events
                        .retain(|event| !(from..end).contains(&event.tick));
                }
                self.extend_cycle(from, end, &cycle)?;
                for section in tail {
                    push_section(&mut self.sections, section.start, section.end, section.take);
                }
                cycle.extent = end - self.start;
            }
            self.cycle = Some(cycle);
        } else if end > source_end {
            let take = self.sections.last().unwrap().take;
            push_section(&mut self.sections, source_end, end, take);
        }
        self.end = end;
        Ok(())
    }

    fn make_cycle(&self, end_tick: u64) -> Cycle {
        let period = end_tick - self.start;
        let sections = self
            .sections
            .iter()
            .take_while(|section| section.start < end_tick)
            .map(|section| Section {
                start: section.start - self.start,
                end: section.end.min(end_tick) - self.start,
                take: section.take,
            })
            .collect();
        let takes = self
            .takes
            .iter()
            .map(|take| {
                let mut events: Vec<_> = take
                    .events
                    .iter()
                    .copied()
                    .filter(|event| event.tick < end_tick)
                    .map(|mut event| {
                        event.tick -= self.start;
                        if let Message::Note { end, .. } = &mut event.message {
                            *end = (*end).min(end_tick) - self.start;
                        }
                        event
                    })
                    .collect();
                events.sort_by_key(|event| (event.tick, event.order));
                Take {
                    id: take.id,
                    events,
                }
            })
            .collect();
        Cycle {
            period,
            phase: 0,
            extent: period,
            takes,
            sections,
        }
    }

    fn extend_cycle(&mut self, from: u64, end: u64, cycle: &Cycle) -> Result<()> {
        let mut event_count = self
            .takes
            .iter()
            .map(|take| take.events.len())
            .sum::<usize>();
        if cycle.takes.iter().all(|take| take.events.is_empty()) && cycle.sections.len() == 1 {
            push_section(&mut self.sections, from, end, cycle.sections[0].take);
            return Ok(());
        }
        if (end - from).div_ceil(cycle.period) > MAX_SECTIONS as u64 {
            return Err("Clip repetition exceeds the MIDI project limits.".into());
        }
        let mut cursor = from;
        let mut phase = (cycle.phase + from - self.start) % cycle.period;
        while cursor < end {
            let span = (cycle.period - phase).min(end - cursor);
            let until = phase + span;
            let first = cycle
                .sections
                .partition_point(|section| section.end <= phase);
            for section in cycle.sections[first..]
                .iter()
                .take_while(|section| section.start < until)
            {
                push_section(
                    &mut self.sections,
                    cursor + (section.start.max(phase) - phase),
                    cursor + (section.end.min(until) - phase),
                    section.take,
                );
            }
            if self.sections.len() > MAX_SECTIONS {
                return Err("Clip repetition exceeds the MIDI section limit.".into());
            }
            for source in &cycle.takes {
                let last = source.events.partition_point(|event| event.tick < until);
                let Some(take) = self.takes.iter_mut().find(|take| take.id == source.id) else {
                    continue;
                };
                for event in &source.events[..last] {
                    let crossing = match event.message {
                        Message::Note { end, .. } => event.tick < phase && end > phase,
                        _ => false,
                    };
                    if event.tick < phase && !crossing {
                        continue;
                    }
                    let mut event = *event;
                    event.tick = cursor + event.tick.saturating_sub(phase);
                    if let Message::Note { end, .. } = &mut event.message {
                        *end =
                            (cursor + (*end).min(cycle.period).saturating_sub(phase)).min(MAX_TICK);
                    }
                    event_count += 1;
                    if event_count > MAX_EVENTS {
                        return Err("Clip repetition exceeds the MIDI event limit.".into());
                    }
                    take.events.push(event);
                }
            }
            cursor += span;
            phase = 0;
        }
        Ok(())
    }

    fn split(&mut self, tick: u64) -> Self {
        let mut right = self.clone();
        right.start = tick;
        for take in &mut right.takes {
            take.events.retain_mut(|event| {
                if event.tick >= tick {
                    return true;
                }
                if self.crossing_note(event, tick) {
                    event.tick = tick;
                    true
                } else {
                    false
                }
            });
        }
        self.end = tick;
        self.sections.retain_mut(|section| {
            section.end = section.end.min(tick);
            section.start < section.end
        });
        right.sections.retain_mut(|section| {
            section.start = section.start.max(tick);
            section.start < section.end
        });
        for take in &mut self.takes {
            take.events.retain_mut(|event| {
                if let Message::Note { end, .. } = &mut event.message {
                    *end = (*end).min(tick);
                }
                event.tick < tick
            });
        }
        if let Some(cycle) = &mut right.cycle {
            let offset = tick - self.start;
            if cycle.extent > offset {
                cycle.phase = (cycle.phase + offset) % cycle.period;
                cycle.extent -= offset;
            } else {
                right.cycle = None;
            }
        }
        if let Some(cycle) = &mut self.cycle {
            cycle.extent = cycle.extent.min(tick - self.start);
        }
        right
    }
}

/// Partition a lane into runs of the same foreground clip, including internal gaps.
pub fn exposed(clips: &[Clip]) -> Vec<([u64; 2], Option<usize>)> {
    let mut edges: Vec<_> = clips
        .iter()
        .enumerate()
        .flat_map(|(index, clip)| [(clip.start, true, index), (clip.end, false, index)])
        .collect();
    edges.sort_unstable();
    let mut edges = edges.into_iter().peekable();
    let mut active = BTreeSet::new();
    let mut spans: Vec<([u64; 2], Option<usize>)> = Vec::new();
    while let Some((tick, starts, index)) = edges.next() {
        let key = (clips[index].layer, index);
        if starts {
            active.insert(key);
        } else {
            active.remove(&key);
        }
        if let Some(&(next, _, _)) = edges.peek().filter(|&&(next, _, _)| next > tick) {
            let foreground = active.last().map(|&(_, index)| index);
            if let Some((range, _)) = spans.last_mut().filter(|(_, index)| *index == foreground) {
                range[1] = next;
            } else {
                spans.push(([tick, next], foreground));
            }
        }
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi::{Event, Mode};

    fn note(tick: u64, end: u64, key: u8) -> Event {
        Event {
            tick,
            order: 0,
            message: Message::Note {
                channel: 0,
                key,
                velocity: 96,
                end,
                release: 12,
                release_order: 1,
            },
        }
    }

    fn edit(arrangement: &mut Arrangement, op: ClipEdit) -> ClipEditResult {
        arrangement.edit(&op, [1, 2].into_iter()).unwrap()
    }

    #[test]
    fn a_group_move_is_atomic_and_preserves_stack_and_hidden_clips() {
        let mut arrangement = Arrangement::default();
        arrangement
            .record(
                1,
                Mode::Merge,
                [0, 400],
                vec![vec![note(10, 350, 60)]],
                false,
            )
            .unwrap();
        arrangement
            .record(
                2,
                Mode::Merge,
                [500, 600],
                vec![vec![note(520, 570, 64)]],
                false,
            )
            .unwrap();
        let lower = arrangement.clips(1)[0].id();
        let upper = arrangement.clips(2)[0].id();
        edit(
            &mut arrangement,
            ClipEdit::Move {
                destinations: vec![(upper, 1)],
                delta: -400,
            },
        );
        let lane = arrangement.clips(1);
        assert_eq!(
            exposed(lane),
            vec![
                ([0, 100], Some(0)),
                ([100, 200], Some(1)),
                ([200, 400], Some(0))
            ]
        );
        let before = arrangement.clone();
        let invalid = ClipEdit::Move {
            destinations: vec![(lower, 2), (upper, 2)],
            delta: -1,
        };
        assert!(arrangement.edit(&invalid, [1, 2].into_iter()).is_err());
        assert_eq!(arrangement, before);
        edit(&mut arrangement, ClipEdit::Delete(vec![upper]));
        assert_eq!(arrangement.clips(1), &before.clips(1)[..1]);
    }

    #[test]
    fn a_split_carries_switch_and_zero_valued_controllers() {
        let control = |tick, order, bytes| Event {
            tick,
            order,
            message: Message::Control(bytes),
        };
        let mut arrangement = Arrangement::default();
        arrangement
            .record(
                1,
                Mode::Merge,
                [0, 800],
                vec![vec![
                    note(0, 600, 60),
                    control(100, 1, [0xb0, 7, 0]),
                    control(120, 2, [0xb0, 11, 0]),
                    control(140, 3, [0xb0, 64, 127]),
                    control(160, 4, [0xb0, 123, 0]),
                ]],
                false,
            )
            .unwrap();
        let clip = arrangement.clips(1)[0].id();
        edit(
            &mut arrangement,
            ClipEdit::Split {
                clips: vec![clip],
                tick: 400,
                confirmed: true,
            },
        );
        let [_, right] = arrangement.clips(1) else {
            panic!()
        };
        let carried = right
            .takes
            .iter()
            .flat_map(|take| &take.events)
            .filter(|event| event.tick == 400)
            .filter_map(|event| event.message.control())
            .collect::<Vec<_>>();
        assert!(
            carried.contains(&[0xb0, 7, 0]),
            "a real zero volume must survive the split"
        );
        assert!(carried.contains(&[0xb0, 11, 0]));
        assert!(
            carried.contains(&[0xb0, 64, 127]),
            "a held sustain pedal must carry into the right splice"
        );
        assert!(
            !carried.contains(&[0xb0, 123, 0]),
            "channel-mode commands are not carried as state"
        );
    }

    #[test]
    fn extending_a_cycle_from_a_mid_cycle_phase_keeps_a_crossing_note() {
        let source = Event {
            tick: 0,
            order: 0,
            message: Message::Note {
                channel: 0,
                key: 60,
                velocity: 100,
                end: 350,
                release: 0,
                release_order: 1,
            },
        };
        let take = Take {
            id: 1,
            events: vec![source],
        };
        let section = Section {
            start: 0,
            end: 350,
            take: 1,
        };
        let mut clip = Clip {
            start: 0,
            end: 400,
            takes: vec![take.clone()],
            sections: vec![section],
            layer: 0,
            cycle: Some(Box::new(Cycle {
                period: 400,
                phase: 100,
                extent: 400,
                takes: vec![take],
                sections: vec![section],
            })),
        };
        clip.resize(800, true).unwrap();
        let notes = clip
            .takes
            .iter()
            .flat_map(|take| &take.events)
            .filter_map(|event| match event.message {
                Message::Note { end, .. } => Some((event.tick, end)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(
            notes.contains(&(400, 650)),
            "the note crossing the mid-cycle seam is clipped at it instead of vanishing: {notes:?}"
        );
    }

    #[test]
    fn splitting_a_legacy_repeat_tail_keeps_the_tail_editable() {
        let json = r#"{
            "mode": "Merge",
            "tracks": {"1": [{
                "start": 0,
                "end": 600,
                "takes": [{"id": 1, "events": []}],
                "sections": [{"start": 0, "end": 600, "take": 1}],
                "cycle": {
                    "period": 200,
                    "phase": 0,
                    "extent": 400,
                    "takes": [{"id": 1, "events": []}],
                    "sections": [{"start": 0, "end": 200, "take": 1}]
                }
            }]},
            "next_take": 2
        }"#;
        let mut arrangement: Arrangement = serde_json::from_str(json).unwrap();
        let clip = arrangement.clips(1)[0].id();
        edit(
            &mut arrangement,
            ClipEdit::Split {
                clips: vec![clip],
                tick: 500,
                confirmed: true,
            },
        );
        let [left, right] = arrangement.clips(1) else {
            panic!()
        };
        assert_eq!((left.end, right.end), (500, 600));
        assert_eq!(right.repeat_extent(), None);
        assert_eq!(
            right.roll_source_bounds(),
            [500, 600],
            "the remaining tail is an ordinary clip the editor can show"
        );
    }

    #[test]
    fn a_looped_clip_lengthened_from_its_lower_edge_adds_whole_cycles() {
        let mut arrangement = Arrangement::default();
        arrangement
            .record(
                1,
                Mode::Merge,
                [0, 200],
                vec![vec![
                    note(0, 100, 60),
                    Event {
                        tick: 60,
                        order: 1,
                        message: Message::CurvedControl {
                            bytes: [0xb0, 11, 64],
                            bend: 0,
                        },
                    },
                ]],
                false,
            )
            .unwrap();
        let clip = arrangement.clips(1)[0].id();
        edit(
            &mut arrangement,
            ClipEdit::Resize {
                clip,
                end: 600,
                repeat: true,
            },
        );
        edit(
            &mut arrangement,
            ClipEdit::Resize {
                clip,
                end: 700,
                repeat: false,
            },
        );
        let item = &arrangement.clips(1)[0];
        assert_eq!(
            (item.end, item.repeat_extent()),
            (800, Some(800)),
            "the lower edge rounds up to whole cycles and leaves no tail"
        );
        assert_eq!(item.repeat_end(), Some(800));
        assert_eq!(item.roll_source_bounds(), [0, 200]);
        assert_eq!(
            item.roll_events([item.start, item.end])
                .into_iter()
                .filter(|event| matches!(event.source.message, Message::Note { .. }))
                .map(|event| event.tick)
                .collect::<Vec<_>>(),
            [0, 200, 400, 600],
            "every visible cycle stays editable in the roll"
        );
        assert!(
            item.active_events().all(|event| event.tick < item.end),
            "no played material sits outside the repeat"
        );
    }

    #[test]
    fn repetitions_and_split_keep_phase_and_take_content() {
        let mut arrangement = Arrangement::default();
        arrangement
            .record(
                1,
                Mode::NewTake,
                [100, 200],
                vec![vec![note(130, 170, 60)], vec![note(120, 170, 67)]],
                false,
            )
            .unwrap();
        let clip = arrangement.clips(1)[0].id();
        edit(
            &mut arrangement,
            ClipEdit::Resize {
                clip,
                end: 400,
                repeat: true,
            },
        );
        let item = &arrangement.clips(1)[0];
        assert_eq!(
            item.active_events()
                .map(|event| event.tick)
                .collect::<Vec<_>>(),
            [120, 220, 320]
        );
        assert_eq!(item.repeat_end(), Some(400));
        edit(
            &mut arrangement,
            ClipEdit::Resize {
                clip,
                end: 430,
                repeat: false,
            },
        );
        assert_eq!(
            arrangement.clips(1)[0].repeat_end(),
            Some(500),
            "lengthening a repeated clip adds whole cycles instead of a tail"
        );
        assert_eq!(arrangement.clips(1)[0].end, 500);
        let before = arrangement.clone();
        assert_eq!(
            edit(
                &mut arrangement,
                ClipEdit::Split {
                    clips: vec![clip],
                    tick: 240,
                    confirmed: false
                }
            ),
            ClipEditResult::NeedsSplitConfirmation
        );
        assert_eq!(arrangement, before);
        edit(
            &mut arrangement,
            ClipEdit::Split {
                clips: vec![clip],
                tick: 240,
                confirmed: true,
            },
        );
        let [left, right] = arrangement.clips(1) else {
            panic!()
        };
        assert_eq!(left.repeat_phase(), 0);
        assert_eq!(right.repeat_phase(), 40);
        assert!(
            left.takes
                .iter()
                .all(|take| matches!(take.events[1].message, Message::Note { end: 240, .. }))
        );
        assert!(right.takes.iter().all(|take| take.events[0].tick == 240));
        let right_id = right.id();
        edit(
            &mut arrangement,
            ClipEdit::Resize {
                clip: right_id,
                end: 550,
                repeat: true,
            },
        );
        assert!(
            arrangement
                .find_clip(right_id)
                .unwrap()
                .1
                .active_events()
                .any(|event| event.tick == 520)
        );
        let copied = edit(&mut arrangement, ClipEdit::Duplicate(vec![right_id]));
        assert!(matches!(copied, ClipEditResult::Applied(ids) if ids[0] != right_id));
        let serialized = serde_json::to_vec(&arrangement).unwrap();
        let decoded: Arrangement = serde_json::from_slice(&serialized).unwrap();
        decoded.validate([1, 2].into_iter()).unwrap();
        assert_eq!(decoded, arrangement);
    }

    #[test]
    fn recording_modes_touch_only_the_exposed_clip() {
        let mut baseline = Arrangement::default();
        baseline
            .record(
                1,
                Mode::Merge,
                [0, 400],
                vec![vec![note(150, 170, 60)]],
                false,
            )
            .unwrap();
        baseline
            .record(
                2,
                Mode::Merge,
                [100, 300],
                vec![vec![note(150, 170, 65)]],
                false,
            )
            .unwrap();
        let upper = baseline.clips(2)[0].id();
        edit(
            &mut baseline,
            ClipEdit::Move {
                destinations: vec![(upper, 1)],
                delta: 0,
            },
        );
        let lower = baseline.clips(1)[0].clone();
        for (mode, expected) in [
            (Mode::Merge, vec![150, 155]),
            (Mode::Replace, vec![155]),
            (Mode::NewTake, vec![155]),
        ] {
            let mut arrangement = baseline.clone();
            arrangement
                .record(1, mode, [140, 180], vec![vec![note(155, 170, 67)]], false)
                .unwrap();
            assert_eq!(arrangement.clips(1)[0], lower);
            let ticks: Vec<_> = arrangement
                .find_clip(upper)
                .unwrap()
                .1
                .active_events()
                .map(|event| event.tick)
                .collect();
            assert_eq!(ticks, expected);
        }
    }

    #[test]
    fn looping_after_a_resize_uses_the_visible_length_and_adds_whole_cycles() {
        let mut arrangement = Arrangement::default();
        arrangement
            .record(
                1,
                Mode::Merge,
                [0, 100],
                vec![vec![note(10, 30, 60)]],
                false,
            )
            .unwrap();
        let id = arrangement.clips(1)[0].id();
        for (end, repeat) in [(350, false), (100, false), (119, true), (221, true)] {
            edit(
                &mut arrangement,
                ClipEdit::Resize {
                    clip: id,
                    end,
                    repeat,
                },
            );
        }
        let clip = &arrangement.clips(1)[0];
        assert_eq!((clip.cycle(), clip.end), (Some(100), 300));
        assert_eq!(
            clip.active_events()
                .map(|event| event.tick)
                .collect::<Vec<_>>(),
            [10, 110, 210]
        );
    }

    #[test]
    fn a_trim_hides_events_and_lengthening_reveals_them() {
        let mut arrangement = Arrangement::default();
        arrangement
            .record(
                1,
                Mode::Merge,
                [0, 400],
                vec![vec![note(150, 170, 60)]],
                false,
            )
            .unwrap();
        let id = arrangement.clips(1)[0].id();
        let before = arrangement.clone();
        edit(
            &mut arrangement,
            ClipEdit::Resize {
                clip: id,
                end: 100,
                repeat: false,
            },
        );
        let clip = &arrangement.clips(1)[0];
        assert_eq!(clip.end, 100);
        assert_eq!(clip.takes[0].events, before.clips(1)[0].takes[0].events);
        edit(
            &mut arrangement,
            ClipEdit::Resize {
                clip: id,
                end: 400,
                repeat: false,
            },
        );
        assert_eq!(arrangement, before);
    }

    #[test]
    fn cutting_on_a_loop_boundary_does_not_split_a_note_from_the_previous_cycle() {
        let mut arrangement = Arrangement::default();
        arrangement
            .record(
                1,
                Mode::Merge,
                [0, 100],
                vec![vec![note(20, 140, 60)]],
                false,
            )
            .unwrap();
        let id = arrangement.clips(1)[0].id();
        edit(
            &mut arrangement,
            ClipEdit::Resize {
                clip: id,
                end: 300,
                repeat: true,
            },
        );
        assert!(!arrangement.split_crosses_notes(&[id], 100));
        assert!(matches!(
            edit(
                &mut arrangement,
                ClipEdit::Split {
                    clips: vec![id],
                    tick: 100,
                    confirmed: false
                }
            ),
            ClipEditResult::Applied(_)
        ));
        let right = &arrangement.clips(1)[1];
        assert_eq!(
            right
                .active_events()
                .map(|event| event.tick)
                .collect::<Vec<_>>(),
            [120, 220]
        );
    }

    #[test]
    fn snap_anchors_at_a_meter_change_and_off_keeps_the_pointer_tick() {
        let mut timeline = Timeline::default();
        timeline.changes.push(crate::timeline::Tempo {
            bar: 2,
            beats: 3,
            unit: 8,
            ..timeline.changes[0]
        });
        let transition = timeline.bar_start(2);
        assert_eq!(Snap::Beat.nearest(&timeline, transition - 100), transition);
        assert_eq!(
            Snap::Beat.nearest(&timeline, transition + 350),
            transition + 480
        );
        assert_eq!(
            Snap::Bar.nearest(&timeline, transition + 800),
            transition + 1_440
        );
        assert_eq!(
            Snap::Off.nearest(&timeline, transition + 350),
            transition + 350
        );
    }
}
