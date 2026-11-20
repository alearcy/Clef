use super::{Clip, Event, MAX_EVENTS, Message, Section};
use crate::Result;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId {
    take: u64,
    index: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct RollEvent {
    pub id: EventId,
    pub source: Event,
    pub tick: u64,
    pub end: u64,
    /// True when the take's section plays this event in the comp.
    pub active: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum EventChange {
    Insert(Event),
    Replace(EventId, Event),
    Remove(EventId),
}

impl Clip {
    pub fn roll_source_bounds(&self) -> [u64; 2] {
        self.cycle
            .as_ref()
            .map_or([self.start, self.end], |cycle| [0, cycle.period])
    }

    pub fn roll_source_tick(&self, tick: u64) -> Option<u64> {
        if !(self.start..self.end).contains(&tick) {
            return None;
        }
        self.cycle.as_ref().map_or(Some(tick), |cycle| {
            (tick - self.start < cycle.extent)
                .then(|| (cycle.phase + tick - self.start) % cycle.period)
        })
    }

    pub fn roll_events(&self, visible: [u64; 2]) -> Vec<RollEvent> {
        self.roll_events_for(None, visible)
    }

    /// Lists every event of one take, marking the ones the comp plays.
    pub fn roll_take_events(&self, take: u64, visible: [u64; 2]) -> Vec<RollEvent> {
        self.roll_events_for(Some(take), visible)
    }

    /// Project-time ranges where the comp plays the given take.
    pub fn take_active_ranges(&self, take: u64) -> Vec<[u64; 2]> {
        self.sections
            .iter()
            .filter(|section| section.take == take)
            .map(|section| [section.start, section.end])
            .collect()
    }

    fn roll_events_for(&self, only: Option<u64>, visible: [u64; 2]) -> Vec<RollEvent> {
        let (takes, sections) = self
            .cycle
            .as_ref()
            .map_or((self.takes.as_slice(), self.sections.as_slice()), |cycle| {
                (cycle.takes.as_slice(), cycle.sections.as_slice())
            });
        let mut events = Vec::new();
        for take in takes {
            if only.is_some_and(|id| id != take.id) {
                continue;
            }
            for (index, &source) in take.events.iter().enumerate() {
                let active = section_take(sections, source.tick) == Some(take.id);
                if only.is_none() && !active {
                    continue;
                }
                let id = EventId {
                    take: take.id,
                    index,
                };
                self.project_event(source, visible, |event| {
                    let end = match event.message {
                        Message::Note { end, .. } => end,
                        _ => event.tick + 1,
                    };
                    events.push(RollEvent {
                        id,
                        source,
                        tick: event.tick,
                        end,
                        active,
                    });
                    events.len() < MAX_EVENTS
                });
                if events.len() == MAX_EVENTS {
                    return events;
                }
            }
        }
        events.sort_by_key(|event| (event.tick, event.source.order));
        events
    }

    fn project_event(
        &self,
        mut event: Event,
        visible: [u64; 2],
        mut emit: impl FnMut(Event) -> bool,
    ) {
        let Some(cycle) = self.cycle.as_ref() else {
            let end = match event.message {
                Message::Note { end, .. } => end.min(self.end),
                _ => event.tick + 1,
            };
            if event.tick < visible[1].min(self.end) && end > visible[0] {
                if let Message::Note { end: note_end, .. } = &mut event.message {
                    *note_end = end;
                }
                emit(event);
            }
            return;
        };
        let left = self.start as i64;
        let right = self.start + cycle.extent;
        let end = match event.message {
            Message::Note { end, .. } => end.min(cycle.period),
            _ => event.tick + 1,
        };
        let window_start = visible[0].max(self.start) as i64;
        let window_end = visible[1].min(right);
        let period = cycle.period as i64;
        let first_base = left - cycle.phase as i64;
        let mut base = first_base
            + ((window_start - first_base - end as i64).div_euclid(period) + 1).max(0) * period;
        while base + (event.tick as i64) < window_end as i64 {
            let attack = base + event.tick as i64;
            if attack >= left || matches!(event.message, Message::Note { .. }) {
                let mut projected = Event {
                    tick: attack.max(left) as u64,
                    ..event
                };
                if let Message::Note { end: note_end, .. } = &mut projected.message {
                    *note_end = ((base + end as i64) as u64).min(right);
                }
                if base + end as i64 > window_start
                    && projected.tick < window_end
                    && !emit(projected)
                {
                    return;
                }
            }
            base += period;
        }
    }

    pub(super) fn edit_events(
        &mut self,
        changes: &[EventChange],
        only_take: Option<u64>,
    ) -> Result<()> {
        let [start, end] = self.roll_source_bounds();
        let (takes, sections) = self
            .cycle
            .as_ref()
            .map_or((self.takes.as_slice(), self.sections.as_slice()), |cycle| {
                (cycle.takes.as_slice(), cycle.sections.as_slice())
            });
        let mut next_order = takes
            .iter()
            .flat_map(|take| &take.events)
            .map(|event| match event.message {
                Message::Note { release_order, .. } => event.order.max(release_order),
                _ => event.order,
            })
            .max()
            .unwrap_or(0);
        let mut affected = HashSet::new();
        let mut before = Vec::new();
        let mut after = Vec::new();
        for change in changes {
            let (id, replacement) = match *change {
                EventChange::Insert(event) => (None, Some(event)),
                EventChange::Replace(id, event) => (Some(id), Some(event)),
                EventChange::Remove(id) => (Some(id), None),
            };
            if only_take.is_some_and(|take| id.is_some_and(|id| id.take != take)) {
                return Err("An edit targets a take outside the edited take.".into());
            }
            let previous = if let Some(id) = id {
                let event = takes
                    .iter()
                    .find(|take| take.id == id.take)
                    .and_then(|take| take.events.get(id.index))
                    .filter(|event| {
                        (start..end).contains(&event.tick)
                            && (only_take.is_some()
                                || section_take(sections, event.tick) == Some(id.take))
                    })
                    .ok_or("The MIDI event is unavailable.")?;
                if !affected.insert(id) {
                    return Err("An edit targets the same MIDI event twice.".into());
                }
                Some(*event)
            } else {
                None
            };
            if let Some(mut event) = replacement {
                event.tick = event.tick.clamp(start, end - 1);
                let position_edited = previous.is_none_or(|old| {
                    old.tick != event.tick
                        || match (old.message, event.message) {
                            (Message::Note { end: old, .. }, Message::Note { end: new, .. }) => {
                                old != new
                            }
                            _ => true,
                        }
                });
                if previous.is_some_and(|old| {
                    old.message.control().is_some() != event.message.control().is_some()
                }) {
                    return Err("An edit cannot change the MIDI event type.".into());
                }
                if let Message::Note {
                    end: note_end,
                    release_order,
                    ..
                } = &mut event.message
                {
                    if position_edited {
                        *note_end = (*note_end).clamp(event.tick + 1, end);
                    }
                    if let Some(Event {
                        message:
                            Message::Note {
                                release_order: old, ..
                            },
                        ..
                    }) = previous
                    {
                        *release_order = old;
                    } else {
                        next_order = next_order
                            .checked_add(1)
                            .ok_or("MIDI event order exhausted.")?;
                        *release_order = next_order;
                    }
                }
                if let Some(previous) = previous {
                    event.order = previous.order;
                    if event == previous {
                        affected.remove(&id.unwrap());
                        continue;
                    }
                } else {
                    next_order = next_order
                        .checked_add(1)
                        .ok_or("MIDI event order exhausted.")?;
                    event.order = next_order;
                }
                let target = only_take
                    .or_else(|| section_take(sections, event.tick))
                    .ok_or("The MIDI edit has no target take.")?;
                after.push((target, event));
            }
            if let (Some(id), Some(previous)) = (id, previous) {
                before.push((id.take, previous));
            }
        }
        if let Some(cycle) = &self.cycle {
            let range = [self.start, self.source_end()];
            let mut removed = HashMap::new();
            let mut removed_count = 0;
            let mut added = Vec::new();
            let section_at_start = self.sections[0].take;
            let original_at_start = section_take(&cycle.sections, cycle.phase).unwrap();
            let target = |take, tick| {
                if tick == self.start && take == original_at_start {
                    section_at_start
                } else {
                    take
                }
            };
            for (take, event) in before {
                self.project_event(event, range, |projected| {
                    *removed
                        .entry((target(take, projected.tick), projected))
                        .or_insert(0) += 1;
                    removed_count += 1;
                    removed_count <= MAX_EVENTS
                });
                if removed_count > MAX_EVENTS {
                    return Err("Clip repetition exceeds the MIDI event limit.".into());
                }
            }
            for &(take, event) in &after {
                self.project_event(event, range, |projected| {
                    added.push((target(take, projected.tick), projected));
                    added.len() <= MAX_EVENTS
                });
                if added.len() > MAX_EVENTS {
                    return Err("Clip repetition exceeds the MIDI event limit.".into());
                }
            }
            let start = self.start;
            let repeat_end = start + cycle.extent;
            for take in &mut self.takes {
                take.events.retain(|event| {
                    if event.tick >= repeat_end {
                        return true;
                    }
                    let mut shown = *event;
                    if let Message::Note { end, .. } = &mut shown.message {
                        let phase = (cycle.phase + shown.tick - start) % cycle.period;
                        *end = (*end).min(repeat_end.min(shown.tick + cycle.period - phase));
                    }
                    let Some(count) = removed
                        .get_mut(&(take.id, shown))
                        .filter(|count| **count != 0)
                    else {
                        return true;
                    };
                    *count -= 1;
                    false
                });
                take.events.extend(
                    added
                        .iter()
                        .filter_map(|&(id, event)| (id == take.id).then_some(event)),
                );
            }
        }
        let takes = self
            .cycle
            .as_mut()
            .map_or(&mut self.takes, |cycle| &mut cycle.takes);
        for take in takes {
            let mut index = 0;
            take.events.retain(|_| {
                let keep = !affected.contains(&EventId {
                    take: take.id,
                    index,
                });
                index += 1;
                keep
            });
            take.events.extend(
                after
                    .iter()
                    .filter_map(|&(id, event)| (id == take.id).then_some(event)),
            );
            take.events.sort_by_key(|event| (event.tick, event.order));
        }
        Ok(())
    }
}

fn section_take(sections: &[Section], tick: u64) -> Option<u64> {
    sections
        .get(sections.partition_point(|section| section.end <= tick))
        .map(|section| section.take)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi::{Arrangement, ClipEdit, ClipEditResult, Mode};

    fn note(tick: u64, end: u64, key: u8) -> Event {
        Event {
            tick,
            order: 1,
            message: Message::Note {
                channel: 2,
                key,
                velocity: 92,
                end,
                release: 5,
                release_order: 2,
            },
        }
    }

    fn edit(arrangement: &mut Arrangement, edit: ClipEdit) -> Vec<super::super::ClipId> {
        let ClipEditResult::Applied(ids) = arrangement.edit(&edit, [1].into_iter()).unwrap() else {
            panic!("expected an applied edit");
        };
        ids
    }

    #[test]
    fn edits_repeat_sources_and_preserves_the_split_siblings() {
        let mut model = Arrangement::default();
        model
            .record(
                1,
                Mode::Merge,
                [100, 200],
                vec![vec![note(110, 170, 60)]],
                false,
            )
            .unwrap();
        let id = model.clips(1)[0].id();
        edit(
            &mut model,
            ClipEdit::Resize {
                clip: id,
                end: 400,
                repeat: true,
            },
        );
        let ids = edit(
            &mut model,
            ClipEdit::Split {
                clips: vec![id],
                tick: 240,
                confirmed: true,
            },
        );
        let right = model.find_clip(ids[1]).unwrap().1;
        let source = right.roll_events([240, 400]);
        assert_eq!(
            source
                .iter()
                .map(|event| (event.tick, event.end))
                .collect::<Vec<_>>(),
            [(240, 270), (310, 370)]
        );
        assert_eq!(source[0].id, source[1].id);
        edit(
            &mut model,
            ClipEdit::EditEvents {
                take: None,
                clip: ids[1],
                label: "Transpose",
                changes: vec![EventChange::Replace(source[0].id, note(10, 70, 67))],
            },
        );
        edit(
            &mut model,
            ClipEdit::Resize {
                clip: ids[1],
                end: 500,
                repeat: true,
            },
        );
        let mapped = |clip: &Clip| {
            let mut notes = clip
                .active_events()
                .filter_map(|event| match event.message {
                    Message::Note { key, .. } => Some((event.tick, key)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            notes.sort_unstable();
            notes
        };
        assert_eq!(
            mapped(model.find_clip(ids[0]).unwrap().1),
            [(110, 60), (210, 60)]
        );
        assert_eq!(
            mapped(model.find_clip(ids[1]).unwrap().1),
            [(240, 67), (310, 67), (410, 67)]
        );
    }

    #[test]
    fn velocity_edit_keeps_a_trimmed_note_tail_and_rejects_invalid_batches() {
        let mut model = Arrangement::default();
        model
            .record(
                1,
                Mode::Merge,
                [0, 160],
                vec![vec![note(16, 144, 60)]],
                false,
            )
            .unwrap();
        let id = model.clips(1)[0].id();
        edit(
            &mut model,
            ClipEdit::Resize {
                clip: id,
                end: 96,
                repeat: false,
            },
        );
        let selected = model.clips(1)[0].roll_events([0, 96])[0];
        assert_eq!(selected.end, 96);
        let mut updated = selected.source;
        if let Message::Note { velocity, .. } = &mut updated.message {
            *velocity = 101;
        }
        edit(
            &mut model,
            ClipEdit::EditEvents {
                take: None,
                clip: id,
                label: "Change Velocity",
                changes: vec![EventChange::Replace(selected.id, updated)],
            },
        );
        edit(
            &mut model,
            ClipEdit::Resize {
                clip: id,
                end: 160,
                repeat: false,
            },
        );
        assert_eq!(model.clips(1)[0].roll_events([0, 160])[0].source, updated);
        let before = model.clone();
        assert!(
            model
                .edit(
                    &ClipEdit::EditEvents {
                        take: None,
                        clip: id,
                        label: "Invalid",
                        changes: vec![EventChange::Insert(Event {
                            tick: 50,
                            order: 0,
                            message: Message::CurvedControl {
                                bytes: [0xb0, 64, 127],
                                bend: 5
                            },
                        })],
                    },
                    [1].into_iter()
                )
                .is_err()
        );
        assert_eq!(model, before);
    }
    #[test]
    fn take_view_lists_every_event_and_marks_the_comp() {
        let mut model = Arrangement::default();
        model
            .record(
                1,
                Mode::NewTake,
                [0, 960],
                vec![vec![note(100, 200, 60)], vec![note(500, 700, 62)]],
                false,
            )
            .unwrap();
        let clip = &model.clips(1)[0];
        let first = clip.takes[0].id;
        let second = clip.takes[1].id;
        let comp = clip.roll_events([0, 960]);
        assert_eq!(comp.len(), 1);
        assert_eq!(comp[0].source.message, note(500, 700, 62).message);
        let take = clip.roll_take_events(first, [0, 960]);
        assert_eq!(take.len(), 1);
        assert!(!take[0].active);
        let take = clip.roll_take_events(second, [0, 960]);
        assert_eq!(take.len(), 1);
        assert!(take[0].active);
        assert_eq!(clip.take_active_ranges(second), vec![[0, 960]]);
    }

    #[test]
    fn editing_an_inactive_take_keeps_the_comp_unchanged() {
        let mut model = Arrangement::default();
        model
            .record(
                1,
                Mode::NewTake,
                [0, 960],
                vec![vec![note(100, 200, 60)], vec![note(500, 700, 62)]],
                false,
            )
            .unwrap();
        let clip = model.clips(1)[0].clone();
        let first = clip.takes[0].id;
        let inactive = clip
            .roll_take_events(first, [0, 960])
            .into_iter()
            .next()
            .unwrap();
        let mut updated = inactive.source;
        updated.tick = 600;
        if let Message::Note { end, .. } = &mut updated.message {
            *end = 800;
        }
        let sections = clip.sections.clone();
        edit(
            &mut model,
            ClipEdit::EditEvents {
                take: Some(first),
                clip: clip.id(),
                label: "Move note",
                changes: vec![EventChange::Replace(inactive.id, updated)],
            },
        );
        let updated_clip = &model.clips(1)[0];
        assert_eq!(updated_clip.sections, sections);
        assert!(matches!(
            updated_clip.roll_events([0, 960]).as_slice(),
            [event] if event.source.message == note(500, 700, 62).message
        ));
        assert_eq!(
            updated_clip.roll_take_events(first, [0, 960])[0]
                .source
                .message,
            updated.message
        );
    }
}
