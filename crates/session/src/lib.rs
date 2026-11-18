pub use clef_audio_io::Devices;
use clef_engine::{
    live::Configuration,
    multitrack::{Broker, Engine},
};
use clef_model::{
    Plugin, Result,
    project::{self, AudioSettings, Project, Track},
};
#[cfg(test)]
use clef_plugin_host::{files, protocol};
use clef_plugin_host::{
    quarantine::{self, Entry},
    rpc::Instrument,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::mpsc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub enum Action {
    Shutdown,
    New,
    Open(PathBuf),
    Save {
        path: PathBuf,
        replace: bool,
        track_name: Option<(u64, String)>,
        use_retained: Vec<(u64, u64)>,
    },
    AddTrack(Option<(u64, String)>),
    SelectTrack(u64, Option<(u64, String)>),
    RemoveTrack(u64),
    MoveTrack(u64, isize),
    Rename(u64, String),
    Select(u64, Plugin),
    Settings(AudioSettings),
    Editor(u64, bool),
    Enable,
    Disable,
    Restart(u64),
    Reactivate(Plugin),
}
#[derive(Clone, Default)]
pub struct TrackView {
    pub id: u64,
    pub status: String,
    pub diagnostics: String,
    pub loading: bool,
    pub lost: bool,
    pub loss_version: u64,
}
#[derive(Clone, Default)]
pub struct Snapshot {
    pub project: Project,
    pub selected: Option<u64>,
    pub tracks: Vec<TrackView>,
    pub quarantine: Vec<Entry>,
    pub quarantine_error: String,
    pub notice: u64,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub loaded: bool,
    pub active: bool,
    pub editor: bool,
    pub failed: bool,
    pub message: String,
    pub actual: String,
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
            .collect()
    }
    fn for_ui(&self) -> Self {
        Self {
            project: Project {
                version: self.project.version,
                next_id: self.project.next_id,
                audio: self.project.audio.clone(),
                tracks: self
                    .project
                    .tracks
                    .iter()
                    .map(|t| Track {
                        id: t.id,
                        name: t.name.clone(),
                        plugin: t.plugin.clone(),
                        state: Vec::new(),
                    })
                    .collect(),
            },
            selected: self.selected,
            tracks: self.tracks.clone(),
            quarantine: self.quarantine.clone(),
            quarantine_error: self.quarantine_error.clone(),
            notice: self.notice,
            path: self.path.clone(),
            dirty: self.dirty,
            loaded: self.loaded,
            active: self.active,
            editor: self.editor,
            failed: self.failed,
            message: self.message.clone(),
            actual: self.actual.clone(),
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
    pending_editor: bool,
    touched: bool,
    lost: bool,
    loading: bool,
    request: u64,
    error: String,
}
struct Loaded {
    instrument: Instrument,
    initial: Vec<u8>,
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
pub struct Session {
    executable: PathBuf,
    silence: Arc<AtomicBool>,
    pub view: Snapshot,
    runtimes: HashMap<u64, Runtime>,
    engine: Option<Engine>,
    jobs: Vec<LoadJob>,
    generation: u64,
    registry: PathBuf,
    registry_error: String,
    file_stamp: Option<FileStamp>,
    last_devices: Instant,
}
impl Session {
    pub fn new(executable: PathBuf) -> Self {
        match quarantine::location() {
            Ok(path) => Self::with_registry(path, executable),
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
        Self {
            executable,
            silence: Arc::new(AtomicBool::new(true)),
            view: Snapshot {
                quarantine: entries,
                ..Default::default()
            },
            runtimes: HashMap::new(),
            engine: None,
            jobs: Vec::new(),
            generation: 0,
            registry,
            registry_error: error,
            file_stamp: None,
            last_devices: Instant::now(),
        }
    }
    fn blocked(&self, plugin: &Plugin) -> bool {
        self.view
            .quarantine
            .iter()
            .any(|e| quarantine::same(&e.plugin, plugin))
    }
    fn stop(&mut self) -> Result<()> {
        self.silence.store(true, Ordering::Release);
        self.view.active = false;
        let mut error = None;
        let mut faults = Vec::new();
        if let Some(mut engine) = self.engine.take() {
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
        for (plugin, reason) in faults {
            self.quarantine(plugin, reason);
        }
        if let Some(e) = error { Err(e) } else { Ok(()) }
    }
    fn unload(&mut self) {
        self.generation += 1;
        for job in &self.jobs {
            job.cancel.store(true, Ordering::Release);
        }
        let _ = self.stop();
        self.runtimes.clear();
    }
    fn close_editors(&mut self) -> Result<()> {
        for runtime in self.runtimes.values_mut() {
            runtime.pending_editor = false;
            if runtime.editor {
                runtime
                    .instrument
                    .as_mut()
                    .ok_or("editor worker unavailable")?
                    .editor(false)?;
                runtime.editor = false;
            }
        }
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
            Vec::new()
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
        let rate = self.view.project.audio.rate;
        let prepare_live = self.engine.is_some();
        if replacement.is_some()
            && self.view.selected == Some(id)
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
                    if state.is_empty() { None } else { Some(&state) },
                    &cancelled,
                )?;
                let initial = if state.is_empty() {
                    instrument.save()?
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
        for track in &mut candidate.tracks {
            if let Some(runtime) = self.runtimes.get_mut(&track.id)
                && let Some(instrument) = runtime.instrument.as_mut()
            {
                instrument.stop()?;
                track.state = instrument.save()?;
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
                runtime.instrument.take();
                runtime.editor = false;
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
            });
        }
        match quarantine::update(&self.registry, &plugin, Some(&reason)) {
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
        match action {
            Action::Shutdown => {}
            Action::New => {
                self.unload();
                self.view.project = Project::default();
                self.view.selected = None;
                self.view.path = None;
                self.view.dirty = false;
                self.file_stamp = None;
                self.view.message = "New project. Add an instrument track to begin.".into();
            }
            Action::Open(path) => {
                let stamp = FileStamp::read(&path)?;
                let project = Project::read(&path)?;
                if FileStamp::read(&path)? != stamp {
                    return Err("project changed while opening; retry Open".into());
                }
                self.unload();
                self.file_stamp = Some(stamp);
                self.view.selected = project.tracks.first().map(|t| t.id);
                self.view.project = project;
                self.view.path = Some(path);
                self.view.dirty = false;
                self.view.notice += 1;
                self.view.message =
                    "Project opened. Audio stopped; instruments load on first explicit use.".into();
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
                self.view.project = candidate;
                self.file_stamp = Some(FileStamp::read(&path)?);
                self.view.path = Some(path);
                self.view.dirty = false;
                for runtime in self.runtimes.values_mut() {
                    runtime.touched = false;
                    runtime.lost = false;
                }
                self.view.message =
                    "Project saved. Audio stopped and editors closed for a coherent snapshot."
                        .into();
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
                self.view.project.tracks.push(Track {
                    id,
                    name: format!("Instrument {}", self.view.project.tracks.len() + 1),
                    plugin: None,
                    state: Vec::new(),
                });
                self.view.project.next_id = next;
                self.view.selected = Some(id);
                self.view.dirty = true;
                if let Some(engine) = &self.engine {
                    engine.clock.select(id);
                }
            }
            Action::SelectTrack(id, rename) => {
                self.track(id)?;
                if let Some((old, name)) = rename {
                    self.rename(old, name)?;
                }
                self.select(Some(id))?;
                if self.track(id)?.plugin.is_some() {
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
                if self.view.selected == Some(id) {
                    self.select(None)?;
                }
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
                self.view.project.tracks.remove(index);
                self.view.dirty = true;
                if self.view.selected.is_none() && !self.view.project.tracks.is_empty() {
                    let next =
                        self.view.project.tracks[index.min(self.view.project.tracks.len() - 1)].id;
                    self.select(Some(next))?;
                    if self.track(next)?.plugin.is_some() {
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
                let track = self.view.project.tracks.remove(index);
                self.view.project.tracks.insert(target, track);
                self.view.dirty = true;
            }
            Action::Rename(id, name) => self.rename(id, name)?,
            Action::Select(id, plugin) => {
                self.track(id)?;
                self.request_load(id, Some(plugin))?;
            }
            Action::Settings(settings) => {
                if self.runtimes.values().any(|r| r.loading) {
                    return Err("wait for instrument loading before reconfiguring".into());
                }
                let mut candidate = self.view.project.clone();
                candidate.audio = settings.clone();
                candidate.validate()?;
                Devices::enumerate()?.validate(&settings)?;
                let mut captured = self.capture(&[])?;
                captured.audio = settings;
                let changed_rate = captured.audio.rate != self.view.project.audio.rate;
                let loaded: Vec<_> = self
                    .runtimes
                    .iter()
                    .filter(|(_, r)| r.instrument.is_some())
                    .map(|(id, _)| *id)
                    .collect();
                self.view.project = captured;
                self.view.dirty = true;
                if changed_rate {
                    self.runtimes.clear();
                    for id in loaded {
                        self.request_load(id, None)?;
                    }
                }
                self.view.message = "Settings applied. Audio is stopped.".into();
            }
            Action::Editor(id, open) => {
                self.track(id)?;
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
                    let runtime = self.runtimes.get_mut(&id).ok_or("worker unavailable")?;
                    runtime
                        .instrument
                        .as_mut()
                        .ok_or("worker unavailable")?
                        .editor(open)?;
                    runtime.editor = open;
                    if open {
                        runtime.touched = true;
                        self.view.dirty = true;
                    }
                }
            }
            Action::Enable => {
                if self.engine.is_some() {
                    return Err("audio already active".into());
                }
                let id = self.view.selected.ok_or("select an instrument track")?;
                Devices::enumerate()?.validate(&self.view.project.audio)?;
                let settings = &self.view.project.audio;
                let device = clef_audio_io::output_by_id(&settings.device)?;
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
                )?;
                self.engine = Some(engine);
                self.view.active = true;
                let ids: Vec<_> = self.runtimes.keys().copied().collect();
                let mut failures = Vec::new();
                for track in ids {
                    if let Err(e) = self.attach(track) {
                        failures.push(e.to_string());
                    }
                }
                self.engine
                    .as_ref()
                    .ok_or("engine unavailable")?
                    .clock
                    .select(id);
                if !self
                    .runtimes
                    .get(&id)
                    .is_some_and(|r| r.instrument.is_some() || r.loading)
                {
                    self.request_load(id, None)?;
                }
                self.view.message = if failures.is_empty() {
                    "Audio active. Live MIDI routes only to the selected ready track.".into()
                } else {
                    format!(
                        "Audio active; some paths unavailable: {}",
                        failures.join("; ")
                    )
                };
            }
            Action::Disable => {
                self.stop()?;
                self.view.message = "Audio stopped.".into();
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
        }
        self.refresh();
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
        track.name = name;
        candidate.validate()?;
        self.view.project = candidate;
        self.view.dirty = true;
        Ok(())
    }
    fn select(&mut self, id: Option<u64>) -> Result<()> {
        if self.view.selected != id {
            self.close_editors()?;
            self.view.selected = id;
            if let Some(engine) = &self.engine {
                engine.clock.select(id.unwrap_or(0));
            }
        }
        Ok(())
    }
    fn attach(&mut self, id: u64) -> Result<()> {
        if let Some(runtime) = self.runtimes.get_mut(&id)
            && runtime.broker.is_none()
            && let (Some(engine), Some(instrument)) =
                (&mut self.engine, runtime.instrument.as_mut())
        {
            match engine.attach(id, instrument) {
                Ok(broker) => {
                    runtime.broker = Some(broker);
                    runtime.error.clear();
                }
                Err(error) => {
                    runtime.error = error.to_string();
                    return Err(error);
                }
            }
            runtime.touched = true;
            self.view.dirty = true;
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
                        continue;
                    }
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
                    if job.replacement {
                        track.plugin = Some(job.plugin);
                        track.state = loaded.initial;
                        runtime.lost = false;
                        runtime.touched = false;
                        self.view.dirty = true;
                    } else if track.state.is_empty() {
                        track.state = loaded.initial;
                    }
                    if let Err(e) = self.attach(job.id) {
                        self.view.message = e.to_string();
                    }
                    if self.view.selected == Some(job.id)
                        && let Some(engine) = &self.engine
                    {
                        engine.clock.select(job.id);
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
                        if self.view.selected == Some(job.id)
                            && self
                                .runtimes
                                .get(&job.id)
                                .is_some_and(|r| r.instrument.is_some())
                            && let Some(engine) = &self.engine
                        {
                            engine.clock.select(job.id);
                        }
                        self.view.message =
                            format!("Instrument unavailable; retained state preserved: {error}");
                    }
                }
            }
        }
    }
    fn refresh(&mut self) {
        self.view.tracks = self.view.project.tracks.iter().map(|track| {
            let runtime = self.runtimes.get(&track.id);
            let status = if track.plugin.is_none() { "Empty" }
                else if track.plugin.as_ref().is_some_and(|p| self.blocked(p)) { "Quarantined" }
                else if track.plugin.as_ref().is_some_and(|p| !p.path.exists()) { "Missing" }
                else if runtime.is_some_and(|r| r.loading) { "Loading" }
                else if self.engine.is_some() && runtime.is_some_and(|r| r.instrument.is_some() && r.broker.is_none()) { "Unavailable" }
                else if runtime.is_some_and(|r| r.instrument.is_some()) { "Ready" }
                else if runtime.is_some_and(|r| !r.error.is_empty()) { "Unavailable" } else { "Not loaded" };
            let diagnostics = runtime.map(|r| {
                if let Some(broker) = &r.broker {
                    let s = &broker.stats;
                    format!("latency {} samples · processed {} blocks · missed {} blocks · transition {} frames · stale {} blocks · MIDI {} / dropped {} · max RPC {} us · max worker {} us",
                        s.latency.load(Ordering::Relaxed),s.processed.load(Ordering::Relaxed),s.misses.load(Ordering::Relaxed),s.transition_frames.load(Ordering::Relaxed),s.stale.load(Ordering::Relaxed),s.midi_received.load(Ordering::Relaxed),s.midi_dropped.load(Ordering::Relaxed),s.max_rpc_us.load(Ordering::Relaxed),s.max_worker_us.load(Ordering::Relaxed))
                } else { r.error.clone() }
            }).unwrap_or_default();
            TrackView { id: track.id, status: status.into(), diagnostics, loading: runtime.is_some_and(|r| r.loading), lost: runtime.is_some_and(|r| r.lost), loss_version: runtime.map_or(0,|r|r.request) }
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
        if let Some(engine) = &self.engine {
            self.view.actual = engine.summary();
        }
    }
    fn poll(&mut self) {
        self.poll_jobs();
        let mut faults = Vec::new();
        for (id, runtime) in &mut self.runtimes {
            if (runtime
                .broker
                .as_ref()
                .is_some_and(|b| b.stats.failed.load(Ordering::Acquire))
                || runtime.instrument.as_mut().is_some_and(|i| i.exited()))
                && let Some(plugin) = self
                    .view
                    .project
                    .tracks
                    .iter()
                    .find(|t| t.id == *id)
                    .and_then(|t| t.plugin.clone())
            {
                faults.push(plugin);
            }
        }
        for plugin in faults {
            self.quarantine(
                plugin,
                "Worker exited, timed out, or returned invalid audio".into(),
            );
        }
        if self.engine.is_some() && self.last_devices.elapsed() > Duration::from_secs(2) {
            self.last_devices = Instant::now();
            if let Err(e) = Devices::enumerate().and_then(|d| d.validate(&self.view.project.audio))
            {
                let _ = self.stop();
                self.view.message = format!("Audio stopped: {e}");
            }
        }
        if self
            .engine
            .as_ref()
            .is_some_and(|e| e.clock.errors.load(Ordering::Relaxed) != 0)
        {
            let _ = self.stop();
            self.view.message =
                "Audio device error. Settings retained; Enable Audio explicitly after recovery."
                    .into();
        }
        self.refresh();
    }
}
impl Drop for Session {
    fn drop(&mut self) {
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
            loop {
                let mut success = None;
                match actions.recv_timeout(Duration::from_millis(30)) {
                    Ok(Action::Shutdown) => break,
                    Ok(action) => {
                        success = Some(true);
                        if let Err(e) = session.apply(action) {
                            session.view.message = e.to_string();
                            success = Some(false);
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                session.poll();
                if updates.send((session.view.for_ui(), success)).is_err() {
                    break;
                }
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
    use super::*;
    use clef_audio_io::{AudioDevice, AudioRange, SupportedBufferSize};

    fn test_executable() -> PathBuf {
        PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/debug/clef"
        ))
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
            track.state = vec![track.id as u8, 0, 255];
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
        session.view.project.tracks[0].state = vec![42; 1024];
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
            }],
            midi: vec![("midi".into(), "Keyboard".into())],
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
        assert_eq!(session.view.project.tracks[0].state, vec![99]);
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
                    .unwrap();
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
            runtime
                .instrument
                .as_mut()
                .unwrap()
                .call(crate::protocol::Request::Editor { open: true })
                .unwrap();
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
            assert_eq!(&track.state, state);
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
        session.view.project.audio =
            Project::read(&root.join("validation-output/T03-GUI-Pianoverse.clef"))
                .unwrap()
                .audio;
        session.view.project.audio.rate = 48000;
        session.view.project.audio.buffer = 128;
        session.view.project.audio.midi = Devices::enumerate()
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
        session.apply(Action::Enable).unwrap();
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
        session.apply(Action::Disable).unwrap();
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
        session.view.project.audio.midi = Devices::enumerate()
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
        session.apply(Action::Enable).unwrap();
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
        session.apply(Action::Disable).unwrap();
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
        session.view.project.audio =
            Project::read(&root.join("validation-output/T03-GUI-Pianoverse.clef"))
                .unwrap()
                .audio;
        session.view.project.audio.midi = midi;
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
        session.apply(Action::Enable).unwrap();
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
        let missing_id = session.view.project.audio.midi.clone();
        drop(source);
        std::thread::sleep(Duration::from_millis(2100));
        session.poll();
        assert!(!session.view.active);
        assert!(session.engine.is_none());
        assert_eq!(session.view.project.audio.midi, missing_id);
        assert!(
            session.view.message.contains("MIDI input unavailable"),
            "{}",
            session.view.message
        );
        session.apply(Action::Disable).unwrap();
    }
}
