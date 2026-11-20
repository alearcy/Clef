use super::{
    Arrangement, Clip, ClipEditResult, MAX_SECTIONS, Section, Take, editing::ClipId, push_section,
};
use crate::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompEdit {
    UseWholeTake,
    SelectRange { start: u64, end: u64 },
    DeleteTake,
    Flatten,
    MoveCut { clip: ClipId, from: u64, to: u64 },
}

impl CompEdit {
    pub fn label(self) -> &'static str {
        match self {
            Self::UseWholeTake => "Use whole take",
            Self::SelectRange { .. } => "Comp take",
            Self::DeleteTake => "Delete Take",
            Self::Flatten => "Flatten Comp",
            Self::MoveCut { .. } => "Move comp cut",
        }
    }
}

impl Arrangement {
    /// Applies a take-lane edit to one track. Lane edits are track-wide except for a cut
    /// drag, which targets one clip. The whole edit is one transaction.
    pub fn comp(
        &mut self,
        track: u64,
        lane: usize,
        edit: CompEdit,
        track_ids: impl Iterator<Item = u64>,
    ) -> Result<ClipEditResult> {
        let ids: Vec<ClipId> = self
            .tracks
            .get(&track)
            .map(|clips| {
                clips
                    .iter()
                    .filter(|clip| lane < clip.takes.len())
                    .map(Clip::id)
                    .collect()
            })
            .unwrap_or_default();
        if ids.is_empty() {
            return Err("The take lane is unavailable.".into());
        }
        let mut candidate = self.clone();
        let mut applied = Vec::new();
        match edit {
            CompEdit::MoveCut { clip, from, to } => {
                let (owner, target) = candidate
                    .find_clip(clip)
                    .ok_or("The MIDI clip was removed.")?;
                if owner != track || lane >= target.takes.len() {
                    return Err("The take lane changed before the edit was applied.".into());
                }
                let index = candidate
                    .tracks
                    .get_mut(&track)
                    .unwrap()
                    .iter_mut()
                    .find(|item| item.id() == clip)
                    .unwrap();
                if index.move_cut(from, to)? {
                    applied.push(clip);
                }
            }
            CompEdit::Flatten => {
                for clip in ids {
                    let id = candidate.make_take(Vec::new())?.id;
                    let index = candidate
                        .tracks
                        .get_mut(&track)
                        .unwrap()
                        .iter_mut()
                        .find(|item| item.id() == clip)
                        .unwrap();
                    if index.flatten_comp(id)? {
                        applied.push(clip);
                    }
                }
            }
            CompEdit::UseWholeTake | CompEdit::SelectRange { .. } | CompEdit::DeleteTake => {
                let clips = candidate.tracks.get_mut(&track).unwrap();
                for clip in clips.iter_mut().filter(|clip| lane < clip.takes.len()) {
                    let id = clip.id();
                    let changed = match edit {
                        CompEdit::UseWholeTake => {
                            let take = clip.takes[lane].id;
                            clip.use_whole_take(take)?
                        }
                        CompEdit::SelectRange { start, end } => {
                            let take = clip.takes[lane].id;
                            clip.select_comp_range(take, start, end)?
                        }
                        CompEdit::DeleteTake => clip.delete_take(lane)?,
                        _ => unreachable!(),
                    };
                    if changed {
                        applied.push(id);
                    }
                }
            }
        }
        if applied.is_empty() {
            return Ok(ClipEditResult::Unchanged);
        }
        candidate.tracks.retain(|_, clips| !clips.is_empty());
        candidate.validate(track_ids)?;
        *self = candidate;
        Ok(ClipEditResult::Applied(applied))
    }
}

impl Arrangement {
    /// Copies the given take index of every clip on a track into new standalone clips,
    /// allocating fresh take identities. The caller owns the new track.
    pub fn copy_take_clips(&mut self, track: u64, lane: usize) -> Result<Vec<Clip>> {
        let sources: Vec<Clip> = self
            .tracks
            .get(&track)
            .map(|clips| {
                clips
                    .iter()
                    .filter(|clip| lane < clip.takes.len())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        if sources.is_empty() {
            return Err("The take lane is unavailable.".into());
        }
        let mut copies = Vec::with_capacity(sources.len());
        for source in sources {
            let take = self.make_take(source.takes[lane].events.clone())?;
            copies.push(Clip {
                start: source.start,
                end: source.end,
                sections: vec![Section {
                    start: source.start,
                    end: source.source_end(),
                    take: take.id,
                }],
                takes: vec![take],
                layer: source.layer,
                cycle: None,
            });
        }
        Ok(copies)
    }
}

impl Clip {
    fn use_whole_take(&mut self, take: u64) -> Result<bool> {
        if !self.takes.iter().any(|item| item.id == take) {
            return Err("The MIDI take was removed.".into());
        }
        let end = self.source_end();
        let mut changed = false;
        if self.sections.len() != 1
            || self.sections[0].start != self.start
            || self.sections[0].end != end
            || self.sections[0].take != take
        {
            self.sections = vec![Section {
                start: self.start,
                end,
                take,
            }];
            changed = true;
        }
        if let Some(cycle) = &mut self.cycle
            && (cycle.sections.len() != 1
                || cycle.sections[0].start != 0
                || cycle.sections[0].end != cycle.period
                || cycle.sections[0].take != take)
        {
            cycle.sections = vec![Section {
                start: 0,
                end: cycle.period,
                take,
            }];
            changed = true;
        }
        Ok(changed)
    }

    fn select_comp_range(&mut self, take: u64, start: u64, end: u64) -> Result<bool> {
        if !self.takes.iter().any(|item| item.id == take) {
            return Err("The MIDI take was removed.".into());
        }
        let start = start.max(self.start);
        let end = end.min(self.end);
        if start >= end {
            return Ok(false);
        }
        let before = self.sections.clone();
        let before_cycle = self.cycle.as_ref().map(|cycle| cycle.sections.clone());
        let mut sections = std::mem::take(&mut self.sections);
        sections = apply_range(sections, start, end, take);
        self.sections = sections;
        if let Some(cycle) = &mut self.cycle {
            let repeat_end = (self.start + cycle.extent).min(self.end);
            if start < repeat_end {
                cycle.sections = apply_cycle_range(
                    std::mem::take(&mut cycle.sections),
                    cycle.period,
                    cycle.phase,
                    self.start,
                    start,
                    end.min(repeat_end),
                    take,
                );
            }
        }
        self.sync_cycle_sections()?;
        let cycle_changed = before_cycle != self.cycle.as_ref().map(|cycle| cycle.sections.clone());
        Ok(self.sections != before || cycle_changed)
    }

    fn delete_take(&mut self, index: usize) -> Result<bool> {
        if index >= self.takes.len() || self.takes.len() < 2 {
            return Ok(false);
        }
        let removed = self.takes[index].id;
        let replacement = self.takes[if index == 0 { 1 } else { index - 1 }].id;
        self.takes.remove(index);
        self.sections = remap_sections(std::mem::take(&mut self.sections), removed, replacement);
        if let Some(cycle) = &mut self.cycle {
            cycle.takes.retain(|take| take.id != removed);
            cycle.sections =
                remap_sections(std::mem::take(&mut cycle.sections), removed, replacement);
            if cycle.takes.len() == 1 {
                cycle.sections = vec![Section {
                    start: 0,
                    end: cycle.period,
                    take: replacement,
                }];
            }
        }
        if self.takes.len() == 1 {
            let end = self.source_end();
            self.sections = vec![Section {
                start: self.start,
                end,
                take: replacement,
            }];
        }
        Ok(true)
    }

    fn flatten_comp(&mut self, id: u64) -> Result<bool> {
        if self.takes.len() < 2 {
            return Ok(false);
        }
        let mut events: Vec<_> = self.active_events().cloned().collect();
        events.sort_by_key(|event| (event.tick, event.order));
        let end = self.source_end();
        self.takes = vec![Take { id, events }];
        self.sections = vec![Section {
            start: self.start,
            end,
            take: id,
        }];
        if let Some(cycle) = &mut self.cycle {
            let sections = cycle.sections.as_slice();
            let mut source: Vec<_> = cycle
                .takes
                .iter()
                .flat_map(|take| {
                    take.events.iter().filter(move |event| {
                        sections
                            .get(sections.partition_point(|section| section.end <= event.tick))
                            .is_some_and(|section| section.take == take.id)
                    })
                })
                .cloned()
                .collect();
            source.sort_by_key(|event| (event.tick, event.order));
            cycle.takes = vec![Take { id, events: source }];
            cycle.sections = vec![Section {
                start: 0,
                end: cycle.period,
                take: id,
            }];
        }
        Ok(true)
    }

    fn move_cut(&mut self, from: u64, to: u64) -> Result<bool> {
        let source_end = self.source_end();
        let index = self
            .sections
            .iter()
            .position(|section| section.start == from)
            .filter(|index| *index > 0)
            .ok_or("The comp cut was removed.")?;
        let lower = if index >= 2 {
            self.sections[index - 2].end
        } else {
            self.start
        };
        let upper = self
            .sections
            .get(index + 1)
            .map_or(source_end, |section| section.start);
        if lower >= upper || upper - lower < 2 {
            return Ok(false);
        }
        let to = to.clamp(lower + 1, upper - 1);
        if to == from {
            return Ok(false);
        }
        self.sections[index - 1].end = to;
        self.sections[index].start = to;
        if let Some(cycle) = &self.cycle {
            let repeat_end = self.start + cycle.extent;
            if from > self.start && from < repeat_end {
                let local = (from - self.start + cycle.phase) % cycle.period;
                if let Some(boundary) = cycle
                    .sections
                    .iter()
                    .position(|section| section.start == local)
                    .filter(|index| *index > 0)
                {
                    let local_lower = if boundary >= 2 {
                        cycle.sections[boundary - 2].end
                    } else {
                        0
                    };
                    let local_upper = cycle
                        .sections
                        .get(boundary + 1)
                        .map_or(cycle.period, |section| section.start);
                    if local_lower < local_upper.saturating_sub(1) {
                        let delta = to as i64 - from as i64;
                        let local_to = (local as i64 + delta)
                            .rem_euclid(cycle.period as i64)
                            .min(local_upper as i64 - 1)
                            .max(local_lower as i64 + 1)
                            as u64;
                        if local_to != local {
                            let cycle = self.cycle.as_mut().unwrap();
                            cycle.sections[boundary - 1].end = local_to;
                            cycle.sections[boundary].start = local_to;
                        }
                    }
                    self.sync_cycle_sections()?;
                }
            }
        }
        Ok(true)
    }

    fn sync_cycle_sections(&mut self) -> Result<bool> {
        let Some(cycle) = &self.cycle else {
            return Ok(false);
        };
        let start = self.start;
        let repeat_end = start + cycle.extent;
        let mut tail = Vec::new();
        for section in &self.sections {
            if section.end > repeat_end {
                tail.push(Section {
                    start: section.start.max(repeat_end),
                    end: section.end,
                    take: section.take,
                });
            }
        }
        let mut sections = Vec::new();
        let mut cursor = start;
        let mut phase = cycle.phase;
        while cursor < repeat_end {
            let span = (cycle.period - phase).min(repeat_end - cursor);
            let until = phase + span;
            for section in cycle
                .sections
                .iter()
                .take_while(|section| section.start < until)
                .filter(|section| section.end > phase)
            {
                push_section(
                    &mut sections,
                    cursor + (section.start.max(phase) - phase),
                    cursor + (section.end.min(until) - phase),
                    section.take,
                );
            }
            if sections.len() > MAX_SECTIONS {
                return Err("The comp exceeds the MIDI section limit.".into());
            }
            cursor += span;
            phase = 0;
        }
        sections.extend(tail);
        if sections != self.sections {
            self.sections = sections;
        }
        Ok(true)
    }
}

fn apply_range(mut sections: Vec<Section>, start: u64, end: u64, take: u64) -> Vec<Section> {
    let mut result = Vec::with_capacity(sections.len() + 2);
    for section in sections.drain(..) {
        if section.end <= start || section.start >= end {
            push_section(&mut result, section.start, section.end, section.take);
            continue;
        }
        if section.start < start {
            push_section(&mut result, section.start, start, section.take);
        }
        push_section(
            &mut result,
            section.start.max(start),
            section.end.min(end),
            take,
        );
        if section.end > end {
            push_section(&mut result, end, section.end, section.take);
        }
    }
    result
}

fn apply_cycle_range(
    mut sections: Vec<Section>,
    period: u64,
    phase: u64,
    clip_start: u64,
    start: u64,
    end: u64,
    take: u64,
) -> Vec<Section> {
    let mut cursor = start;
    while cursor < end {
        let local = (cursor - clip_start + phase) % period;
        let span = (period - local).min(end - cursor);
        sections = apply_range(sections, local, local + span, take);
        cursor += span;
    }
    sections
}

fn remap_sections(mut sections: Vec<Section>, from: u64, to: u64) -> Vec<Section> {
    let mut result = Vec::with_capacity(sections.len());
    for section in sections.drain(..) {
        let take = if section.take == from {
            to
        } else {
            section.take
        };
        push_section(&mut result, section.start, section.end, take);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi::{Event, Message, Mode};

    fn note(tick: u64, end: u64, key: u8) -> Event {
        Event {
            tick,
            order: key.into(),
            message: Message::Note {
                channel: 0,
                key,
                velocity: 90,
                end,
                release: 12,
                release_order: u32::from(key) + 1,
            },
        }
    }

    fn take_arrangement(passes: usize) -> Arrangement {
        let mut arrangement = Arrangement::default();
        arrangement
            .record(
                1,
                Mode::NewTake,
                [0, 960],
                (0..passes)
                    .map(|index| {
                        vec![note(
                            100 + index as u64 * 300,
                            200 + index as u64 * 300,
                            60 + index as u8,
                        )]
                    })
                    .collect(),
                false,
            )
            .unwrap();
        arrangement
    }

    fn attacks(arrangement: &Arrangement) -> Vec<u64> {
        let mut ticks: Vec<_> = arrangement.clips(1)[0]
            .active_events()
            .map(|event| event.tick)
            .collect();
        ticks.sort_unstable();
        ticks
    }

    #[test]
    fn comp_selection_follows_the_attack_rule_across_takes() {
        let mut arrangement = take_arrangement(2);
        let result = arrangement
            .comp(1, 0, CompEdit::UseWholeTake, [1].into_iter())
            .unwrap();
        assert!(matches!(result, ClipEditResult::Applied(_)));
        assert_eq!(attacks(&arrangement), vec![100]);
        let result = arrangement
            .comp(
                1,
                1,
                CompEdit::SelectRange {
                    start: 300,
                    end: 960,
                },
                [1].into_iter(),
            )
            .unwrap();
        assert!(matches!(result, ClipEditResult::Applied(_)));
        assert_eq!(attacks(&arrangement), vec![100, 400]);
        let result = arrangement
            .comp(
                1,
                1,
                CompEdit::SelectRange {
                    start: 300,
                    end: 960,
                },
                [1].into_iter(),
            )
            .unwrap();
        assert!(matches!(result, ClipEditResult::Unchanged));
        let clip = &arrangement.clips(1)[0];
        assert_eq!(clip.sections.len(), 2);
        assert_eq!(clip.sections[0].start, 0);
        assert_eq!(clip.sections[0].end, 300);
        assert_eq!(clip.sections[1].take, clip.takes[1].id);
    }

    #[test]
    fn comp_selection_across_clips_is_one_edit() {
        let mut arrangement = take_arrangement(2);
        arrangement
            .record(
                2,
                Mode::NewTake,
                [1000, 2000],
                vec![vec![note(1100, 1200, 64)], vec![note(1500, 1600, 65)]],
                false,
            )
            .unwrap();
        let result = arrangement
            .comp(
                1,
                0,
                CompEdit::SelectRange {
                    start: 0,
                    end: 3000,
                },
                [1, 2].into_iter(),
            )
            .unwrap();
        assert!(matches!(result, ClipEditResult::Applied(ref ids) if ids.len() == 1));
        assert_eq!(attacks(&arrangement), vec![100]);
    }

    #[test]
    fn deleting_a_take_passes_sections_to_the_previous_or_next_take() {
        let mut arrangement = take_arrangement(3);
        let ids: Vec<_> = arrangement.clips(1)[0]
            .takes
            .iter()
            .map(|take| take.id)
            .collect();
        arrangement
            .comp(
                1,
                1,
                CompEdit::SelectRange {
                    start: 400,
                    end: 960,
                },
                [1].into_iter(),
            )
            .unwrap();
        arrangement
            .comp(1, 1, CompEdit::DeleteTake, [1].into_iter())
            .unwrap();
        let clip = &arrangement.clips(1)[0];
        assert_eq!(clip.takes.len(), 2);
        assert!(clip.sections.iter().all(|section| section.take != ids[1]));
        assert!(clip.sections.iter().any(|section| section.take == ids[0]));
        arrangement
            .comp(1, 0, CompEdit::DeleteTake, [1].into_iter())
            .unwrap();
        let clip = &arrangement.clips(1)[0];
        assert_eq!(clip.takes.len(), 1);
        assert_eq!(clip.sections.len(), 1);
        assert_eq!(clip.sections[0].take, clip.takes[0].id);
    }

    #[test]
    fn flatten_keeps_the_comp_sequence_and_removes_the_other_takes() {
        let mut arrangement = take_arrangement(2);
        arrangement
            .comp(
                1,
                0,
                CompEdit::SelectRange { start: 0, end: 300 },
                [1].into_iter(),
            )
            .unwrap();
        arrangement
            .comp(
                1,
                1,
                CompEdit::SelectRange {
                    start: 300,
                    end: 960,
                },
                [1].into_iter(),
            )
            .unwrap();
        arrangement
            .comp(1, 0, CompEdit::Flatten, [1].into_iter())
            .unwrap();
        let clip = &arrangement.clips(1)[0];
        assert_eq!(clip.takes.len(), 1);
        assert_eq!(clip.takes[0].events.len(), 2);
        assert_eq!(clip.sections.len(), 1);
        assert_eq!(attacks(&arrangement), vec![100, 400]);
    }

    #[test]
    fn moving_a_cut_stops_at_the_neighbouring_cuts() {
        let mut arrangement = take_arrangement(2);
        arrangement
            .comp(
                1,
                1,
                CompEdit::SelectRange {
                    start: 300,
                    end: 600,
                },
                [1].into_iter(),
            )
            .unwrap();
        arrangement
            .comp(1, 0, CompEdit::UseWholeTake, [1].into_iter())
            .unwrap();
        arrangement
            .comp(
                1,
                1,
                CompEdit::SelectRange {
                    start: 300,
                    end: 600,
                },
                [1].into_iter(),
            )
            .unwrap();
        let cut = arrangement.clips(1)[0].sections[1].start;
        assert_eq!(cut, 300);
        arrangement
            .comp(
                1,
                0,
                CompEdit::MoveCut {
                    clip: arrangement.clips(1)[0].id(),
                    from: cut,
                    to: 900,
                },
                [1].into_iter(),
            )
            .unwrap();
        let sections = &arrangement.clips(1)[0].sections;
        assert_eq!(sections[0].end, 599);
        assert_eq!(sections[1].start, 599);
    }

    #[test]
    fn take_to_new_track_copy_keeps_events_and_allocates_an_identity() {
        let mut arrangement = take_arrangement(2);
        let source = arrangement.clips(1)[0].clone();
        let copies = arrangement.copy_take_clips(1, 1).unwrap();
        assert_eq!(copies.len(), 1);
        assert_eq!(copies[0].start, source.start);
        assert_eq!(copies[0].end, source.end);
        assert_eq!(copies[0].takes.len(), 1);
        assert_eq!(copies[0].takes[0].events, source.takes[1].events,);
        assert_ne!(copies[0].takes[0].id, source.takes[1].id);
    }

    #[test]
    fn comp_edit_updates_the_repeat_source() {
        let mut arrangement = take_arrangement(2);
        let id = arrangement.clips(1)[0].id();
        arrangement
            .edit(
                &crate::midi::ClipEdit::Resize {
                    clip: id,
                    end: 1920,
                    repeat: true,
                },
                [1].into_iter(),
            )
            .unwrap();
        let clip = &arrangement.clips(1)[0];
        assert_eq!(clip.cycle().unwrap(), 960);
        arrangement
            .comp(1, 0, CompEdit::UseWholeTake, [1].into_iter())
            .unwrap();
        let clip = &arrangement.clips(1)[0];
        let cycle = clip.cycle.as_ref().unwrap();
        assert_eq!(cycle.sections.len(), 1);
        assert_eq!(cycle.sections[0].take, clip.takes[0].id);
        assert!(
            clip.sections
                .iter()
                .all(|section| section.take == clip.takes[0].id)
        );
    }
}
