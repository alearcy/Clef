use clef_model::{
    Plugin, Result,
    midi::{Clip, Event, Mode, Section, Take},
    project::{Project, Send as TrackSend, Track, TrackMode},
    timeline::Tempo,
};
use std::{collections::VecDeque, sync::Arc};

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

pub(super) struct SourceRoute {
    pub track: u64,
    pub before: Option<u64>,
    pub after: Option<u64>,
}

pub(super) struct SourceSend {
    pub track: u64,
    pub before: Vec<TrackSend>,
    pub after: Vec<TrackSend>,
}

pub(super) struct InstrumentState {
    pub plugin: Option<Plugin>,
    pub state: Arc<[u8]>,
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
    Bus {
        slot: Box<TrackSlot>,
        source: Option<SourceRoute>,
        send: Option<SourceSend>,
    },
    Output {
        track: u64,
        before: Option<u64>,
        after: Option<u64>,
    },
    Sends {
        track: u64,
        before: Vec<TrackSend>,
        after: Vec<TrackSend>,
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
    Midi(ClipChange),
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

/// The changed run of one track's clip list. Clips before `prefix` and the last `suffix`
/// clips are unchanged and are not stored, so a one-clip edit costs one clip in history.
pub(super) struct ClipChange {
    id: u64,
    prefix: usize,
    suffix: usize,
    before: Vec<Clip>,
    after: Vec<Clip>,
}

impl ClipChange {
    /// The difference between two clip lists of `id`, or `None` when they are equal.
    pub(super) fn new(id: u64, before: &[Clip], after: &[Clip]) -> Option<Self> {
        let prefix = before
            .iter()
            .zip(after)
            .take_while(|(before, after)| before == after)
            .count();
        if prefix == before.len() && prefix == after.len() {
            return None;
        }
        let suffix = before[prefix..]
            .iter()
            .rev()
            .zip(after[prefix..].iter().rev())
            .take_while(|(before, after)| before == after)
            .count();
        Some(Self {
            id,
            prefix,
            suffix,
            before: before[prefix..before.len() - suffix].to_vec(),
            after: after[prefix..after.len() - suffix].to_vec(),
        })
    }

    fn bytes(&self) -> usize {
        clip_bytes(&self.before) + clip_bytes(&self.after)
    }

    fn apply(&self, project: &mut Project, forward: bool) -> Result<()> {
        if !project.tracks.iter().any(|track| track.id == self.id) {
            return Err("History track is unavailable.".into());
        }
        let (expected, clips) = if forward {
            (&self.before, &self.after)
        } else {
            (&self.after, &self.before)
        };
        let current = project.midi.clips(self.id);
        let end = self.prefix + expected.len();
        if current.len() != end + self.suffix || current[self.prefix..end] != expected[..] {
            return Err("The arrangement changed since this history entry.".into());
        }
        let tracks = &mut std::sync::Arc::make_mut(&mut project.midi).tracks;
        let list = tracks.entry(self.id).or_default();
        list.splice(self.prefix..end, clips.iter().cloned());
        if list.is_empty() {
            tracks.remove(&self.id);
        }
        Ok(())
    }
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
            Self::Bus { send: Some(_), .. } => "New Bus Send",
            Self::Bus {
                source: Some(_), ..
            } => "Route to New Bus",
            Self::Bus {
                source: None,
                send: None,
                ..
            } => "Add Bus",
            Self::Output { .. } => "Change Output",
            Self::Sends { .. } => "Edit Sends",
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
            Self::Midi(_) => "Record MIDI",
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
                        + slot.track.state.len()
                        + plugin_bytes(slot.track.plugin.as_ref())
                        + insert_bytes(&slot.track.inserts)
                        + clip_bytes(&slot.clips)
                }
                Self::Bus { slot, send, .. } => {
                    size_of::<TrackSlot>()
                        + slot.track.name.capacity()
                        + slot.track.state.len()
                        + plugin_bytes(slot.track.plugin.as_ref())
                        + insert_bytes(&slot.track.inserts)
                        + clip_bytes(&slot.clips)
                        + send.as_ref().map_or(0, |send| {
                            (send.before.capacity() + send.after.capacity())
                                * size_of::<TrackSend>()
                        })
                }
                Self::Sends { before, after, .. } => {
                    (before.capacity() + after.capacity()) * size_of::<TrackSend>()
                }
                Self::Rename { before, after, .. } => before.capacity() + after.capacity(),
                Self::Instrument { before, after, .. } => {
                    before.state.len()
                        + after.state.len()
                        + plugin_bytes(before.plugin.as_ref())
                        + plugin_bytes(after.plugin.as_ref())
                }
                Self::Midi(change) => change.bytes(),
                Self::Clips { tracks, .. } => {
                    tracks.capacity() * size_of::<ClipChange>()
                        + tracks.iter().map(ClipChange::bytes).sum::<usize>()
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
                | Self::Output { .. }
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
            Self::Track { slot, .. } | Self::TakeToNewTrack { slot } | Self::Bus { slot, .. } => {
                state == &*slot.track.state
            }
            Self::Instrument { before, after, .. } => {
                let expected = if forward { &before.state } else { &after.state };
                state == &expected[..]
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
            Self::Bus { slot, source, send } => {
                if forward {
                    project
                        .tracks
                        .insert(slot.index.min(project.tracks.len()), slot.track.clone());
                    if let Some(source) = source
                        && let Some(track) = project
                            .tracks
                            .iter_mut()
                            .find(|track| track.id == source.track)
                    {
                        track.output = source.after;
                    }
                } else {
                    let index = track_index(project, slot.track.id)?;
                    project.tracks.remove(index);
                    if let Some(source) = source
                        && let Some(track) = project
                            .tracks
                            .iter_mut()
                            .find(|track| track.id == source.track)
                    {
                        track.output = source.before;
                    }
                }
                if let Some(send) = send {
                    let index = track_index(project, send.track)?;
                    project.tracks[index].sends.clone_from(if forward {
                        &send.after
                    } else {
                        &send.before
                    });
                }
            }
            Self::Output {
                track,
                before,
                after,
            } => {
                let index = track_index(project, *track)?;
                project.tracks[index].output = if forward { *after } else { *before };
            }
            Self::Sends {
                track,
                before,
                after,
            } => {
                let index = track_index(project, *track)?;
                project.tracks[index]
                    .sends
                    .clone_from(if forward { after } else { before });
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
                project.tracks[index].state = state.state.clone();
            }
            Self::Midi(change) => change.apply(project, forward)?,
            Self::Clips { tracks, .. } => {
                for change in tracks {
                    change.apply(project, forward)?;
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
                state: Arc::default(),
            },
            after: InstrumentState {
                plugin: None,
                state: vec![0; capacity].into(),
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

    #[test]
    fn a_one_note_clip_edit_stores_only_the_changed_clip_and_round_trips() {
        use clef_model::midi::{Arrangement, Message};
        let note = |tick| Event {
            tick,
            order: tick as u32,
            message: Message::Note {
                channel: 0,
                key: 60,
                velocity: 90,
                end: tick + 1,
                release: 0,
                release_order: 0,
            },
        };
        let mut arrangement = Arrangement::default();
        for clip in 0..16_u64 {
            let start = clip * 10_000;
            arrangement
                .record(
                    1,
                    Mode::Merge,
                    [start, start + 4_000],
                    vec![(start..start + 4_000).step_by(2).map(note).collect()],
                    false,
                )
                .unwrap();
        }
        let before = arrangement.clips(1).to_vec();
        assert_eq!(before.len(), 16);
        let mut after = before.clone();
        after[7].takes[0].events[3].message = Message::Note {
            channel: 0,
            key: 64,
            velocity: 90,
            end: after[7].takes[0].events[3].tick + 1,
            release: 0,
            release_order: 0,
        };
        assert!(ClipChange::new(1, &before, &before).is_none());
        let change = ClipChange::new(1, &before, &after).unwrap();
        let one_clip = clip_bytes(&before[7..8].to_vec());
        assert_eq!(change.bytes(), 2 * one_clip);
        let edit = Edit::Clips {
            label: "Edit Notes",
            tracks: vec![change],
        };
        assert!(edit.bytes() < 3 * one_clip + 1024);
        assert!(edit.bytes() * 4 < clip_bytes(&before));

        let mut project = Project::default();
        project.tracks.push(Track {
            id: 1,
            name: "Piano".into(),
            plugin: None,
            instrument_bypassed: false,
            instrument_offline: false,
            record_mode: TrackMode::Global,
            mix: Default::default(),
            inserts: Vec::new(),
            take_lanes: Default::default(),
            kind: Default::default(),
            output: None,
            sends: Vec::new(),
            state: Arc::default(),
        });
        project.midi = Arc::new(arrangement);
        edit.apply(&mut project, true).unwrap();
        assert_eq!(project.midi.clips(1), after.as_slice());
        assert!(edit.apply(&mut project, true).is_err());
        edit.apply(&mut project, false).unwrap();
        assert_eq!(project.midi.clips(1), before.as_slice());

        // Removing and inserting clips across tracks keeps the same results.
        let removed = ClipChange::new(1, &before, &before[..15]).unwrap();
        let emptied = ClipChange::new(1, &before[..1], &[]).unwrap();
        assert_eq!(removed.bytes(), one_clip_bytes(&before[15]));
        Edit::Midi(removed).apply(&mut project, true).unwrap();
        assert_eq!(project.midi.clips(1), &before[..15]);
        let mut single = project.clone();
        Arc::make_mut(&mut single.midi)
            .tracks
            .insert(1, before[..1].to_vec());
        Edit::Midi(emptied).apply(&mut single, true).unwrap();
        assert!(!single.midi.tracks.contains_key(&1));
    }

    fn one_clip_bytes(clip: &Clip) -> usize {
        clip_bytes(&vec![clip.clone()])
    }
}
