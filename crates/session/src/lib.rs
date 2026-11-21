pub use clef_audio_io::Devices;
pub use clef_engine::recording::{NoteMark, NotePreview};
pub use clef_engine::transport::{Phase as TransportPhase, View as TransportView};
use clef_engine::{
    live::Configuration,
    multitrack::{Broker, Engine},
    recording::{self, Captured, ControllerBaseline, Projection},
    transport::{Command as TransportCommand, Prepared, SampleMap},
};
use clef_model::{
    Plugin, Result,
    midi::{Arrangement, ClipEdit, ClipEditResult, ClipId, CompEdit, Mode},
    project::{
        self, ArrangementView, AudioSettings, Project, Send as TrackSend, TakeLanes, Track,
        TrackKind, TrackMode,
    },
    timeline::{MAX_TICK, StopMode, Tempo},
};
#[cfg(test)]
use clef_plugin_host::{files, protocol};
use clef_plugin_host::{
    quarantine::{self, Entry},
    rpc::Instrument,
};
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::mpsc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
mod audio;
mod edit_history;
mod inserts;
mod instruments;
pub use inserts::{InsertAction, InsertView};
mod mixer;
pub use mixer::{Meter, MixChange};
mod recovery;
mod shortcuts;
pub use edit_history::HistoryView;
use edit_history::{
    ClipChange, Edit, History, InstrumentState, SourceRoute, SourceSend, TrackSlot,
};
use recovery::{Journal, RecentChange, Store};
pub use recovery::{RecentProject, RecentStatus, RecoveryEntry};

const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(60);
/// Device and plugin-path rescans share this cadence.
const RESCAN_INTERVAL: Duration = Duration::from_secs(2);
/// Per-block engine counters are republished at most this often, so an idle session does not
/// wake the UI on every poll.
const VOLATILE_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Default)]
struct Volatile {
    refreshed: Option<Instant>,
    tracks: HashMap<u64, String>,
}
impl Volatile {
    /// Whether volatile diagnostics may be read again; marks them read when they may.
    fn due(&mut self, now: Instant) -> bool {
        let due = self
            .refreshed
            .is_none_or(|at| now.saturating_duration_since(at) >= VOLATILE_INTERVAL);
        if due {
            self.refreshed = Some(now);
        }
        due
    }
    /// The diagnostics of `id`, read now if due or not yet known, and otherwise retained.
    fn track(&mut self, due: bool, id: u64, read: impl FnOnce() -> String) -> String {
        match self.tracks.entry(id) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                if due {
                    entry.insert(read());
                }
                entry.get().clone()
            }
            std::collections::hash_map::Entry::Vacant(entry) => entry.insert(read()).clone(),
        }
    }
}

/// Plugin bundle existence, checked once per path and rechecked on the rescan cadence so
/// refreshes and UI frames make no filesystem calls.
#[derive(Default)]
struct Paths {
    checked: Option<Instant>,
    exists: HashMap<PathBuf, bool>,
}
impl Paths {
    fn exists(&mut self, path: &std::path::Path) -> bool {
        if let Some(&exists) = self.exists.get(path) {
            return exists;
        }
        let exists = path.exists();
        self.exists.insert(path.to_owned(), exists);
        exists
    }
    fn rescan<'a>(&mut self, now: Instant, paths: impl Iterator<Item = &'a std::path::Path>) {
        if self
            .checked
            .is_some_and(|at| now.saturating_duration_since(at) < RESCAN_INTERVAL)
        {
            return;
        }
        self.checked = Some(now);
        self.exists = paths.map(|path| (path.to_owned(), path.exists())).collect();
    }
}

pub enum Action {
    Shutdown,
    PreviewMix(Option<MixChange>),
    CommitMix(MixChange),
    ResetMeter(u64, clef_model::mixer::Target),
    Insert(u64, clef_model::mixer::Target, InsertAction),
    InstrumentBypass {
        epoch: u64,
        id: u64,
        bypassed: bool,
    },
    InstrumentOffline {
        epoch: u64,
        id: u64,
        offline: bool,
    },
    New,
    Open(PathBuf),
    DiscardBeforeClose,
    /// Write a changed arrangement view of a clean, saved project before the app closes.
    SaveViewBeforeClose,
    Recover(String),
    DiscardRecovery(String),
    RemoveRecent(PathBuf),
    RelinkRecent(PathBuf, PathBuf),
    Save {
        path: PathBuf,
        replace: bool,
        track_name: Option<(u64, String)>,
        use_retained: Vec<(u64, u64)>,
    },
    AddTrack(Option<(u64, String)>),
    AddBus {
        assign: Option<u64>,
    },
    AddBusSend {
        track: u64,
    },
    SetOutput {
        epoch: u64,
        track: u64,
        output: Option<u64>,
    },
    Send {
        epoch: u64,
        track: u64,
        edit: SendEdit,
    },
    SelectTrack(u64, Option<(u64, String)>),
    RemoveTrack(u64),
    MoveTrack(u64, isize),
    Rename(u64, String),
    EditClips {
        expected: Arc<Arrangement>,
        edit: ClipEdit,
    },
    Audition {
        clip: clef_model::midi::ClipId,
        channel: u8,
        key: u8,
        velocity: u8,
    },
    ArrangementView(ArrangementView),
    /// T40: the piano roll's saved zoom and pitch-row height (view state).
    PianoRollView(clef_model::project::PianoRollView),
    TrackView {
        track: u64,
        lanes: TakeLanes,
    },
    CompEdit {
        expected: Arc<Arrangement>,
        track: u64,
        lane: usize,
        edit: CompEdit,
    },
    TakeToNewTrack {
        expected: Arc<Arrangement>,
        track: u64,
        lane: usize,
    },
    SeekHistory {
        version: u64,
        target: usize,
    },
    Select(u64, Plugin),
    /// Apply audio settings to Clef's global preferences or, with `project`, as this
    /// project's override; audio restarts with them (T39).
    Settings {
        audio: AudioSettings,
        project: bool,
    },
    /// Replace the saved keyboard-shortcut changes (T18).
    SetShortcuts(BTreeMap<String, Option<String>>),
    Editor(u64, bool),
    Restart(u64),
    Reactivate(Plugin),
    Transport(TransportAction),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendEdit {
    Add {
        target: u64,
    },
    Remove {
        id: u64,
    },
    SetTarget {
        id: u64,
        target: u64,
    },
    SetPre {
        id: u64,
        pre: bool,
    },
    SetEnabled {
        id: u64,
        enabled: bool,
    },
    /// Transient drag value: reaches the engine, not the project or history, and is not
    /// acknowledged to the UI.
    PreviewLevel {
        id: u64,
        level: i16,
    },
    /// Ends a preview without an edit, for example when the drag is cancelled.
    CancelPreview,
    CommitLevel {
        id: u64,
        level: i16,
    },
}

impl SendEdit {
    /// Whether the edit is a transient preview that the session does not acknowledge.
    pub fn is_preview(self) -> bool {
        matches!(self, Self::PreviewLevel { .. } | Self::CancelPreview)
    }
}

#[derive(Clone, Copy)]
pub enum TransportAction {
    Play,
    Record,
    Arm(u64),
    SetRecordMode(Mode),
    SetTrackRecordMode(u64, TrackMode),
    Stop,
    Locate(u64),
    SetTempo(Tempo),
    RemoveTempo(u32),
    SetLoopRange([u64; 2]),
    ToggleLoop,
    ToggleClick,
    ToggleCountIn,
    SetStopMode(StopMode),
}
#[derive(Clone, Default, PartialEq, Eq)]
pub struct TrackView {
    pub id: u64,
    pub status: String,
    /// The instrument plugin bundle is missing, from the session's cached path check.
    pub missing: bool,
    pub diagnostics: String,
    pub loading: bool,
    pub lost: bool,
    pub loss_version: u64,
}
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub project: Project,
    pub selected: Option<u64>,
    pub tracks: Vec<TrackView>,
    pub inserts: HashMap<u64, InsertView>,
    pub quarantine: Vec<Entry>,
    pub quarantine_error: String,
    pub notice: u64,
    pub path: Option<PathBuf>,
    pub project_open: bool,
    pub read_only: bool,
    pub recovered_from: Option<PathBuf>,
    pub history_ready: bool,
    pub history_message: String,
    pub recents: Vec<RecentProject>,
    pub recoveries: Vec<RecoveryEntry>,
    pub dirty: bool,
    /// The saved project's arrangement view (scroll, zoom, snap) changed since it was last
    /// written; it is saved silently when leaving the project (T37 L4).
    pub view_unsaved: bool,
    pub edit_history: HistoryView,
    pub loaded: bool,
    pub active: bool,
    pub editor: bool,
    pub failed: bool,
    pub message: String,
    pub actual: String,
    /// The audio configuration in use while audio runs, after any fallback (T39).
    pub audio: Option<AudioSettings>,
    /// Clef's global audio preferences, when saved.
    pub audio_preferences: Option<AudioSettings>,
    /// The user's keyboard-shortcut changes by command id (`None` = cleared), as saved in the
    /// global preferences (T18). The UI owns their meaning.
    pub shortcuts: Arc<BTreeMap<String, Option<String>>>,
    pub transport: TransportView,
    pub armed: Option<u64>,
    pub recording: bool,
    pub record_unavailable: Option<&'static str>,
    pub recording_region: Option<[u64; 2]>,
    pub recording_notes: Option<Arc<NotePreview>>,
    pub clip_edit_selection: Option<Vec<ClipId>>,
    pub project_epoch: u64,
    pub track_meters: std::collections::BTreeMap<u64, Meter>,
    pub master_meter: Meter,
}
impl Snapshot {
    pub fn track(&self) -> Option<&Track> {
        self.project
            .tracks
            .iter()
            .find(|t| Some(t.id) == self.selected)
    }
    pub fn state(&self, id: u64) -> Option<&TrackView> {
        self.tracks.iter().find(|t| t.id == id)
    }
    pub fn losses(&self) -> Vec<String> {
        self.project
            .tracks
            .iter()
            .filter(|t| self.state(t.id).is_some_and(|s| s.lost))
            .map(|t| t.name.clone())
            .chain(
                self.project
                    .tracks
                    .iter()
                    .flat_map(|track| &track.inserts)
                    .chain(&self.project.master_inserts)
                    .filter(|insert| {
                        self.inserts
                            .get(&insert.id)
                            .is_some_and(|state| state.loss_version.is_some())
                    })
                    .map(|insert| insert.plugin.name.clone()),
            )
            .collect()
    }
    pub fn retained_losses(&self) -> Vec<(u64, u64)> {
        self.tracks
            .iter()
            .filter(|track| track.lost)
            .map(|track| (track.id, track.loss_version))
            .chain(
                self.inserts
                    .iter()
                    .filter_map(|(&id, view)| view.loss_version.map(|version| (id, version))),
            )
            .collect()
    }
    fn for_ui(&self) -> Self {
        fn without_state(inserts: &[project::Insert]) -> Vec<project::Insert> {
            inserts
                .iter()
                .map(|insert| project::Insert {
                    state: Arc::default(),
                    ..insert.clone()
                })
                .collect()
        }
        Self {
            project: Project {
                piano_roll_view: Default::default(),
                version: self.project.version,
                next_id: self.project.next_id,
                audio: self.project.audio.clone(),
                timeline: self.project.timeline.clone(),
                record_mode: self.project.record_mode,
                midi: self.project.midi.clone(),
                arrangement_view: self.project.arrangement_view,
                master: self.project.master,
                master_inserts: without_state(&self.project.master_inserts),
                tracks: self
                    .project
                    .tracks
                    .iter()
                    .map(|t| Track {
                        colour: t.colour,
                        id: t.id,
                        name: t.name.clone(),
                        plugin: t.plugin.clone(),
                        instrument_bypassed: t.instrument_bypassed,
                        instrument_offline: t.instrument_offline,
                        record_mode: t.record_mode,
                        mix: t.mix,
                        inserts: without_state(&t.inserts),
                        take_lanes: t.take_lanes.clone(),
                        kind: t.kind,
                        output: t.output,
                        sends: t.sends.clone(),
                        state: Arc::default(),
                    })
                    .collect(),
            },
            selected: self.selected,
            tracks: self.tracks.clone(),
            inserts: self.inserts.clone(),
            quarantine: self.quarantine.clone(),
            quarantine_error: self.quarantine_error.clone(),
            notice: self.notice,
            path: self.path.clone(),
            project_open: self.project_open,
            read_only: self.read_only,
            recovered_from: self.recovered_from.clone(),
            history_ready: self.history_ready,
            history_message: self.history_message.clone(),
            recents: self.recents.clone(),
            recoveries: self.recoveries.clone(),
            dirty: self.dirty,
            view_unsaved: self.view_unsaved,
            edit_history: self.edit_history.clone(),
            loaded: self.loaded,
            active: self.active,
            editor: self.editor,
            failed: self.failed,
            message: self.message.clone(),
            actual: self.actual.clone(),
            audio: self.audio.clone(),
            audio_preferences: self.audio_preferences.clone(),
            shortcuts: self.shortcuts.clone(),
            transport: self.transport,
            armed: self.armed,
            recording: self.recording,
            record_unavailable: self.record_unavailable,
            recording_region: self.recording_region,
            recording_notes: self.recording_notes.clone(),
            clip_edit_selection: self.clip_edit_selection.clone(),
            project_epoch: self.project_epoch,
            track_meters: self.track_meters.clone(),
            master_meter: self.master_meter,
        }
    }
}
#[derive(PartialEq, Eq)]
struct FileStamp {
    length: u64,
    modified: std::time::SystemTime,
    inode: u64,
}
impl FileStamp {
    fn read(path: &std::path::Path) -> Result<Self> {
        use std::os::unix::fs::MetadataExt;
        let metadata = std::fs::metadata(path)?;
        Ok(Self {
            length: metadata.len(),
            modified: metadata.modified()?,
            inode: metadata.ino(),
        })
    }
}
#[derive(Default)]
struct Runtime {
    instrument: Option<Instrument>,
    broker: Option<Broker>,
    editor: bool,
    editor_before: Option<Vec<u8>>,
    pending_editor: bool,
    touched: bool,
    lost: bool,
    loading: bool,
    request: u64,
    error: String,
}
struct Loaded {
    instrument: Instrument,
    initial: Arc<[u8]>,
    prepared: bool,
}
struct LoadJob {
    id: u64,
    generation: u64,
    request: u64,
    plugin: Plugin,
    replacement: bool,
    cancel: Arc<AtomicBool>,
    receive: mpsc::Receiver<std::result::Result<Loaded, String>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
struct Capture {
    target: u64,
    mode: Mode,
    baseline: ControllerBaseline,
    map: SampleMap,
    input: Vec<Captured>,
    window: Option<Projection>,
    preview_key: Option<(u64, usize)>,
    preview: Option<recording::LivePreview>,
    interruption: Option<String>,
    stopping: bool,
}

pub struct Session {
    executable: PathBuf,
    silence: Arc<AtomicBool>,
    pub view: Snapshot,
    runtimes: HashMap<u64, Runtime>,
    insert_workers: HashMap<u64, inserts::Runtime>,
    insert_loss_version: u64,
    engine: Option<Engine>,
    jobs: Vec<LoadJob>,
    generation: u64,
    registry: PathBuf,
    registry_error: String,
    file_stamp: Option<FileStamp>,
    last_devices: Instant,
    recording: Option<Capture>,
    deferred_play: Option<bool>,
    history: Option<Store>,
    journal: Option<Journal>,
    checkpoint: Option<std::thread::JoinHandle<std::result::Result<(), String>>>,
    last_checkpoint: Instant,
    checkpoint_take: bool,
    edits: History,
    mixer: mixer::Mixer,
    send_preview: Option<SendPreview>,
    other_changes: bool,
    /// The editor views as last written to or read from the project file.
    saved_view: (
        clef_model::project::ArrangementView,
        clef_model::project::PianoRollView,
    ),
    volatile: Volatile,
    paths: Paths,
    preferences: audio::Preferences,
    shortcuts_path: Option<PathBuf>,
    /// Start audio automatically when a project opens (the app; tests start it explicitly).
    auto_audio: bool,
    audio_pending: bool,
    /// The sample rate loaded workers run at.
    worker_rate: u32,
}

/// A transient send-level drag value applied to the engine but not to the project.
#[derive(Clone, Copy)]
struct SendPreview {
    epoch: u64,
    track: u64,
    id: u64,
    level: i16,
}
impl Session {
    pub fn new(executable: PathBuf) -> Self {
        match quarantine::location() {
            Ok(path) => {
                let mut session = Self::with_registry(path, executable);
                session.auto_audio = true;
                session
            }
            Err(e) => {
                let mut s = Self::with_registry(PathBuf::new(), executable);
                s.registry_error = e.to_string();
                s
            }
        }
    }
    fn with_registry(registry: PathBuf, executable: PathBuf) -> Self {
        let (entries, error) = match quarantine::read(&registry) {
            Ok(v) => (v, String::new()),
            Err(e) => (Vec::new(), e.to_string()),
        };
        let history = registry
            .parent()
            .filter(|p| p.is_absolute())
            .map(|p| Store(p.to_owned()));
        let (preferences, preferences_error) = registry
            .parent()
            .filter(|p| p.is_absolute())
            .map(|p| audio::Preferences::read(p.join("audio.json")))
            .unwrap_or_default();
        let shortcuts_path = registry
            .parent()
            .filter(|p| p.is_absolute())
            .map(|p| p.join("shortcuts.json"));
        let (shortcuts, shortcuts_error) = shortcuts_path
            .as_deref()
            .map(shortcuts::read)
            .unwrap_or_default();
        Self {
            executable,
            silence: Arc::new(AtomicBool::new(true)),
            view: Snapshot {
                quarantine: entries,
                audio_preferences: preferences.audio.clone(),
                shortcuts: Arc::new(shortcuts),
                message: [preferences_error, shortcuts_error]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" "),
                ..Default::default()
            },
            runtimes: HashMap::new(),
            insert_workers: HashMap::new(),
            insert_loss_version: 1,
            engine: None,
            jobs: Vec::new(),
            generation: 0,
            registry,
            registry_error: error,
            file_stamp: None,
            last_devices: Instant::now(),
            recording: None,
            deferred_play: None,
            history,
            journal: None,
            checkpoint: None,
            last_checkpoint: Instant::now(),
            checkpoint_take: false,
            edits: History::default(),
            mixer: mixer::Mixer::default(),
            send_preview: None,
            other_changes: false,
            saved_view: Default::default(),
            volatile: Volatile::default(),
            paths: Paths::default(),
            preferences,
            shortcuts_path,
            auto_audio: false,
            audio_pending: false,
            worker_rate: clef_plugin_host::protocol::RATE,
        }
    }

    fn load_history(&mut self) {
        if let Some(store) = &self.history {
            match store.recents() {
                Ok(recents) => self.view.recents = recents,
                Err(error) => {
                    self.view.history_message = format!("Recent projects unavailable: {error}")
                }
            }
            match store.recoveries() {
                Ok(recoveries) => self.view.recoveries = recoveries,
                Err(error) => self.view.history_message = format!("Recovery unavailable: {error}"),
            }
        } else {
            self.view.history_message =
                "Clef's Application Support directory is unavailable.".into();
        }
        self.view.history_ready = true;
    }

    fn update_recents(&mut self, change: RecentChange<'_>) -> Result<()> {
        let store = self
            .history
            .as_ref()
            .ok_or("Project history is unavailable.")?;
        self.view.recents = store.update_recents(change, &self.view.recents)?;
        Ok(())
    }

    fn note_recent(&mut self, path: &std::path::Path) {
        if let Err(error) = self.update_recents(RecentChange::Use(path)) {
            self.view.history_message = format!("Could not update recent projects: {error}");
        }
    }

    fn finish_checkpoint(&mut self) {
        if let Some(job) = self.checkpoint.take() {
            match job.join() {
                Ok(Ok(())) => {
                    if self.view.history_message.starts_with("Autosave ") {
                        self.view.history_message.clear();
                    }
                }
                Ok(Err(error)) => self.view.history_message = format!("Autosave failed: {error}"),
                Err(_) => {
                    self.view.history_message = "Autosave worker stopped unexpectedly.".into()
                }
            }
        }
    }

    fn retire_journal(&mut self) -> Result<()> {
        self.finish_checkpoint();
        if let Some(journal) = &self.journal {
            journal.retire()?;
        }
        self.journal = None;
        self.last_checkpoint = Instant::now();
        self.checkpoint_take = false;
        Ok(())
    }

    fn poll_autosave(&mut self, now: Instant) {
        if self
            .checkpoint
            .as_ref()
            .is_some_and(|job| job.is_finished())
        {
            self.finish_checkpoint();
        }
        if self.checkpoint.is_some()
            || !self.view.project_open
            || !self.view.dirty
            || (!self.checkpoint_take
                && now.duration_since(self.last_checkpoint) < AUTOSAVE_INTERVAL)
        {
            return;
        }
        self.last_checkpoint = now;
        self.checkpoint_take = false;
        if let Err(error) = self.start_checkpoint() {
            self.view.history_message = format!("Autosave failed: {error}");
        }
    }

    fn start_checkpoint(&mut self) -> Result<()> {
        if self.journal.is_none() {
            let store = self
                .history
                .as_ref()
                .ok_or("Project history is unavailable.")?;
            let original = self
                .view
                .path
                .as_ref()
                .map(std::path::absolute)
                .transpose()?;
            self.journal = Some(store.create(original.as_deref())?);
        }
        let directory = self.journal.as_ref().unwrap().path.clone();
        let project = self.view.project.clone();
        self.checkpoint = Some(
            std::thread::Builder::new()
                .name("clef-autosave".into())
                .spawn(move || {
                    recovery::checkpoint(&directory, &project).map_err(|error| error.to_string())
                })?,
        );
        Ok(())
    }

    fn install_project(
        &mut self,
        project: Project,
        path: Option<PathBuf>,
        stamp: Option<FileStamp>,
    ) {
        self.unload();
        self.file_stamp = stamp;
        self.view.selected = project.tracks.first().map(|t| t.id);
        self.view.armed = None;
        self.saved_view = (project.arrangement_view, project.piano_roll_view);
        self.view.project = project;
        self.view.transport = TransportView::default();
        self.view.read_only = path.as_deref().is_some_and(recovery::read_only);
        self.view.path = path;
        self.view.recovered_from = None;
        self.view.dirty = false;
        self.edits.reset();
        self.other_changes = false;
        self.send_preview = None;
        self.view.project_open = true;
        self.view.project_epoch = self.view.project_epoch.wrapping_add(1);
        self.audio_pending = true;
    }

    fn view_unsaved(&self) -> bool {
        self.view.project_open
            && !self.view.read_only
            && self.view.path.is_some()
            && (
                self.view.project.arrangement_view,
                self.view.project.piano_roll_view,
            ) != self.saved_view
    }

    /// When leaving a clean, saved project whose only change is the arrangement view, write
    /// it without prompting. The file then differs only by that view. A failure loses only
    /// the view and never blocks leaving the project (T37 L4).
    fn save_view_before_leaving(&mut self) {
        if self.view.dirty || self.view.recording || !self.view_unsaved() {
            return;
        }
        let Some(path) = self.view.path.clone() else {
            return;
        };
        let result = (|| -> Result<FileStamp> {
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)?;
            file.try_lock()
                .map_err(|e| format!("project busy in another session: {e}"))?;
            if self.file_stamp.as_ref() != Some(&FileStamp::read(&path)?) {
                return Err("project changed outside this session".into());
            }
            self.view.project.save(&path, true)?;
            FileStamp::read(&path)
        })();
        match result {
            Ok(stamp) => {
                self.file_stamp = Some(stamp);
                self.saved_view = (
                    self.view.project.arrangement_view,
                    self.view.project.piano_roll_view,
                );
            }
            Err(error) => eprintln!("Arrangement view not saved: {error}"),
        }
    }

    fn mark_other_change(&mut self) {
        self.other_changes = true;
        self.view.dirty = true;
    }

    fn record_edit(&mut self, edit: Edit) -> bool {
        let retained = self.edits.record(edit);
        self.view.dirty = true;
        if !retained {
            self.view.message =
                "Edit applied. History cleared because the edit exceeded 64 MiB.".into();
        }
        retained
    }

    fn history_available(&self) -> bool {
        self.view.project_open
            && !self.view.recording
            && !self.view.transport.pending
            && self.view.transport.phase == TransportPhase::Stopped
            && self.runtimes.values().all(|runtime| !runtime.loading)
    }

    fn capture_track_state(&mut self, id: u64) -> Result<Arc<[u8]>> {
        let Some(runtime) = self.runtimes.get_mut(&id) else {
            return Ok(self.track(id)?.state.clone());
        };
        if runtime.lost {
            return Err("The instrument's latest state was lost. Save with retained state before changing this track.".into());
        }
        let Some(instrument) = runtime.instrument.as_mut() else {
            return Ok(self.track(id)?.state.clone());
        };
        if let Some((engine, broker)) = self.engine.as_mut().zip(runtime.broker.take()) {
            engine.detach(broker, instrument)?;
            let state = instrument.save();
            runtime.broker = Some(engine.attach(id, instrument)?);
            state.map(Into::into)
        } else {
            instrument.save().map(Into::into)
        }
    }

    fn release_track_worker(&mut self, id: u64) {
        if let Some(mut runtime) = self.runtimes.remove(&id)
            && let (Some(engine), Some(broker), Some(instrument)) = (
                &mut self.engine,
                runtime.broker.take(),
                runtime.instrument.as_mut(),
            )
        {
            let _ = engine.detach(broker, instrument);
        }
        for job in &self.jobs {
            if job.id == id {
                job.cancel.store(true, Ordering::Release);
            }
        }
    }

    fn seek_history(&mut self, version: u64, target: usize) -> Result<()> {
        self.refresh_transport();
        if !self.history_available() {
            return Err(
                "Stop transport and wait for instrument loads before using edit history.".into(),
            );
        }
        self.edits.target(version, target)?;
        let mut cursor = self.edits.cursor();
        if cursor == target {
            return Ok(());
        }
        // Validate the complete jump before publishing the timeline and replacing workers.
        let mut candidate = self.view.project.clone();
        let mut release = Vec::new();
        while let Some((edit, forward)) = self.edits.step(cursor, target) {
            if let Some(id) = edit.worker_to_release(forward) {
                let mut captured = None;
                if !release.contains(&id) && self.track(id).is_ok() {
                    if self.runtimes.get(&id).is_some_and(|runtime| runtime.editor) {
                        return Err("Close the affected instrument editor before changing its state in history.".into());
                    }
                    captured = Some(self.capture_track_state(id)?);
                    release.push(id);
                }
                let previous = candidate
                    .tracks
                    .iter()
                    .find(|track| track.id == id)
                    .ok_or("History track is unavailable.")?;
                let (edit, _) = self.edits.step(cursor, target).expect("history step");
                if !edit.matches_state(captured.as_deref().unwrap_or(&*previous.state), forward) {
                    return Err("The instrument state changed since this history entry. Its current state was preserved.".into());
                }
                release.push(id);
            }
            let (edit, forward) = self.edits.step(cursor, target).expect("history step");
            edit.apply(&mut candidate, forward)?;
            cursor = if forward { cursor + 1 } else { cursor - 1 };
        }
        candidate.validate()?;
        self.capture_removed_inserts(&candidate)?;
        let timing = match &self.engine {
            Some(engine) if candidate.timeline != self.view.project.timeline => {
                Some(Prepared::new(&candidate.timeline, engine.rate)?)
            }
            _ => None,
        };
        // Prepare the insert chains first: the engine sees no part of the jump unless every
        // affected chain is ready.
        self.reconcile_inserts(&candidate, false)?;
        let retimed = timing.is_some();
        if let Err(error) = self.publish_history_playback(&candidate, timing) {
            // Only the arrangement can fail after new timing was queued; restore both halves.
            let previous = self.view.project.clone();
            if let Some(engine) = self.engine.as_mut()
                && retimed
                && let Ok(timing) = Prepared::new(&previous.timeline, engine.rate)
            {
                let _ = engine.transport.settings(timing);
            }
            if let Err(rollback) = self.reconcile_inserts(&previous, true) {
                self.view.message = format!("Effect chains could not be restored: {rollback}");
            }
            return Err(error);
        }
        self.sync_instrument_bypasses(&candidate);
        self.send_preview = None;
        for id in release {
            self.release_track_worker(id);
        }
        self.view.project = candidate;
        if self.view.selected.is_none_or(|id| self.track(id).is_err()) {
            self.view.selected = self.view.project.tracks.first().map(|track| track.id);
        }
        if self.view.armed.is_some_and(|id| self.track(id).is_err()) {
            self.view.armed = None;
        }
        self.update_live_target();
        if let Some(id) = self.live_target()
            && self.engine.is_some()
            && let Ok(track) = self.track(id)
            && !track.instrument_offline
            && track
                .plugin
                .as_ref()
                .is_some_and(|plugin| !self.blocked(plugin) && plugin.path.exists())
        {
            let _ = self.request_load(id, None);
        }
        self.edits.seek(target);
        self.view.dirty = self.edits.dirty()
            || self.other_changes
            || self.runtimes.values().any(|runtime| runtime.editor);
        if self.view.dirty {
            self.checkpoint_take = true;
        } else if let Err(error) = self.retire_journal() {
            self.view.history_message =
                format!("Saved revision restored, but recovery could not be removed: {error}");
        }
        self.view.message = "Edit history position changed.".into();
        Ok(())
    }
    fn publish_history_playback(
        &mut self,
        candidate: &Project,
        timing: Option<Prepared>,
    ) -> Result<()> {
        let Some(engine) = self.engine.as_mut() else {
            return Ok(());
        };
        if let Some(timing) = timing {
            engine.transport.settings(timing)?;
        }
        if candidate.midi != self.view.project.midi
            || candidate.timeline.changes != self.view.project.timeline.changes
        {
            engine.set_arrangement(&candidate.midi, &candidate.timeline)?;
        }
        Ok(())
    }
    fn blocked(&self, plugin: &Plugin) -> bool {
        self.view
            .quarantine
            .iter()
            .any(|e| quarantine::same(&e.plugin, plugin))
    }
    fn stop(&mut self) -> Result<()> {
        let capture_result = self.finish_recording(None);
        self.deferred_play = None;
        self.view.recording = false;
        self.silence.store(true, Ordering::Release);
        self.view.active = false;
        self.view.audio = None;
        let mut error = None;
        let mut faults = Vec::new();
        if let Some(mut engine) = self.engine.take() {
            let tick = engine.transport.view().tick;
            self.view.transport = TransportView {
                tick,
                audible_tick: tick,
                ..Default::default()
            };
            self.view.actual = engine.summary();
            for (id, runtime) in &mut self.runtimes {
                if let (Some(broker), Some(instrument)) =
                    (runtime.broker.take(), runtime.instrument.as_mut())
                    && let Err(e) = engine.detach(broker, instrument)
                {
                    runtime.error = e.to_string();
                    if let Some(plugin) = self
                        .view
                        .project
                        .tracks
                        .iter()
                        .find(|t| t.id == *id)
                        .and_then(|t| t.plugin.clone())
                    {
                        faults.push((plugin, e.to_string()));
                    }
                    error = Some(e);
                }
            }
        }
        self.stop_inserts();
        for (plugin, reason) in faults {
            self.quarantine(plugin, reason);
        }
        if let Some(e) = error {
            Err(e)
        } else {
            capture_result.map(|_| ())
        }
    }
    fn unload(&mut self) {
        self.generation += 1;
        for job in &self.jobs {
            job.cancel.store(true, Ordering::Release);
        }
        let _ = self.stop();
        self.runtimes.clear();
        self.insert_workers.clear();
    }
    fn close_editors(&mut self) -> Result<()> {
        let mut open = Vec::new();
        for (&id, runtime) in &mut self.runtimes {
            runtime.pending_editor = false;
            if runtime.editor {
                open.push(id);
            }
        }
        for id in open {
            self.set_track_editor(id, false)?;
        }
        Ok(())
    }

    fn set_track_editor(&mut self, id: u64, open: bool) -> Result<()> {
        let runtime = self.runtimes.get(&id).ok_or("editor worker unavailable")?;
        if runtime.editor == open {
            // A native close may not have reached the session poll yet. Consume it
            // before an open request so the click is not swallowed by stale state.
            let closed = open
                && self
                    .runtimes
                    .get_mut(&id)
                    .and_then(|runtime| runtime.instrument.as_mut())
                    .is_some_and(Instrument::editor_closed);
            if !closed {
                return Ok(());
            }
            self.finish_track_editor(id)?;
        }
        let runtime = self
            .runtimes
            .get_mut(&id)
            .ok_or("editor worker unavailable")?;
        let instrument = runtime
            .instrument
            .as_mut()
            .ok_or("editor worker unavailable")?;
        if open {
            let baseline = if runtime.broker.is_none() {
                instrument.save().ok()
            } else {
                None
            };
            instrument.editor(true)?;
            runtime.editor_before = baseline;
            runtime.editor = true;
            runtime.touched = true;
            self.view.dirty = true;
            if runtime.editor_before.is_none() {
                self.mark_other_change();
            }
            return Ok(());
        }
        instrument.editor(false)?;
        self.finish_track_editor(id)
    }

    fn finish_track_editor(&mut self, id: u64) -> Result<()> {
        let runtime = self
            .runtimes
            .get_mut(&id)
            .ok_or("editor worker unavailable")?;
        runtime.editor = false;
        if let Some(before) = runtime.editor_before.take() {
            if runtime.broker.is_some() {
                self.mark_other_change();
                return Ok(());
            }
            let after = match runtime
                .instrument
                .as_mut()
                .ok_or("editor worker unavailable")?
                .save()
            {
                Ok(after) => after,
                Err(error) => {
                    self.mark_other_change();
                    return Err(error);
                }
            };
            runtime.touched = false;
            let track = self
                .view
                .project
                .tracks
                .iter_mut()
                .find(|track| track.id == id)
                .ok_or("editor track unavailable")?;
            let after: Arc<[u8]> = after.into();
            track.state = after.clone();
            let plugin = track.plugin.clone();
            if *before != *after {
                self.record_edit(Edit::Instrument {
                    id,
                    before: InstrumentState {
                        plugin: plugin.clone(),
                        state: before.into(),
                    },
                    after: InstrumentState {
                        plugin,
                        state: after,
                    },
                    editor: true,
                });
            }
        }
        self.view.dirty = self.other_changes
            || self.edits.dirty()
            || self.runtimes.values().any(|runtime| runtime.editor);
        Ok(())
    }
    fn track(&self, id: u64) -> Result<&Track> {
        self.view
            .project
            .tracks
            .iter()
            .find(|t| t.id == id)
            .ok_or_else(|| "track no longer exists".into())
    }
    fn request_load(&mut self, id: u64, replacement: Option<Plugin>) -> Result<()> {
        let track = self.track(id)?;
        if track.instrument_offline {
            return Err("Instrument track is offline. Bring it online first.".into());
        }
        let plugin = replacement
            .clone()
            .or_else(|| track.plugin.clone())
            .ok_or("choose an instrument first")?;
        if self.registry.as_os_str().is_empty() {
            return Err("quarantine registry location unavailable".into());
        }
        // Re-read before every load so another Clef instance cannot silently clear protection.
        for entry in quarantine::read(&self.registry)? {
            if !self.blocked(&entry.plugin) {
                self.view.quarantine.push(entry);
            }
        }
        if self.blocked(&plugin) {
            return Err("Plugin is quarantined. Reactivate explicitly before retrying.".into());
        }
        if self.jobs.len() >= crate::project::MAX_TRACKS {
            return Err("wait for pending instrument loads".into());
        }
        let runtime = self.runtimes.entry(id).or_default();
        if runtime.loading {
            return Err("instrument is already loading".into());
        }
        if replacement.is_none() && runtime.instrument.is_some() {
            return Ok(());
        }
        let state = if replacement.is_some() {
            Arc::default()
        } else {
            self.track(id)?.state.clone()
        };
        let runtime = self.runtimes.entry(id).or_default();
        runtime.loading = true;
        runtime.request += 1;
        let request = runtime.request;
        runtime.error.clear();
        let (send, receive) = mpsc::channel();
        let identity = plugin.clone();
        let rate = self.worker_rate;
        let prepare_live = self.engine.is_some();
        if replacement.is_some()
            && self.live_target() == Some(id)
            && let Some(engine) = &self.engine
        {
            engine.clock.select(0);
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let cancelled = cancel.clone();
        let executable = self.executable.clone();
        let thread = std::thread::spawn(move || {
            let result: Result<Loaded> = (|| {
                if cancelled.load(Ordering::Acquire) {
                    return Err("load cancelled".into());
                }
                let mut instrument = Instrument::load_selected_cancellable(
                    &executable,
                    &identity.path,
                    &identity.id,
                    rate,
                    if state.is_empty() {
                        None
                    } else {
                        Some(&*state)
                    },
                    &cancelled,
                )?;
                let initial = if state.is_empty() {
                    instrument.save()?.into()
                } else {
                    state
                };
                if cancelled.load(Ordering::Acquire) {
                    return Err("load cancelled".into());
                }
                // Activation can be slow: keep it in the per-track load job, away
                // from the session action loop and every healthy audio path.
                if prepare_live {
                    instrument.start(false)?;
                }
                Ok(Loaded {
                    instrument,
                    initial,
                    prepared: prepare_live,
                })
            })();
            let _ = send.send(result.map_err(|e| e.to_string()));
        });
        self.jobs.push(LoadJob {
            id,
            generation: self.generation,
            request,
            plugin,
            replacement: replacement.is_some(),
            cancel,
            receive,
            thread: Some(thread),
        });
        Ok(())
    }
    fn capture(&mut self, use_retained: &[(u64, u64)]) -> Result<Project> {
        self.stop()?;
        self.close_editors()?;
        if self
            .runtimes
            .iter()
            .any(|(id, r)| r.lost && !use_retained.contains(&(*id, r.request)))
        {
            return Err(
                "Save requires explicit consent to use retained state for crashed tracks".into(),
            );
        }
        let mut candidate = self.view.project.clone();
        self.capture_inserts(&mut candidate, use_retained)?;
        for track in &mut candidate.tracks {
            if let Some(runtime) = self.runtimes.get_mut(&track.id)
                && let Some(instrument) = runtime.instrument.as_mut()
            {
                instrument.stop()?;
                track.state = instrument.save()?.into();
                if let Some(retained) = self
                    .view
                    .project
                    .tracks
                    .iter_mut()
                    .find(|t| t.id == track.id)
                {
                    retained.state = track.state.clone();
                }
            }
        }
        candidate.validate()?;
        Ok(candidate)
    }
    fn merge_persisted_quarantine(&mut self, mut entries: Vec<Entry>, cleared: Option<&Plugin>) {
        let mut pending = false;
        for entry in &self.view.quarantine {
            if cleared.is_some_and(|p| quarantine::same(p, &entry.plugin)) {
                continue;
            }
            if !entries
                .iter()
                .any(|e| quarantine::same(&e.plugin, &entry.plugin))
            {
                entries.push(entry.clone());
                pending = true;
            }
        }
        self.view.quarantine = entries;
        self.registry_error = if pending {
            "Some quarantine records remain only in memory; persistence across restart is not guaranteed.".into()
        } else {
            String::new()
        };
    }
    fn quarantine(&mut self, plugin: Plugin, reason: String) {
        self.quarantine_kind(plugin, reason, false);
    }

    fn quarantine_kind(&mut self, plugin: Plugin, reason: String, effect: bool) {
        let reason: String = reason.chars().take(1024).collect();
        for track in &self.view.project.tracks {
            if track
                .plugin
                .as_ref()
                .is_some_and(|p| quarantine::same(p, &plugin))
            {
                let runtime = self.runtimes.entry(track.id).or_default();
                if let (Some(engine), Some(broker), Some(instrument)) = (
                    &mut self.engine,
                    runtime.broker.take(),
                    runtime.instrument.as_mut(),
                ) {
                    let _ = engine.detach(broker, instrument);
                }
                runtime.lost |= runtime.touched;
                self.other_changes |= runtime.lost;
                runtime.instrument.take();
                runtime.editor = false;
                runtime.editor_before = None;
                runtime.pending_editor = false;
                runtime.error = reason.clone();
                runtime.loading = false;
                runtime.request += 1;
            }
        }
        if !self.blocked(&plugin) {
            self.view.quarantine.push(Entry {
                plugin: plugin.clone(),
                reason: reason.clone(),
                time: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                effect,
            });
        }
        match quarantine::update_kind(&self.registry, &plugin, Some(&reason), effect) {
            Ok(entries) => self.merge_persisted_quarantine(entries, None),
            Err(e) => {
                self.registry_error =
                    format!("Quarantine retained in memory but could not be persisted: {e}")
            }
        }
        self.view.message = format!(
            "{} quarantined: {reason}. Other plugin identities continue. Unsnapshotted edits may be lost.",
            plugin.name
        );
    }
    pub fn apply(&mut self, action: Action) -> Result<()> {
        self.view.clip_edit_selection = None;
        if self.view.recording {
            match &action {
                Action::Shutdown
                | Action::New
                | Action::Open(_)
                | Action::DiscardBeforeClose
                | Action::Recover(_)
                | Action::Save { .. }
                | Action::Settings { .. } => {
                    return Err("Stop recording before changing or saving the project.".into());
                }
                Action::RemoveTrack(id) | Action::Select(id, _) | Action::Restart(id)
                    if Some(*id) == self.view.armed =>
                {
                    return Err("Stop recording before changing the armed instrument.".into());
                }
                _ => {}
            }
        }
        match action {
            Action::PreviewMix(change) => {
                self.preview_mix(change)?;
                self.publish_mix();
                return Ok(());
            }
            Action::CommitMix(change) => {
                let result = self.commit_mix(change);
                self.publish_mix();
                result?;
            }
            Action::ResetMeter(epoch, target) => {
                if epoch == self.view.project_epoch {
                    self.reset_meter(target);
                }
            }
            Action::Insert(epoch, target, action) => self.edit_inserts(epoch, target, action)?,
            Action::InstrumentBypass {
                epoch,
                id,
                bypassed,
            } => self.set_instrument_bypass(epoch, id, bypassed)?,
            Action::InstrumentOffline { epoch, id, offline } => {
                self.set_instrument_offline(epoch, id, offline)?
            }
            Action::Shutdown => {}
            Action::New => {
                self.save_view_before_leaving();
                self.retire_journal()?;
                self.install_project(Project::default(), None, None);
                self.view.message = "New project. Add an instrument track to begin.".into();
            }
            Action::Open(path) => {
                let candidate = (|| -> Result<_> {
                    if !std::fs::metadata(&path)?.is_file() {
                        return Err("Not a regular project file.".into());
                    }
                    let stamp = FileStamp::read(&path)?;
                    let project = Project::read(&path)?;
                    if FileStamp::read(&path)? != stamp {
                        return Err("project changed while opening; retry Open".into());
                    }
                    Ok((stamp, project))
                })();
                let (stamp, project) = candidate.inspect_err(|_| {
                    if let Some(recent) = self
                        .view
                        .recents
                        .iter_mut()
                        .find(|recent| recent.path == path)
                    {
                        recent.status = recovery::probe(&path);
                    }
                })?;
                self.save_view_before_leaving();
                self.retire_journal()?;
                self.note_recent(&path);
                self.install_project(project, Some(path), Some(stamp));
                self.view.notice += 1;
                self.view.message =
                    "Project opened. Audio stopped; instruments load on first explicit use.".into();
            }
            Action::DiscardBeforeClose => {
                self.retire_journal()?;
            }
            Action::SaveViewBeforeClose => self.save_view_before_leaving(),
            Action::Recover(id) => {
                let store = self
                    .history
                    .as_ref()
                    .ok_or("Project history is unavailable.")?;
                let journal = store.acquire(&id)?;
                let project = journal.recover()?;
                let original = journal.original();
                self.retire_journal()?;
                self.install_project(project, None, None);
                self.view.recovered_from = original;
                self.mark_other_change();
                self.view.notice += 1;
                self.view.recoveries.retain(|entry| entry.id != id);
                self.view.message = "Recovered project. Use Save As to keep this copy.".into();
                self.journal = Some(journal);
            }
            Action::DiscardRecovery(id) => {
                let store = self
                    .history
                    .as_ref()
                    .ok_or("Project history is unavailable.")?;
                store.acquire(&id)?.retire()?;
                self.view.recoveries.retain(|entry| entry.id != id);
                self.view.message = "Recovery discarded.".into();
            }
            Action::RemoveRecent(path) => self.update_recents(RecentChange::Remove(&path))?,
            Action::RelinkRecent(old, new) => {
                self.update_recents(RecentChange::Relink(&old, &new))?
            }
            Action::Save {
                path,
                replace,
                track_name,
                use_retained,
            } => {
                if self.runtimes.values().any(|r| r.loading) {
                    return Err("wait for instrument loading before saving".into());
                }
                if let Some((id, name)) = track_name {
                    self.rename(id, name)?;
                }
                self.view.dirty = true;
                let _lock = if replace && path.exists() {
                    let file = std::fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open(&path)?;
                    file.try_lock()
                        .map_err(|e| format!("project busy in another session: {e}"))?;
                    Some(file)
                } else {
                    None
                };
                if replace
                    && self.view.path.as_ref() == Some(&path)
                    && self.file_stamp.as_ref() != Some(&FileStamp::read(&path)?)
                {
                    return Err("project changed outside this session; use Save As".into());
                }
                let candidate = self.capture(&use_retained)?;
                if replace
                    && self.view.path.as_ref() == Some(&path)
                    && self.file_stamp.as_ref() != Some(&FileStamp::read(&path)?)
                {
                    return Err("project changed during capture; use Save As".into());
                }
                candidate.save(&path, replace)?;
                self.saved_view = (candidate.arrangement_view, candidate.piano_roll_view);
                self.view.project = candidate;
                self.file_stamp = Some(FileStamp::read(&path)?);
                self.note_recent(&path);
                self.view.path = Some(path);
                self.view.read_only = false;
                self.view.recovered_from = None;
                self.view.dirty = false;
                self.other_changes = false;
                self.edits.save();
                for runtime in self.runtimes.values_mut() {
                    runtime.touched = false;
                    runtime.lost = false;
                }
                self.clear_insert_losses();
                self.view.message =
                    "Project saved. Audio stopped and editors closed for a coherent snapshot."
                        .into();
                if let Err(error) = self.retire_journal() {
                    self.view.history_message =
                        format!("Project saved, but old recovery could not be removed: {error}");
                }
            }
            Action::AddTrack(rename) => {
                if let Some((id, name)) = rename {
                    self.rename(id, name)?;
                }
                if self.view.project.tracks.len() >= crate::project::MAX_TRACKS {
                    return Err("project track safety limit reached".into());
                }
                let id = self.view.project.next_id;
                let next = id.checked_add(1).ok_or("track ID exhausted")?;
                self.select(None)?;
                let track = Track {
                    colour: Some(clef_model::project::next_track_colour(
                        self.view.project.tracks.len(),
                    )),
                    id,
                    name: format!(
                        "Instrument {}",
                        self.view
                            .project
                            .tracks
                            .iter()
                            .filter(|track| track.is_instrument())
                            .count()
                            + 1
                    ),
                    plugin: None,
                    instrument_bypassed: false,
                    instrument_offline: false,
                    record_mode: clef_model::project::TrackMode::Global,
                    mix: Default::default(),
                    inserts: Vec::new(),
                    take_lanes: clef_model::project::TakeLanes::default(),
                    kind: clef_model::project::TrackKind::Instrument,
                    output: None,
                    sends: Vec::new(),
                    state: Arc::default(),
                };
                let index = self.view.project.tracks.len();
                self.view.project.tracks.push(track.clone());
                self.view.project.next_id = next;
                self.view.selected = Some(id);
                self.record_edit(Edit::Track {
                    slot: Box::new(TrackSlot {
                        track,
                        index,
                        clips: Vec::new(),
                    }),
                    added: true,
                });
                if let Some(engine) = &self.engine {
                    engine.clock.select(id);
                }
            }
            Action::AddBus { assign } => self.add_bus(assign, None)?,
            Action::AddBusSend { track } => self.add_bus(None, Some(track))?,
            Action::SetOutput {
                epoch,
                track,
                output,
            } => self.set_output(epoch, track, output)?,
            Action::Send { epoch, track, edit } => self.edit_send(epoch, track, edit)?,
            Action::SelectTrack(id, rename) => {
                self.track(id)?;
                if let Some((old, name)) = rename {
                    self.rename(old, name)?;
                }
                self.select(Some(id))?;
                if self.track(id)?.plugin.is_some() && !self.track(id)?.instrument_offline {
                    self.request_load(id, None)?;
                }
            }
            Action::RemoveTrack(id) => {
                let index = self
                    .view
                    .project
                    .tracks
                    .iter()
                    .position(|t| t.id == id)
                    .ok_or("track no longer exists")?;
                if self.view.project.tracks[index].is_bus() {
                    let sources = self.view.project.has_sources(id);
                    if sources > 0 {
                        return Err(format!(
                            "Bus still has {sources} sources. Reassign them before removing it."
                        )
                        .into());
                    }
                }
                if self.runtimes.get(&id).is_some_and(|runtime| runtime.editor) {
                    self.set_track_editor(id, false)?;
                }
                let state = self.capture_track_state(id)?;
                let mut candidate = self.view.project.clone();
                let mut track = candidate.tracks.remove(index);
                self.capture_removed_inserts(&candidate)?;
                track
                    .inserts
                    .clone_from(&self.view.project.tracks[index].inserts);
                track.state = state;
                let clips = Arc::make_mut(&mut candidate.midi)
                    .tracks
                    .remove(&id)
                    .unwrap_or_default();
                if let Some(engine) = &self.engine {
                    engine.set_arrangement(&candidate.midi, &candidate.timeline)?;
                }
                if self.view.selected == Some(id) {
                    self.select(None)?;
                }
                self.release_track_worker(id);
                self.reconcile_inserts(&candidate, false)?;
                self.view.project = candidate;
                if self.view.armed == Some(id) {
                    self.view.armed = None;
                    self.update_live_target();
                }
                self.record_edit(Edit::Track {
                    slot: Box::new(TrackSlot {
                        track,
                        index,
                        clips,
                    }),
                    added: false,
                });
                if self.view.selected.is_none() && !self.view.project.tracks.is_empty() {
                    let next =
                        self.view.project.tracks[index.min(self.view.project.tracks.len() - 1)].id;
                    self.select(Some(next))?;
                    if self.track(next)?.plugin.is_some() && !self.track(next)?.instrument_offline {
                        let _ = self.request_load(next, None);
                    }
                }
            }
            Action::MoveTrack(id, delta) => {
                let index = self
                    .view
                    .project
                    .tracks
                    .iter()
                    .position(|t| t.id == id)
                    .ok_or("track no longer exists")?;
                let target = index
                    .checked_add_signed(delta)
                    .filter(|n| *n < self.view.project.tracks.len())
                    .ok_or("cannot move beyond track list")?;
                if target != index {
                    let track = self.view.project.tracks.remove(index);
                    self.view.project.tracks.insert(target, track);
                    self.record_edit(Edit::Move {
                        id,
                        before: index,
                        after: target,
                    });
                }
            }
            Action::Rename(id, name) => self.rename(id, name)?,
            Action::EditClips { expected, edit } => self.edit_clips(&expected, &edit)?,
            Action::Audition {
                clip,
                channel,
                key,
                velocity,
            } => {
                self.refresh_transport();
                if self.history_available()
                    && let Some((track, _)) = self.view.project.midi.find_clip(clip)
                    && let Some(engine) = &self.engine
                {
                    engine.audition(track, channel, key, velocity);
                }
            }
            Action::ArrangementView(view) => {
                view.validate()?;
                if !self.view.project_open {
                    return Err("Open a project before changing its arrangement view.".into());
                }
                // View-only: saved with the next save or autosave, but never a reason for one.
                self.view.project.arrangement_view = view;
            }
            Action::PianoRollView(view) => {
                view.validate()?;
                if !self.view.project_open {
                    return Err("Open a project before changing its piano-roll view.".into());
                }
                self.view.project.piano_roll_view = view;
            }
            Action::TrackView { track, lanes } => {
                lanes.validate()?;
                if !self.view.project_open {
                    return Err("Open a project before changing its take lanes.".into());
                }
                let position = self
                    .view
                    .project
                    .tracks
                    .iter()
                    .position(|item| item.id == track)
                    .ok_or("track no longer exists")?;
                if self.view.project.tracks[position].take_lanes != lanes {
                    self.view.project.tracks[position].take_lanes = lanes;
                    self.mark_other_change();
                }
            }
            Action::CompEdit {
                expected,
                track,
                lane,
                edit,
            } => self.comp_edit(&expected, track, lane, edit)?,
            Action::TakeToNewTrack {
                expected,
                track,
                lane,
            } => self.take_to_new_track(&expected, track, lane)?,
            Action::SeekHistory { version, target } => self.seek_history(version, target)?,
            Action::Select(id, plugin) => {
                self.track(id)?;
                self.request_load(id, Some(plugin))?;
            }
            Action::Settings { audio, project } => {
                if self.runtimes.values().any(|r| r.loading) {
                    return Err("wait for instrument loading before reconfiguring".into());
                }
                audio.validate()?;
                let devices = Devices::enumerate()?;
                devices.validate(&audio)?;
                let name = devices
                    .audio
                    .iter()
                    .find(|device| device.id == audio.device)
                    .map(|device| device.name.clone())
                    .unwrap_or_default();
                self.stop()?;
                let override_audio = project.then(|| audio.clone());
                if !project {
                    self.preferences.write(audio, name)?;
                    self.view.audio_preferences = self.preferences.audio.clone();
                }
                if self.view.project.audio != override_audio {
                    self.view.project.audio = override_audio;
                    self.mark_other_change();
                }
                self.start_audio()?;
                if self.view.message == "Audio active." {
                    self.view.message = "Settings applied. Audio restarted.".into();
                }
            }
            Action::SetShortcuts(saved) => {
                let path = self
                    .shortcuts_path
                    .as_deref()
                    .ok_or("Clef's Application Support directory is unavailable.")?;
                shortcuts::write(path, &saved)
                    .map_err(|error| format!("Keyboard shortcuts were not saved: {error}"))?;
                self.view.shortcuts = Arc::new(saved);
            }
            Action::Editor(id, open) => {
                if self.track(id)?.instrument_offline {
                    return Err("Instrument track is offline. Bring it online first.".into());
                }
                if open {
                    self.select(Some(id))?;
                }
                if !self
                    .runtimes
                    .get(&id)
                    .is_some_and(|r| r.instrument.is_some())
                {
                    self.request_load(id, None)?;
                    self.runtimes.entry(id).or_default().pending_editor = open;
                    self.view.message = "Loading instrument editor…".into();
                } else {
                    self.set_track_editor(id, open)?;
                }
            }
            Action::Restart(id) => {
                self.track(id)?;
                if self.engine.is_some()
                    && self
                        .runtimes
                        .get(&id)
                        .is_some_and(|r| r.instrument.is_some())
                {
                    self.attach(id)?;
                } else {
                    self.request_load(id, None)?;
                }
            }
            Action::Reactivate(plugin) => {
                let entries = quarantine::update(&self.registry, &plugin, None)?;
                self.merge_persisted_quarantine(entries, Some(&plugin));
                self.view.message = "Plugin reactivated. Select a track or Retry to load it; audio state is unchanged.".into();
            }
            Action::Transport(action) => self.transport(action)?,
        }
        self.publish_mix();
        self.refresh();
        Ok(())
    }

    fn live_target(&self) -> Option<u64> {
        self.view.armed.or(self.view.selected)
    }

    fn edit_clips(&mut self, expected: &Arc<Arrangement>, edit: &ClipEdit) -> Result<()> {
        self.refresh_transport();
        if !self.history_available() {
            return Err(
                "Stop transport and wait for instrument loads before editing clips.".into(),
            );
        }
        let previous = &self.view.project.midi;
        if !Arc::ptr_eq(previous, expected) {
            return Err("The arrangement changed before the edit was applied.".into());
        }
        let track_ids = || self.view.project.tracks.iter().map(|track| track.id);
        let mut candidate = (**previous).clone();
        let selection = match candidate.edit(edit, track_ids())? {
            ClipEditResult::Applied(selection) => selection,
            ClipEditResult::Unchanged => return Ok(()),
            ClipEditResult::NeedsSplitConfirmation => {
                return Err("Confirm splitting the crossed MIDI notes.".into());
            }
        };
        let tracks = track_ids()
            .filter_map(|id| ClipChange::new(id, previous.clips(id), candidate.clips(id)))
            .collect();
        if let Some(engine) = &self.engine {
            engine.set_arrangement(&candidate, &self.view.project.timeline)?;
        }
        self.view.project.midi = Arc::new(candidate);
        self.view.clip_edit_selection = Some(selection);
        self.view.message = edit.label().into();
        self.record_edit(Edit::Clips {
            label: edit.label(),
            tracks,
        });
        Ok(())
    }

    fn comp_edit(
        &mut self,
        expected: &Arc<Arrangement>,
        track_id: u64,
        lane: usize,
        edit: CompEdit,
    ) -> Result<()> {
        self.refresh_transport();
        let can_comp = self.view.project_open
            && !self.view.recording
            && !self.view.transport.pending
            && matches!(
                self.view.transport.phase,
                TransportPhase::Stopped | TransportPhase::Playing
            )
            && self.runtimes.values().all(|runtime| !runtime.loading);
        if !can_comp {
            return Err("Wait for the transport and instrument loads before comping.".into());
        }
        let previous = &self.view.project.midi;
        if !Arc::ptr_eq(previous, expected) {
            return Err("The arrangement changed before the comp edit was applied.".into());
        }
        let track_ids = || self.view.project.tracks.iter().map(|track| track.id);
        let mut candidate = (**previous).clone();
        let selection = match candidate.comp(track_id, lane, edit, track_ids())? {
            ClipEditResult::Applied(selection) => selection,
            ClipEditResult::Unchanged => return Ok(()),
            ClipEditResult::NeedsSplitConfirmation => {
                return Err("Confirm splitting the crossed MIDI notes.".into());
            }
        };
        let tracks = track_ids()
            .filter_map(|id| ClipChange::new(id, previous.clips(id), candidate.clips(id)))
            .collect();
        if let Some(engine) = &self.engine {
            engine.set_arrangement(&candidate, &self.view.project.timeline)?;
        }
        self.view.project.midi = Arc::new(candidate);
        self.view.clip_edit_selection = Some(selection);
        self.view.message = edit.label().into();
        self.record_edit(Edit::Clips {
            label: edit.label(),
            tracks,
        });
        if matches!(edit, CompEdit::DeleteTake | CompEdit::Flatten)
            && let Some(track) = self
                .view
                .project
                .tracks
                .iter_mut()
                .find(|track| track.id == track_id)
        {
            if edit == CompEdit::DeleteTake {
                track.take_lanes.remove(lane);
            }
            if self
                .view
                .project
                .midi
                .clips(track.id)
                .iter()
                .map(|clip| clip.takes.len())
                .max()
                .unwrap_or(0)
                < 2
            {
                track.take_lanes = TakeLanes::default();
            }
            self.mark_other_change();
        }
        Ok(())
    }

    fn take_to_new_track(
        &mut self,
        expected: &Arc<Arrangement>,
        track: u64,
        lane: usize,
    ) -> Result<()> {
        self.refresh_transport();
        if !self.history_available() {
            return Err(
                "Stop transport and wait for instrument loads before copying a take.".into(),
            );
        }
        if !Arc::ptr_eq(&self.view.project.midi, expected) {
            return Err("The arrangement changed before the take was copied.".into());
        }
        let owner = self
            .view
            .project
            .tracks
            .iter()
            .position(|item| item.id == track)
            .ok_or("track no longer exists")?;
        let name = self.view.project.tracks[owner].take_lanes.name(lane);
        if self.view.project.tracks.len() >= crate::project::MAX_TRACKS {
            return Err("project track safety limit reached".into());
        }
        let mut candidate = self.view.project.clone();
        let id = candidate.next_id;
        let next = id.checked_add(1).ok_or("track ID exhausted")?;
        let clips = std::sync::Arc::make_mut(&mut candidate.midi).copy_take_clips(track, lane)?;
        let new_track = Track {
            colour: Some(clef_model::project::next_track_colour(
                candidate.tracks.len(),
            )),
            id,
            name,
            plugin: None,
            instrument_bypassed: false,
            instrument_offline: false,
            record_mode: TrackMode::Global,
            mix: Default::default(),
            inserts: Vec::new(),
            take_lanes: TakeLanes::default(),
            kind: clef_model::project::TrackKind::Instrument,
            output: None,
            sends: Vec::new(),
            state: Arc::default(),
        };
        candidate.tracks.insert(owner + 1, new_track.clone());
        candidate.next_id = next;
        std::sync::Arc::make_mut(&mut candidate.midi)
            .tracks
            .insert(id, clips.clone());
        candidate.validate()?;
        if let Some(engine) = &self.engine {
            engine.set_arrangement(&candidate.midi, &candidate.timeline)?;
            engine.clock.select(id);
        }
        self.view.project = candidate;
        self.view.selected = Some(id);
        self.record_edit(Edit::TakeToNewTrack {
            slot: Box::new(TrackSlot {
                track: new_track,
                index: owner + 1,
                clips,
            }),
        });
        self.view.message = "Take copied to a new track.".into();
        Ok(())
    }

    fn update_live_target(&self) {
        if let Some(engine) = &self.engine {
            engine.clock.select(self.live_target().unwrap_or(0));
        }
    }

    fn update_arrangement(&self) -> Result<()> {
        if let Some(engine) = &self.engine {
            engine.set_arrangement(&self.view.project.midi, &self.view.project.timeline)?;
        }
        Ok(())
    }

    fn ready(&self, id: u64) -> bool {
        self.runtimes.get(&id).is_some_and(|runtime| {
            !runtime.loading
                && runtime
                    .broker
                    .as_ref()
                    .is_some_and(|broker| !broker.stats.failed.load(Ordering::Acquire))
        })
    }

    fn record_unavailable(&self) -> Option<&'static str> {
        if self.engine.is_none() {
            Some("Audio is off. Choose an output in Audio/MIDI Settings to record.")
        } else if self.view.armed.is_none() {
            Some("Arm an instrument track to record.")
        } else if !self.view.armed.is_some_and(|id| self.ready(id)) {
            Some("The armed instrument is unavailable or still loading.")
        } else {
            None
        }
    }

    fn request_play(&mut self, record: bool) -> Result<()> {
        if self.engine.is_none() {
            return Err("Audio is off. Choose an output in Audio/MIDI Settings.".into());
        }
        if record && let Some(reason) = self.record_unavailable() {
            return Err(reason.into());
        }
        if self.view.transport.phase != TransportPhase::Stopped || self.deferred_play.is_some() {
            return Err("Stop transport before starting a new pass.".into());
        }
        if self.view.transport.tick >= MAX_TICK && !self.view.project.timeline.looping {
            return Err("Locate before the end of the timeline to start playback.".into());
        }
        let ids: Vec<_> = self.view.project.midi.tracks.keys().copied().collect();
        let mut warnings = Vec::new();
        for id in ids {
            let offline = self
                .view
                .project
                .tracks
                .iter()
                .find(|track| track.id == id)
                .is_some_and(|track| track.instrument_offline);
            if offline {
                continue;
            }
            if !self.ready(id)
                && !self
                    .runtimes
                    .get(&id)
                    .is_some_and(|runtime| runtime.loading)
                && let Err(error) = self.request_load(id, None)
            {
                warnings.push(format!("{}: {error}", self.track(id)?.name));
            }
        }
        if !warnings.is_empty() {
            self.view.message =
                format!("Some MIDI tracks are unavailable: {}", warnings.join("; "));
        }
        if self.play_waiting() {
            self.deferred_play = Some(record);
            self.view.recording = record;
            Ok(())
        } else {
            self.start_play(record)
        }
    }

    /// Start the audio engine with the effective configuration (T39): the project override,
    /// else the global preferences, else first-launch defaults, with the agreed fallbacks.
    pub(crate) fn start_audio(&mut self) -> Result<()> {
        if self.engine.is_some() {
            return Ok(());
        }
        let devices = Devices::enumerate()?;
        let resolved = audio::resolve(
            &devices,
            self.view.project.audio.as_ref(),
            self.preferences.audio.as_ref(),
            &self.preferences.device_name,
        )?;
        let settings = resolved.settings;
        if settings.rate != self.worker_rate
            && (!self.runtimes.is_empty() || !self.insert_workers.is_empty())
        {
            // Workers run at the engine rate: keep their state and reload them at the new one.
            let captured = self.capture(&[])?;
            if captured != self.view.project {
                self.view.project = captured;
                self.mark_other_change();
            }
            self.runtimes.clear();
            self.insert_workers.clear();
        }
        self.worker_rate = settings.rate;
        let selected = self.live_target();
        let device = clef_audio_io::output_by_id(&settings.device)?;
        let timing = Prepared::new(&self.view.project.timeline, settings.rate)?;
        self.silence.store(false, Ordering::Release);
        let engine = Engine::start(
            Configuration {
                device,
                pair: settings.pair,
                midi_index: None,
                midi_id: Some(settings.midi.clone()),
                rate: settings.rate,
                buffer: settings.buffer,
            },
            self.silence.clone(),
            timing,
            self.view.transport.tick,
        )?;
        self.engine = Some(engine);
        self.view.audio = Some(settings);
        self.volatile = Volatile::default();
        self.view.actual.clear();
        self.sync_instrument_bypasses(&self.view.project);
        if let Err(error) = self.reconcile_inserts(&self.view.project.clone(), true) {
            let _ = self.stop();
            return Err(error);
        }
        self.publish_mix();
        self.update_arrangement()?;
        self.view.active = true;
        let ids: Vec<_> = self.runtimes.keys().copied().collect();
        let mut failures = Vec::new();
        for track in ids {
            if let Err(e) = self.attach(track) {
                failures.push(e.to_string());
            }
        }
        if let Some(id) = selected {
            self.engine
                .as_ref()
                .ok_or("engine unavailable")?
                .clock
                .select(id);
            if self.track(id)?.plugin.is_some()
                && !self.track(id)?.instrument_offline
                && !self
                    .runtimes
                    .get(&id)
                    .is_some_and(|r| r.instrument.is_some() || r.loading)
            {
                self.request_load(id, None)?;
            }
        }
        failures.extend(self.preload_instruments());
        let mut message = resolved.notices;
        if !failures.is_empty() {
            message.push(format!("Some paths unavailable: {}", failures.join("; ")));
        }
        self.view.message = if message.is_empty() {
            "Audio active.".into()
        } else {
            message.join(" ")
        };
        Ok(())
    }

    /// T38 R1: request a background load for every instrument track that can play, so the
    /// first Play does not attach brokers and realign latency while the transport starts.
    /// Returns one message per track whose request was refused.
    fn preload_instruments(&mut self) -> Vec<String> {
        let eligible: Vec<_> = self
            .view
            .project
            .tracks
            .iter()
            .filter(|track| {
                track.kind == TrackKind::Instrument
                    && !track.instrument_offline
                    && track
                        .plugin
                        .as_ref()
                        .is_some_and(|plugin| !self.blocked(plugin) && plugin.path.exists())
                    && !self
                        .runtimes
                        .get(&track.id)
                        .is_some_and(|r| r.instrument.is_some() || r.loading)
            })
            .map(|track| (track.id, track.name.clone()))
            .collect();
        eligible
            .into_iter()
            .filter_map(|(id, name)| {
                self.request_load(id, None)
                    .err()
                    .map(|error| format!("{name}: {error}"))
            })
            .collect()
    }

    /// T38 R2: deferred Play waits while any online track is loading and until the engine
    /// has delivered the current latency alignment to every attached track.
    fn play_waiting(&self) -> bool {
        let loading = self.view.project.tracks.iter().any(|track| {
            !track.instrument_offline
                && self
                    .runtimes
                    .get(&track.id)
                    .is_some_and(|runtime| runtime.loading)
        });
        loading || self.engine.as_ref().is_some_and(|engine| !engine.aligned())
    }

    fn start_play(&mut self, record: bool) -> Result<()> {
        if !record {
            return self
                .engine
                .as_mut()
                .ok_or("audio unavailable")?
                .transport
                .send(TransportCommand::Play);
        }
        if let Some(reason) = self.record_unavailable() {
            return Err(reason.into());
        }
        let target = self.view.armed.ok_or("no armed instrument")?;
        let mode = self
            .track(target)?
            .record_mode
            .effective(self.view.project.record_mode);
        let engine = self.engine.as_mut().ok_or("audio unavailable")?;
        let map = SampleMap::new(&self.view.project.timeline, engine.rate)?;
        engine.mute_recording(target, mode != Mode::Merge);
        let baseline = match engine.begin_capture(target) {
            Ok(baseline) => baseline,
            Err(error) => {
                engine.mute_recording(target, false);
                return Err(error);
            }
        };
        self.view.recording = true;
        self.recording = Some(Capture {
            target,
            mode,
            baseline,
            map,
            input: Vec::with_capacity(recording::MAX_CAPTURE_MESSAGES),
            window: None,
            preview_key: None,
            preview: None,
            interruption: None,
            stopping: false,
        });
        Ok(())
    }

    fn finish_recording(&mut self, failure: Option<String>) -> Result<bool> {
        let Some(mut capture) = self.recording.take() else {
            return Ok(false);
        };
        self.view.recording = false;
        self.view.recording_region = None;
        self.view.recording_notes = None;
        let engine = self
            .engine
            .as_mut()
            .ok_or("audio unavailable while finalizing MIDI")?;
        engine.end_capture();
        let overflow = engine.drain_capture(&mut capture.input);
        engine.release_live(capture.target);
        engine.mute_recording(capture.target, false);
        let end = engine.recording_end();
        let window = capture.window.or_else(|| engine.recording_window());
        let mut reason = failure
            .or(capture.interruption)
            .or_else(|| overflow.then(|| "MIDI capture queue or recording limit exceeded".into()));
        let mut recovered = false;
        let mut retained_in_history = true;
        if let Some(window) = window {
            capture.input.sort_by_key(|event| event.at);
            let end = if overflow {
                capture
                    .input
                    .last()
                    .map_or(end, |event| end.min(event.at.saturating_add(1)))
            } else {
                end
            };
            let take =
                recording::assemble(&capture.map, window, end, capture.baseline, &capture.input)?;
            if take.truncated && reason.is_none() {
                reason = Some("MIDI recording exceeded the take or event limit".into());
            }
            let mut candidate = (*self.view.project.midi).clone();
            recovered = candidate.record(
                capture.target,
                capture.mode,
                take.range,
                take.passes,
                reason.is_some(),
            )?;
            if recovered {
                engine.set_arrangement(&candidate, &self.view.project.timeline)?;
                let edit = ClipChange::new(
                    capture.target,
                    self.view.project.midi.clips(capture.target),
                    candidate.clips(capture.target),
                );
                self.view.project.midi = Arc::new(candidate);
                if let Some(edit) = edit {
                    retained_in_history = self.record_edit(Edit::Midi(edit));
                }
                self.checkpoint_take = true;
            }
        }
        self.view.message = match (reason, recovered) {
            (Some(reason), true) => {
                format!("Recording interrupted: {reason}. Captured MIDI retained as new takes.")
            }
            (Some(reason), false) => format!("Recording interrupted: {reason}. No MIDI recovered."),
            (None, true) => "MIDI recording saved in the arrangement.".into(),
            (None, false) => "No MIDI recorded.".into(),
        };
        if !retained_in_history {
            self.view
                .message
                .push_str(" Edit exceeded 64 MiB; history cleared.");
        }
        Ok(true)
    }

    fn poll_recording(&mut self) -> Result<()> {
        let Some(target) = self.recording.as_ref().map(|capture| capture.target) else {
            return Ok(());
        };
        let failed = !self.ready(target);
        let capture = self.recording.as_mut().unwrap();
        let engine = self
            .engine
            .as_mut()
            .ok_or("audio unavailable while recording")?;
        let before = capture.input.len();
        let overflow = engine.drain_capture(&mut capture.input);
        if capture.input[before.saturating_sub(1)..]
            .windows(2)
            .any(|pair| pair[0].at > pair[1].at)
        {
            capture.input.sort_by_key(|event| event.at);
            capture.preview_key = None;
            capture.preview = None;
        }
        let timeline = &self.view.project.timeline;
        if capture.window.is_none() {
            capture.window = engine.recording_window();
        }
        let state = engine.transport.view();
        if !capture.stopping && (overflow || failed) {
            let reason = if failed {
                "armed instrument failed"
            } else {
                "MIDI capture queue or recording limit exceeded"
            };
            capture.interruption = Some(reason.into());
            engine.transport.send(TransportCommand::Stop)?;
            capture.stopping = true;
        }
        self.view.recording_region = capture.window.and_then(|window| {
            if state.phase != TransportPhase::Playing {
                return None;
            }
            let now = engine.recording_end();
            let (pass, frame) = window.position(now)?;
            let due = capture.input.partition_point(|event| event.at <= now);
            if capture.preview_key != Some((pass, due)) {
                capture.preview_key = Some((pass, due));
                let preview = match &mut capture.preview {
                    Some(preview) if preview.projection() == window => preview,
                    slot => slot.insert(recording::LivePreview::new(window, capture.baseline)),
                };
                self.view.recording_notes = Some(Arc::new(preview.update(
                    &capture.map,
                    pass,
                    &capture.input[..due],
                )));
            }
            let start = if timeline.looping {
                timeline.loop_range[0]
            } else {
                capture.map.tick_at(window.origin)
            };
            Some([start, capture.map.tick_at(frame).max(start)])
        });
        if state.phase == TransportPhase::Stopped && !engine.transport.view().pending {
            self.finish_recording(None)?;
        }
        Ok(())
    }

    fn transport(&mut self, action: TransportAction) -> Result<()> {
        use TransportAction::*;
        self.refresh_transport();
        if self.view.transport.pending && !matches!(action, Stop) {
            return Err("transport is waiting for the audio device".into());
        }
        if self.view.recording && !matches!(action, Stop | ToggleClick) {
            return Err("Stop recording before changing the transport.".into());
        }
        match action {
            Play => self.request_play(false)?,
            Record => self.request_play(true)?,
            Arm(id) => {
                let armed = (self.view.armed != Some(id)).then_some(id);
                if armed.is_some() {
                    self.track(id)?
                        .plugin
                        .as_ref()
                        .ok_or("choose an instrument first")?;
                    if !self
                        .runtimes
                        .get(&id)
                        .is_some_and(|runtime| runtime.loading || runtime.instrument.is_some())
                    {
                        self.request_load(id, None)?;
                    }
                }
                self.view.armed = armed;
                self.update_live_target();
            }
            SetRecordMode(mode) => {
                if self.view.project.record_mode != mode {
                    if self.view.recording
                        || self.view.transport.pending
                        || self.runtimes.values().any(|runtime| runtime.loading)
                    {
                        return Err("Wait for the transport and instrument loads before changing the record mode.".into());
                    }
                    let before = self.view.project.record_mode;
                    self.view.project.record_mode = mode;
                    self.record_edit(Edit::RecordMode {
                        before,
                        after: mode,
                    });
                    self.view.message = format!("Record mode: {}", mode.label());
                }
            }
            SetTrackRecordMode(id, mode) => {
                let position = self
                    .view
                    .project
                    .tracks
                    .iter()
                    .position(|track| track.id == id)
                    .ok_or("track no longer exists")?;
                let before = self.view.project.tracks[position].record_mode;
                if before != mode {
                    if self.view.recording
                        || self.view.transport.pending
                        || self.runtimes.values().any(|runtime| runtime.loading)
                    {
                        return Err("Wait for the transport and instrument loads before changing the record mode.".into());
                    }
                    self.view.project.tracks[position].record_mode = mode;
                    self.record_edit(Edit::TrackMode {
                        id,
                        before,
                        after: mode,
                    });
                    self.view.message = format!("Track record mode: {}", mode.label());
                }
            }
            Stop => {
                if self.deferred_play.take().is_some() {
                    self.view.recording = false;
                } else if let Some(engine) = &mut self.engine {
                    engine.transport.send(TransportCommand::Stop)?;
                    if let Some(capture) = &mut self.recording {
                        capture.stopping = true;
                    }
                }
            }
            Locate(tick) if tick <= MAX_TICK => {
                if let Some(engine) = &mut self.engine {
                    engine.transport.send(TransportCommand::Locate(tick))?;
                } else {
                    self.view.transport.tick = tick;
                    self.view.transport.audible_tick = tick;
                }
            }
            Locate(_) => return Err("position exceeds timeline limit".into()),
            edit => {
                if self.view.transport.phase != TransportPhase::Stopped
                    && matches!(edit, SetTempo(_) | RemoveTempo(_) | SetLoopRange(_))
                {
                    return Err("stop playback before editing the tempo map or loop range".into());
                }
                let mut timeline = self.view.project.timeline.clone();
                match edit {
                    SetTempo(tempo) => {
                        match timeline.changes.binary_search_by_key(&tempo.bar, |t| t.bar) {
                            Ok(index) => timeline.changes[index] = tempo,
                            Err(index) => timeline.changes.insert(index, tempo),
                        }
                    }
                    RemoveTempo(bar) => timeline.changes.retain(|t| t.bar != bar),
                    SetLoopRange(range) => timeline.loop_range = range,
                    ToggleLoop => timeline.looping = !timeline.looping,
                    ToggleClick => timeline.click = !timeline.click,
                    ToggleCountIn => timeline.count_in = !timeline.count_in,
                    SetStopMode(mode) => timeline.stop_mode = mode,
                    _ => unreachable!(),
                }
                if timeline != self.view.project.timeline {
                    timeline.validate()?;
                    if let Some(engine) = &mut self.engine {
                        engine
                            .transport
                            .settings(Prepared::new(&timeline, engine.rate)?)?;
                        if matches!(edit, SetTempo(_) | RemoveTempo(_)) {
                            engine.set_arrangement(&self.view.project.midi, &timeline)?;
                        }
                    }
                    let previous = std::mem::replace(&mut self.view.project.timeline, timeline);
                    match edit {
                        SetTempo(_) | RemoveTempo(_) => {
                            self.record_edit(Edit::Tempo {
                                before: previous.changes,
                                after: self.view.project.timeline.changes.clone(),
                            });
                        }
                        SetLoopRange(_) => {
                            self.record_edit(Edit::Loop {
                                before: previous.loop_range,
                                after: self.view.project.timeline.loop_range,
                            });
                        }
                        _ => self.mark_other_change(),
                    }
                }
            }
        }
        Ok(())
    }

    fn refresh_transport(&mut self) {
        if let Some(engine) = &self.engine {
            self.view.transport = engine.transport.view();
            if self.deferred_play.is_some() {
                self.view.transport.pending = true;
                self.view.transport.play_pending = true;
            }
        }
    }
    fn add_bus(&mut self, assign: Option<u64>, send: Option<u64>) -> Result<()> {
        if !self.view.project_open {
            return Err("Open a project before adding a bus.".into());
        }
        if self.view.project.tracks.len() >= crate::project::MAX_TRACKS {
            return Err("project track safety limit reached".into());
        }
        let id = self.view.project.next_id;
        let send_id = id.checked_add(1).ok_or("track ID exhausted")?;
        let next = if send.is_some() {
            send_id.checked_add(1).ok_or("track ID exhausted")?
        } else {
            send_id
        };
        let count = self
            .view
            .project
            .tracks
            .iter()
            .filter(|track| track.is_bus())
            .count()
            + 1;
        let bus = Track {
            colour: Some(clef_model::project::next_track_colour(
                self.view.project.tracks.len(),
            )),
            id,
            name: format!("Bus {count}"),
            plugin: None,
            instrument_bypassed: false,
            instrument_offline: false,
            record_mode: TrackMode::Global,
            mix: Default::default(),
            inserts: Vec::new(),
            take_lanes: TakeLanes::default(),
            kind: TrackKind::Bus,
            output: None,
            sends: Vec::new(),
            state: Arc::default(),
        };
        let mut candidate = self.view.project.clone();
        let index = candidate.tracks.len();
        candidate.tracks.push(bus.clone());
        let source = if let Some(source_id) = assign {
            let source_index = candidate
                .tracks
                .iter()
                .position(|track| track.id == source_id && track.is_instrument())
                .ok_or("Track is unavailable.")?;
            let before = candidate.tracks[source_index].output;
            candidate.tracks[source_index].output = Some(id);
            Some(SourceRoute {
                track: source_id,
                before,
                after: Some(id),
            })
        } else {
            None
        };
        let send_change = if let Some(source_id) = send {
            let source_index = candidate
                .tracks
                .iter()
                .position(|track| track.id == source_id && track.is_instrument())
                .ok_or("Track is unavailable.")?;
            let before = candidate.tracks[source_index].sends.clone();
            candidate.tracks[source_index].sends.push(TrackSend {
                id: send_id,
                target: id,
                level: clef_model::mixer::Control::Fader.bounds().0,
                pre: false,
                enabled: true,
            });
            let after = candidate.tracks[source_index].sends.clone();
            Some(SourceSend {
                track: source_id,
                before,
                after,
            })
        } else {
            None
        };
        candidate.next_id = next;
        candidate.validate()?;
        self.view.project = candidate;
        if source.is_some() || send_change.is_some() {
            self.record_edit(Edit::Bus {
                slot: Box::new(TrackSlot {
                    track: bus,
                    index,
                    clips: Vec::new(),
                }),
                source,
                send: send_change,
            });
        } else {
            self.record_edit(Edit::Track {
                slot: Box::new(TrackSlot {
                    track: bus,
                    index,
                    clips: Vec::new(),
                }),
                added: true,
            });
        }
        self.select(Some(send.unwrap_or(id)))?;
        self.publish_mix();
        Ok(())
    }

    fn set_output(&mut self, epoch: u64, track: u64, output: Option<u64>) -> Result<()> {
        if epoch != self.view.project_epoch || !self.view.project_open {
            return Err("The mixer channel is no longer available.".into());
        }
        let index = self
            .view
            .project
            .tracks
            .iter()
            .position(|candidate| candidate.id == track && candidate.is_instrument())
            .ok_or("Mixer channel is unavailable.")?;
        if let Some(bus) = output
            && self.view.project.bus(bus).is_none()
        {
            return Err("Output bus is unavailable.".into());
        }
        let before = self.view.project.tracks[index].output;
        if before == output {
            return Ok(());
        }
        let mut candidate = self.view.project.clone();
        candidate.tracks[index].output = output;
        candidate.validate()?;
        self.view.project = candidate;
        self.record_edit(Edit::Output {
            track,
            before,
            after: output,
        });
        self.publish_mix();
        Ok(())
    }

    fn edit_send(&mut self, epoch: u64, track: u64, edit: SendEdit) -> Result<()> {
        // Any send edit ends an earlier preview; a new preview replaces it.
        let cleared = self.send_preview.take().is_some();
        let result = self.apply_send_edit(epoch, track, edit);
        if cleared && result.is_err() {
            self.publish_mix();
        }
        result
    }

    fn apply_send_edit(&mut self, epoch: u64, track: u64, edit: SendEdit) -> Result<()> {
        if epoch != self.view.project_epoch || !self.view.project_open {
            return Err("The mixer channel is no longer available.".into());
        }
        let index = self
            .view
            .project
            .tracks
            .iter()
            .position(|candidate| candidate.id == track && candidate.is_instrument())
            .ok_or("Mixer channel is unavailable.")?;
        if let SendEdit::PreviewLevel { id, level } = edit {
            if !self.view.project.tracks[index]
                .sends
                .iter()
                .any(|send| send.id == id)
            {
                return Err("Send is no longer available.".into());
            }
            if !clef_model::mixer::Control::Fader.accepts(level) {
                return Err("Send level is outside its supported range.".into());
            }
            self.send_preview = Some(SendPreview {
                epoch,
                track,
                id,
                level,
            });
            self.publish_mix();
            return Ok(());
        }
        if edit == SendEdit::CancelPreview {
            self.publish_mix();
            return Ok(());
        }
        if let SendEdit::CommitLevel { id, level } = edit {
            if !clef_model::mixer::Control::Fader.accepts(level) {
                return Err("Send level is outside its supported range.".into());
            }
            let before = self.view.project.tracks[index].sends.clone();
            let mut candidate = self.view.project.clone();
            candidate.tracks[index]
                .sends
                .iter_mut()
                .find(|send| send.id == id)
                .ok_or("Send is no longer available.")?
                .level = level;
            if candidate.tracks[index].sends == before {
                return Ok(());
            }
            candidate.validate()?;
            self.view.project = candidate;
            self.record_edit(Edit::Sends {
                track,
                before,
                after: self.view.project.tracks[index].sends.clone(),
            });
            self.publish_mix();
            return Ok(());
        }
        let mut candidate = self.view.project.clone();
        let before = candidate.tracks[index].sends.clone();
        match edit {
            SendEdit::Add { target } => {
                if candidate.bus(target).is_none() {
                    return Err("Send destination bus is unavailable.".into());
                }
                if candidate.tracks[index]
                    .sends
                    .iter()
                    .any(|send| send.target == target)
                {
                    return Err("This track already sends to that bus.".into());
                }
                let id = candidate.next_id;
                candidate.next_id = id.checked_add(1).ok_or("send ID exhausted")?;
                candidate.tracks[index].sends.push(TrackSend {
                    id,
                    target,
                    level: clef_model::mixer::Control::Fader.bounds().0,
                    pre: false,
                    enabled: true,
                });
            }
            SendEdit::Remove { id } => {
                let position = candidate.tracks[index]
                    .sends
                    .iter()
                    .position(|send| send.id == id)
                    .ok_or("Send is no longer available.")?;
                candidate.tracks[index].sends.remove(position);
            }
            SendEdit::SetTarget { id, target } => {
                if candidate.bus(target).is_none() {
                    return Err("Send destination bus is unavailable.".into());
                }
                if candidate.tracks[index]
                    .sends
                    .iter()
                    .any(|send| send.id != id && send.target == target)
                {
                    return Err("This track already sends to that bus.".into());
                }
                candidate.tracks[index]
                    .sends
                    .iter_mut()
                    .find(|send| send.id == id)
                    .ok_or("Send is no longer available.")?
                    .target = target;
            }
            SendEdit::SetPre { id, pre } => {
                candidate.tracks[index]
                    .sends
                    .iter_mut()
                    .find(|send| send.id == id)
                    .ok_or("Send is no longer available.")?
                    .pre = pre;
            }
            SendEdit::SetEnabled { id, enabled } => {
                candidate.tracks[index]
                    .sends
                    .iter_mut()
                    .find(|send| send.id == id)
                    .ok_or("Send is no longer available.")?
                    .enabled = enabled;
            }
            SendEdit::PreviewLevel { .. }
            | SendEdit::CancelPreview
            | SendEdit::CommitLevel { .. } => {
                return Ok(());
            }
        }
        if candidate.tracks[index].sends == before {
            return Ok(());
        }
        candidate.validate()?;
        self.view.project = candidate;
        self.record_edit(Edit::Sends {
            track,
            before,
            after: self.view.project.tracks[index].sends.clone(),
        });
        self.publish_mix();
        Ok(())
    }

    fn rename(&mut self, id: u64, name: String) -> Result<()> {
        let mut candidate = self.view.project.clone();
        let track = candidate
            .tracks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or("track no longer exists")?;
        if track.name == name {
            return Ok(());
        }
        let before = std::mem::replace(&mut track.name, name.clone());
        candidate.validate()?;
        self.view.project = candidate;
        self.record_edit(Edit::Rename {
            id,
            before,
            after: name,
        });
        Ok(())
    }
    fn select(&mut self, id: Option<u64>) -> Result<()> {
        if self.view.selected != id {
            self.close_editors()?;
            self.view.selected = id;
            self.update_live_target();
        }
        Ok(())
    }
    fn attach(&mut self, id: u64) -> Result<()> {
        let bypass = self
            .view
            .project
            .tracks
            .iter()
            .find(|track| track.id == id)
            .is_some_and(|track| track.instrument_bypassed);
        if let Some(runtime) = self.runtimes.get_mut(&id)
            && runtime.broker.is_none()
            && let (Some(engine), Some(instrument)) =
                (&mut self.engine, runtime.instrument.as_mut())
        {
            match engine.attach(id, instrument) {
                Ok(broker) => {
                    engine.set_instrument_bypass(id, bypass);
                    runtime.broker = Some(broker);
                    runtime.error.clear();
                }
                Err(error) => {
                    runtime.error = error.to_string();
                    return Err(error);
                }
            }
            runtime.touched = true;
            self.mark_other_change();
        }
        Ok(())
    }
    fn poll_jobs(&mut self) {
        let mut index = 0;
        while index < self.jobs.len() {
            let result = match self.jobs[index].receive.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => {
                    index += 1;
                    continue;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    Err("instrument load thread exited unexpectedly".into())
                }
            };
            let mut job = self.jobs.remove(index);
            if let Some(thread) = job.thread.take() {
                let _ = thread.join();
            }
            if job.cancel.load(Ordering::Acquire)
                || job.generation != self.generation
                || self.track(job.id).is_err()
                || !self
                    .runtimes
                    .get(&job.id)
                    .is_some_and(|r| r.request == job.request)
            {
                continue;
            }
            self.runtimes.entry(job.id).or_default().loading = false;
            match result {
                Ok(mut loaded) => {
                    // Audio may have been enabled while a stopped-session load ran.
                    // Continue activation on the track job rather than blocking the
                    // action loop or touching another track's processing path.
                    if self.engine.is_some() && !loaded.prepared {
                        let (tx, rx) = mpsc::channel();
                        job.receive = rx;
                        let cancel = job.cancel.clone();
                        job.thread = Some(std::thread::spawn(move || {
                            let result = (|| -> Result<Loaded> {
                                if cancel.load(Ordering::Acquire) {
                                    return Err("load cancelled".into());
                                }
                                loaded.instrument.start(false)?;
                                loaded.prepared = true;
                                if cancel.load(Ordering::Acquire) {
                                    return Err("load cancelled".into());
                                }
                                Ok(loaded)
                            })();
                            let _ = tx.send(result.map_err(|e| e.to_string()));
                        }));
                        self.runtimes.entry(job.id).or_default().loading = true;
                        self.jobs.push(job);
                        continue;
                    }
                    if self.blocked(&job.plugin) {
                        if self.live_target() == Some(job.id) {
                            self.update_live_target();
                        }
                        continue;
                    }
                    let previous = if job.replacement {
                        let capture = (|| -> Result<_> {
                            if self
                                .runtimes
                                .get(&job.id)
                                .is_some_and(|runtime| runtime.editor)
                            {
                                self.set_track_editor(job.id, false)?;
                            }
                            Ok(InstrumentState {
                                state: self.capture_track_state(job.id)?,
                                plugin: self.track(job.id)?.plugin.clone(),
                            })
                        })();
                        match capture {
                            Ok(state) => Some(state),
                            Err(error) => {
                                self.update_live_target();
                                self.view.message =
                                    format!("Instrument replacement canceled: {error}");
                                continue;
                            }
                        }
                    } else {
                        None
                    };
                    let runtime = self.runtimes.entry(job.id).or_default();
                    if let (Some(engine), Some(broker), Some(instrument)) = (
                        &mut self.engine,
                        runtime.broker.take(),
                        runtime.instrument.as_mut(),
                    ) {
                        let _ = engine.detach(broker, instrument);
                    }
                    runtime.instrument = Some(loaded.instrument);
                    runtime.editor = false;
                    runtime.error.clear();
                    let track = self
                        .view
                        .project
                        .tracks
                        .iter_mut()
                        .find(|t| t.id == job.id)
                        .expect("checked track");
                    if let Some(before) = previous {
                        track.plugin = Some(job.plugin);
                        track.state = loaded.initial;
                        runtime.lost = false;
                        runtime.touched = false;
                        let after = InstrumentState::from(&*track);
                        if before.plugin != after.plugin || before.state != after.state {
                            self.record_edit(Edit::Instrument {
                                id: job.id,
                                before,
                                after,
                                editor: false,
                            });
                        }
                    } else if track.state.is_empty() {
                        track.state = loaded.initial;
                    }
                    if let Err(e) = self.attach(job.id) {
                        self.view.message = e.to_string();
                    }
                    if self.live_target() == Some(job.id) {
                        self.update_live_target();
                    }
                    if self.view.selected == Some(job.id)
                        && self.runtimes.get(&job.id).is_some_and(|r| r.pending_editor)
                        && let Err(e) = self.apply(Action::Editor(job.id, true))
                    {
                        self.view.message = e.to_string();
                    }
                }
                Err(error) => {
                    let fatal = error.contains("worker exited")
                        || error.contains("timed out")
                        || error.contains("invalid worker")
                        || error.contains("thread exited");
                    self.runtimes.entry(job.id).or_default().error = error.clone();
                    if fatal {
                        self.quarantine(job.plugin, error);
                    } else {
                        self.view.message =
                            format!("Instrument unavailable; retained state preserved: {error}");
                    }
                    if self.live_target() == Some(job.id) {
                        self.update_live_target();
                    }
                }
            }
        }
    }
    fn refresh(&mut self) {
        let mut volatile = std::mem::take(&mut self.volatile);
        let due = volatile.due(Instant::now());
        self.refresh_insert_views();
        self.refresh_transport();
        self.view.edit_history = self.edits.view(self.history_available());
        self.view.view_unsaved = self.view_unsaved();
        self.view.record_unavailable = self.record_unavailable();
        let missing: HashMap<u64, bool> = self
            .view
            .project
            .tracks
            .iter()
            .filter_map(|track| {
                let plugin = track.plugin.as_ref()?;
                Some((track.id, !self.paths.exists(&plugin.path)))
            })
            .collect();
        volatile.tracks.retain(|id, _| {
            self.runtimes
                .get(id)
                .is_some_and(|runtime| runtime.broker.is_some())
        });
        self.view.tracks = self.view.project.tracks.iter().map(|track| {
            let runtime = self.runtimes.get(&track.id);
            let missing = missing.get(&track.id).copied().unwrap_or(false);
            let status = if track.instrument_offline { "Offline" }
                else if track.plugin.is_none() { "Empty" }
                else if track.plugin.as_ref().is_some_and(|p| self.blocked(p)) { "Quarantined" }
                else if missing { "Missing" }
                else if runtime.is_some_and(|r| r.loading) { "Loading" }
                else if self.engine.is_some() && runtime.is_some_and(|r| r.instrument.is_some() && r.broker.is_none()) { "Unavailable" }
                else if runtime.is_some_and(|r| r.instrument.is_some()) { "Ready" }
                else if runtime.is_some_and(|r| !r.error.is_empty()) { "Unavailable" } else { "Not loaded" };
            let diagnostics = runtime.map(|r| {
                if let Some(broker) = &r.broker {
                    volatile.track(due, track.id, || {
                    let s = &broker.stats;
                    let error = s
                        .error
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .clone()
                        .unwrap_or_default();
                    format!("latency {} samples · processed {} blocks · missed {} blocks · transition {} frames · stale {} blocks · MIDI {} / dropped {} · playback overflows {} · max RPC {} us · max worker {} us{}",
                        s.latency.load(Ordering::Relaxed),s.processed.load(Ordering::Relaxed),s.misses.load(Ordering::Relaxed),s.transition_frames.load(Ordering::Relaxed),s.stale.load(Ordering::Relaxed),s.midi_received.load(Ordering::Relaxed),s.midi_dropped.load(Ordering::Relaxed),s.playback_dropped.load(Ordering::Relaxed),s.max_rpc_us.load(Ordering::Relaxed),s.max_worker_us.load(Ordering::Relaxed), if error.is_empty() { String::new() } else { format!(" · {error}") })
                    })
                } else { r.error.clone() }
            }).unwrap_or_default();
            TrackView { id: track.id, status: status.into(), missing, diagnostics, loading: runtime.is_some_and(|r| r.loading), lost: runtime.is_some_and(|r| r.lost), loss_version: runtime.map_or(0,|r|r.request) }
        }).collect();
        let selected = self.view.selected.and_then(|id| self.runtimes.get(&id));
        self.view.loaded = selected.is_some_and(|r| r.instrument.is_some());
        self.view.editor = selected.is_some_and(|r| r.editor);
        self.view.failed = self.view.selected.is_some_and(|id| {
            self.view.state(id).is_some_and(|s| {
                matches!(s.status.as_str(), "Quarantined" | "Missing" | "Unavailable")
            })
        });
        self.view.quarantine_error = self.registry_error.clone();
        if let Some(engine) = &self.engine
            && (due || self.view.actual.is_empty())
        {
            self.view.actual = engine.summary();
        }
        self.volatile = volatile;
    }
    fn poll(&mut self) {
        // T39: audio starts by itself once a project is open; a failure waits for the next
        // apply, open or launch.
        if self.auto_audio
            && self.audio_pending
            && self.view.project_open
            && self.engine.is_none()
            && !self.view.recording
        {
            self.audio_pending = false;
            if let Err(error) = self.start_audio() {
                self.view.message = error.to_string();
            }
        }
        self.poll_inserts();
        let closed: Vec<_> = self
            .runtimes
            .iter_mut()
            .filter_map(|(&id, runtime)| {
                (runtime.editor
                    && runtime
                        .instrument
                        .as_mut()
                        .is_some_and(Instrument::editor_closed))
                .then_some(id)
            })
            .collect();
        for id in closed {
            if let Err(error) = self.finish_track_editor(id) {
                self.view.message = error.to_string();
            }
        }
        self.poll_jobs();
        if let Some(record) = self.deferred_play.filter(|_| !self.play_waiting()) {
            self.deferred_play = None;
            if let Err(error) = self.start_play(record) {
                self.view.recording = false;
                self.view.message = error.to_string();
            }
        }
        let mut faults = Vec::new();
        for (id, runtime) in &mut self.runtimes {
            let broker_failed = runtime
                .broker
                .as_ref()
                .is_some_and(|b| b.stats.failed.load(Ordering::Acquire));
            let worker_exited = runtime.instrument.as_mut().is_some_and(|i| i.exited());
            if (broker_failed || worker_exited)
                && let Some(plugin) = self
                    .view
                    .project
                    .tracks
                    .iter()
                    .find(|t| t.id == *id)
                    .and_then(|t| t.plugin.clone())
            {
                let detail = runtime
                    .broker
                    .as_ref()
                    .and_then(|broker| {
                        broker
                            .stats
                            .error
                            .lock()
                            .unwrap_or_else(|error| error.into_inner())
                            .clone()
                    })
                    .or_else(|| (!runtime.error.is_empty()).then(|| runtime.error.clone()))
                    .unwrap_or_else(|| "worker stopped responding".into());
                faults.push((plugin, detail));
            }
        }
        for (plugin, reason) in faults {
            self.quarantine(plugin, reason);
        }
        self.paths.rescan(
            Instant::now(),
            self.view
                .project
                .tracks
                .iter()
                .filter_map(|track| track.plugin.as_ref())
                .chain(
                    self.view
                        .project
                        .tracks
                        .iter()
                        .flat_map(|track| &track.inserts)
                        .chain(&self.view.project.master_inserts)
                        .map(|insert| &insert.plugin),
                )
                .map(|plugin| plugin.path.as_path()),
        );
        if self.engine.is_some() && self.last_devices.elapsed() > RESCAN_INTERVAL {
            self.last_devices = Instant::now();
            let active = self.view.audio.clone().unwrap_or_default();
            let devices = Devices::enumerate();
            if let Ok(devices) = &devices
                && !active.midi.is_empty()
                && !devices.midi.iter().any(|(id, _)| id == &active.midi)
                && let Some(audio) = &mut self.view.audio
            {
                // MIDI is optional (T39): report the loss once and keep audio running.
                audio.midi.clear();
                self.view.message =
                    "MIDI input unavailable; choose one in Audio/MIDI Settings.".into();
            }
            if let Err(e) = devices.and_then(|d| {
                d.validate(&AudioSettings {
                    midi: String::new(),
                    ..active
                })
            }) {
                match self.finish_recording(Some(e.to_string())) {
                    Ok(false) => {
                        self.view.message = format!(
                            "Audio stopped: {e}. Apply Audio/MIDI Settings to restart audio."
                        )
                    }
                    Err(error) => self.view.message = error.to_string(),
                    Ok(true) => {}
                }
                let _ = self.stop();
            }
        }
        if self
            .engine
            .as_ref()
            .is_some_and(|e| e.clock.errors.load(Ordering::Relaxed) != 0)
        {
            match self.finish_recording(Some("audio device error".into())) {
                Ok(false) => self.view.message =
                    "Audio device error. Settings retained; apply Audio/MIDI Settings to restart audio.".into(),
                Err(error) => self.view.message = error.to_string(),
                Ok(true) => {}
            }
            let _ = self.stop();
        }
        if let Err(error) = self.poll_recording() {
            self.view.message = error.to_string();
        }
        if let Some(engine) = &mut self.engine {
            engine.flush_mix();
        }
        self.update_meters();
        self.refresh();
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.finish_checkpoint();
        self.unload();
        for job in &mut self.jobs {
            if let Some(thread) = job.thread.take() {
                let _ = thread.join();
            }
        }
    }
}
pub struct Backend {
    pub silence: Arc<AtomicBool>,
    pub send: mpsc::Sender<Action>,
    pub receive: mpsc::Receiver<(Snapshot, Option<bool>)>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Backend {
    pub fn spawn(executable: PathBuf, notify: impl Fn() + Send + 'static) -> Self {
        let (send, actions) = mpsc::channel();
        let (updates, receive) = mpsc::channel();
        let silence = Arc::new(AtomicBool::new(true));
        let stopping = silence.clone();
        let thread = std::thread::spawn(move || {
            let mut session = Session::new(executable);
            session.silence = stopping;
            session.load_history();
            let mut last_view = None;
            loop {
                let mut success = None;
                match actions.recv_timeout(Duration::from_millis(30)) {
                    Ok(Action::Shutdown) => break,
                    Ok(action) => {
                        let acknowledge = !matches!(
                            action,
                            Action::PreviewMix(_)
                                | Action::Send {
                                    edit: SendEdit::PreviewLevel { .. } | SendEdit::CancelPreview,
                                    ..
                                }
                        );
                        let result = session.apply(action);
                        if let Err(ref error) = result {
                            session.view.message = error.to_string();
                        }
                        success = acknowledge.then(|| result.is_ok());
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                session.poll();
                session.poll_autosave(Instant::now());
                let view = session.view.for_ui();
                if success.is_none() && last_view.as_ref() == Some(&view) {
                    continue;
                }
                if updates.send((view.clone(), success)).is_err() {
                    break;
                }
                last_view = Some(view);
                notify();
            }
        });
        Self {
            silence,
            send,
            receive,
            thread: Some(thread),
        }
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        let _ = self.send.send(Action::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    /// The output device saved by the T03 GUI fixture (a version 4 project, whose audio block
    /// Clef no longer reads since T39).
    fn saved_output(root: &std::path::Path) -> AudioSettings {
        let bytes = std::fs::read(root.join("validation-output/T03-GUI-Pianoverse.clef")).unwrap();
        let length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let metadata: serde_json::Value = serde_json::from_slice(&bytes[16..16 + length]).unwrap();
        serde_json::from_value(metadata["audio"].clone()).unwrap()
    }
    use super::*;

    fn seek(session: &mut Session, target: usize) -> Result<()> {
        session.apply(Action::SeekHistory {
            version: session.view.edit_history.version,
            target,
        })
    }

    #[test]
    fn mixer_preview_is_transient_and_commit_has_one_undo_entry() {
        use clef_model::mixer::{Control, Target};
        let (_directory, mut session) = isolated();
        session.apply(Action::New).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        let target = Target::Track(1);
        let initial = session.view.edit_history.cursor;
        let change = MixChange {
            epoch: session.view.project_epoch,
            target,
            control: Control::Gain,
            value: 80,
        };
        session.apply(Action::PreviewMix(Some(change))).unwrap();
        session
            .apply(Action::PreviewMix(Some(MixChange {
                value: 100,
                ..change
            })))
            .unwrap();
        assert_eq!(session.view.project.mix(target).unwrap().trim, 0);
        assert_eq!(session.view.edit_history.cursor, initial);
        session.apply(Action::PreviewMix(None)).unwrap();
        session.apply(Action::CommitMix(change)).unwrap();
        assert_eq!(session.view.project.mix(target).unwrap().trim, 80);
        assert_eq!(session.view.edit_history.cursor, initial + 1);
        seek(&mut session, initial).unwrap();
        assert_eq!(session.view.project.mix(target).unwrap().trim, 0);
        seek(&mut session, initial + 1).unwrap();
        assert_eq!(session.view.project.mix(target).unwrap().trim, 80);
        assert!(
            session
                .apply(Action::CommitMix(MixChange {
                    epoch: change.epoch.wrapping_add(1),
                    value: 0,
                    ..change
                }))
                .is_err()
        );
        assert_eq!(session.view.project.mix(target).unwrap().trim, 80);
    }

    #[test]
    fn arrangement_edit_history_is_atomic_and_view_settings_round_trip() {
        let (directory, mut session) = isolated();
        session.apply(Action::New).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        let apply = |session: &mut Session, edit| {
            session.apply(Action::EditClips {
                expected: session.view.project.midi.clone(),
                edit,
            })
        };
        apply(
            &mut session,
            ClipEdit::Create {
                track: 1,
                start: 0,
                end: 960,
            },
        )
        .unwrap();
        let id = session.view.clip_edit_selection.as_ref().unwrap()[0];
        let settings = ArrangementView {
            track_height: 44,
            snap: clef_model::midi::Snap::Sixteenth,
            offset: 240,
            zoom_milli: 72_000,
        };
        session.apply(Action::ArrangementView(settings)).unwrap();
        let path = directory.0.join("Arrangement.clef");
        save(&mut session, path.clone(), false, false).unwrap();
        let saved_cursor = session.view.edit_history.cursor;
        let old = session.view.project.midi.clone();
        apply(
            &mut session,
            ClipEdit::Move {
                destinations: vec![(id, 2)],
                delta: 240,
            },
        )
        .unwrap();
        assert!(
            session
                .apply(Action::EditClips {
                    expected: old,
                    edit: ClipEdit::Delete(vec![id])
                })
                .is_err()
        );
        assert_eq!(session.view.edit_history.labels.last(), Some(&"Move Clips"));
        assert_eq!(session.view.project.midi.find_clip(id).unwrap().0, 2);
        seek(&mut session, saved_cursor).unwrap();
        assert!(!session.view.dirty);
        assert_eq!(session.view.project.midi.find_clip(id).unwrap().0, 1);
        seek(&mut session, saved_cursor + 1).unwrap();
        assert_eq!(session.view.project.midi.find_clip(id).unwrap().0, 2);
        session.apply(Action::Open(path)).unwrap();
        assert_eq!(session.view.project.arrangement_view, settings);
        assert_eq!(session.view.project.midi.find_clip(id).unwrap().0, 1);
    }

    #[test]
    fn history_seeks_across_track_edits_and_preserves_saved_revision() {
        let (directory, mut session) = isolated();
        session.apply(Action::New).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        session
            .apply(Action::AddTrack(Some((1, "Strings".into()))))
            .unwrap();
        let saved = directory.0.join("Saved.clef");
        save(&mut session, saved.clone(), false, false).unwrap();
        let at_save = session.view.edit_history.cursor;
        assert_eq!(
            session.view.edit_history.labels,
            ["Add Track", "Rename Track", "Add Track"]
        );
        session.apply(Action::MoveTrack(2, -1)).unwrap();
        session.apply(Action::RemoveTrack(1)).unwrap();

        seek(&mut session, at_save).unwrap();
        assert_eq!(
            session.view.project.tracks,
            Project::read(&saved).unwrap().tracks
        );
        assert!(!session.view.dirty);
        let stale = session.view.edit_history.version;
        let copy = directory.0.join("Copy.clef");
        save(&mut session, copy.clone(), false, false).unwrap();
        assert_eq!(session.view.edit_history.labels.len(), 5);
        seek(&mut session, 5).unwrap();
        assert_eq!(
            session
                .view
                .project
                .tracks
                .iter()
                .map(|track| track.id)
                .collect::<Vec<_>>(),
            [2]
        );
        assert!(session.view.dirty);
        assert!(
            session
                .apply(Action::SeekHistory {
                    version: stale,
                    target: at_save
                })
                .is_err()
        );
        seek(&mut session, at_save).unwrap();
        session.apply(Action::MoveTrack(2, 0)).unwrap();
        assert_eq!(session.view.edit_history.labels.len(), 5);
        session.apply(Action::Rename(2, "Piano".into())).unwrap();
        assert_eq!(session.view.edit_history.labels.len(), 4);
        assert!(seek(&mut session, 5).is_err());
        assert!(
            session
                .apply(Action::Open(directory.0.join("Missing.clef")))
                .is_err()
        );
        assert_eq!(session.view.edit_history.labels.len(), 4);
        session.apply(Action::Open(copy)).unwrap();
        assert!(session.view.edit_history.labels.is_empty());
        assert!(!session.view.dirty);
    }

    #[test]
    fn a_piano_roll_gesture_is_one_persisted_undo_step() {
        use clef_model::midi::{Event, EventChange, Message};
        let (directory, mut session) = isolated();
        session.apply(Action::New).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        session
            .apply(Action::EditClips {
                expected: session.view.project.midi.clone(),
                edit: ClipEdit::Create {
                    track: 1,
                    start: 0,
                    end: 960,
                },
            })
            .unwrap();
        let cursor = session.view.edit_history.cursor;
        let before = session.view.project.midi.clone();
        let clip = before.clips(1)[0].id();
        let edit = ClipEdit::EditEvents {
            take: None,
            clip,
            label: "Draw MIDI Events",
            changes: vec![
                EventChange::Insert(Event {
                    tick: 120,
                    order: 0,
                    message: Message::Note {
                        channel: 1,
                        key: 60,
                        velocity: 92,
                        end: 240,
                        release: 0,
                        release_order: 0,
                    },
                }),
                EventChange::Insert(Event {
                    tick: 120,
                    order: 0,
                    message: Message::CurvedControl {
                        bytes: [0xb1, 11, 75],
                        bend: -25,
                    },
                }),
            ],
        };
        session
            .apply(Action::EditClips {
                expected: before.clone(),
                edit: edit.clone(),
            })
            .unwrap();
        let after = session.view.project.midi.clone();
        assert_eq!(session.view.edit_history.cursor, cursor + 1);
        assert!(
            session
                .apply(Action::EditClips {
                    expected: before.clone(),
                    edit
                })
                .is_err()
        );
        seek(&mut session, cursor).unwrap();
        assert_eq!(session.view.project.midi, before);
        seek(&mut session, cursor + 1).unwrap();
        assert_eq!(session.view.project.midi, after);
        let path = directory.0.join("Piano Roll.clef");
        save(&mut session, path.clone(), false, false).unwrap();
        assert_eq!(Project::read(&path).unwrap().midi, after);
    }

    #[test]
    fn history_preserves_unrelated_settings_and_checks_transport() {
        let (_directory, mut session) = isolated();
        session.apply(Action::New).unwrap();
        let old_range = session.view.project.timeline.loop_range;
        let range = old_range.map(|tick| tick + 960);
        session
            .apply(Action::Transport(TransportAction::SetLoopRange(range)))
            .unwrap();
        session
            .apply(Action::Transport(TransportAction::ToggleClick))
            .unwrap();
        let click = session.view.project.timeline.click;
        session.view.transport.phase = TransportPhase::Playing;
        assert!(seek(&mut session, 0).is_err());
        session.view.transport.phase = TransportPhase::Stopped;
        session.view.transport.pending = true;
        assert!(seek(&mut session, 0).is_err());
        session.view.transport.pending = false;
        session.view.recording = true;
        assert!(seek(&mut session, 0).is_err());
        session.view.recording = false;
        seek(&mut session, 0).unwrap();
        assert_eq!(session.view.project.timeline.loop_range, old_range);
        assert_eq!(session.view.project.timeline.click, click);
        assert!(session.view.dirty);
        seek(&mut session, 1).unwrap();
        assert_eq!(session.view.project.timeline.loop_range, range);
    }

    #[test]
    fn history_restores_removed_midi_and_unavailable_instrument() {
        use clef_model::midi::{Event, Message};
        let (directory, mut session) = isolated();
        session.apply(Action::New).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        let identity = plugin(directory.0.join("Missing.vst3"), "missing");
        let track = &mut session.view.project.tracks[0];
        track.plugin = Some(identity.clone());
        track.state = vec![42].into();
        let midi = Arc::make_mut(&mut session.view.project.midi);
        midi.record(
            1,
            Mode::NewTake,
            [0, 960],
            vec![vec![Event {
                tick: 0,
                order: 0,
                message: Message::Control([0xb0, 1, 64]),
            }]],
            false,
        )
        .unwrap();
        let expected = session.view.project.clone();
        session.quarantine(identity, "fixture quarantine".into());
        session.apply(Action::RemoveTrack(1)).unwrap();
        assert!(session.view.project.midi.tracks.is_empty());
        seek(&mut session, 1).unwrap();
        assert_eq!(session.view.project, expected);
        assert!(session.jobs.is_empty());
        assert!(session.view.tracks[0].status == "Quarantined");
        seek(&mut session, 2).unwrap();
        assert!(session.view.project.tracks.is_empty());
        assert!(session.view.project.midi.tracks.is_empty());
    }

    #[test]
    fn history_seek_is_atomic_when_an_instrument_has_unjournaled_changes() {
        let (directory, mut session) = isolated();
        session.apply(Action::New).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        let before = InstrumentState::from(&session.view.project.tracks[0]);
        session.view.project.tracks[0].plugin =
            Some(plugin(directory.0.join("Missing.vst3"), "missing"));
        session.view.project.tracks[0].state = vec![1].into();
        let after = InstrumentState::from(&session.view.project.tracks[0]);
        session.record_edit(Edit::Instrument {
            id: 1,
            before,
            after,
            editor: false,
        });
        session.apply(Action::Rename(1, "Piano".into())).unwrap();
        seek(&mut session, 0).unwrap();
        assert!(session.view.project.tracks.is_empty());
        seek(&mut session, 3).unwrap();
        assert_eq!(session.view.project.tracks[0].state[..], [1]);
        session.view.project.tracks[0].state = vec![2].into();
        assert!(seek(&mut session, 0).is_err());
        assert_eq!(session.view.edit_history.cursor, 3);
        assert_eq!(session.view.project.tracks[0].name, "Piano");
        assert_eq!(session.view.project.tracks[0].state[..], [2]);
    }

    #[test]
    #[ignore = "requires built Clef, TestSynth and native worker WindowServer access"]
    fn native_t08_replacement_restores_an_isolated_workers_captured_state() {
        let (directory, mut session) = isolated();
        let instruments = scan_test_synth(&directory.0);
        let first = instruments
            .iter()
            .find(|plugin| plugin.name == "VST3 Host Test Synth")
            .unwrap()
            .clone();
        let second = instruments
            .into_iter()
            .find(|plugin| plugin.name == "VST3 Host Test Synth (Single)")
            .unwrap();
        let settle = |session: &mut Session| {
            let deadline = Instant::now() + Duration::from_secs(10);
            while !session.jobs.is_empty() && Instant::now() < deadline {
                session.poll();
                std::thread::sleep(Duration::from_millis(20));
            }
            assert!(session.jobs.is_empty());
        };
        session.apply(Action::New).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        session.apply(Action::Select(1, first.clone())).unwrap();
        settle(&mut session);
        assert_eq!(
            session.view.edit_history.labels,
            ["Add Track", "Assign Instrument"]
        );
        let runtime = session.runtimes.get_mut(&1).unwrap();
        // Use the deterministic worker to simulate a snapshot-capable editor changing opaque state.
        runtime.instrument = Some(
            Instrument::load(&test_executable(), std::path::Path::new("@fixture"), None).unwrap(),
        );
        let worker = runtime.instrument.as_mut().unwrap();
        runtime.editor = true;
        runtime.editor_before = Some(worker.save().unwrap());
        worker
            .call(protocol::Request::FixtureEdit { value: 73 })
            .unwrap();
        session.finish_track_editor(1).unwrap();
        session.apply(Action::Select(1, second.clone())).unwrap();
        settle(&mut session);
        assert_eq!(
            session.view.edit_history.labels,
            [
                "Add Track",
                "Assign Instrument",
                "Edit Instrument State",
                "Replace Instrument",
            ]
        );
        seek(&mut session, 3).unwrap();
        assert_eq!(session.track(1).unwrap().plugin.as_ref(), Some(&first));
        assert_eq!(session.track(1).unwrap().state[..], [73]);
        seek(&mut session, 2).unwrap();
        assert_ne!(session.track(1).unwrap().state[..], [73]);
        seek(&mut session, 4).unwrap();
        assert_eq!(session.track(1).unwrap().plugin.as_ref(), Some(&second));

        session.request_load(1, None).unwrap();
        settle(&mut session);
        assert!(session.runtimes[&1].instrument.is_some());
        let previous = session.track(1).unwrap().clone();
        session
            .apply(Action::Select(
                1,
                plugin(PathBuf::from("/missing-t08.vst3"), "invalid"),
            ))
            .unwrap();
        settle(&mut session);
        assert_eq!(session.track(1).unwrap(), &previous);
        assert_eq!(session.view.edit_history.cursor, 4);
        assert_eq!(session.view.edit_history.labels.len(), 4);
    }

    #[test]
    fn autosave_uses_retained_state_and_keeps_failed_transitions_recoverable() {
        let directory = project::TemporaryDirectory::new("t07-session").unwrap();
        let new_session =
            || Session::with_registry(directory.0.join("quarantine.json"), PathBuf::new());
        let mut session = new_session();
        session.apply(Action::New).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        let track = &mut session.view.project.tracks[0];
        track.plugin = Some(Plugin {
            id: "fixture".into(),
            name: "Fixture".into(),
            format: "CLAP".into(),
            path: "/Fixture.clap".into(),
        });
        track.state = vec![7, 9].into();
        let project = session.view.project.clone();
        session.view.recording = true;
        session.view.editor = true;
        session.view.transport.phase = TransportPhase::Playing;
        let start = session.last_checkpoint;
        session.poll_autosave(start + AUTOSAVE_INTERVAL - Duration::from_secs(1));
        assert!(session.journal.is_none());
        session.poll_autosave(start + AUTOSAVE_INTERVAL);
        session.finish_checkpoint();
        assert!(session.view.dirty && session.view.editor && session.view.recording);
        assert_eq!(session.view.transport.phase, TransportPhase::Playing);
        assert_eq!(
            session.journal.as_ref().unwrap().recover().unwrap(),
            project
        );
        session.checkpoint_take = true;
        session.poll_autosave(start + AUTOSAVE_INTERVAL + Duration::from_secs(1));
        assert!(session.checkpoint.is_some());
        session.finish_checkpoint();
        drop(session);

        let mut session = new_session();
        session.load_history();
        let id = session.view.recoveries[0].id.clone();
        session.apply(Action::Recover(id)).unwrap();
        assert!(session.view.dirty && session.view.project_open && session.view.path.is_none());
        assert_eq!(session.view.project, project);
        assert!(
            session
                .apply(Action::Open(directory.0.join("missing.clef")))
                .is_err()
        );
        let missing_parent = directory.0.join("missing").join("copy.clef");
        assert!(save(&mut session, missing_parent, false, false).is_err());
        assert_eq!(
            session.journal.as_ref().unwrap().recover().unwrap(),
            project
        );
        let copy = directory.0.join("copy.clef");
        save(&mut session, copy.clone(), false, false).unwrap();
        assert!(!session.view.dirty);
        assert_eq!(Project::read(&copy).unwrap(), project);
        assert!(
            session
                .history
                .as_ref()
                .unwrap()
                .recoveries()
                .unwrap()
                .is_empty()
        );

        session.apply(Action::Rename(1, "Unsaved".into())).unwrap();
        session.start_checkpoint().unwrap();
        session.apply(Action::DiscardBeforeClose).unwrap();
        assert!(
            session
                .history
                .as_ref()
                .unwrap()
                .recoveries()
                .unwrap()
                .is_empty()
        );
        assert_eq!(Project::read(&copy).unwrap(), project);
    }
    use clef_audio_io::{AudioDevice, AudioRange, SupportedBufferSize};

    #[test]
    fn unchanged_idle_snapshots_do_not_repaint_but_commands_still_complete() {
        let backend = Backend::spawn(PathBuf::new(), || {});
        let (_, completed) = backend
            .receive
            .recv_timeout(Duration::from_secs(3))
            .unwrap();
        assert_eq!(completed, None);
        assert!(matches!(
            backend.receive.recv_timeout(Duration::from_millis(120)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));

        backend
            .send
            .send(Action::Transport(TransportAction::Locate(0)))
            .unwrap();
        let (_, completed) = backend
            .receive
            .recv_timeout(Duration::from_secs(3))
            .unwrap();
        assert_eq!(completed, Some(true));
    }

    #[test]
    fn volatile_diagnostics_refresh_at_most_once_per_second() {
        let processed = std::sync::atomic::AtomicU64::new(0);
        let read = || {
            format!(
                "processed {} blocks",
                processed.fetch_add(1, Ordering::Relaxed)
            )
        };
        let mut volatile = Volatile::default();
        let start = Instant::now();
        let first = volatile.due(start);
        assert!(first);
        assert_eq!(volatile.track(first, 1, read), "processed 0 blocks");
        // The engine counter keeps moving, but polls inside one second publish the same text.
        for poll in 1..33 {
            let due = volatile.due(start + Duration::from_millis(30 * poll));
            assert!(!due);
            assert_eq!(volatile.track(due, 1, read), "processed 0 blocks");
        }
        // A newly attached track is shown immediately.
        assert_eq!(volatile.track(false, 2, read), "processed 1 blocks");
        let due = volatile.due(start + VOLATILE_INTERVAL);
        assert!(due);
        assert_eq!(volatile.track(due, 1, read), "processed 2 blocks");
    }

    #[test]
    fn idle_polls_inside_one_second_publish_no_new_view() {
        let (_directory, mut session) = isolated();
        session.apply(Action::New).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        session.poll();
        let view = session.view.for_ui();
        for _ in 0..3 {
            session.poll();
            session.poll_autosave(Instant::now());
            assert!(session.view.for_ui() == view);
        }
    }

    #[test]
    fn plugin_paths_are_cached_and_rechecked_on_the_rescan_cadence() {
        let (directory, mut session) = isolated();
        session.apply(Action::New).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        let bundle = directory.0.join("Late.vst3");
        session.view.project.tracks[0].plugin = Some(plugin(bundle.clone(), "late"));
        session.poll();
        assert!(session.view.state(1).unwrap().missing);
        assert_eq!(session.view.state(1).unwrap().status, "Missing");
        std::fs::create_dir(&bundle).unwrap();
        session.poll();
        assert!(
            session.view.state(1).unwrap().missing,
            "refresh must not touch the filesystem"
        );
        session.paths.checked = Some(Instant::now() - RESCAN_INTERVAL);
        session.poll();
        assert!(!session.view.state(1).unwrap().missing);
        assert_ne!(session.view.state(1).unwrap().status, "Missing");
    }

    #[test]
    fn send_level_previews_are_transient_and_the_commit_is_one_edit() {
        let (_directory, mut session) = isolated();
        session.apply(Action::New).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        session.apply(Action::AddBusSend { track: 1 }).unwrap();
        let epoch = session.view.project_epoch;
        let send = session.view.project.tracks[0].sends[0].clone();
        let cursor = session.view.edit_history.cursor;
        session.view.dirty = false;
        let preview = |level| Action::Send {
            epoch,
            track: 1,
            edit: SendEdit::PreviewLevel { id: send.id, level },
        };
        session.apply(preview(-100)).unwrap();
        session.apply(preview(-90)).unwrap();
        assert_eq!(session.view.project.tracks[0].sends[0], send);
        assert!(!session.view.dirty);
        assert_eq!(session.view.edit_history.cursor, cursor);
        assert_eq!(session.send_preview.map(|preview| preview.level), Some(-90));
        session
            .apply(Action::Send {
                epoch,
                track: 1,
                edit: SendEdit::CommitLevel {
                    id: send.id,
                    level: -90,
                },
            })
            .unwrap();
        assert!(session.send_preview.is_none());
        assert_eq!(session.view.project.tracks[0].sends[0].level, -90);
        assert_eq!(session.view.edit_history.cursor, cursor + 1);
        seek(&mut session, cursor).unwrap();
        assert_eq!(session.view.project.tracks[0].sends[0].level, send.level);

        session.apply(preview(-80)).unwrap();
        session
            .apply(Action::Send {
                epoch,
                track: 1,
                edit: SendEdit::CancelPreview,
            })
            .unwrap();
        assert!(session.send_preview.is_none());
        assert_eq!(session.view.project.tracks[0].sends[0].level, send.level);
        session.apply(preview(-70)).unwrap();
        session.apply(Action::New).unwrap();
        assert!(session.send_preview.is_none());
        assert!(SendEdit::CancelPreview.is_preview());
        assert!(!SendEdit::CommitLevel { id: 1, level: 0 }.is_preview());
    }

    #[test]
    fn arrangement_scroll_and_zoom_never_start_an_autosave() {
        let (_directory, mut session) = isolated();
        session.apply(Action::New).unwrap();
        let view = ArrangementView {
            track_height: 44,
            snap: clef_model::midi::Snap::Sixteenth,
            offset: 960,
            zoom_milli: 48_000,
        };
        session.apply(Action::ArrangementView(view)).unwrap();
        assert_eq!(session.view.project.arrangement_view, view);
        assert!(!session.view.dirty);
        session.poll_autosave(session.last_checkpoint + AUTOSAVE_INTERVAL);
        assert!(session.checkpoint.is_none() && session.journal.is_none());
        // The view is saved with the next real autosave.
        session.apply(Action::AddTrack(None)).unwrap();
        session.poll_autosave(session.last_checkpoint + AUTOSAVE_INTERVAL);
        session.finish_checkpoint();
        let recovered = session.journal.as_ref().unwrap().recover().unwrap();
        assert_eq!(recovered.arrangement_view, view);
    }

    #[test]
    fn new_tracks_take_cycling_colours_and_the_roll_view_is_view_state() {
        let (directory, mut session) = isolated();
        session.apply(Action::New).unwrap();
        for _ in 0..3 {
            session.apply(Action::AddTrack(None)).unwrap();
        }
        let colours: Vec<_> = session
            .view
            .project
            .tracks
            .iter()
            .map(|track| track.colour)
            .collect();
        assert_eq!(
            colours,
            clef_model::project::TRACK_COLOURS[..3]
                .iter()
                .copied()
                .map(Some)
                .collect::<Vec<_>>()
        );
        let path = directory.0.join("Roll.clef");
        save(&mut session, path.clone(), false, false).unwrap();
        let roll = clef_model::project::PianoRollView {
            zoom_milli: 150_000,
            row_height: 20,
        };
        session.apply(Action::PianoRollView(roll)).unwrap();
        session.refresh();
        assert!(session.view.view_unsaved && !session.view.dirty);
        assert!(
            session
                .apply(Action::PianoRollView(clef_model::project::PianoRollView {
                    row_height: 30,
                    ..roll
                }))
                .is_err()
        );
        session.apply(Action::SaveViewBeforeClose).unwrap();
        let saved = Project::read(&path).unwrap();
        assert_eq!(saved.piano_roll_view, roll);
        assert_eq!(
            saved.tracks[2].colour,
            Some(clef_model::project::TRACK_COLOURS[2])
        );
    }

    #[test]
    fn a_view_only_change_is_saved_silently_when_leaving_a_clean_saved_project() {
        let (directory, mut session) = isolated();
        let view = ArrangementView {
            track_height: 44,
            snap: clef_model::midi::Snap::Sixteenth,
            offset: 960,
            zoom_milli: 48_000,
        };
        // Untitled: nothing is written.
        session.apply(Action::New).unwrap();
        session.apply(Action::ArrangementView(view)).unwrap();
        session.refresh();
        assert!(!session.view.view_unsaved);
        session.apply(Action::SaveViewBeforeClose).unwrap();

        let path = directory.0.join("View.clef");
        save(&mut session, path.clone(), false, false).unwrap();
        let initial = view;
        let scrolled = ArrangementView {
            offset: 1920,
            ..view
        };
        session.apply(Action::ArrangementView(scrolled)).unwrap();
        session.refresh();
        assert!(session.view.view_unsaved && !session.view.dirty);
        session.apply(Action::SaveViewBeforeClose).unwrap();
        assert_eq!(Project::read(&path).unwrap().arrangement_view, scrolled);
        session.refresh();
        assert!(!session.view.view_unsaved);

        // New also writes the view first; the saved file is then reopened with it.
        session.apply(Action::ArrangementView(initial)).unwrap();
        session.apply(Action::New).unwrap();
        assert_eq!(Project::read(&path).unwrap().arrangement_view, initial);
        session.apply(Action::Open(path.clone())).unwrap();
        assert_eq!(session.view.project.arrangement_view, initial);

        // A dirty project keeps the prompt path: leaving without saving writes nothing.
        session.apply(Action::ArrangementView(scrolled)).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        assert!(session.view.dirty);
        session.apply(Action::New).unwrap();
        assert_eq!(Project::read(&path).unwrap().arrangement_view, initial);
    }

    fn test_executable() -> PathBuf {
        PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/debug/clef"
        ))
    }

    fn scan_test_synth(directory: &std::path::Path) -> Vec<Plugin> {
        let bundle = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../vendor/rust-vst3-host/test_plugins/TestSynth.vst3"
        ))
        .canonicalize()
        .unwrap();
        let catalog = directory.join("catalog.json");
        let scan = std::process::Command::new(test_executable())
            .arg("scan-bundle")
            .arg(bundle)
            .arg(&catalog)
            .status()
            .unwrap();
        assert!(scan.success());
        serde_json::from_slice(&std::fs::read(catalog).unwrap()).unwrap()
    }

    #[test]
    #[ignore = "requires built Clef with the VST3 TestSynth and CLAP MIDI test fixtures"]
    fn native_t10_delivers_mapped_and_raw_cc_to_isolated_instruments() {
        let (directory, _) = isolated();
        let synth = scan_test_synth(&directory.0)
            .into_iter()
            .find(|plugin| plugin.name == "VST3 Host Test Synth")
            .unwrap();
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
        let bundle = root.join("validation-output/fixtures/ClefLatencyCLAP.clap");
        let result = directory.0.join("clap-scan.json");
        assert!(
            std::process::Command::new(test_executable())
                .arg("scan-bundle")
                .arg(bundle)
                .arg(&result)
                .status()
                .unwrap()
                .success()
        );
        let clap: Vec<Plugin> = serde_json::from_slice(&std::fs::read(result).unwrap()).unwrap();
        let render = |plugin: &Plugin, cc, value| {
            let mut instrument = Instrument::load_selected(
                &test_executable(),
                &plugin.path,
                &plugin.id,
                48_000,
                None,
            )
            .unwrap();
            instrument.start(true).unwrap();
            let position = clef_engine::sequence::START;
            let control = protocol::Midi {
                offset: 0,
                bytes: [0xb0, cc, value],
            };
            let attack = protocol::Midi {
                offset: 5,
                bytes: [0x90, 60, 100],
            };
            let mut frames = instrument
                .process(position, &[control, attack], 128)
                .unwrap()
                .0;
            for block in 1..64 {
                frames.extend(
                    instrument
                        .process(position + 128 * block, &[], 128)
                        .unwrap()
                        .0,
                );
            }
            frames
        };
        let low = render(&synth, 74, 5);
        let high = render(&synth, 74, 120);
        let difference = low
            .iter()
            .zip(&high)
            .map(|(a, b)| (a[0] - b[0]).abs())
            .fold(0.0_f32, f32::max);
        assert!(difference > 0.001, "VST3 CC74 difference: {difference}");
        assert!((render(&clap[0], 11, 127)[5][0] - 0.25).abs() < 0.0001);
        assert_eq!(render(&clap[0], 11, 0)[5][0], 0.0);
    }

    fn isolated() -> (crate::project::TemporaryDirectory, Session) {
        let directory = crate::project::TemporaryDirectory::new("t04-session").unwrap();
        let session =
            Session::with_registry(directory.0.join("quarantine.json"), test_executable());
        (directory, session)
    }
    fn plugin(path: PathBuf, id: &str) -> Plugin {
        Plugin {
            path,
            id: id.into(),
            name: id.into(),
            format: "VST3".into(),
        }
    }
    fn save(session: &mut Session, path: PathBuf, replace: bool, use_retained: bool) -> Result<()> {
        let consent = if use_retained {
            session
                .runtimes
                .iter()
                .filter(|(_, r)| r.lost)
                .map(|(id, r)| (*id, r.request))
                .collect()
        } else {
            Vec::new()
        };
        session.apply(Action::Save {
            path,
            replace,
            track_name: None,
            use_retained: consent,
        })
    }
    #[test]
    fn stable_ids_survive_reorder_removal_and_reopen_without_workers() {
        let (directory, mut session) = isolated();
        for _ in 0..8 {
            session.apply(Action::AddTrack(None)).unwrap();
        }
        session.apply(Action::Rename(1, "Piano".into())).unwrap();
        session.apply(Action::MoveTrack(1, 7)).unwrap();
        assert_eq!(session.view.project.tracks[7].id, 1);
        session.apply(Action::RemoveTrack(4)).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        assert_eq!(session.view.selected, Some(9));
        let path = directory.0.join("Session.clef");
        save(&mut session, path.clone(), false, false).unwrap();
        let expected = session.view.project.clone();
        session.apply(Action::New).unwrap();
        session.apply(Action::Open(path)).unwrap();
        assert_eq!(session.view.project, expected);
        assert!(session.runtimes.is_empty());
        assert!(session.jobs.is_empty());
        assert!(!session.view.active);
        assert!(!session.view.dirty);
        let before = session.view.project.clone();
        assert!(
            session
                .apply(Action::Open(directory.0.join("missing.clef")))
                .is_err()
        );
        assert_eq!(session.view.project, before);
    }
    #[test]
    fn buses_outputs_sends_and_bus_removal_refusal_round_trip_without_workers() {
        let (directory, mut session) = isolated();
        session.apply(Action::New).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        session.apply(Action::AddBus { assign: Some(1) }).unwrap();
        assert_eq!(session.view.project.tracks[2].kind, TrackKind::Bus);
        assert_eq!(session.view.project.tracks[0].output, Some(3));
        let history = session.view.edit_history.clone();
        session
            .apply(Action::SeekHistory {
                version: history.version,
                target: history.cursor - 1,
            })
            .unwrap();
        assert_eq!(session.view.project.tracks.len(), 2);
        assert_eq!(session.view.project.tracks[0].output, None);
        let history = session.view.edit_history.clone();
        session
            .apply(Action::SeekHistory {
                version: history.version,
                target: history.cursor + 1,
            })
            .unwrap();
        assert_eq!(session.view.project.tracks[2].kind, TrackKind::Bus);
        assert_eq!(session.view.project.tracks[0].output, Some(3));
        let epoch = session.view.project_epoch;
        session
            .apply(Action::SetOutput {
                epoch,
                track: 2,
                output: Some(3),
            })
            .unwrap();
        session
            .apply(Action::Send {
                epoch,
                track: 2,
                edit: SendEdit::Add { target: 3 },
            })
            .unwrap();
        let send = session.view.project.tracks[1].sends[0].id;
        assert!(session.view.project.tracks[1].sends[0].enabled);
        for enabled in [false, true] {
            session
                .apply(Action::Send {
                    epoch,
                    track: 2,
                    edit: SendEdit::SetEnabled { id: send, enabled },
                })
                .unwrap();
            assert_eq!(session.view.project.tracks[1].sends[0].enabled, enabled);
        }
        let silence = session.view.project.tracks[1].sends[0].level;
        session
            .apply(Action::Send {
                epoch,
                track: 2,
                edit: SendEdit::PreviewLevel {
                    id: send,
                    level: -120,
                },
            })
            .unwrap();
        session
            .apply(Action::Send {
                epoch,
                track: 2,
                edit: SendEdit::CommitLevel {
                    id: send,
                    level: -120,
                },
            })
            .unwrap();
        assert_eq!(session.view.project.tracks[1].sends[0].level, -120);
        let history = session.view.edit_history.clone();
        session
            .apply(Action::SeekHistory {
                version: history.version,
                target: history.cursor - 1,
            })
            .unwrap();
        assert_eq!(session.view.project.tracks[1].sends[0].level, silence);
        let history = session.view.edit_history.clone();
        session
            .apply(Action::SeekHistory {
                version: history.version,
                target: history.cursor + 1,
            })
            .unwrap();
        assert_eq!(session.view.project.tracks[1].sends[0].level, -120);
        assert_eq!(
            session
                .apply(Action::Send {
                    epoch,
                    track: 2,
                    edit: SendEdit::Add { target: 3 },
                })
                .unwrap_err()
                .to_string(),
            "This track already sends to that bus."
        );
        let path = directory.0.join("Routing.clef");
        save(&mut session, path.clone(), false, false).unwrap();
        let expected = session.view.project.clone();
        let reopened =
            Session::with_registry(directory.0.join("quarantine.json"), test_executable());
        let mut reopened = reopened;
        reopened.apply(Action::Open(path)).unwrap();
        assert_eq!(reopened.view.project, expected);
        assert!(reopened.view.project.tracks[2].is_bus());

        assert!(session.apply(Action::RemoveTrack(3)).is_err());
        assert_eq!(
            session
                .apply(Action::RemoveTrack(3))
                .unwrap_err()
                .to_string(),
            "Bus still has 2 sources. Reassign them before removing it."
        );
        session
            .apply(Action::Send {
                epoch,
                track: 2,
                edit: SendEdit::Remove { id: send },
            })
            .unwrap();
        session
            .apply(Action::SetOutput {
                epoch,
                track: 1,
                output: None,
            })
            .unwrap();
        session
            .apply(Action::SetOutput {
                epoch,
                track: 2,
                output: None,
            })
            .unwrap();
        session.apply(Action::RemoveTrack(3)).unwrap();
        assert_eq!(session.view.project.tracks.len(), 2);
    }

    #[test]
    fn add_send_to_new_bus_is_one_undo_step() {
        let (_directory, mut session) = isolated();
        session.apply(Action::New).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        let source = session.view.project.tracks[0].id;
        session.apply(Action::AddBusSend { track: source }).unwrap();
        let bus = session.view.project.tracks.last().unwrap().id;
        assert!(session.view.project.tracks.last().unwrap().is_bus());
        let send = session.view.project.tracks[0].sends[0].clone();
        assert_eq!(send.target, bus);
        assert_eq!(send.id, bus + 1);
        assert_eq!(session.view.selected, Some(source));
        let history = session.view.edit_history.clone();
        assert_eq!(history.labels.last(), Some(&"New Bus Send"));
        session
            .apply(Action::SeekHistory {
                version: history.version,
                target: history.cursor - 1,
            })
            .unwrap();
        assert_eq!(session.view.project.tracks.len(), 1);
        assert!(session.view.project.tracks[0].sends.is_empty());
        let history = session.view.edit_history.clone();
        session
            .apply(Action::SeekHistory {
                version: history.version,
                target: history.cursor + 1,
            })
            .unwrap();
        assert!(session.view.project.tracks.last().unwrap().is_bus());
        assert_eq!(session.view.project.tracks[0].sends, vec![send]);
    }

    #[test]
    fn save_keeps_unloaded_missing_and_quarantined_state_and_requires_loss_consent() {
        let (directory, mut session) = isolated();
        for _ in 0..3 {
            session.apply(Action::AddTrack(None)).unwrap();
        }
        for track in &mut session.view.project.tracks {
            track.plugin = Some(plugin(
                directory.0.join(format!("{}.vst3", track.id)),
                "same",
            ));
            track.state = vec![track.id as u8, 0, 255].into();
        }
        let identity = session.view.project.tracks[0].plugin.clone().unwrap();
        session.runtimes.insert(
            1,
            Runtime {
                touched: true,
                ..Default::default()
            },
        );
        session.quarantine(identity.clone(), "deliberate crash".into());
        assert!(session.runtimes[&1].lost);
        let path = directory.0.join("Preserved.clef");
        assert!(save(&mut session, path.clone(), false, false).is_err());
        assert!(!path.exists());
        let expected = session.view.project.clone();
        save(&mut session, path.clone(), false, true).unwrap();
        assert_eq!(Project::read(&path).unwrap(), expected);
        let reopened =
            Session::with_registry(directory.0.join("quarantine.json"), test_executable());
        assert!(reopened.blocked(&identity));
        session.apply(Action::Reactivate(identity.clone())).unwrap();
        assert!(!session.blocked(&identity));
        assert!(session.runtimes.values().all(|r| r.instrument.is_none()));
        assert!(!session.view.active);
    }
    #[test]
    fn quarantine_covers_matching_instances_but_preserves_other_identities() {
        let (directory, mut session) = isolated();
        for _ in 0..3 {
            session.apply(Action::AddTrack(None)).unwrap();
        }
        let identity = plugin(directory.0.join("Synth.vst3"), "entry-a");
        session.view.project.tracks[0].plugin = Some(identity.clone());
        session.view.project.tracks[1].plugin = Some(identity.clone());
        session.view.project.tracks[2].plugin = Some(plugin(identity.path.clone(), "entry-b"));
        session.quarantine(identity.clone(), "crash".into());
        session.refresh();
        assert_eq!(session.view.state(1).unwrap().status, "Quarantined");
        assert_eq!(session.view.state(2).unwrap().status, "Quarantined");
        assert_eq!(session.view.state(3).unwrap().status, "Missing");
        assert!(session.request_load(1, None).is_err());
        assert!(session.jobs.is_empty());
        session.apply(Action::Reactivate(identity)).unwrap();
        assert!(
            quarantine::read(&directory.0.join("quarantine.json"))
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn external_changes_and_competing_save_locks_preserve_disk_and_dirty_state() {
        let (directory, mut session) = isolated();
        session.apply(Action::AddTrack(None)).unwrap();
        let path = directory.0.join("Session.clef");
        save(&mut session, path.clone(), false, false).unwrap();
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        file.lock().unwrap();
        session.apply(Action::Rename(1, "Unsaved".into())).unwrap();
        assert!(save(&mut session, path.clone(), true, false).is_err());
        assert!(session.view.dirty);
        assert_eq!(Project::read(&path).unwrap().tracks[0].name, "Instrument 1");
        drop(file);
        let mut external = Project::read(&path).unwrap();
        external.tracks[0].name = "External".into();
        external.save(&path, true).unwrap();
        assert!(save(&mut session, path.clone(), true, false).is_err());
        assert_eq!(Project::read(&path).unwrap(), external);
    }
    #[test]
    fn invalid_names_and_identity_operations_are_atomic_and_ui_omits_blobs() {
        let (_, mut session) = isolated();
        session.apply(Action::AddTrack(None)).unwrap();
        assert!(session.apply(Action::Rename(1, "  ".into())).is_err());
        assert!(session.apply(Action::SelectTrack(99, None)).is_err());
        assert!(session.apply(Action::MoveTrack(1, -1)).is_err());
        session.view.project.tracks[0].state = vec![42; 1024].into();
        assert!(session.view.for_ui().project.tracks[0].state.is_empty());
        assert_eq!(session.view.project.tracks[0].state.len(), 1024);
    }
    #[test]
    fn unsupported_configuration_and_missing_devices_are_rejected() {
        let devices = Devices {
            audio: vec![AudioDevice {
                id: "device".into(),
                name: "Test".into(),
                configurations: vec![AudioRange {
                    channels: 2,
                    min_rate: 44100,
                    max_rate: 48000,
                    buffer: SupportedBufferSize::Range { min: 32, max: 512 },
                }],
                current_rate: Some(48000),
            }],
            midi: vec![("midi".into(), "Keyboard".into())],
            default_output: None,
        };
        let mut settings = AudioSettings {
            device: "device".into(),
            midi: "midi".into(),
            ..Default::default()
        };
        assert!(devices.validate(&settings).is_ok());
        settings.rate = 96000;
        assert!(devices.validate(&settings).is_err());
        settings.rate = 48000;
        settings.buffer = 1024;
        assert!(devices.validate(&settings).is_err());
        settings.buffer = 128;
        settings.midi = "missing".into();
        assert!(devices.validate(&settings).is_err());
    }
    #[test]
    fn cancelled_or_obsolete_load_completion_cannot_quarantine_or_replace_current_tracks() {
        let (directory, mut session) = isolated();
        session.apply(Action::AddTrack(None)).unwrap();
        let identity = plugin(directory.0.join("Old.vst3"), "old");
        let (tx, rx) = mpsc::channel();
        tx.send(Err("plugin worker exited".into())).unwrap();
        session.jobs.push(LoadJob {
            id: 1,
            generation: 0,
            request: 0,
            plugin: identity,
            replacement: true,
            cancel: Arc::new(AtomicBool::new(false)),
            receive: rx,
            thread: None,
        });
        session.apply(Action::New).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        session.poll();
        assert!(session.view.project.tracks[0].plugin.is_none());
        assert!(session.view.quarantine.is_empty());
        assert!(session.jobs.is_empty());
    }
    #[test]
    fn retained_state_consent_does_not_cover_a_later_failure() {
        let (directory, mut session) = isolated();
        session.apply(Action::AddTrack(None)).unwrap();
        let identity = plugin(directory.0.join("Crash.vst3"), "crash");
        session.view.project.tracks[0].plugin = Some(identity.clone());
        session.runtimes.insert(
            1,
            Runtime {
                touched: true,
                ..Default::default()
            },
        );
        session.quarantine(identity.clone(), "first crash".into());
        let consent = vec![(1, session.runtimes[&1].request)];
        session.quarantine(identity, "second crash".into());
        let path = directory.0.join("RequiresNewConsent.clef");
        assert!(
            session
                .apply(Action::Save {
                    path: path.clone(),
                    replace: false,
                    track_name: None,
                    use_retained: consent
                })
                .is_err()
        );
        assert!(!path.exists());
    }
    #[test]
    #[ignore = "requires built Clef, two owned deterministic fixture workers and WindowServer"]
    fn native_nth_capture_failure_preserves_disk_and_previous_successful_capture() {
        let (directory, mut session) = isolated();
        for index in 0..2 {
            session.apply(Action::AddTrack(None)).unwrap();
            let path = directory.0.join(format!("Fixture{index}.vst3"));
            std::fs::create_dir(&path).unwrap();
            session.view.project.tracks[index].plugin =
                Some(plugin(path, &format!("fixture-{index}")));
            session.runtimes.insert(
                index as u64 + 1,
                Runtime {
                    instrument: Some(
                        Instrument::load(
                            &test_executable(),
                            std::path::Path::new("@fixture"),
                            None,
                        )
                        .unwrap(),
                    ),
                    ..Default::default()
                },
            );
        }
        let path = directory.0.join("Coherent.clef");
        save(&mut session, path.clone(), false, false).unwrap();
        let before = Project::read(&path).unwrap();
        session
            .runtimes
            .get_mut(&1)
            .unwrap()
            .instrument
            .as_mut()
            .unwrap()
            .call(crate::protocol::Request::FixtureEdit { value: 99 })
            .unwrap();
        let runtime = session.runtimes.get_mut(&2).unwrap();
        runtime.touched = true;
        let bad = runtime.instrument.as_mut().unwrap();
        bad.start(false).unwrap();
        bad.call(crate::protocol::Request::FixtureFault { hang: false })
            .unwrap();
        assert!(bad.process(0, &[], 128).is_err());
        assert!(save(&mut session, path.clone(), true, false).is_err());
        assert_eq!(Project::read(&path).unwrap(), before);
        assert!(session.view.dirty);
        assert_eq!(session.view.project.tracks[0].state[..], [99]);
        session.poll();
        assert_eq!(session.view.state(2).unwrap().status, "Quarantined");
        assert!(session.runtimes[&1].instrument.is_some());
    }
    #[test]
    #[ignore = "requires saved commercial T03 projects, T02 state fixtures, built Clef and WindowServer"]
    fn commercial_changed_state_survives_multitrack_save_and_lazy_reopen() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
        let (directory, mut session) = isolated();
        let cases = [
            (
                "T03-GUI-ZebraHZ-VST3.clef",
                "zebrahz-vst3-preset-change-master40.state",
            ),
            (
                "T03-GUI-ZebraHZ-CLAP.clef",
                "zebrahz-clap-preset-change-master40.state",
            ),
            ("T03-GUI-Pianoverse.clef", "pianoverse-AA-Y7-width65.state"),
        ];
        for (index, (project, state)) in cases.iter().enumerate() {
            session.apply(Action::AddTrack(None)).unwrap();
            let imported = Project::read(&root.join("validation-output").join(project)).unwrap();
            session.view.project.tracks[index].plugin = imported.tracks[0].plugin.clone();
            session.view.project.tracks[index].state =
                crate::files::read_state(&root.join("validation-output/states").join(state))
                    .unwrap()
                    .into();
            let id = index as u64 + 1;
            session.request_load(id, None).unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(120);
        while !session.jobs.is_empty() && Instant::now() < deadline {
            session.poll();
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(session.jobs.is_empty());
        assert_eq!(
            session
                .runtimes
                .values()
                .filter(|r| r.instrument.is_some())
                .count(),
            3,
            "{}",
            session.view.message
        );
        for runtime in session.runtimes.values_mut() {
            runtime.instrument.as_mut().unwrap().editor(true).unwrap();
            runtime.editor = true;
            runtime.touched = true;
            runtime.instrument.as_mut().unwrap().editor(false).unwrap();
            runtime.editor = false;
        }
        for runtime in session.runtimes.values_mut() {
            runtime.instrument.as_mut().unwrap().stop().unwrap();
        }
        let expected: Vec<_> = (1..=3)
            .map(|id| {
                session
                    .runtimes
                    .get_mut(&id)
                    .unwrap()
                    .instrument
                    .as_mut()
                    .unwrap()
                    .save()
                    .unwrap()
            })
            .collect();
        let path = directory.0.join("Commercial.clef");
        save(&mut session, path.clone(), false, false).unwrap();
        for (track, state) in session.view.project.tracks.iter().zip(&expected) {
            assert_eq!(&track.state[..], &state[..]);
        }
        session.apply(Action::New).unwrap();
        session.apply(Action::Open(path)).unwrap();
        assert!(session.runtimes.is_empty());
        for id in 1..=3 {
            session.apply(Action::SelectTrack(id, None)).unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(120);
        while !session.jobs.is_empty() && Instant::now() < deadline {
            session.poll();
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(session.jobs.is_empty());
        for (id, state) in (1..=3).zip(expected) {
            session
                .runtimes
                .get_mut(&id)
                .unwrap()
                .instrument
                .as_mut()
                .unwrap()
                .stop()
                .unwrap();
            assert_eq!(
                session
                    .runtimes
                    .get_mut(&id)
                    .unwrap()
                    .instrument
                    .as_mut()
                    .unwrap()
                    .save()
                    .unwrap(),
                state
            );
        }
        assert!(!session.view.active);
        assert!(!session.view.dirty);
        std::fs::write(root.join("validation-output/T04-commercial-roundtrip.json"),serde_json::to_vec_pretty(&serde_json::json!({"passed":true,"tracks":3,"formats":["ZebraHZ VST3","ZebraHZ CLAP","Pianoverse VST3"],"scope":"Changed state roundtrip, lazy reopen, original editor lifecycle; no listening claim"})).unwrap()).unwrap();
    }
    #[test]
    #[ignore = "requires explicitly rebuilt/bundled VST3/CLAP latency fixtures, saved EVO output and WindowServer"]
    fn native_multitrack_latency_notifications_rebuild_compensation_without_worker_failure() {
        use midir::os::unix::VirtualOutput;
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
        let (directory, mut session) = isolated();
        let name = format!("Clef T04 latency {}", std::process::id());
        let mut source = midir::MidiOutput::new(&name)
            .unwrap()
            .create_virtual(&name)
            .unwrap();
        session.view.project.audio = Some(saved_output(root));
        session.view.project.audio.as_mut().unwrap().rate = 48000;
        session.view.project.audio.as_mut().unwrap().buffer = 128;
        session.view.project.audio.as_mut().unwrap().midi = Devices::enumerate()
            .unwrap()
            .midi
            .iter()
            .find(|(_, n)| n.contains(&name))
            .unwrap()
            .0
            .clone();
        for (index, bundle) in ["ClefLatencyVST3.vst3", "ClefLatencyCLAP.clap"]
            .iter()
            .enumerate()
        {
            session.apply(Action::AddTrack(None)).unwrap();
            let bundle = root.join("validation-output/fixtures").join(bundle);
            let report = directory.0.join(format!("scan-{index}.json"));
            assert!(
                std::process::Command::new(test_executable())
                    .arg("scan-bundle")
                    .arg(&bundle)
                    .arg(&report)
                    .status()
                    .unwrap()
                    .success()
            );
            let entries: Vec<Plugin> =
                serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
            session.view.project.tracks[index].plugin = Some(entries[0].clone());
            session.request_load(index as u64 + 1, None).unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(60);
        while !session.jobs.is_empty() && Instant::now() < deadline {
            session.poll();
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(session.jobs.is_empty());
        session.view.selected = Some(1);
        session.start_audio().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            session.poll();
            if session.engine.as_ref().is_some_and(|e| {
                e.clock
                    .tracks
                    .iter()
                    .filter(|s| s.id.load(Ordering::Relaxed) != 0)
                    .count()
                    == 2
                    && e.clock
                        .tracks
                        .iter()
                        .filter(|s| s.id.load(Ordering::Relaxed) != 0)
                        .all(|s| s.latency.load(Ordering::Relaxed) == 257)
            }) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(session.view.active, "{}", session.view.message);
        let engine = session.engine.as_ref().unwrap();
        assert_eq!(engine.clock.latency.load(Ordering::Relaxed), 257);
        assert!(
            engine
                .clock
                .tracks
                .iter()
                .filter(|s| s.id.load(Ordering::Relaxed) != 0)
                .all(|s| s.latency.load(Ordering::Relaxed) == 257
                    && !s.failed.load(Ordering::Relaxed))
        );
        for id in 1..=2 {
            session.apply(Action::SelectTrack(id, None)).unwrap();
            std::thread::sleep(Duration::from_millis(30));
            source.send(&[0x90, 60, 80]).unwrap();
            std::thread::sleep(Duration::from_millis(80));
            source.send(&[0x80, 60, 0]).unwrap();
        }
        std::thread::sleep(Duration::from_millis(80));
        session.poll();
        let engine = session.engine.as_ref().unwrap();
        let rows:Vec<_>=engine.clock.tracks.iter().filter(|s|s.id.load(Ordering::Relaxed)!=0).map(|s|serde_json::json!({"id":s.id.load(Ordering::Relaxed),"latency":s.latency.load(Ordering::Relaxed),"blocks":s.processed.load(Ordering::Relaxed),"misses":s.misses.load(Ordering::Relaxed),"transition_frames":s.transition_frames.load(Ordering::Relaxed),"stale":s.stale.load(Ordering::Relaxed),"midi":s.midi_received.load(Ordering::Relaxed),"delivered":s.midi_delivered.load(Ordering::Relaxed),"peak":f32::from_bits(s.peak.load(Ordering::Relaxed))})).collect();
        assert_eq!(rows.len(), 2);
        let report = serde_json::json!({"summary":engine.summary(),"device":session.view.project.audio,"tracks":rows,"passed":rows.iter().all(|r|r["peak"].as_f64().unwrap()>0.0)});
        std::fs::write(
            root.join("validation-output/T04-latency-notifications.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        eprintln!("{report}");
        assert!(
            rows.iter().all(|r| r["peak"].as_f64().unwrap() > 0.0),
            "latency paths: {rows:?}"
        );
        session.stop().unwrap();
    }
    #[test]
    #[ignore = "requires generated T04-Review.clef, eight commercial workers, saved output device, virtual MIDI and WindowServer"]
    fn native_eight_commercial_instruments_process_independently() {
        use midir::os::unix::VirtualOutput;
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
        let (_, mut session) = isolated();
        session.view.project =
            Project::read(&root.join("validation-output/T04-Review.clef")).unwrap();
        let name = format!("Clef T04 commercial {}", std::process::id());
        let mut source = midir::MidiOutput::new(&name)
            .unwrap()
            .create_virtual(&name)
            .unwrap();
        session.view.project.audio.as_mut().unwrap().midi = Devices::enumerate()
            .unwrap()
            .midi
            .iter()
            .find(|(_, n)| n.contains(&name))
            .unwrap()
            .0
            .clone();
        for id in 1..=8 {
            session.request_load(id, None).unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(120);
        while !session.jobs.is_empty() && Instant::now() < deadline {
            session.poll();
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(session.jobs.is_empty());
        assert_eq!(
            session
                .runtimes
                .values()
                .filter(|r| r.instrument.is_some())
                .count(),
            8,
            "{}",
            session.view.message
        );
        session.view.selected = Some(1);
        session.start_audio().unwrap();
        std::thread::sleep(Duration::from_millis(500));
        let warmup:Vec<_>=session.engine.as_ref().unwrap().clock.tracks.iter().filter(|s|s.id.load(Ordering::Relaxed)!=0).map(|s|serde_json::json!({"id":s.id.load(Ordering::Relaxed),"misses":s.misses.load(Ordering::Relaxed),"transition_frames":s.transition_frames.load(Ordering::Relaxed)})).collect();
        let observation_start = session
            .engine
            .as_ref()
            .unwrap()
            .clock
            .played
            .load(Ordering::Relaxed);
        for stats in &session.engine.as_ref().unwrap().clock.tracks {
            stats.misses.store(0, Ordering::Relaxed);
            stats.transition_frames.store(0, Ordering::Relaxed);
        }
        for id in 1..=8 {
            session.apply(Action::SelectTrack(id, None)).unwrap();
            std::thread::sleep(Duration::from_millis(80));
            source.send(&[0x90, 60, 50]).unwrap();
            std::thread::sleep(Duration::from_millis(180));
            source.send(&[0x80, 60, 0]).unwrap();
            std::thread::sleep(Duration::from_millis(200));
            session.poll();
        }
        let engine = session
            .engine
            .as_ref()
            .expect("healthy engine must continue");
        let rows:Vec<_>=engine.clock.tracks.iter().filter(|s|s.id.load(Ordering::Relaxed)!=0).map(|s|serde_json::json!({
            "id":s.id.load(Ordering::Relaxed),"blocks":s.processed.load(Ordering::Relaxed),"midi":s.midi_received.load(Ordering::Relaxed),
            "peak":f32::from_bits(s.peak.load(Ordering::Relaxed)),"misses":s.misses.load(Ordering::Relaxed),"transition_frames":s.transition_frames.load(Ordering::Relaxed),
            "max_rpc_us":s.max_rpc_us.load(Ordering::Relaxed),"max_worker_us":s.max_worker_us.load(Ordering::Relaxed),"latency":s.latency.load(Ordering::Relaxed)})).collect();
        let report = serde_json::json!({"summary":engine.summary(),"device":session.view.project.audio,"tracks":rows,"warmup_ms":500,"warmup_counters":warmup,"observation_start_frame":observation_start,"miss_counter_scope":"Reset after warm-up; block counts and maxima are lifetime values",
            "scope":"Native processing and virtual MIDI; not physical keyboard or listening acceptance"});
        std::fs::write(
            root.join("validation-output/T04-eight-commercial.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        eprintln!("{report}");
        assert_eq!(rows.len(), 8);
        assert!(rows.iter().all(|r| r["blocks"].as_u64().unwrap() > 0
            && r["midi"].as_u64().unwrap() > 0
            && r["peak"].as_f64().unwrap() > 0.000001));
        session.stop().unwrap();
    }
    #[test]
    #[ignore = "requires rebuilt Clef and TestSynth, a CoreAudio output and CoreMIDI virtual source"]
    fn native_t06_records_routes_reopens_and_plays_an_instrument_clip() {
        use midir::os::unix::VirtualOutput;
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
        let (directory, mut session) = isolated();
        session.apply(Action::New).unwrap();
        let name = format!("Clef T06 fixture {}", std::process::id());
        let mut source = midir::MidiOutput::new(&name)
            .unwrap()
            .create_virtual(&name)
            .unwrap();
        let devices = Devices::enumerate().unwrap();
        let midi = devices
            .midi
            .iter()
            .find(|(_, label)| label == &name)
            .unwrap()
            .0
            .clone();
        let mut settings = saved_output(root);
        settings.midi = midi;
        devices.validate(&settings).unwrap();
        session.view.project.audio = Some(settings);

        let synth = scan_test_synth(&directory.0)
            .into_iter()
            .find(|plugin| plugin.name == "VST3 Host Test Synth")
            .unwrap();
        for _ in 0..2 {
            session.apply(Action::AddTrack(None)).unwrap();
        }
        for track in &mut session.view.project.tracks {
            track.plugin = Some(synth.clone());
        }
        fn wait(session: &mut Session, condition: impl Fn(&Session) -> bool) {
            let deadline = Instant::now() + Duration::from_secs(8);
            loop {
                session.poll();
                if condition(session) {
                    return;
                }
                assert!(Instant::now() < deadline, "{}", session.view.message);
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        session.apply(Action::SelectTrack(1, None)).unwrap();
        session.start_audio().unwrap();
        wait(&mut session, |s| s.ready(1));
        session.apply(Action::Editor(1, true)).unwrap();
        wait(&mut session, |s| s.view.editor);
        session
            .apply(Action::Transport(TransportAction::Arm(2)))
            .unwrap();
        wait(&mut session, |s| s.ready(2));
        assert_eq!(session.view.selected, Some(1));
        assert_eq!(session.live_target(), Some(2));
        session
            .apply(Action::Transport(TransportAction::ToggleCountIn))
            .unwrap();
        wait(&mut session, |s| !s.view.transport.pending);
        session
            .apply(Action::Transport(TransportAction::Record))
            .unwrap();
        wait(&mut session, |s| {
            s.view.transport.phase == TransportPhase::CountingIn
        });
        source.send(&[0x90, 55, 64]).unwrap();
        source.send(&[0x80, 55, 0]).unwrap();
        session
            .apply(Action::Transport(TransportAction::Stop))
            .unwrap();
        wait(&mut session, |s| !s.view.recording);
        assert!(session.view.project.midi.clips(2).is_empty());
        session
            .apply(Action::Transport(TransportAction::ToggleCountIn))
            .unwrap();
        wait(&mut session, |s| !s.view.transport.pending);
        session
            .apply(Action::Transport(TransportAction::SetRecordMode(
                Mode::NewTake,
            )))
            .unwrap();
        session
            .apply(Action::Transport(TransportAction::SetLoopRange([0, 960])))
            .unwrap();
        wait(&mut session, |s| !s.view.transport.pending);
        session
            .apply(Action::Transport(TransportAction::ToggleLoop))
            .unwrap();
        wait(&mut session, |s| !s.view.transport.pending);
        session
            .apply(Action::Transport(TransportAction::Record))
            .unwrap();
        wait(&mut session, |s| s.view.recording_region.is_some());
        session.last_checkpoint = Instant::now() - AUTOSAVE_INTERVAL;
        session.poll_autosave(Instant::now());
        assert!(session.checkpoint.is_some());
        source.send(&[0xb0, 1, 42]).unwrap();
        source.send(&[0x90, 60, 64]).unwrap();
        wait(&mut session, |s| {
            s.view.recording_notes.as_ref().is_some_and(|notes| {
                notes
                    .current
                    .iter()
                    .any(|note| note.key == 60 && note.end.is_none())
            })
        });
        std::thread::sleep(Duration::from_millis(100));
        source.send(&[0x80, 60, 19]).unwrap();
        wait(&mut session, |s| {
            s.view.recording_notes.as_ref().is_some_and(|notes| {
                notes
                    .current
                    .iter()
                    .any(|note| note.key == 60 && note.end.is_some_and(|end| end > note.start))
            })
        });
        wait(&mut session, |s| {
            s.view.recording_notes.as_ref().is_some_and(|notes| {
                notes.current.is_empty() && notes.previous.iter().any(|note| note.key == 60)
            })
        });
        source.send(&[0x90, 65, 62]).unwrap();
        wait(&mut session, |s| {
            s.view.recording_notes.as_ref().is_some_and(|notes| {
                notes
                    .current
                    .iter()
                    .any(|note| note.key == 65 && note.end.is_none())
                    && notes.previous.iter().any(|note| note.key == 60)
            })
        });
        std::thread::sleep(Duration::from_millis(60));
        source.send(&[0x80, 65, 23]).unwrap();
        wait(&mut session, |s| {
            s.view.recording_notes.as_ref().is_some_and(|notes| {
                notes
                    .current
                    .iter()
                    .any(|note| note.key == 65 && note.end.is_some())
                    && notes.previous.iter().any(|note| note.key == 60)
            })
        });
        std::thread::sleep(Duration::from_millis(50));
        session
            .apply(Action::Transport(TransportAction::Stop))
            .unwrap();
        wait(&mut session, |s| !s.view.recording);
        assert!(session.view.recording_notes.is_none());
        assert!(session.checkpoint_take);
        assert!(session.view.editor && session.view.active);
        session.finish_checkpoint();
        session.poll_autosave(Instant::now());
        session.finish_checkpoint();
        assert_eq!(
            session.journal.as_ref().unwrap().recover().unwrap().midi,
            session.view.project.midi
        );
        let clips = session.view.project.midi.clips(2);
        assert_eq!(clips.len(), 1, "{}", session.view.message);
        assert_eq!(clips[0].takes.len(), 2);
        assert!(clips[0].takes[0].events.iter().any(|event| matches!(
            event.message,
            clef_model::midi::Message::Note {
                key: 60,
                velocity: 64,
                release: 19,
                ..
            }
        )));
        assert!(clips[0].active_events().any(|event| matches!(
            event.message,
            clef_model::midi::Message::Note {
                key: 65,
                release: 23,
                ..
            }
        )));
        assert!(session.view.project.midi.clips(1).is_empty());
        let recorded = session.view.project.midi.clips(2).to_vec();
        let recorded_revision = session.view.edit_history.cursor;
        assert_eq!(
            session.view.edit_history.labels[recorded_revision - 1],
            "Record MIDI"
        );
        seek(&mut session, recorded_revision - 1).unwrap();
        assert!(session.view.project.midi.clips(2).is_empty());
        seek(&mut session, recorded_revision).unwrap();
        assert_eq!(session.view.project.midi.clips(2), recorded);
        let received = |s: &Session, id| {
            s.runtimes[&id]
                .broker
                .as_ref()
                .unwrap()
                .stats
                .midi_received
                .load(Ordering::Relaxed)
        };
        assert_eq!(received(&session, 1), 0);
        assert!(received(&session, 2) >= 3);
        session
            .apply(Action::Transport(TransportAction::Arm(2)))
            .unwrap();
        source.send(&[0x90, 62, 64]).unwrap();
        wait(&mut session, |s| received(s, 1) > 0);
        source.send(&[0x80, 62, 0]).unwrap();

        let path = directory.0.join("Recorded.clef");
        let expected = session.view.project.midi.clone();
        save(&mut session, path.clone(), false, false).unwrap();
        session.apply(Action::Open(path)).unwrap();
        assert_eq!(session.view.project.midi, expected);
        assert!(!session.view.active);
        assert_eq!(session.view.armed, None);
        session.start_audio().unwrap();
        session
            .apply(Action::Transport(TransportAction::Play))
            .unwrap();
        let stats = |s: &Session| {
            s.runtimes
                .get(&2)?
                .broker
                .as_ref()
                .map(|broker| broker.stats.clone())
        };
        wait(&mut session, |s| {
            stats(s).is_some_and(|stats| {
                stats.playback_blocks.load(Ordering::Relaxed) >= 2
                    && f32::from_bits(stats.peak.load(Ordering::Relaxed)) > 0.00001
            })
        });
        let stats = stats(&session).unwrap();
        assert_eq!(stats.midi_received.load(Ordering::Relaxed), 0);
        assert_eq!(stats.playback_dropped.load(Ordering::Relaxed), 0);
        session
            .apply(Action::Transport(TransportAction::Stop))
            .unwrap();
        wait(&mut session, |s| {
            s.view.transport.phase == TransportPhase::Stopped && !s.view.transport.pending
        });

        // T09: publish edited MIDI to the other live worker and play only its repeated segment.
        let clip = session.view.project.midi.clips(2)[0].id();
        let edit = |session: &mut Session, edit| {
            session.apply(Action::EditClips {
                expected: session.view.project.midi.clone(),
                edit,
            })
        };
        edit(
            &mut session,
            ClipEdit::Move {
                destinations: vec![(clip, 1)],
                delta: 0,
            },
        )
        .unwrap();
        edit(
            &mut session,
            ClipEdit::Resize {
                clip,
                end: 1_920,
                repeat: true,
            },
        )
        .unwrap();
        assert!(session.view.project.midi.clips(2).is_empty());
        assert!(
            session.view.project.midi.clips(1)[0]
                .active_events()
                .any(|event| event.tick >= 960)
        );
        assert!(session.view.project.timeline.looping);
        session
            .apply(Action::Transport(TransportAction::ToggleLoop))
            .unwrap();
        wait(&mut session, |s| !s.view.transport.pending);
        session
            .apply(Action::Transport(TransportAction::Locate(960)))
            .unwrap();
        wait(&mut session, |s| !s.view.transport.pending);
        session
            .apply(Action::Transport(TransportAction::Play))
            .unwrap();
        wait(&mut session, |s| {
            s.runtimes[&1].broker.as_ref().is_some_and(|broker| {
                broker.stats.playback_blocks.load(Ordering::Relaxed) >= 2
                    && f32::from_bits(broker.stats.peak.load(Ordering::Relaxed)) > 0.00001
            })
        });
        session
            .apply(Action::Transport(TransportAction::Stop))
            .unwrap();
        wait(&mut session, |s| {
            s.view.transport.phase == TransportPhase::Stopped && !s.view.transport.pending
        });
        seek(&mut session, 0).unwrap();
        assert_eq!(session.view.project.midi.clips(2), expected.clips(2));
        assert!(session.view.project.midi.clips(1).is_empty());
        session
            .apply(Action::Transport(TransportAction::ToggleLoop))
            .unwrap();
        wait(&mut session, |s| !s.view.transport.pending);

        session
            .apply(Action::Transport(TransportAction::Arm(2)))
            .unwrap();
        for mode in [Mode::Merge, Mode::Replace] {
            session
                .apply(Action::Transport(TransportAction::Locate(0)))
                .unwrap();
            wait(&mut session, |s| !s.view.transport.pending);
            session
                .apply(Action::Transport(TransportAction::SetRecordMode(mode)))
                .unwrap();
            session
                .apply(Action::Transport(TransportAction::Record))
                .unwrap();
            wait(&mut session, |s| s.view.recording_region.is_some());
            source.send(&[0x90, 70, 48]).unwrap();
            std::thread::sleep(Duration::from_millis(30));
            source.send(&[0x80, 70, 0]).unwrap();
            wait(&mut session, |s| {
                s.view
                    .recording_notes
                    .as_ref()
                    .is_some_and(|notes| notes.previous.iter().any(|note| note.key == 70))
            });
            session
                .apply(Action::Transport(TransportAction::Stop))
                .unwrap();
            wait(&mut session, |s| !s.view.recording);
        }
        let before_recovery = session.view.project.midi.clone();
        session
            .apply(Action::Transport(TransportAction::ToggleLoop))
            .unwrap();
        wait(&mut session, |s| !s.view.transport.pending);
        session
            .apply(Action::Transport(TransportAction::Locate(0)))
            .unwrap();
        wait(&mut session, |s| !s.view.transport.pending);
        session
            .apply(Action::Transport(TransportAction::SetRecordMode(
                Mode::Replace,
            )))
            .unwrap();
        session
            .apply(Action::Transport(TransportAction::Record))
            .unwrap();
        wait(&mut session, |s| s.view.recording_region.is_some());
        source.send(&[0x90, 67, 62]).unwrap();
        std::thread::sleep(Duration::from_millis(50));
        source.send(&[0x80, 67, 17]).unwrap();
        wait(&mut session, |s| {
            s.recording
                .as_ref()
                .is_some_and(|take| take.input.len() >= 2)
        });
        drop(source);
        session.last_devices = Instant::now() - Duration::from_secs(3);
        wait(&mut session, |s| !s.view.active && !s.view.recording);
        let recovered = &session.view.project.midi.clips(2)[0];
        let original = &before_recovery.clips(2)[0];
        assert_eq!(
            recovered.active_events().collect::<Vec<_>>(),
            original.active_events().collect::<Vec<_>>()
        );
        assert_eq!(
            recovered.takes.len(),
            original.takes.len() + 1,
            "{}",
            session.view.message
        );
        assert!(recovered.takes.last().unwrap().events.iter().any(|event| {
            matches!(
                event.message,
                clef_model::midi::Message::Note { key: 67, .. }
            )
        }));
        assert!(session.view.message.contains("Recording interrupted"));
        session.stop().unwrap();
    }

    #[test]
    #[ignore = "opens saved T03 audio output, creates owned virtual MIDI and eight fixture workers; build Clef first"]
    fn native_multitrack_fixture_routing_and_failure_isolation() {
        use midir::os::unix::VirtualOutput;
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
        let (directory, mut session) = isolated();
        let name = format!("Clef T04 fixture {}", std::process::id());
        let mut source = midir::MidiOutput::new(&name)
            .unwrap()
            .create_virtual(&name)
            .unwrap();
        let devices = Devices::enumerate().unwrap();
        let midi = devices
            .midi
            .iter()
            .find(|(_, label)| label.contains(&name))
            .unwrap()
            .0
            .clone();
        session.view.project.audio = Some(saved_output(root));
        session.view.project.audio.as_mut().unwrap().midi = midi;
        for index in 0..8 {
            session.apply(Action::AddTrack(None)).unwrap();
            let identity = directory.0.join(format!("Fixture{index}.vst3"));
            std::fs::create_dir(&identity).unwrap();
            session.view.project.tracks[index].plugin =
                Some(plugin(identity, &format!("fixture-{index}")));
            let instrument =
                Instrument::load(&test_executable(), std::path::Path::new("@fixture"), None)
                    .unwrap();
            session.runtimes.insert(
                index as u64 + 1,
                Runtime {
                    instrument: Some(instrument),
                    ..Default::default()
                },
            );
        }
        session.apply(Action::SelectTrack(1, None)).unwrap();
        session.start_audio().unwrap();
        for _ in 0..20 {
            source.send(&[0x90, 60, 80]).unwrap();
            std::thread::sleep(Duration::from_millis(10));
            source.send(&[0x80, 60, 0]).unwrap();
        }
        assert!(
            session.runtimes[&1]
                .broker
                .as_ref()
                .unwrap()
                .stats
                .midi_received
                .load(Ordering::Relaxed)
                > 0
        );
        assert!(
            session
                .runtimes
                .iter()
                .filter(|(id, _)| **id != 1)
                .all(|(_, r)| r
                    .broker
                    .as_ref()
                    .unwrap()
                    .stats
                    .midi_received
                    .load(Ordering::Relaxed)
                    == 0)
        );
        let old_received = session.runtimes[&1]
            .broker
            .as_ref()
            .unwrap()
            .stats
            .midi_received
            .load(Ordering::Relaxed);
        session.apply(Action::SelectTrack(3, None)).unwrap();
        for _ in 0..20 {
            source.send(&[0x90, 62, 80]).unwrap();
            std::thread::sleep(Duration::from_millis(10));
            source.send(&[0x80, 62, 0]).unwrap();
        }
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(
            session.runtimes[&1]
                .broker
                .as_ref()
                .unwrap()
                .stats
                .midi_received
                .load(Ordering::Relaxed),
            old_received
        );
        assert!(
            session.runtimes[&3]
                .broker
                .as_ref()
                .unwrap()
                .stats
                .midi_received
                .load(Ordering::Relaxed)
                > 0
        );
        let initial:Vec<_>=session.engine.as_ref().unwrap().clock.tracks.iter().filter(|s|s.id.load(Ordering::Relaxed)!=0).map(|s|serde_json::json!({"id":s.id.load(Ordering::Relaxed),"blocks":s.processed.load(Ordering::Relaxed),"max_rpc_us":s.max_rpc_us.load(Ordering::Relaxed),"misses":s.misses.load(Ordering::Relaxed)})).collect();
        assert_eq!(initial.len(), 8);
        assert!(initial.iter().all(|s| s["blocks"].as_u64().unwrap() > 0));
        session.apply(Action::SelectTrack(1, None)).unwrap();
        // A hung worker must not block the other seven paths or stop the device.
        session
            .runtimes
            .get_mut(&2)
            .unwrap()
            .instrument
            .as_mut()
            .unwrap()
            .call(crate::protocol::Request::FixtureFault { hang: true })
            .unwrap();
        for _ in 0..130 {
            source.send(&[0x90, 60, 80]).unwrap();
            std::thread::sleep(Duration::from_millis(10));
            source.send(&[0x80, 60, 0]).unwrap();
            session.poll();
        }
        assert!(session.view.active);
        assert_eq!(session.view.state(2).unwrap().status, "Quarantined");
        assert!(session.runtimes[&1].instrument.is_some());
        let clock = &session.engine.as_ref().unwrap().clock;
        let report = serde_json::json!({"device":session.view.project.audio,"summary":session.engine.as_ref().unwrap().summary(),
            "initial_eight_workers":initial,
            "healthy_tracks":session.runtimes.values().filter(|r|r.instrument.is_some()).count(),
            "tracks":clock.tracks.iter().filter(|s|s.id.load(Ordering::Relaxed)!=0).map(|s|serde_json::json!({
                "id":s.id.load(Ordering::Relaxed),"blocks":s.processed.load(Ordering::Relaxed),"midi":s.midi_received.load(Ordering::Relaxed),
                "peak":f32::from_bits(s.peak.load(Ordering::Relaxed)),"misses":s.misses.load(Ordering::Relaxed),"transition_frames":s.transition_frames.load(Ordering::Relaxed),"max_rpc_us":s.max_rpc_us.load(Ordering::Relaxed),"max_worker_us":s.max_worker_us.load(Ordering::Relaxed)})).collect::<Vec<_>>()});
        assert_eq!(report["healthy_tracks"], 7);
        assert!(
            report["tracks"]
                .as_array()
                .unwrap()
                .iter()
                .all(|s| s["blocks"].as_u64().unwrap() > 0)
        );
        assert!(
            report["tracks"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["peak"].as_f64().unwrap() > 0.0)
        );
        std::fs::write(
            root.join("validation-output/T04-native-fixture.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        eprintln!("{report}");
        let missing_id = session.view.project.audio.as_ref().unwrap().midi.clone();
        drop(source);
        std::thread::sleep(Duration::from_millis(2100));
        session.poll();
        // T39: MIDI is optional; losing it is reported and audio keeps running.
        assert!(session.view.active);
        assert!(session.engine.is_some());
        assert_eq!(
            session.view.project.audio.as_ref().unwrap().midi,
            missing_id
        );
        assert!(
            session.view.message.contains("MIDI input unavailable"),
            "{}",
            session.view.message
        );
        session.stop().unwrap();
    }
    fn note(tick: u64, end: u64, key: u8) -> clef_model::midi::Event {
        clef_model::midi::Event {
            tick,
            order: 0,
            message: clef_model::midi::Message::Note {
                channel: 0,
                key,
                velocity: 90,
                end,
                release: 12,
                release_order: 1,
            },
        }
    }

    fn take_fixture(session: &mut Session) {
        session.apply(Action::New).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        let midi = std::sync::Arc::make_mut(&mut session.view.project.midi);
        midi.record(
            1,
            Mode::NewTake,
            [0, 960],
            vec![vec![note(100, 200, 60)], vec![note(500, 700, 62)]],
            false,
        )
        .unwrap();
    }

    #[test]
    fn record_modes_use_track_overrides_and_are_undoable() {
        let (_directory, mut session) = isolated();
        session.apply(Action::New).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        session
            .apply(Action::Transport(TransportAction::SetRecordMode(
                Mode::NewTake,
            )))
            .unwrap();
        session
            .apply(Action::Transport(TransportAction::SetTrackRecordMode(
                1,
                TrackMode::Replace,
            )))
            .unwrap();
        assert_eq!(session.view.project.record_mode, Mode::NewTake);
        assert_eq!(
            session.view.project.tracks[0].record_mode,
            TrackMode::Replace
        );
        assert_eq!(session.view.project.midi.mode, Mode::Merge);
        let cursor = session.view.edit_history.cursor;
        assert_eq!(cursor, 3);
        session
            .apply(Action::SeekHistory {
                version: session.view.edit_history.version,
                target: 2,
            })
            .unwrap();
        assert_eq!(
            session.view.project.tracks[0].record_mode,
            TrackMode::Global
        );
        session
            .apply(Action::SeekHistory {
                version: session.view.edit_history.version,
                target: 1,
            })
            .unwrap();
        assert_eq!(session.view.project.record_mode, Mode::Merge);
        session
            .apply(Action::SeekHistory {
                version: session.view.edit_history.version,
                target: 3,
            })
            .unwrap();
        assert_eq!(session.view.project.record_mode, Mode::NewTake);
        assert_eq!(
            session.view.project.tracks[0].record_mode,
            TrackMode::Replace
        );
    }

    #[test]
    fn comp_actions_are_one_step_and_take_to_new_track_is_undoable() {
        let (_directory, mut session) = isolated();
        take_fixture(&mut session);
        let expected = session.view.project.midi.clone();
        let cursor = session.view.edit_history.cursor;
        session
            .apply(Action::CompEdit {
                expected,
                track: 1,
                lane: 0,
                edit: CompEdit::UseWholeTake,
            })
            .unwrap();
        assert_eq!(session.view.edit_history.cursor, cursor + 1);
        assert_eq!(
            session.view.edit_history.labels.last().copied(),
            Some("Use whole take")
        );
        let clip = &session.view.project.midi.clips(1)[0];
        assert_eq!(clip.sections.len(), 1);
        assert_eq!(clip.sections[0].take, clip.takes[0].id);
        session
            .apply(Action::SeekHistory {
                version: session.view.edit_history.version,
                target: cursor,
            })
            .unwrap();
        let clip = &session.view.project.midi.clips(1)[0];
        assert_eq!(clip.sections.last().unwrap().take, clip.takes[1].id);

        let expected = session.view.project.midi.clone();
        session
            .apply(Action::TakeToNewTrack {
                expected,
                track: 1,
                lane: 1,
            })
            .unwrap();
        assert_eq!(session.view.project.tracks.len(), 2);
        assert!(session.view.project.tracks[1].plugin.is_none());
        assert_eq!(session.view.project.midi.clips(2).len(), 1);
        assert_eq!(
            session.view.edit_history.labels.last().copied(),
            Some("Take to New Track")
        );
        session
            .apply(Action::SeekHistory {
                version: session.view.edit_history.version,
                target: cursor,
            })
            .unwrap();
        assert_eq!(session.view.project.tracks.len(), 1);
        assert!(session.view.project.midi.clips(2).is_empty());
    }

    #[test]
    fn comp_edits_apply_while_the_transport_plays() {
        let (_directory, mut session) = isolated();
        take_fixture(&mut session);
        session.view.transport.phase = TransportPhase::Playing;
        let expected = session.view.project.midi.clone();
        session
            .apply(Action::CompEdit {
                expected,
                track: 1,
                lane: 0,
                edit: CompEdit::UseWholeTake,
            })
            .unwrap();
        let clip = &session.view.project.midi.clips(1)[0];
        assert_eq!(clip.sections.len(), 1);
        assert_eq!(clip.sections[0].take, clip.takes[0].id);
        session.view.transport.phase = TransportPhase::Stopped;
        let target = session.view.edit_history.cursor - 1;
        session
            .apply(Action::SeekHistory {
                version: session.view.edit_history.version,
                target,
            })
            .unwrap();
        assert_eq!(
            session.view.project.midi.clips(1)[0]
                .sections
                .last()
                .unwrap()
                .take,
            session.view.project.midi.clips(1)[0].takes[1].id
        );
    }

    #[test]
    fn take_lane_view_state_is_saved_but_not_undoable() {
        let (directory, mut session) = isolated();
        take_fixture(&mut session);
        let lanes = TakeLanes {
            visible: true,
            lanes: vec![clef_model::project::TakeLane {
                name: Some("Close".into()),
                height: 64,
            }],
        };
        let cursor = session.view.edit_history.cursor;
        session
            .apply(Action::TrackView {
                track: 1,
                lanes: lanes.clone(),
            })
            .unwrap();
        assert_eq!(session.view.project.tracks[0].take_lanes, lanes);
        assert_eq!(session.view.edit_history.cursor, cursor);
        assert!(session.view.dirty);
        let path = directory.0.join("Takes.clef");
        save(&mut session, path.clone(), false, false).unwrap();
        let expected = session.view.project.clone();
        session.apply(Action::New).unwrap();
        session.apply(Action::Open(path)).unwrap();
        assert_eq!(session.view.project, expected);
        assert_eq!(session.view.project.tracks[0].take_lanes, lanes);
    }

    #[test]
    fn engine_start_preloads_every_playable_instrument_and_play_waits_for_loads() {
        let (directory, mut session) = isolated();
        while session.view.project.tracks.len() < 4 {
            session.apply(Action::AddTrack(None)).unwrap();
        }
        let present = directory.0.join("Present.vst3");
        std::fs::write(&present, b"").unwrap();
        let quarantined = directory.0.join("Quarantined.vst3");
        std::fs::write(&quarantined, b"").unwrap();
        let tracks = &mut session.view.project.tracks;
        tracks[0].plugin = Some(plugin(present.clone(), "present"));
        tracks[1].plugin = Some(plugin(present, "offline"));
        tracks[1].instrument_offline = true;
        tracks[2].plugin = Some(plugin(quarantined.clone(), "quarantined"));
        tracks[3].plugin = Some(plugin(directory.0.join("Missing.vst3"), "missing"));
        session.view.quarantine.push(Entry {
            plugin: plugin(quarantined, "quarantined"),
            reason: "test".into(),
            time: 0,
            effect: false,
        });
        let ids: Vec<_> = session.view.project.tracks.iter().map(|t| t.id).collect();
        assert!(!session.play_waiting());
        assert!(session.preload_instruments().is_empty());
        let loading = |s: &Session, id| s.runtimes.get(&id).is_some_and(|r| r.loading);
        assert!(
            loading(&session, ids[0]),
            "playable track loads in the background"
        );
        for id in &ids[1..] {
            assert!(
                !loading(&session, *id),
                "offline, quarantined and missing tracks stay idle"
            );
        }
        assert!(
            session.play_waiting(),
            "Play waits while a track is loading"
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        while loading(&session, ids[0]) && Instant::now() < deadline {
            session.poll();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!loading(&session, ids[0]));
        assert!(!session.play_waiting(), "a failed load ends the wait");
        assert!(session.preload_instruments().is_empty());
    }

    #[test]
    #[ignore = "requires local validation-output/T12-listening-v2.clef, its plugins, the saved output device and WindowServer"]
    fn native_t12_first_play_starts_on_an_aligned_graph() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
        let (_directory, mut session) = isolated();
        session
            .apply(Action::Open(
                root.join("validation-output/T12-listening-v2.clef"),
            ))
            .unwrap();
        session.start_audio().unwrap();
        // Press Play at once, as a user would right after opening.
        session
            .apply(Action::Transport(TransportAction::Play))
            .unwrap();
        let mut passes = Vec::new();
        for pass in 0..2 {
            let deadline = Instant::now() + Duration::from_secs(60);
            while session.view.transport.phase != TransportPhase::Playing {
                assert!(Instant::now() < deadline, "{}", session.view.message);
                session.poll();
                std::thread::sleep(Duration::from_millis(2));
            }
            let clock = session.engine.as_ref().unwrap().clock.clone();
            let start = clock.transitions.load(Ordering::Relaxed);
            let latency = clock.latency.load(Ordering::Relaxed);
            // Covers the T12 CLAP fixture's one-time restart after 48 000 processed samples.
            let end = Instant::now() + Duration::from_millis(2500);
            while Instant::now() < end {
                session.poll();
                std::thread::sleep(Duration::from_millis(5));
            }
            let transitions = clock.transitions.load(Ordering::Relaxed) - start;
            assert_eq!(clock.latency.load(Ordering::Relaxed), latency);
            passes.push(transitions);
            eprintln!("pass {pass}: {transitions} transition frames, latency {latency}");
            session
                .apply(Action::Transport(TransportAction::Stop))
                .unwrap();
            for _ in 0..100 {
                session.poll();
                std::thread::sleep(Duration::from_millis(10));
            }
            if pass == 0 {
                session
                    .apply(Action::Transport(TransportAction::Play))
                    .unwrap();
            }
        }
        assert!(
            passes[0] <= passes[1] + protocol::BLOCK as u64,
            "first Play must not realign more than a later Play: {passes:?}"
        );
    }

    #[test]
    #[ignore = "opens the macOS default audio output and WindowServer"]
    fn native_audio_starts_by_itself_and_settings_scope_global_or_project() {
        let (directory, mut session) = isolated();
        session.auto_audio = true;
        session.apply(Action::New).unwrap();
        session.poll();
        assert!(session.view.active, "{}", session.view.message);
        let in_use = session.view.audio.clone().unwrap();
        let devices = Devices::enumerate().unwrap();
        assert_eq!(
            Some(&in_use.device),
            devices.default_output.as_ref(),
            "first launch uses the default output"
        );
        assert!(
            !directory.0.join("audio.json").exists(),
            "nothing is written before an apply"
        );

        let mut global = in_use.clone();
        global.buffer = 256;
        session
            .apply(Action::Settings {
                audio: global.clone(),
                project: false,
            })
            .unwrap();
        assert!(session.view.active);
        assert_eq!(session.view.audio.as_ref().unwrap().buffer, 256);
        assert_eq!(session.view.audio_preferences, Some(global.clone()));
        assert_eq!(session.view.project.audio, None);
        assert!(directory.0.join("audio.json").exists());

        let mut local = global.clone();
        local.buffer = 512;
        session
            .apply(Action::Settings {
                audio: local.clone(),
                project: true,
            })
            .unwrap();
        assert_eq!(session.view.project.audio, Some(local));
        assert_eq!(session.view.audio.as_ref().unwrap().buffer, 512);
        assert_eq!(
            session.view.audio_preferences,
            Some(global.clone()),
            "an override keeps the global preferences"
        );
        assert!(session.view.dirty);

        session
            .apply(Action::Settings {
                audio: global.clone(),
                project: false,
            })
            .unwrap();
        assert_eq!(
            session.view.project.audio, None,
            "applying to all projects removes the override"
        );
        assert_eq!(session.view.audio.as_ref().unwrap().buffer, 256);
        session.stop().unwrap();
    }
}
