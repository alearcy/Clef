use clef_model::{
    Plugin, Result,
    midi::{Clip, Event, Mode, Section, Take},
    project::{Project, Track, TrackMode},
    timeline::Tempo,
};
use std::collections::VecDeque;

const MAX_ENTRIES: usize = 100;
const MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HistoryView {
    pub labels: Vec<&'static str>,
    pub cursor: usize,
    pub version: u64,
    pub available: bool,
}

#[derive(Default)]
pub(super) struct History {
    entries: VecDeque<(u64, Edit, usize)>,
    cursor: usize,
    base: u64,
    version: u64,
    next_revision: u64,
    saved: u64,
    bytes: usize,
}

impl History {
    fn revision(&self) -> u64 {
        if self.cursor == 0 {
            self.base
        } else {
            self.entries[self.cursor - 1].0
        }
    }

    pub(super) fn reset(&mut self) {
        let version = self.version + 1;
        *self = Self {
            version,
            ..Self::default()
        };
    }

    pub(super) fn save(&mut self) {
        self.saved = self.revision();
    }

    pub(super) fn dirty(&self) -> bool {
        self.saved != self.revision()
    }

    pub(super) fn record(&mut self, edit: Edit) -> bool {
        let size = edit.bytes();
        self.version += 1;
        self.next_revision += 1;
        if size > MAX_BYTES {
            self.entries.clear();
            self.cursor = 0;
            self.bytes = 0;
            self.base = self.next_revision;
            return false;
        }
        while self.entries.len() > self.cursor {
            self.bytes -= self.entries.pop_back().expect("redo entry").2;
        }
        self.entries.push_back((self.next_revision, edit, size));
        self.bytes += size;
        self.cursor += 1;
        while self.entries.len() > MAX_ENTRIES || self.bytes > MAX_BYTES {
            let (revision, _, bytes) = self.entries.pop_front().expect("eviction entry");
            self.base = revision;
            self.bytes -= bytes;
            self.cursor -= 1;
        }
        true
    }

    pub(super) fn view(&self, available: bool) -> HistoryView {
        HistoryView {
            labels: self
                .entries
                .iter()
                .map(|(_, edit, _)| edit.label())
                .collect(),
            cursor: self.cursor,
            version: self.version,
            available,
        }
    }

    pub(super) fn target(&self, version: u64, target: usize) -> Result<()> {
        if version != self.version || target > self.entries.len() {
            return Err("Edit history changed. Choose a revision again.".into());
        }
        Ok(())
    }

    pub(super) fn cursor(&self) -> usize {
        self.cursor
    }

    pub(super) fn step(&self, cursor: usize, target: usize) -> Option<(&Edit, bool)> {
        match target.cmp(&cursor) {
            std::cmp::Ordering::Less => Some((&self.entries[cursor - 1].1, false)),
            std::cmp::Ordering::Greater => Some((&self.entries[cursor].1, true)),
            std::cmp::Ordering::Equal => None,
        }
    }

    pub(super) fn seek(&mut self, target: usize) {
        self.cursor = target;
        self.version += 1;
    }
}

pub(super) struct TrackSlot {
    pub track: Track,
    pub index: usize,
    pub clips: Vec<Clip>,
}

pub(super) struct InstrumentState {
    pub plugin: Option<Plugin>,
    pub state: Vec<u8>,
}

impl From<&Track> for InstrumentState {
    fn from(track: &Track) -> Self {
        Self {
            plugin: track.plugin.clone(),
            state: track.state.clone(),
        }
    }
}

pub(super) enum Edit {
    Inserts {
        target: clef_model::mixer::Target,
        before: Vec<clef_model::project::Insert>,
        after: Vec<clef_model::project::Insert>,
        label: &'static str,
    },
    Mix {
        target: clef_model::mixer::Target,
        control: clef_model::mixer::Control,
        before: i16,
        after: i16,
    },
    Track {
        slot: Box<TrackSlot>,
        added: bool,
    },
    InstrumentBypass {
        id: u64,
        before: bool,
        after: bool,
    },
    InstrumentOffline {
        id: u64,
        before: bool,
        after: bool,
    },
    TakeToNewTrack {
        slot: Box<TrackSlot>,
    },
    RecordMode {
        before: Mode,
        after: Mode,
    },
    TrackMode {
        id: u64,
        before: TrackMode,
        after: TrackMode,
    },
    Move {
        id: u64,
        before: usize,
        after: usize,
    },
    Rename {
        id: u64,
        before: String,
        after: String,
    },
    Instrument {
        id: u64,
        before: InstrumentState,
        after: InstrumentState,
        editor: bool,
    },
    Midi {
        id: u64,
        before: Vec<Clip>,
        after: Vec<Clip>,
    },
    Clips {
        label: &'static str,
        tracks: Vec<ClipChange>,
    },
    Tempo {
        before: Vec<Tempo>,
        after: Vec<Tempo>,
    },
    Loop {
        before: [u64; 2],
        after: [u64; 2],
    },
}

pub(super) struct ClipChange {
    pub id: u64,
    pub before: Vec<Clip>,
    pub after: Vec<Clip>,
}

fn plugin_bytes(plugin: Option<&Plugin>) -> usize {
    plugin.map_or(0, |plugin| {
        plugin.path.capacity()
            + plugin.id.capacity()
            + plugin.name.capacity()
            + plugin.format.capacity()
    })
}

fn clip_bytes(clips: &Vec<Clip>) -> usize {
    clips.capacity() * size_of::<Clip>()
        + clips
            .iter()
            .map(|clip| {
                clip.sections.capacity() * size_of::<Section>()
                    + clip.repeat_source_bytes()
                    + clip.takes.capacity() * size_of::<Take>()
                    + clip
                        .takes
                        .iter()
                        .map(|take| take.events.capacity() * size_of::<Event>())
                        .sum::<usize>()
            })
            .sum::<usize>()
}

fn insert_bytes(slots: &[clef_model::project::Insert]) -> usize {
    slots
        .iter()
        .map(|slot| {
            size_of::<clef_model::project::Insert>()
                + slot.state.len()
                + plugin_bytes(Some(&slot.plugin))
        })
        .sum()
}

impl Edit {
    fn label(&self) -> &'static str {
        match self {
            Self::Inserts { label, .. } => label,
            Self::Track { added: true, .. } => "Add Track",
            Self::Track { added: false, .. } => "Remove Track",
            Self::InstrumentBypass { .. } => "Toggle Instrument Bypass",
            Self::InstrumentOffline { .. } => "Toggle Instrument Offline",
            Self::TakeToNewTrack { .. } => "Take to New Track",
            Self::Mix { control, .. } => control.label(),
            Self::RecordMode { .. } => "Change Record Mode",
            Self::TrackMode { .. } => "Change Track Record Mode",
            Self::Move { .. } => "Move Track",
            Self::Rename { .. } => "Rename Track",
            Self::Instrument { editor: true, .. } => "Edit Instrument State",
            Self::Instrument { before, .. } if before.plugin.is_none() => "Assign Instrument",
            Self::Instrument { .. } => "Replace Instrument",
            Self::Midi { .. } => "Record MIDI",
            Self::Clips { label, .. } => label,
            Self::Tempo { .. } => "Edit Tempo and Meter",
            Self::Loop { .. } => "Edit Loop Range",
        }
    }

    fn bytes(&self) -> usize {
        size_of::<(u64, Self, usize)>()
            + match self {
                Self::Inserts { before, after, .. } => insert_bytes(before) + insert_bytes(after),
                Self::Track { slot, .. } | Self::TakeToNewTrack { slot } => {
                    size_of::<TrackSlot>()
                        + slot.track.name.capacity()
                        + slot.track.state.capacity()
                        + plugin_bytes(slot.track.plugin.as_ref())
                        + insert_bytes(&slot.track.inserts)
                        + clip_bytes(&slot.clips)
                }
                Self::Rename { before, after, .. } => before.capacity() + after.capacity(),
                Self::Instrument { before, after, .. } => {
                    before.state.capacity()
                        + after.state.capacity()
                        + plugin_bytes(before.plugin.as_ref())
                        + plugin_bytes(after.plugin.as_ref())
                }
                Self::Midi { before, after, .. } => clip_bytes(before) + clip_bytes(after),
                Self::Clips { tracks, .. } => {
                    tracks.capacity() * size_of::<ClipChange>()
                        + tracks
                            .iter()
                            .map(|change| clip_bytes(&change.before) + clip_bytes(&change.after))
                            .sum::<usize>()
                }
                Self::Tempo { before, after } => {
                    (before.capacity() + after.capacity()) * size_of::<Tempo>()
                }
                Self::Move { .. }
                | Self::Loop { .. }
                | Self::RecordMode { .. }
                | Self::TrackMode { .. }
                | Self::InstrumentBypass { .. }
                | Self::InstrumentOffline { .. }
                | Self::Mix { .. } => 0,
            }
    }

    pub(super) fn worker_to_release(&self, forward: bool) -> Option<u64> {
        match self {
            Self::Track { slot, added } if *added != forward => Some(slot.track.id),
            Self::InstrumentOffline { id, .. } => Some(*id),
            Self::Instrument { id, .. } => Some(*id),
            _ => None,
        }
    }

    pub(super) fn matches_state(&self, state: &[u8], forward: bool) -> bool {
        match self {
            Self::Track { slot, .. } | Self::TakeToNewTrack { slot } => state == slot.track.state,
            Self::Instrument { before, after, .. } => {
                state == if forward { &before.state } else { &after.state }
            }
            _ => true,
        }
    }

    pub(super) fn apply(&self, project: &mut Project, forward: bool) -> Result<()> {
        let track_index = |project: &Project, id| {
            project
                .tracks
                .iter()
                .position(|track| track.id == id)
                .ok_or("History track is unavailable.")
        };
        match self {
            Self::Inserts {
                target,
                before,
                after,
                ..
            } => {
                let chain = project
                    .inserts_mut(*target)
                    .ok_or("History mixer channel is unavailable.")?;
                let states: std::collections::HashMap<_, _> = chain
                    .iter()
                    .map(|slot| (slot.id, slot.state.clone()))
                    .collect();
                *chain = if forward { after } else { before }.clone();
                for slot in chain {
                    if let Some(state) = states.get(&slot.id) {
                        slot.state = state.clone();
                    }
                }
            }
            Self::Track { slot, added } => {
                if forward == *added {
                    project
                        .tracks
                        .insert(slot.index.min(project.tracks.len()), slot.track.clone());
                    if !slot.clips.is_empty() {
                        std::sync::Arc::make_mut(&mut project.midi)
                            .tracks
                            .insert(slot.track.id, slot.clips.clone());
                    }
                } else {
                    let index = track_index(project, slot.track.id)?;
                    project.tracks.remove(index);
                    std::sync::Arc::make_mut(&mut project.midi)
                        .tracks
                        .remove(&slot.track.id);
                }
            }
            Self::TakeToNewTrack { slot } => {
                if forward {
                    project
                        .tracks
                        .insert(slot.index.min(project.tracks.len()), slot.track.clone());
                    if !slot.clips.is_empty() {
                        std::sync::Arc::make_mut(&mut project.midi)
                            .tracks
                            .insert(slot.track.id, slot.clips.clone());
                    }
                } else {
                    let index = track_index(project, slot.track.id)?;
                    project.tracks.remove(index);
                    std::sync::Arc::make_mut(&mut project.midi)
                        .tracks
                        .remove(&slot.track.id);
                }
            }
            Self::InstrumentBypass { id, before, after } => {
                let track = project
                    .tracks
                    .iter_mut()
                    .find(|track| track.id == *id)
                    .ok_or("History track is unavailable.")?;
                track.instrument_bypassed = if forward { *after } else { *before };
            }
            Self::InstrumentOffline { id, before, after } => {
                let track = project
                    .tracks
                    .iter_mut()
                    .find(|track| track.id == *id)
                    .ok_or("History track is unavailable.")?;
                track.instrument_offline = if forward { *after } else { *before };
            }
            Self::Mix {
                target,
                control,
                before,
                after,
            } => {
                project.set_mix(*target, *control, if forward { *after } else { *before })?;
            }
            Self::RecordMode { before, after } => {
                project.record_mode = if forward { *after } else { *before };
            }
            Self::TrackMode { id, before, after } => {
                let index = track_index(project, *id)?;
                project.tracks[index].record_mode = if forward { *after } else { *before };
            }
            Self::Move { id, before, after } => {
                let index = track_index(project, *id)?;
                let track = project.tracks.remove(index);
                project
                    .tracks
                    .insert(if forward { *after } else { *before }, track);
            }
            Self::Rename { id, before, after } => {
                let index = track_index(project, *id)?;
                project.tracks[index]
                    .name
                    .clone_from(if forward { after } else { before });
            }
            Self::Instrument {
                id, before, after, ..
            } => {
                let index = track_index(project, *id)?;
                let state = if forward { after } else { before };
                project.tracks[index].plugin.clone_from(&state.plugin);
                project.tracks[index].state.clone_from(&state.state);
            }
            Self::Midi { id, before, after } => {
                track_index(project, *id)?;
                let clips = if forward { after } else { before };
                let tracks = &mut std::sync::Arc::make_mut(&mut project.midi).tracks;
                if clips.is_empty() {
                    tracks.remove(id);
                } else {
                    tracks.insert(*id, clips.clone());
                }
            }
            Self::Clips { tracks, .. } => {
                for ClipChange { id, before, after } in tracks {
                    track_index(project, *id)?;
                    let (expected, clips) = if forward {
                        (before, after)
                    } else {
                        (after, before)
                    };
                    if project.midi.clips(*id) != expected.as_slice() {
                        return Err("The arrangement changed since this history entry.".into());
                    }
                    let arrangements = &mut std::sync::Arc::make_mut(&mut project.midi).tracks;
                    if clips.is_empty() {
                        arrangements.remove(id);
                    } else {
                        arrangements.insert(*id, clips.clone());
                    }
                }
            }
            Self::Tempo { before, after } => {
                project
                    .timeline
                    .changes
                    .clone_from(if forward { after } else { before });
            }
            Self::Loop { before, after } => {
                project.timeline.loop_range = if forward { *after } else { *before };
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_evicts_oldest_entries_by_count_and_bytes() {
        let mut history = History::default();
        history.save();
        for index in 1..=MAX_ENTRIES as u64 + 1 {
            assert!(history.record(Edit::Loop {
                before: [0, index],
                after: [0, index + 1],
            }));
        }
        assert_eq!(history.view(true).labels.len(), MAX_ENTRIES);
        assert_eq!(history.base, 1);
        history.seek(0);
        assert!(history.dirty());

        let blob = |capacity| Edit::Instrument {
            id: 1,
            before: InstrumentState {
                plugin: None,
                state: Vec::new(),
            },
            after: InstrumentState {
                plugin: None,
                state: Vec::with_capacity(capacity),
            },
            editor: false,
        };
        history.reset();
        assert!(history.record(blob(MAX_BYTES / 2)));
        assert!(history.record(blob(MAX_BYTES / 2)));
        assert_eq!(history.view(true).labels.len(), 1);
        assert!(history.bytes <= MAX_BYTES);
        assert!(!history.record(blob(MAX_BYTES)));
        assert!(history.view(true).labels.is_empty());
        assert_eq!(history.bytes, 0);
        assert!(history.dirty());
    }
}
