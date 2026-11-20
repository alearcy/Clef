use clef_model::{
    Plugin, Result,
    midi::CompEdit,
    project::{self, AudioSettings, TrackMode},
};
use clef_plugin_host::{
    catalog::{self, Catalog},
    quarantine,
};
use clef_session::{Action, Backend, Devices, Snapshot, TransportAction};
use eframe::egui::{self, Color32, Key, KeyboardShortcut, Modifiers};
use std::{
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};
mod primitives {
    pub mod history;
    pub(crate) mod inserts;
    pub mod mixer;
    pub mod piano_roll;
    pub mod project;
    pub mod timeline;
    pub mod transport;
}
mod arrangement;
#[doc(hidden)]
pub mod gallery;
mod mixer;
mod piano_roll;
mod startup;
mod theme;
mod transport;

const BUSY_FEEDBACK_DELAY: Duration = Duration::from_millis(100);

mod colors {
    use super::Color32;
    pub const BACKGROUND: Color32 = Color32::from_rgb(0x10, 0x1a, 0x1c);
    pub const PANEL: Color32 = Color32::from_rgb(0x17, 0x23, 0x26);
    pub const FIELD: Color32 = Color32::from_rgb(0x1f, 0x2f, 0x32);
    pub const CANVAS: Color32 = Color32::from_rgb(0x11, 0x1c, 0x1f);
    pub const TEXT: Color32 = Color32::from_rgb(0xe3, 0xe8, 0xe5);
    pub const SECONDARY: Color32 = Color32::from_rgb(0xa4, 0xb5, 0xae);
    pub const ACCENT: Color32 = Color32::from_rgb(0x78, 0xb5, 0xaa);
    pub const PEACOCK: Color32 = Color32::from_rgb(0x33, 0x6a, 0x64);
    pub const DISABLED: Color32 = Color32::from_rgb(0x74, 0x84, 0x7d);
    pub const DANGER: Color32 = Color32::from_rgb(0xc7, 0x84, 0x7c);
    pub const DIVIDER: Color32 = Color32::from_rgb(0x2b, 0x3b, 0x3d);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Command {
    New,
    Open,
    Save,
    Undo,
    Redo,
    EditHistory,
    ShowMixer,
    MixerPrevious,
    MixerNext,
    MixerMute,
    MixerSolo,
    ResetMixerClip,
    InsertAdd,
    InsertAfter,
    InsertOpen,
    InsertBypass,
    InsertMoveUp,
    InsertMoveDown,
    InsertRemove,
    InstrumentBypass,
    InstrumentOffline,
    CreateClip,
    SelectAllClips,
    DuplicateClips,
    DeleteClips,
    SplitClips,
    MoveClipsLeft,
    MoveClipsRight,
    MoveClipsUp,
    MoveClipsDown,
    ResizeClipShorter,
    ResizeClipLonger,
    LoopClipShorter,
    LoopClipLonger,
    SaveAs,
    Close,
    OpenRecent,
    LocateRecent,
    RemoveRecent,
    ShowRecovery,
    RecoverProject,
    DiscardRecovery,
    RecoveryLater,
    Settings,
    Commands,
    AddTrack,
    AddBus,
    Instrument,
    Editor,
    CloseEditor,
    Restart,
    Rescan,
    Refresh,
    Rename,
    ApplySettings,
    RemoveTrack,
    MoveUp,
    MoveDown,
    PreviousTrack,
    NextTrack,
    Quarantine,
    Reactivate,
    Play,
    Record,
    ArmTrack,
    RecordMerge,
    RecordReplace,
    RecordNewTake,
    RecordTrackGlobal,
    RecordTrackMerge,
    RecordTrackReplace,
    RecordTrackNewTake,
    ShowTakeLanes,
    UseWholeTake,
    DeleteTake,
    FlattenComp,
    TakeToNewTrack,
    RenameTake,
    EditTake,
    Stop,
    PlayStop,
    ReturnToStart,
    ToggleLoop,
    ToggleClick,
    ToggleCountIn,
    TempoMap,
    TransportSettings,
    KeepStopPosition,
    ReturnStopPosition,
}

impl Command {
    fn is_clip(self) -> bool {
        use Command::*;
        matches!(
            self,
            CreateClip
                | SelectAllClips
                | DuplicateClips
                | DeleteClips
                | SplitClips
                | MoveClipsLeft
                | MoveClipsRight
                | MoveClipsUp
                | MoveClipsDown
                | ResizeClipShorter
                | ResizeClipLonger
                | LoopClipShorter
                | LoopClipLonger
        )
    }
}

struct Binding {
    command: Command,
    title: &'static str,
    shortcut: Option<KeyboardShortcut>,
}
fn commands() -> Vec<Binding> {
    use Command::*;
    [
        (New, "New Project", Some(Key::N)),
        (Open, "Open Project…", Some(Key::O)),
        (Save, "Save", Some(Key::S)),
        (SaveAs, "Save As…", None),
        (Redo, "Redo", Some(Key::Z)),
        (Undo, "Undo", Some(Key::Z)),
        (EditHistory, "Edit History…", None),
        (ShowMixer, "Show Mixer", Some(Key::M)),
        (MixerPrevious, "Mixer: Previous Channel", None),
        (MixerNext, "Mixer: Next Channel", None),
        (MixerMute, "Mixer: Toggle Mute", None),
        (MixerSolo, "Mixer: Toggle Solo", None),
        (ResetMixerClip, "Mixer: Clear Clip Indicator", None),
        (InsertAdd, "Add Audio Effect…", None),
        (InsertAfter, "Insert Effect After…", None),
        (InsertOpen, "Open Effect Editor", None),
        (InsertBypass, "Toggle Effect Bypass", None),
        (InsertMoveUp, "Move Effect Up", None),
        (InsertMoveDown, "Move Effect Down", None),
        (InsertRemove, "Remove Effect", None),
        (InstrumentBypass, "Toggle Instrument Bypass", None),
        (InstrumentOffline, "Toggle Instrument Offline", None),
        (CreateClip, "Create Clip", Some(Key::Enter)),
        (SelectAllClips, "Select All Clips", Some(Key::A)),
        (DuplicateClips, "Duplicate", Some(Key::D)),
        (DeleteClips, "Delete", Some(Key::Backspace)),
        (SplitClips, "Split at Playhead", Some(Key::E)),
        (MoveClipsLeft, "Move Clips Earlier", Some(Key::ArrowLeft)),
        (MoveClipsRight, "Move Clips Later", Some(Key::ArrowRight)),
        (MoveClipsUp, "Move Clips Up a Track", Some(Key::ArrowUp)),
        (
            MoveClipsDown,
            "Move Clips Down a Track",
            Some(Key::ArrowDown),
        ),
        (ResizeClipShorter, "Shorten Clip", Some(Key::ArrowLeft)),
        (ResizeClipLonger, "Lengthen Clip", Some(Key::ArrowRight)),
        (LoopClipShorter, "Shorten Clip Loop", Some(Key::ArrowLeft)),
        (LoopClipLonger, "Extend Clip Loop", Some(Key::ArrowRight)),
        (Close, "Close Clef", Some(Key::Q)),
        (OpenRecent, "Open", None),
        (LocateRecent, "Locate…", None),
        (RemoveRecent, "Remove", None),
        (ShowRecovery, "Recovery…", None),
        (RecoverProject, "Recover", None),
        (DiscardRecovery, "Discard…", None),
        (RecoveryLater, "Later", None),
        (Settings, "Audio/MIDI Settings", None),
        (Commands, "Commands / Keyboard Shortcuts", Some(Key::K)),
        (AddTrack, "Add Instrument Track", None),
        (AddBus, "Add Bus", None),
        (Instrument, "Choose Instrument", None),
        (Editor, "Open Editor", None),
        (CloseEditor, "Close Editor", None),
        (Restart, "Restart Instrument", None),
        (Rescan, "Rescan Plugins", None),
        (Refresh, "Refresh Devices", None),
        (Rename, "Rename Track", None),
        (ApplySettings, "Apply Audio/MIDI Settings", None),
        (RemoveTrack, "Remove Track…", None),
        (MoveUp, "Move Track Up", None),
        (MoveDown, "Move Track Down", None),
        (PreviousTrack, "Select Previous Track", None),
        (NextTrack, "Select Next Track", None),
        (Quarantine, "Show Quarantined Plugins…", None),
        (Reactivate, "Reactivate Plugin…", None),
        (Play, "Play", None),
        (Record, "Record MIDI", None),
        (ArmTrack, "Toggle Record Arm for Selected Track", None),
        (RecordMerge, "Record Mode: Merge", None),
        (RecordReplace, "Record Mode: Replace", None),
        (RecordNewTake, "Record Mode: New take", None),
        (RecordTrackGlobal, "Track Record Mode: Global", None),
        (RecordTrackMerge, "Track Record Mode: Merge", None),
        (RecordTrackReplace, "Track Record Mode: Replace", None),
        (RecordTrackNewTake, "Track Record Mode: New take", None),
        (ShowTakeLanes, "Show/Hide Take Lanes", None),
        (UseWholeTake, "Use Whole Take", None),
        (DeleteTake, "Delete Take", None),
        (FlattenComp, "Flatten Comp", None),
        (TakeToNewTrack, "Take to New Track", None),
        (RenameTake, "Rename Take", None),
        (EditTake, "Edit Take", None),
        (Stop, "Stop", None),
        (PlayStop, "Play / Stop", Some(Key::Space)),
        (ReturnToStart, "Return to Start", None),
        (ToggleLoop, "Loop", None),
        (ToggleClick, "Metronome", None),
        (ToggleCountIn, "One-Bar Count-In", None),
        (TempoMap, "Tempo and Meter Map…", None),
        (TransportSettings, "Transport Settings…", None),
        (KeepStopPosition, "Stop Mode: Keep Position", None),
        (ReturnStopPosition, "Stop Mode: Return to Play Start", None),
    ]
    .into_iter()
    .map(|(command, title, key)| Binding {
        command,
        title,
        shortcut: key.map(|key| {
            KeyboardShortcut::new(
                match command {
                    ShowMixer => Modifiers::COMMAND | Modifiers::SHIFT,
                    PlayStop => Modifiers::NONE,
                    DeleteClips => Modifiers::NONE,
                    MoveClipsLeft | MoveClipsRight | MoveClipsUp | MoveClipsDown => Modifiers::ALT,
                    ResizeClipShorter | ResizeClipLonger => Modifiers::ALT | Modifiers::SHIFT,
                    LoopClipShorter | LoopClipLonger => Modifiers::COMMAND | Modifiers::ALT,
                    Redo => Modifiers::COMMAND | Modifiers::SHIFT,
                    _ => Modifiers::COMMAND,
                },
                key,
            )
        }),
    })
    .collect()
}
#[derive(Clone)]
enum Transition {
    New,
    Open(PathBuf),
    Close,
}
enum Confirm {
    Unsaved(Transition),
    Replace(Plugin),
    Restart,
    Remove(u64),
    Reactivate(Plugin),
    Retained(bool),
    Overwrite(PathBuf),
    SplitClips(
        std::sync::Arc<clef_model::midi::Arrangement>,
        clef_model::midi::ClipEdit,
    ),
}
#[derive(Clone, Copy)]
enum FileMode {
    Open,
    Locate,
    Save,
}
struct FilePicker {
    mode: FileMode,
    folder: PathBuf,
    name: String,
    error: String,
    /// The listing of `folder`, read once per folder: navigating or reopening the picker
    /// reads it again, and frames do no directory I/O.
    listing: Option<std::result::Result<Vec<PickerEntry>, String>>,
}
struct PickerEntry {
    path: PathBuf,
    name: String,
    folder: bool,
}
fn list_folder(folder: &std::path::Path) -> std::result::Result<Vec<PickerEntry>, String> {
    let mut entries: Vec<_> = std::fs::read_dir(folder)
        .map_err(|error| error.to_string())?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter_map(|path| {
            let folder = path.is_dir();
            let name = path.file_name()?.to_string_lossy().into_owned();
            (!name.starts_with('.') && (folder || path.extension().is_some_and(|e| e == "clef")))
                .then_some(PickerEntry { path, name, folder })
        })
        .collect();
    entries.sort_by(|a, b| (!a.folder, a.path.file_name()).cmp(&(!b.folder, b.path.file_name())));
    Ok(entries)
}
impl FilePicker {
    fn new(mode: FileMode, current: Option<&PathBuf>) -> Self {
        let folder = current
            .and_then(|p| p.parent().map(|p| p.to_owned()))
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("/"))
            });
        Self {
            mode,
            folder,
            name: current
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled.clef".into()),
            error: String::new(),
            listing: None,
        }
    }
}

unsafe extern "C" {
    fn clef_install_quit_gate() -> bool;
    fn clef_take_quit_request() -> bool;
    fn clef_remove_quit_gate();
}
struct QuitGate;
impl Drop for QuitGate {
    fn drop(&mut self) {
        unsafe {
            clef_remove_quit_gate();
        }
    }
}

pub fn run(executable: PathBuf) -> Result<()> {
    eframe::run_native(
        "Clef",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1024.0, 640.0])
                .with_min_inner_size([760.0, 480.0]),
            ..Default::default()
        },
        Box::new(move |cc| {
            let mut app = App::new(cc.egui_ctx.clone(), executable);
            if !unsafe { clef_install_quit_gate() } {
                return Err("Could not install cancellable application quit".into());
            }
            app.quit_gate = Some(QuitGate);
            app.refresh_devices(cc.egui_ctx.clone());
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| e.to_string().into())
}

struct App {
    transport_ui: transport::TransportUi,
    arrangement_ui: arrangement::ArrangementUi,
    piano_roll: piano_roll::PianoRoll,
    piano_roll_open: bool,
    mixer_ui: mixer::MixerUi,
    executable: PathBuf,
    quit_gate: Option<QuitGate>,
    backend: Backend,
    view: Snapshot,
    actions_in_flight: usize,
    busy_since: Option<Instant>,
    busy_visible: bool,
    pending_after_save: Option<Transition>,
    awaiting_save: bool,
    awaiting_close: bool,
    startup: startup::Startup,
    exiting: bool,
    confirm: Option<Confirm>,
    file: Option<FilePicker>,
    settings_open: bool,
    /// Audio/MIDI Settings applies to this project only (an override) instead of all projects.
    settings_project: bool,
    settings: AudioSettings,
    devices: Devices,
    device_job: Option<mpsc::Receiver<std::result::Result<Devices, String>>>,
    picker_open: bool,
    catalog: Catalog,
    scan_job: Option<crate::catalog::ScanJob>,
    plugin_search: String,
    bindings: Vec<Binding>,
    commands_open: bool,
    history_window: primitives::history::HistoryWindow,
    command_search: String,
    rebinding: Option<usize>,
    rename: String,
    status: String,
    retained_approved: Vec<(u64, u64)>,
    quarantine_open: bool,
    quarantine_notice: bool,
    last_notice: u64,
    track_focus: Option<u64>,
    rename_focus: bool,
    keyboard_mode: bool,
}
impl App {
    fn new(context: egui::Context, executable: PathBuf) -> Self {
        primitives::project::install_font(&context);
        let mut style = (*context.global_style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = colors::BACKGROUND;
        style.visuals.window_fill = colors::PANEL;
        style.visuals.extreme_bg_color = colors::FIELD;
        style.visuals.faint_bg_color = colors::PANEL;
        style.visuals.override_text_color = Some(colors::TEXT);
        style.visuals.selection.bg_fill = colors::PEACOCK;
        style.visuals.selection.stroke = egui::Stroke::new(1.0_f32, colors::ACCENT);
        for widget in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.open,
        ] {
            widget.bg_fill = colors::PANEL;
            widget.weak_bg_fill = colors::PANEL;
            widget.bg_stroke = egui::Stroke::NONE;
            widget.corner_radius = egui::CornerRadius::same(2);
        }
        style.visuals.widgets.inactive.bg_fill = colors::PANEL;
        style.visuals.widgets.inactive.weak_bg_fill = colors::PANEL;
        style.visuals.widgets.hovered.bg_fill = colors::FIELD;
        style.visuals.widgets.hovered.weak_bg_fill = colors::FIELD;
        style.visuals.widgets.active.bg_fill = colors::FIELD;
        style.visuals.widgets.active.weak_bg_fill = colors::FIELD;
        style.visuals.widgets.open.bg_fill = colors::FIELD;
        style.visuals.widgets.open.weak_bg_fill = colors::FIELD;
        style.spacing.item_spacing = egui::vec2(12.0, 8.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.spacing.interact_size.y = 24.0;
        style.interaction.selectable_labels = false;
        context.set_global_style(style);
        Self {
            transport_ui: transport::TransportUi::default(),
            arrangement_ui: arrangement::ArrangementUi::default(),
            piano_roll: piano_roll::PianoRoll::default(),
            piano_roll_open: false,
            mixer_ui: mixer::MixerUi::default(),
            quit_gate: None,
            backend: Backend::spawn(executable.clone(), move || context.request_repaint()),
            executable,
            view: Snapshot::default(),
            actions_in_flight: 0,
            busy_since: None,
            busy_visible: false,
            pending_after_save: None,
            awaiting_save: false,
            awaiting_close: false,
            startup: startup::Startup::default(),
            exiting: false,
            confirm: None,
            file: None,
            settings_open: false,
            settings_project: false,
            settings: AudioSettings::default(),
            devices: Devices::default(),
            device_job: None,
            picker_open: false,
            catalog: Catalog::default(),
            scan_job: None,
            plugin_search: String::new(),
            bindings: commands(),
            commands_open: false,
            history_window: primitives::history::HistoryWindow::default(),
            command_search: String::new(),
            rebinding: None,
            rename: String::new(),
            status: "Create a project or open an existing .clef file.".into(),
            retained_approved: Vec::new(),
            quarantine_open: false,
            quarantine_notice: false,
            last_notice: 0,
            track_focus: None,
            rename_focus: false,
            keyboard_mode: false,
        }
    }
    fn send(&mut self, action: Action) {
        if self.busy_since.is_some() && !matches!(action, Action::Transport(TransportAction::Stop))
        {
            return;
        }
        // A mixer drag survives another action: its release commits when the session is idle
        // (T37 L3). Escape, focus loss and dialogs still cancel it in `mixer_logic`.
        let loop_range = match &action {
            Action::Transport(TransportAction::SetLoopRange(range)) => Some(*range),
            _ => None,
        };
        let record_submitted = matches!(&action, Action::Transport(TransportAction::Record));
        match self.backend.send.send(action) {
            Ok(()) => {
                self.actions_in_flight += 1;
                self.busy_since.get_or_insert_with(Instant::now);
                if let Some(range) = loop_range {
                    self.transport_ui.pending_loop_range = Some(range);
                }
                self.transport_ui.record_submitted |= record_submitted;
                self.status.clear();
            }
            Err(_) => {
                self.status =
                    "Session service unavailable. Save recovery is unavailable; restart Clef."
                        .into()
            }
        }
    }
    fn enabled(&self, command: Command) -> bool {
        let may_interrupt = command == Command::Stop
            || command == Command::PlayStop && self.play_stop_command() == Command::Stop;
        (self.busy_since.is_none() || may_interrupt) && self.command_available(command, true)
    }
    fn command_available(&self, command: Command, wait_for_transport: bool) -> bool {
        if self.confirm.is_some() || self.file.is_some() {
            return false;
        }
        use Command::*;
        if !self.view.project_open {
            return self.startup.command_available(command, &self.view);
        }
        if self.view.recording {
            match command {
                New | Open | Close | Save | SaveAs | ApplySettings => return false,
                RemoveTrack | Instrument | Restart if self.view.selected == self.view.armed => {
                    return false;
                }
                _ => {}
            }
        }
        match command {
            command if command.is_clip() => {
                !self.mixer_ui.open && self.clip_command_enabled(command)
            }
            ShowMixer | MixerPrevious | MixerNext | MixerMute | MixerSolo | ResetMixerClip => {
                self.mixer_command_available(command)
            }
            InsertAdd | InsertAfter | InsertOpen | InsertBypass | InsertMoveUp | InsertMoveDown
            | InsertRemove => self.insert_command_available(command),
            InstrumentBypass | InstrumentOffline => self.instrument_command_available(command),
            Undo => self.view.edit_history.available && self.view.edit_history.cursor > 0,
            Redo => {
                self.view.edit_history.available
                    && self.view.edit_history.cursor < self.view.edit_history.labels.len()
            }
            OpenRecent | LocateRecent | RemoveRecent | ShowRecovery | RecoverProject
            | DiscardRecovery | RecoveryLater => false,
            AddTrack | AddBus => self.view.project.tracks.len() < crate::project::MAX_TRACKS,
            RemoveTrack | MoveUp | MoveDown | PreviousTrack | NextTrack => {
                let index = self
                    .view
                    .project
                    .tracks
                    .iter()
                    .position(|t| Some(t.id) == self.view.selected);
                match command {
                    MoveUp | PreviousTrack => index.is_some_and(|i| i > 0),
                    MoveDown | NextTrack => {
                        index.is_some_and(|i| i + 1 < self.view.project.tracks.len())
                    }
                    _ => index.is_some(),
                }
            }
            Quarantine => true,
            Reactivate => self
                .view
                .track()
                .and_then(|t| t.plugin.as_ref())
                .is_some_and(|p| {
                    self.view
                        .quarantine
                        .iter()
                        .any(|e| crate::quarantine::same(&e.plugin, p))
                }),
            Save | SaveAs | ApplySettings if self.view.tracks.iter().any(|t| t.loading) => false,
            Instrument => self
                .view
                .track()
                .is_some_and(|track| !track.instrument_offline),
            Rename => self.view.track().is_some(),
            Editor => {
                self.view
                    .track()
                    .is_some_and(|t| t.plugin.is_some() && !t.instrument_offline)
                    && !self.view.failed
            }
            CloseEditor => self.view.editor,
            Restart => self
                .view
                .track()
                .is_some_and(|t| t.plugin.is_some() && !t.instrument_offline),
            Rescan => self.scan_job.is_none(),
            Refresh => self.device_job.is_none(),
            ApplySettings => self.settings_open,
            Play | Record | ArmTrack | RecordMerge | RecordReplace | RecordNewTake | Stop
            | PlayStop | ReturnToStart | ToggleLoop | ToggleClick | ToggleCountIn | TempoMap
            | TransportSettings | KeepStopPosition | ReturnStopPosition => {
                self.transport_enabled(command, wait_for_transport)
            }
            RecordTrackGlobal | RecordTrackMerge | RecordTrackReplace | RecordTrackNewTake => {
                self.view.selected.is_some()
                    && !self.view.recording
                    && !self.view.transport.pending
                    && self.busy_since.is_none()
                    && self.view.tracks.iter().all(|track| !track.loading)
            }
            ShowTakeLanes => {
                self.view
                    .selected
                    .and_then(|id| self.view.project.tracks.iter().find(|t| t.id == id))
                    .is_some_and(|track| {
                        crate::arrangement::ArrangementUi::take_count(
                            &self.view.project.midi,
                            track.id,
                        ) >= 2
                    })
                    && !self.view.recording
            }
            UseWholeTake | DeleteTake | FlattenComp | TakeToNewTrack | RenameTake | EditTake => {
                self.take_command_enabled(command)
            }
            _ => true,
        }
    }
    fn button(&self, ui: &mut egui::Ui, command: Command) -> bool {
        let binding = self.bindings.iter().find(|b| b.command == command).unwrap();
        let hint = binding
            .shortcut
            .map(|s| ui.ctx().format_shortcut(&s))
            .unwrap_or_default();
        let title = match command {
            Command::ShowMixer if self.mixer_ui.open => "Sequencer",
            Command::ShowMixer => "Mixer",
            Command::Settings if self.mixer_ui.open => "Settings",
            Command::Commands if self.mixer_ui.open => "Commands",
            _ => binding.title,
        };
        let description = self.command_label(binding);
        ui.add_enabled(
            self.command_available(command, false),
            egui::Button::new(title),
        )
        .on_hover_text(if hint.is_empty() {
            description
        } else {
            format!("{description} ({hint})")
        })
        .clicked()
            && self.enabled(command)
    }
    fn has_unsaved_changes(&self) -> bool {
        self.view.dirty || self.view.track().is_some_and(|t| t.name != self.rename)
    }
    fn transition(&mut self, next: Transition, ctx: &egui::Context) {
        if self.view.recording {
            self.status = "Stop recording before leaving the project.".into();
            return;
        }
        if self.has_unsaved_changes() {
            self.confirm = Some(Confirm::Unsaved(next));
        } else {
            self.perform(next, ctx);
        }
    }
    fn perform(&mut self, next: Transition, ctx: &egui::Context) {
        match next {
            Transition::New => {
                self.rename.clear();
                self.send(Action::New);
            }
            Transition::Open(path) => {
                self.rename.clear();
                self.send(Action::Open(path));
            }
            Transition::Close => {
                let action = if self.has_unsaved_changes() {
                    Some(Action::DiscardBeforeClose)
                } else {
                    // Only the view changed: the session writes it silently (T37 L4).
                    self.view
                        .view_unsaved
                        .then_some(Action::SaveViewBeforeClose)
                };
                let actions = self.actions_in_flight;
                if let Some(action) = action {
                    self.awaiting_close = true;
                    self.send(action);
                }
                if self.actions_in_flight == actions {
                    // Nothing to send, or the session is unavailable: close at once.
                    self.awaiting_close = false;
                    self.exiting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }
    fn save(&mut self, as_new: bool) {
        if self
            .view
            .retained_losses()
            .iter()
            .any(|loss| !self.retained_approved.contains(loss))
        {
            self.confirm = Some(Confirm::Retained(as_new));
            return;
        }
        if !as_new
            && !self.view.read_only
            && let Some(path) = self.view.path.clone()
        {
            self.awaiting_save = true;
            self.send(Action::Save {
                path,
                replace: true,
                track_name: self.view.selected.map(|id| (id, self.rename.clone())),
                use_retained: self.retained_approved.clone(),
            });
        } else {
            let mut picker = FilePicker::new(
                FileMode::Save,
                self.view
                    .path
                    .as_ref()
                    .or(self.view.recovered_from.as_ref()),
            );
            if let Some(stem) = self
                .view
                .recovered_from
                .as_ref()
                .and_then(|p| p.file_stem())
            {
                picker.name = format!("{}-recovered.clef", stem.to_string_lossy());
            } else if self.view.read_only
                && let Some(stem) = self.view.path.as_ref().and_then(|p| p.file_stem())
            {
                picker.name = format!("{}-copy.clef", stem.to_string_lossy());
            }
            self.file = Some(picker);
        }
    }
    fn refresh_devices(&mut self, ctx: egui::Context) {
        if self.device_job.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.device_job = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(Devices::enumerate().map_err(|e| e.to_string()));
            ctx.request_repaint();
        });
    }
    fn scan(&mut self, ctx: egui::Context) {
        if self.scan_job.is_some() {
            return;
        }
        self.scan_job = Some(crate::catalog::ScanJob::spawn(
            self.executable.clone(),
            move || ctx.request_repaint(),
        ));
    }

    fn dispatch(&mut self, command: Command, ctx: &egui::Context) {
        if !self.enabled(command) {
            return;
        }
        use Command::*;
        match command {
            CreateClip | SelectAllClips | DuplicateClips | DeleteClips | SplitClips
            | MoveClipsLeft | MoveClipsRight | MoveClipsUp | MoveClipsDown | ResizeClipShorter
            | ResizeClipLonger | LoopClipShorter | LoopClipLonger => {
                self.dispatch_clip_command(command)
            }
            New => self.transition(Transition::New, ctx),
            Open => self.file = Some(FilePicker::new(FileMode::Open, self.view.path.as_ref())),
            Save => self.save(false),
            SaveAs => self.save(true),
            Undo => self.seek_history(self.view.edit_history.cursor - 1),
            Redo => self.seek_history(self.view.edit_history.cursor + 1),
            EditHistory => self.history_window.open = true,
            ShowMixer | MixerPrevious | MixerNext | MixerMute | MixerSolo | ResetMixerClip => {
                self.dispatch_mixer(command)
            }
            InsertAdd | InsertAfter | InsertOpen | InsertBypass | InsertMoveUp | InsertMoveDown
            | InsertRemove => self.dispatch_insert(command, ctx),
            InstrumentBypass | InstrumentOffline => self.dispatch_instrument(command, ctx),
            Close => self.transition(Transition::Close, ctx),
            OpenRecent | LocateRecent | RemoveRecent | ShowRecovery | RecoverProject
            | DiscardRecovery | RecoveryLater => self.dispatch_startup(command, ctx),
            Settings => {
                // T39: edit the override when the project has one, else the global preferences.
                self.settings_project = self.view.project.audio.is_some();
                self.settings = self
                    .view
                    .project
                    .audio
                    .clone()
                    .or_else(|| self.view.audio_preferences.clone())
                    .or_else(|| self.view.audio.clone())
                    .unwrap_or_default();
                self.settings_open = true;
                self.refresh_devices(ctx.clone());
            }
            Commands => self.commands_open = true,
            AddTrack => {
                let rename = self
                    .view
                    .track()
                    .filter(|t| t.name != self.rename)
                    .map(|t| (t.id, self.rename.clone()));
                self.send(Action::AddTrack(rename));
            }
            AddBus => self.send(Action::AddBus { assign: None }),
            Instrument => {
                self.picker_open = true;
                if self.catalog.plugins.is_empty() {
                    self.scan(ctx.clone());
                }
            }
            Editor => {
                if let Some(id) = self.view.selected {
                    self.send(Action::Editor(id, true));
                }
            }
            CloseEditor => {
                if let Some(id) = self.view.selected {
                    self.send(Action::Editor(id, false));
                }
            }
            Restart => self.confirm = Some(Confirm::Restart),
            Rescan => self.scan(ctx.clone()),
            Refresh => self.refresh_devices(ctx.clone()),
            Rename => {
                if let Some(id) = self.view.selected {
                    self.send(Action::Rename(id, self.rename.clone()));
                }
            }
            RemoveTrack => {
                if let Some(id) = self.view.selected {
                    self.confirm = Some(Confirm::Remove(id));
                }
            }
            MoveUp | MoveDown => {
                if let Some(id) = self.view.selected {
                    self.send(Action::MoveTrack(
                        id,
                        if command == MoveUp { -1 } else { 1 },
                    ));
                }
            }
            PreviousTrack | NextTrack => {
                if let Some(index) = self
                    .view
                    .project
                    .tracks
                    .iter()
                    .position(|t| Some(t.id) == self.view.selected)
                {
                    let next = if command == PreviousTrack {
                        index.saturating_sub(1)
                    } else {
                        (index + 1).min(self.view.project.tracks.len() - 1)
                    };
                    self.select_track(self.view.project.tracks[next].id);
                }
            }
            Quarantine => self.quarantine_open = true,
            Reactivate => {
                if let Some(plugin) = self.view.track().and_then(|t| t.plugin.clone()) {
                    self.confirm = Some(Confirm::Reactivate(plugin));
                }
            }
            ApplySettings => self.send(Action::Settings {
                audio: self.settings.clone(),
                project: self.settings_project,
            }),
            Play | Record | ArmTrack | RecordMerge | RecordReplace | RecordNewTake | Stop
            | PlayStop | ReturnToStart | ToggleLoop | ToggleClick | ToggleCountIn | TempoMap
            | TransportSettings | KeepStopPosition | ReturnStopPosition => {
                self.dispatch_transport(command)
            }
            RecordTrackGlobal | RecordTrackMerge | RecordTrackReplace | RecordTrackNewTake => {
                let mode = match command {
                    RecordTrackGlobal => TrackMode::Global,
                    RecordTrackMerge => TrackMode::Merge,
                    RecordTrackReplace => TrackMode::Replace,
                    _ => TrackMode::NewTake,
                };
                if let Some(id) = self.view.selected {
                    self.send(Action::Transport(TransportAction::SetTrackRecordMode(
                        id, mode,
                    )));
                }
            }
            ShowTakeLanes => {
                if let Some(track) = self.view.track() {
                    let mut lanes = track.take_lanes.clone();
                    lanes.visible = !lanes.visible;
                    if !lanes.visible {
                        self.arrangement_ui.focused_lane = None;
                    }
                    self.send(Action::TrackView {
                        track: track.id,
                        lanes,
                    });
                }
            }
            UseWholeTake | DeleteTake | FlattenComp | TakeToNewTrack | RenameTake | EditTake => {
                self.dispatch_take_command(command);
            }
        }
    }

    fn dispatch_take_command(&mut self, command: Command) {
        let Some((track, lane, clip)) = self.arrangement_ui.take_lane_target() else {
            return;
        };
        use Command::*;
        match command {
            UseWholeTake => self.apply_take_action(crate::arrangement::TakeAction::Comp {
                track,
                lane,
                edit: CompEdit::UseWholeTake,
            }),
            DeleteTake => self.apply_take_action(crate::arrangement::TakeAction::Comp {
                track,
                lane,
                edit: CompEdit::DeleteTake,
            }),
            FlattenComp => self.apply_take_action(crate::arrangement::TakeAction::Comp {
                track,
                lane,
                edit: CompEdit::Flatten,
            }),
            TakeToNewTrack => self
                .apply_take_action(crate::arrangement::TakeAction::TakeToNewTrack { track, lane }),
            RenameTake => {
                let name = self
                    .view
                    .project
                    .tracks
                    .iter()
                    .find(|item| item.id == track)
                    .map(|item| item.take_lanes.name(lane))
                    .unwrap_or_default();
                self.arrangement_ui.begin_take_rename(track, lane, name);
            }
            EditTake => {
                self.apply_take_action(crate::arrangement::TakeAction::EditTake {
                    track,
                    lane,
                    clip,
                });
            }
            _ => {}
        }
    }

    fn take_command_enabled(&self, command: Command) -> bool {
        let Some((track, lane)) = self.arrangement_ui.focused_take_lane() else {
            return false;
        };
        let Some(track) = self
            .view
            .project
            .tracks
            .iter()
            .find(|item| item.id == track)
        else {
            return false;
        };
        let present = self
            .view
            .project
            .midi
            .clips(track.id)
            .iter()
            .filter(|clip| lane < clip.takes.len())
            .count();
        match command {
            Command::RenameTake => present > 0,
            Command::EditTake => present > 0 && !self.view.recording,
            _ => present > 0 && !self.view.recording && self.view.edit_history.available,
        }
    }
    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok((view, completed)) = self.backend.receive.try_recv() {
            if self.view.selected != view.selected {
                self.mixer_ui.follow_instrument_selection(view.selected);
            }
            if self.history_window.pending
                && let Some(success) = completed
            {
                self.history_window.pending = false;
                self.history_window.error = (!success).then(|| view.message.clone());
            }
            if self.view.selected != view.selected
                || self.rename.is_empty()
                || self.view.track().is_none_or(|t| t.name == self.rename)
                || view.track().is_none()
            {
                self.rename = view.track().map(|t| t.name.clone()).unwrap_or_default();
            }
            if self.last_notice != view.notice {
                self.last_notice = view.notice;
                self.quarantine_notice = view.project.tracks.iter().any(|t| {
                    t.plugin.as_ref().is_some_and(|p| {
                        view.quarantine
                            .iter()
                            .any(|e| crate::quarantine::same(&e.plugin, p))
                    })
                });
            }
            self.arrangement_ui.acknowledge(
                self.view.project_epoch != view.project_epoch,
                completed.is_some(),
                view.clip_edit_selection.as_deref(),
                &view.project.midi,
                view.project.arrangement_view,
            );
            let project_changed = self.view.project_epoch != view.project_epoch;
            if completed == Some(false) && self.mixer_ui.commit_pending() {
                self.status.clone_from(&view.message);
            }
            if project_changed || completed.is_some() {
                self.mixer_ui.acknowledge(project_changed);
            }
            let midi_changed = !std::sync::Arc::ptr_eq(&self.view.project.midi, &view.project.midi);
            self.piano_roll
                .complete(&view, completed.is_some(), project_changed, midi_changed);
            if project_changed {
                self.piano_roll_open = false;
            }
            self.view = view;
            self.startup.update(&self.view);
            if let Some(success) = completed {
                self.actions_in_flight -= 1;
                if self.actions_in_flight == 0 {
                    self.busy_since = None;
                }
                self.transport_ui.pending_loop_range = None;
                self.transport_ui.record_submitted = false;
                if self.awaiting_close {
                    self.awaiting_close = false;
                    if success {
                        self.exiting = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                if self.awaiting_save {
                    self.awaiting_save = false;
                    self.retained_approved.clear();
                    if success {
                        if let Some(next) = self.pending_after_save.take() {
                            self.perform(next, ctx);
                        }
                    } else {
                        self.pending_after_save = None;
                    }
                }
            }
        }
        if let Some(job) = &self.scan_job
            && let Ok(report) = job.receive.try_recv()
        {
            self.catalog = report;
            self.scan_job = None;
        }
        if let Some(job) = &self.device_job
            && let Ok(result) = job.try_recv()
        {
            match result {
                Ok(devices) => self.devices = devices,
                Err(e) => {
                    self.status =
                        format!("Device enumeration failed: {e}. Refresh Devices to retry.")
                }
            }
            self.device_job = None;
        }
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if let Some(index) = self.rebinding {
            let event = ctx.input(|i| {
                i.events.iter().find_map(|e| {
                    if let egui::Event::Key {
                        key,
                        modifiers,
                        pressed: true,
                        repeat: false,
                        ..
                    } = e
                    {
                        Some((*key, *modifiers))
                    } else {
                        None
                    }
                })
            });
            if let Some((key, modifiers)) = event {
                if key == Key::Escape {
                    self.rebinding = None;
                } else if modifiers.command || modifiers.ctrl || modifiers.alt {
                    let shortcut = KeyboardShortcut::new(modifiers, key);
                    for binding in &mut self.bindings {
                        if binding.shortcut == Some(shortcut) {
                            binding.shortcut = None;
                        }
                    }
                    self.bindings[index].shortcut = Some(shortcut);
                    self.rebinding = None;
                }
            }
            return;
        }
        let text_has_focus = ctx
            .memory(|memory| memory.focused())
            .is_some_and(|id| egui::TextEdit::load_state(ctx, id).is_some());
        let selected = self
            .bindings
            .iter()
            .filter(|b| self.enabled(b.command))
            .filter(|b| b.command != Command::PlayStop || !self.mixer_ui.open || !primitives::mixer::activation_has_focus(ctx))
            .filter(|b| !b.command.is_clip() || !text_has_focus && self.arrangement_ui.focused(ctx))
            .filter(|b| !text_has_focus || !matches!(b.command, Command::Undo | Command::Redo))
            .find_map(|b| {
                b.shortcut
                    .filter(|s| {
                        (s.modifiers != Modifiers::NONE
                            || !ctx.output(|output| output.ime.is_some()))
                            && ctx.input_mut(|i| {
                                if !b.command.is_clip() && !matches!(b.command, Command::Undo | Command::Redo) {
                                    return i.consume_shortcut(s);
                                }
                                let mut consumed = false;
                                i.events.retain(|event| {
                                    let matched = matches!(event, egui::Event::Key {
                                        key, modifiers, pressed: true, ..
                                    } if *key == s.logical_key && modifiers.matches_exact(s.modifiers));
                                    consumed |= matched;
                                    !matched
                                });
                                consumed
                            })
                    })
                    .map(|_| b.command)
            });
        if let Some(command) = selected {
            self.dispatch(command, ctx);
        }
    }
    fn track_row(
        ui: &mut egui::Ui,
        name: &str,
        caption: &str,
        width: f32,
        selected: bool,
        enabled: bool,
        keyboard_mode: bool,
    ) -> egui::Response {
        let full = format!("{name}\n{caption}");
        let response = ui.add_enabled(
            enabled,
            egui::Button::new("")
                .fill(colors::PANEL)
                .stroke(if selected {
                    egui::Stroke::new(1.5_f32, colors::ACCENT.gamma_multiply(0.8))
                } else {
                    egui::Stroke::NONE
                })
                .min_size(egui::vec2(width, theme::ARRANGEMENT_TRACK_HEIGHT)),
        );
        if response.hovered() && !selected {
            ui.painter()
                .rect_filled(response.rect.shrink(2.0), 2.0, colors::FIELD);
        }
        if response.has_focus() && keyboard_mode {
            ui.painter().rect_stroke(
                response.rect.shrink(4.0),
                2.0,
                egui::Stroke::new(1.0_f32, colors::ACCENT),
                egui::StrokeKind::Inside,
            );
        }
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, selected, &full)
        });
        for (text, size, color, y) in [
            (name, 13.0, colors::TEXT, 6.0),
            (caption, 11.0, colors::SECONDARY, 24.0),
        ] {
            let color = if enabled { color } else { colors::DISABLED };
            let mut job = egui::text::LayoutJob::simple_singleline(
                text.to_owned(),
                egui::FontId::proportional(size),
                color,
            );
            job.wrap.max_width = (width - 16.0).max(1.0);
            job.wrap.max_rows = 1;
            let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
            ui.painter()
                .galley(response.rect.left_top() + egui::vec2(8.0, y), galley, color);
        }
        response.on_hover_text(full)
    }
    fn select_track(&mut self, id: u64) {
        let rename = self
            .view
            .track()
            .filter(|t| t.name != self.rename)
            .map(|t| (t.id, self.rename.clone()));
        self.track_focus = Some(id);
        self.send(Action::SelectTrack(id, rename));
    }
    fn quarantine_windows(&mut self, ctx: &egui::Context) {
        if self.quarantine_notice {
            let mut dismiss = false;
            let mut show = false;
            let notice=egui::Modal::new(egui::Id::new("quarantine-notice")).show(ctx,|ui| {
                ui.set_max_width(560.0); ui.heading("This project contains quarantined plugins");
                ui.label("Saved states are retained. These instruments will not load until explicitly reactivated.");
                egui::ScrollArea::vertical().max_height(240.0).show(ui,|ui| {
                    for track in &self.view.project.tracks { if let Some(plugin)=&track.plugin && let Some(entry)=self.view.quarantine.iter().find(|e|crate::quarantine::same(&e.plugin,plugin)) {ui.label(format!("{} · {} · {}",plugin.name,track.name,entry.reason));} }
                });
                if ui.button("Show Quarantined Plugins…").clicked() {show=true; dismiss=true;}
                if ui.button("Continue").clicked() || ui.input(|i|i.key_pressed(Key::Enter)||i.key_pressed(Key::Escape)) {dismiss=true;}
            });
            if dismiss || notice.should_close() {
                self.quarantine_notice = false;
            }
            if show {
                self.quarantine_open = true;
            }
        }
        if self.quarantine_open {
            let mut open = true;
            let mut reactivate = None;
            egui::Window::new("Quarantined Plugins")
                .open(&mut open)
                .default_width(560.0)
                .show(ctx, |ui| {
                    ui.label(
                        "Reactivation clears the block; instruments load only on explicit use.",
                    );
                    if self.view.quarantine.is_empty() {
                        ui.label("No quarantined plugins.");
                    }
                    egui::ScrollArea::vertical()
                        .max_height(320.0)
                        .show(ui, |ui| {
                            for entry in &self.view.quarantine {
                                ui.label(format!(
                                    "{} · {}",
                                    entry.plugin.name, entry.plugin.format
                                ));
                                ui.colored_label(colors::SECONDARY, &entry.reason);
                                if ui
                                    .add_enabled(
                                        self.busy_since.is_none(),
                                        egui::Button::new("Reactivate Plugin…"),
                                    )
                                    .clicked()
                                {
                                    reactivate = Some(entry.plugin.clone());
                                }
                            }
                        });
                    if !self.view.quarantine_error.is_empty() {
                        ui.colored_label(colors::DANGER, &self.view.quarantine_error);
                    }
                });
            self.quarantine_open = open;
            if let Some(plugin) = reactivate {
                self.confirm = Some(Confirm::Reactivate(plugin));
            }
        }
    }
    fn main_surface(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut selected = None;
        ui.horizontal_wrapped(|ui| {
            for command in [Command::New, Command::Open, Command::Save, Command::SaveAs] {
                if self.button(ui, command) {
                    selected = Some(command);
                }
            }
            let commands = [Command::Undo, Command::Redo, Command::EditHistory];
            let rows = commands.map(|command| {
                let binding = self
                    .bindings
                    .iter()
                    .find(|binding| binding.command == command)
                    .unwrap();
                let label = self.command_label(binding);
                let shortcut = binding
                    .shortcut
                    .map(|shortcut| ctx.format_shortcut(&shortcut))
                    .unwrap_or_default();
                (label, shortcut, self.enabled(command))
            });
            if let Some(index) = primitives::history::edit_menu(
                ui,
                rows.each_ref().map(|(label, shortcut, enabled)| {
                    (label.as_str(), shortcut.as_str(), *enabled)
                }),
            ) {
                selected = Some(commands[index]);
            }
            for command in [Command::ShowMixer, Command::Settings, Command::Commands] {
                if self.button(ui, command) {
                    selected = Some(command);
                }
            }
        });
        let project_title = |ui: &mut egui::Ui| {
            ui.heading(
                self.view
                    .path
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Untitled".into()),
            );
            if self.has_unsaved_changes() {
                ui.colored_label(colors::SECONDARY, "Unsaved changes");
            }
        };
        let audio_control = |ui: &mut egui::Ui| {
            ui.set_min_height(ui.spacing().interact_size.y);
            ui.colored_label(
                if self.view.active {
                    colors::ACCENT
                } else {
                    colors::SECONDARY
                },
                if self.view.active {
                    "Audio active"
                } else {
                    "Audio off"
                },
            );
            if self.busy_visible {
                ui.spinner();
                ui.label("Working…");
            }
        };
        if self.piano_roll_open || self.mixer_ui.open {
            ui.horizontal_wrapped(|ui| {
                project_title(ui);
                audio_control(ui);
            });
        } else {
            ui.add_space(20.0);
            ui.horizontal(project_title);
            ui.add_space(12.0);
            ui.horizontal_wrapped(audio_control);
        }
        ui.add_space(12.0);
        selected = self.transport_bar(ui).or(selected);
        ui.add_space(8.0);
        if self.mixer_ui.open {
            self.show_mixer(ui);
            if let Some(command) = selected {
                self.dispatch(command, ctx);
            }
            return;
        }
        let mut choose = None;
        let mut transport_action = None;
        let mut track_mode = None;
        let height = (ui.available_height() - 78.0).max(150.0);
        let arrangement_height = if self.piano_roll_open {
            self.piano_roll.split_height(height)
        } else {
            height
        };
        let details_width = if ui.available_width() >= 920.0 {
            292.0
        } else {
            244.0
        };
        let arrangement_width =
            (ui.available_width() - details_width - ui.spacing().item_spacing.x).max(300.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(arrangement_width);
                let result = self.timeline_surface(ui, arrangement_height, arrangement_width);
                choose = result.0;
                transport_action = result.2;
                if result.1 { selected = Some(Command::AddTrack); }
            });
            egui::Frame::new().fill(colors::PANEL).inner_margin(12.0).show(ui, |ui| { ui.vertical(|ui| {
                ui.set_width(details_width - 24.0);
                egui::ScrollArea::vertical().id_salt("track-details").max_height(arrangement_height).show(ui,|ui| {
                    if let Some(track) = self.view.track().cloned() {
                        ui.label(egui::RichText::new("Selected instrument track").size(16.0));
                        let label = ui.label("Track name");
                        let edit = ui.add_enabled(!self.busy_visible,egui::TextEdit::singleline(&mut self.rename).id_salt(("track-name",track.id)).desired_width(f32::INFINITY)).labelled_by(label.id);
                        if self.rename_focus { edit.request_focus(); self.rename_focus=false; }
                        if edit.has_focus() && ui.input_mut(|i|i.consume_key(Modifiers::NONE,Key::Escape)) { self.rename=track.name.clone(); }
                        if edit.lost_focus() && ui.input(|i|i.key_pressed(Key::Enter)) { selected=Some(Command::Rename); }
                        ui.horizontal_wrapped(|ui| { for command in [Command::Rename,Command::MoveUp,Command::MoveDown,Command::RemoveTrack] {
                            if self.button(ui,command) {selected=Some(command);}
                        }});
                        ui.add_space(12.0);
                        ui.label(track.plugin.as_ref().map(|p|format!("{} · {}",p.name,p.format)).unwrap_or_else(||"No instrument".into()));
                        let state = self.view.state(track.id).map(|s|s.status.as_str()).unwrap_or("Empty");
                        ui.colored_label(if self.view.failed {colors::DANGER} else {colors::SECONDARY},state);
                        ui.add_space(8.0);
                        ui.label("Record mode");
                        let mode_enabled = !self.view.recording
                            && !self.view.transport.pending
                            && self.busy_since.is_none()
                            && self.view.tracks.iter().all(|track| !track.loading);
                        ui.add_enabled_ui(mode_enabled, |ui| {
                            let mut chosen = track.record_mode;
                            egui::ComboBox::from_id_salt(("track-record-mode", track.id))
                                .selected_text(chosen.label())
                                .show_ui(ui, |ui| {
                                    for mode in TrackMode::ALL {
                                        ui.selectable_value(&mut chosen, mode, mode.label());
                                    }
                                });
                            if chosen != track.record_mode {
                                track_mode = Some(chosen);
                            }
                        });
                        ui.colored_label(
                            colors::SECONDARY,
                            "Global follows the transport record mode.",
                        );
                        ui.horizontal_wrapped(|ui| { for command in [Command::Instrument,Command::Editor,Command::CloseEditor] {if self.button(ui,command) {selected=Some(command);}} });
                        ui.horizontal_wrapped(|ui| { for command in [Command::Restart,Command::Reactivate,Command::Quarantine] {if self.button(ui,command) {selected=Some(command);}} });
                        ui.add_space(12.0);
                        let audio = self.view.audio.clone().unwrap_or_default();
                        let midi = self.devices.midi.iter().find(|(id,_)| id == &audio.midi).map(|(_,n)| n.as_str()).unwrap_or("MIDI input unavailable / not selected");
                        let target = self.view.armed.and_then(|id| self.view.project.tracks.iter().find(|track| track.id == id));
                        ui.label(format!("MIDI: {midi} / {}", target.map_or("selected track", |track| track.name.as_str())));
                        ui.label(format!("Output: stereo {}–{}",audio.pair[0]+1,audio.pair[1]+1));
                    } else {
                        ui.label("Add an instrument track, choose a plugin, then play the selected track.");
                    }
                });
            }); });
        });
        if let Some(mode) = track_mode
            && let Some(id) = self.view.selected
        {
            self.send(Action::Transport(TransportAction::SetTrackRecordMode(
                id, mode,
            )));
        }
        if let Some(id) = self.arrangement_ui.open_roll.take()
            && let Some((_, clip)) = self.view.project.midi.find_clip(id)
        {
            self.piano_roll.open(id, clip);
            self.piano_roll_open = true;
            ui.ctx().request_repaint();
        }
        if self.piano_roll_open {
            self.piano_roll.gutter(ui, height);
            let editor_height = ui.available_height();
            let enabled = self.view.edit_history.available
                && self.busy_since.is_none()
                && self.confirm.is_none()
                && self.file.is_none();
            if let Some(action) = self.piano_roll.show(
                ui,
                editor_height,
                &self.view,
                &self.arrangement_ui.selected,
                enabled,
                &mut self.piano_roll_open,
            ) {
                self.send(action);
            }
        }
        egui::ScrollArea::vertical()
            .id_salt("session-status")
            .max_height(ui.available_height().max(1.0))
            .show(ui, |ui| {
                ui.collapsing("Track diagnostics", |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("track-diagnostics")
                        .max_height(100.0)
                        .show(ui, |ui| {
                            for track in &self.view.project.tracks {
                                if let Some(state) = self.view.state(track.id) {
                                    ui.label(format!(
                                        "{} · {} · {}",
                                        track.name, state.status, state.diagnostics
                                    ));
                                }
                            }
                        });
                });
                if !self.view.quarantine_error.is_empty() {
                    ui.colored_label(colors::DANGER, &self.view.quarantine_error);
                }
                if !self.view.history_message.is_empty() {
                    ui.colored_label(colors::DANGER, &self.view.history_message);
                }
                ui.add_space(8.0);
                if !self.view.actual.is_empty() {
                    ui.colored_label(colors::SECONDARY, &self.view.actual);
                }
                if self.view.recording {
                    let label = match self.view.transport.phase {
                        clef_session::TransportPhase::CountingIn => "Count-in",
                        clef_session::TransportPhase::Playing
                            if self.view.recording_region.is_some() =>
                        {
                            "Recording MIDI"
                        }
                        _ => "Preparing MIDI recording",
                    };
                    ui.colored_label(colors::DANGER, label);
                } else if let Some(reason) = self.view.record_unavailable {
                    ui.colored_label(colors::SECONDARY, reason);
                }
                let message = if self.status.is_empty() {
                    &self.view.message
                } else {
                    &self.status
                };
                ui.colored_label(
                    if self.view.failed {
                        colors::DANGER
                    } else {
                        colors::SECONDARY
                    },
                    message,
                );
            });
        if let Some(action) = transport_action {
            self.send(Action::Transport(action));
        }
        if let Some(id) = choose {
            self.select_track(id);
        }
        if let Some(command) = selected {
            self.dispatch(command, ctx);
        }
    }
    fn plugin_picker(&mut self, ctx: &egui::Context) {
        if !self.picker_open {
            return;
        }
        let mut open = true;
        let mut picked = None;
        let mut rescan = false;
        egui::Window::new("Choose Instrument").open(&mut open).default_width(560.0).show(ctx,|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.plugin_search).hint_text("Search instruments by name or format"));
            if self.button(ui,Command::Rescan) { rescan = true; }
            if ui.button("Show Quarantined Plugins…").clicked() { self.quarantine_open=true; }
            if self.scan_job.is_some() { ui.horizontal(|ui| { ui.spinner(); ui.label("Scanning plugins in isolated processes…"); }); }
            let search = self.plugin_search.to_lowercase();
            egui::ScrollArea::vertical().max_height(320.0).show(ui,|ui| {
                for plugin in &self.catalog.plugins {
                    if format!("{} {}",plugin.name,plugin.format).to_lowercase().contains(&search) && ui.add_enabled(self.busy_since.is_none() && !self.view.quarantine.iter().any(|e| crate::quarantine::same(&e.plugin,plugin)),egui::Button::new(format!("{} · {}{}",plugin.name,plugin.format,if self.view.quarantine.iter().any(|e| crate::quarantine::same(&e.plugin,plugin)) {" · Quarantined"} else {""})).frame(false)).on_hover_text(plugin.path.display().to_string()).clicked() { picked = Some(plugin.clone()); }
                }
                if self.catalog.plugins.is_empty() && self.scan_job.is_none() { ui.label("No instruments found. Install a VST3 or CLAP instrument, then Rescan."); }
            });
            if !self.catalog.failures.is_empty() { ui.collapsing(format!("{} scan failures",self.catalog.failures.len()),|ui| { egui::ScrollArea::vertical().id_salt("scan-failures").max_height(120.0).show(ui, |ui| { for failure in &self.catalog.failures { ui.colored_label(colors::DANGER,failure); } }); }); }
        });
        self.picker_open = open;
        if rescan {
            self.dispatch(Command::Rescan, ctx);
        }
        if let Some(plugin) = picked {
            self.picker_open = false;
            if self.view.track().is_some_and(|t| t.plugin.is_some()) {
                self.confirm = Some(Confirm::Replace(plugin));
            } else {
                if let Some(id) = self.view.selected {
                    self.send(Action::Select(id, plugin));
                }
            }
        }
    }
    fn settings(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut open = true;
        let mut action = None;
        egui::Window::new("Audio/MIDI Settings").open(&mut open).default_width(580.0).show(ctx,|ui| {
            ui.horizontal(|ui| {
                ui.label("Applies to");
                ui.selectable_value(&mut self.settings_project, false, "All projects");
                ui.selectable_value(&mut self.settings_project, true, "This project");
            });
            ui.colored_label(colors::SECONDARY, "All projects saves Clef's audio preferences. This project saves an override in the project file.");
            ui.label("Applying restarts audio.");
            if self.button(ui,Command::Refresh) { action = Some(Command::Refresh); }
            if self.device_job.is_some() { ui.spinner(); }
            ui.add_space(12.0);
            let name = self.devices.audio.iter().find(|d| d.id == self.settings.device).map(|d| d.name.as_str()).unwrap_or("Choose an audio output");
            egui::ComboBox::from_label("Audio output").selected_text(name).show_ui(ui,|ui| { for device in &self.devices.audio { ui.selectable_value(&mut self.settings.device,device.id.clone(),&device.name); } });
            let selected = self.devices.audio.iter().find(|d| d.id == self.settings.device);
            let channels = selected.and_then(|d| d.configurations.iter().map(|c| c.channels).max()).unwrap_or(0);
            egui::ComboBox::from_label("Stereo output pair").selected_text(format!("{}–{}",self.settings.pair[0]+1,self.settings.pair[1]+1)).show_ui(ui,|ui| {
                for left in (0..channels.saturating_sub(1) as usize).step_by(2) { ui.selectable_value(&mut self.settings.pair,[left,left+1],format!("{}–{}",left+1,left+2)); }
            });
            egui::ComboBox::from_label("Sample rate").selected_text(format!("{} Hz",self.settings.rate)).show_ui(ui,|ui| {
                let mut rates = vec![8000,11025,16000,22050,32000,44100,48000,88200,96000,176400,192000,352800,384000];
                if let Some(device) = selected { for c in &device.configurations { rates.extend([c.min_rate,c.max_rate]); } } rates.sort();rates.dedup();
                for rate in rates { if selected.is_some_and(|d| d.configurations.iter().any(|c| (c.min_rate..=c.max_rate).contains(&rate))) { ui.selectable_value(&mut self.settings.rate,rate,format!("{rate} Hz")); } }
            });
            egui::ComboBox::from_label("Buffer size").selected_text(format!("{} frames",self.settings.buffer)).show_ui(ui,|ui| {
                let mut values = vec![16,32,64,128,256,512,1024,2048,4096];
                if let Some(device) = selected { for c in &device.configurations { if let clef_audio_io::SupportedBufferSize::Range { min,max } = c.buffer { values.extend([min,max]); } } } values.sort(); values.dedup();
                for frames in values { let mut candidate = self.settings.clone(); candidate.buffer = frames;
                    if selected.is_some_and(|d| d.configurations.iter().any(|c| c.supports(&candidate))) { ui.selectable_value(&mut self.settings.buffer,frames,format!("{frames} frames")); }
                }
            });
            ui.add_space(12.0);
            let midi = self.devices.midi.iter().find(|(id,_)| id == &self.settings.midi).map(|(_,n)| n.as_str()).unwrap_or("Choose a MIDI input");
            egui::ComboBox::from_label("MIDI input").selected_text(midi).show_ui(ui,|ui| { for (id,name) in &self.devices.midi { ui.selectable_value(&mut self.settings.midi,id.clone(),name); } });
            ui.add_space(12.0);
            match self.devices.validate(&self.settings) { Ok(()) => { if self.button(ui,Command::ApplySettings) { action = Some(Command::ApplySettings); } }, Err(e) => { ui.colored_label(colors::SECONDARY,e.to_string()); } }
            if !self.view.actual.is_empty() { ui.label(&self.view.actual); }
            ui.label("The device may use a different callback size; actual values appear after audio starts.");
        });
        self.settings_open = open;
        if let Some(command) = action {
            self.dispatch(command, ctx);
        }
    }
    fn files(&mut self, ctx: &egui::Context) {
        let Some(picker) = &mut self.file else {
            return;
        };
        let mut open = true;
        let mut chosen = None;
        let mut navigate = None;
        let title = match picker.mode {
            FileMode::Open => "Open Project",
            FileMode::Locate => "Locate Project",
            FileMode::Save => "Save Project",
        };
        egui::Window::new(title)
            .open(&mut open)
            .default_width(620.0)
            .show(ctx, |ui| {
                ui.label(picker.folder.display().to_string());
                if ui.button("Parent Folder").clicked() {
                    navigate = picker.folder.parent().map(|p| p.to_owned());
                }
                let listing = picker
                    .listing
                    .get_or_insert_with(|| list_folder(&picker.folder));
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| match listing {
                        Ok(entries) => {
                            for entry in entries.iter() {
                                if ui
                                    .selectable_label(
                                        picker.name == entry.name,
                                        format!(
                                            "{}{}",
                                            entry.name,
                                            if entry.folder { "/" } else { "" }
                                        ),
                                    )
                                    .clicked()
                                {
                                    if entry.folder {
                                        navigate = Some(entry.path.clone());
                                    } else {
                                        picker.name.clone_from(&entry.name);
                                    }
                                }
                            }
                        }
                        Err(error) => {
                            ui.colored_label(colors::DANGER, error.as_str());
                        }
                    });
                ui.horizontal(|ui| {
                    let label = ui.label("File name or absolute path");
                    ui.text_edit_singleline(&mut picker.name)
                        .labelled_by(label.id);
                });
                ui.colored_label(colors::DANGER, &picker.error);
                if ui.button(title).clicked() {
                    let mut path = picker.folder.join(picker.name.trim());
                    if picker.name.trim().is_empty() {
                        picker.error = "Enter a project file name.".into();
                    } else {
                        if matches!(picker.mode, FileMode::Save) && path.extension().is_none() {
                            path.set_extension("clef");
                        }
                        chosen = Some((picker.mode, path));
                    }
                }
            });
        if let Some(folder) = navigate {
            picker.folder = folder;
            picker.listing = None;
        }
        if !open {
            self.file = None;
            self.pending_after_save = None;
            self.retained_approved.clear();
        }
        if let Some((mode, path)) = chosen {
            self.file = None;
            match mode {
                FileMode::Open => self.transition(Transition::Open(path), ctx),
                FileMode::Locate => {
                    if let Some(old) = self.startup.selected.clone() {
                        self.send(Action::RelinkRecent(old, path));
                    }
                }
                FileMode::Save => {
                    if path.exists() {
                        self.confirm = Some(Confirm::Overwrite(path));
                    } else {
                        self.awaiting_save = true;
                        self.send(Action::Save {
                            path,
                            replace: false,
                            track_name: self.view.selected.map(|id| (id, self.rename.clone())),
                            use_retained: self.retained_approved.clone(),
                        });
                    }
                }
            }
        }
    }
    fn confirmation(&mut self, ctx: &egui::Context) {
        let Some(confirm) = &self.confirm else {
            return;
        };
        let message = match confirm {
            Confirm::Unsaved(_) => "Save changes before continuing?",
            Confirm::Replace(_) => {
                "Replace the instrument? Its current plugin state will be discarded."
            }
            Confirm::Restart => {
                "Retry this unavailable instrument from retained state? Quarantined plugins require Reactivate first."
            }
            Confirm::Overwrite(_) => "Replace the existing project file?",
            Confirm::SplitClips(_, _) => {
                "This cut crosses MIDI notes. Split the notes at the playhead?"
            }
            Confirm::Remove(_) => "Remove this track and discard its unsaved instrument state?",
            Confirm::Reactivate(_) => {
                "Reactivate this plugin for all affected tracks? Unsnapshotted edits in terminated instances cannot be restored. Tracks remain unloaded until explicit use."
            }
            Confirm::Retained(_) => {
                "Some tracks lost unsnapshotted edits in a plugin failure. Save using their retained state?"
            }
        };
        let mut choice = None;
        egui::Modal::new(egui::Id::new("confirmation")).show(ctx, |ui| {
            ui.set_max_width(460.0);
            ui.label(message);
            match confirm {
                Confirm::Remove(id) => {
                    if let Some(track) = self.view.project.tracks.iter().find(|t| t.id == *id) {
                        ui.label(&track.name);
                    }
                }
                Confirm::Reactivate(plugin) => {
                    ui.label(&plugin.name);
                    for track in &self.view.project.tracks {
                        if track
                            .plugin
                            .as_ref()
                            .is_some_and(|p| crate::quarantine::same(p, plugin))
                        {
                            ui.label(&track.name);
                        }
                    }
                }
                Confirm::Retained(_) => {
                    for name in self.view.losses() {
                        ui.label(name);
                    }
                }
                _ => {}
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui
                    .button(if matches!(confirm, Confirm::Unsaved(_)) {
                        "Save"
                    } else if matches!(confirm, Confirm::Retained(_)) {
                        "Save Using Retained State"
                    } else if matches!(confirm, Confirm::SplitClips(_, _)) {
                        "Split Notes"
                    } else {
                        "Continue"
                    })
                    .clicked()
                {
                    choice = Some(0);
                }
                if matches!(confirm, Confirm::Unsaved(_)) && ui.button("Discard").clicked() {
                    choice = Some(1);
                }
                if ui.button("Cancel").clicked() {
                    choice = Some(2);
                }
            });
        });
        if matches!(confirm, Confirm::SplitClips(_, _))
            && ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape))
        {
            choice = Some(2);
        }
        if let Some(choice) = choice {
            self.resolve_confirmation(choice, ctx);
        }
    }
    fn resolve_confirmation(&mut self, choice: usize, ctx: &egui::Context) {
        let confirm = self.confirm.take().unwrap();
        if choice == 2 {
            self.pending_after_save = None;
            self.retained_approved.clear();
            return;
        }
        match confirm {
            Confirm::Unsaved(next) => {
                if choice == 1 {
                    self.perform(next, ctx);
                } else {
                    self.pending_after_save = Some(next);
                    self.save(false);
                }
            }
            Confirm::Replace(plugin) => {
                if let Some(id) = self.view.selected {
                    self.send(Action::Select(id, plugin));
                }
            }
            Confirm::Restart => {
                if let Some(id) = self.view.selected {
                    self.send(Action::Restart(id));
                }
            }
            Confirm::Remove(id) => self.send(Action::RemoveTrack(id)),
            Confirm::Reactivate(plugin) => self.send(Action::Reactivate(plugin)),
            Confirm::SplitClips(expected, edit) => self.send_clip_edit(expected, edit),
            Confirm::Retained(as_new) => {
                self.retained_approved = self.view.retained_losses();
                self.save(as_new);
            }
            Confirm::Overwrite(path) => {
                self.awaiting_save = true;
                self.send(Action::Save {
                    path,
                    replace: true,
                    track_name: self.view.selected.map(|id| (id, self.rename.clone())),
                    use_retained: self.retained_approved.clone(),
                });
            }
        }
    }

    fn seek_history(&mut self, target: usize) {
        self.history_window.error = None;
        self.send(Action::SeekHistory {
            version: self.view.edit_history.version,
            target,
        });
        self.history_window.pending = self.busy_since.is_some();
    }

    fn command_label(&self, binding: &Binding) -> String {
        let history = &self.view.edit_history;
        if binding.command == Command::ShowMixer && self.mixer_ui.open {
            return "Show Sequencer".into();
        }
        let edit = match binding.command {
            Command::Undo => history
                .cursor
                .checked_sub(1)
                .and_then(|index| history.labels.get(index)),
            Command::Redo => history.labels.get(history.cursor),
            _ => None,
        };
        edit.map_or_else(
            || binding.title.into(),
            |edit| format!("{} {edit}", binding.title),
        )
    }

    fn command_window(&mut self, ctx: &egui::Context) {
        if !self.commands_open {
            return;
        }
        let mut open = true;
        let mut selected = None;
        let mut rebind = None;
        egui::Window::new("Commands / Keyboard Shortcuts").open(&mut open).default_width(560.0).show(ctx,|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.command_search).hint_text("Search commands"));
            ui.label("Bindings apply to this Clef session. Set: press Command/Ctrl/Option + key; Escape cancels.");
            egui::ScrollArea::vertical().max_height(420.0).show(ui,|ui| {
                for (index,binding) in self.bindings.iter().enumerate() {
                    let title = self.command_label(binding);
                    if !title.to_lowercase().contains(&self.command_search.to_lowercase()) { continue; }
                    ui.horizontal(|ui| {
                        if ui.add_enabled(self.enabled(binding.command),egui::Button::new(title)).clicked() { selected = Some(binding.command); }
                        ui.label(binding.shortcut.map(|s| ctx.format_shortcut(&s)).unwrap_or_else(|| "Unbound".into()));
                        if ui.button(if self.rebinding == Some(index) { "Press keys…" } else { "Set" }).clicked() { rebind = Some(index); }
                    });
                }
            });
        });
        self.commands_open = open;
        if let Some(index) = rebind {
            self.rebinding = Some(index);
        }
        if let Some(command) = selected {
            self.commands_open = false;
            self.dispatch(command, ctx);
        }
    }
}
impl eframe::App for App {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        colors::BACKGROUND.to_normalized_gamma_f32()
    }
    fn logic(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        ctx.input(|input| {
            for event in &input.events {
                match event {
                    egui::Event::Key { pressed: true, .. } => self.keyboard_mode = true,
                    egui::Event::PointerButton { pressed: true, .. } => self.keyboard_mode = false,
                    _ => {}
                }
            }
        });
        self.poll(ctx);
        if self.confirm.is_none() && self.file.is_none() {
            self.arrangement_ui.cancel(ctx);
        }
        self.mixer_logic(ctx);
        self.shortcuts(ctx);
        self.busy_visible = self
            .busy_since
            .is_some_and(|started| started.elapsed() >= BUSY_FEEDBACK_DELAY);
        let name = self
            .view
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into());
        let title = if self.view.project_open {
            format!(
                "{}{} — Clef",
                name,
                if self.has_unsaved_changes() { " *" } else { "" }
            )
        } else {
            "Clef".into()
        };
        if !ctx.input(|i| i.viewport().title.as_deref() == Some(title.as_str())) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
        let os_quit = self.quit_gate.is_some() && unsafe { clef_take_quit_request() };
        if (os_quit || ctx.input(|i| i.viewport().close_requested())) && !self.exiting {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.busy_since.is_none() && self.confirm.is_none() {
                self.transition(Transition::Close, ctx);
            }
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.view.project_open {
            egui::Frame::new().inner_margin(12.0).show(ui, |ui| {
                self.main_surface(ui, &ctx);
            });
        } else {
            self.startup_surface(ui, &ctx);
        }
        self.plugin_picker(&ctx);
        self.settings(&ctx);
        self.transport_windows(&ctx);
        self.command_window(&ctx);
        let history_enabled =
            self.view.edit_history.available && self.enabled(Command::EditHistory);
        if self.view.project_open
            && self.history_window.open
            && let Some(target) =
                self.history_window
                    .show(&ctx, &self.view.edit_history, history_enabled)
        {
            self.seek_history(target);
        }
        self.files(&ctx);
        self.quarantine_windows(&ctx);
        self.confirmation(&ctx);
        if let Some(started) = self.busy_since
            && !self.busy_visible
        {
            ctx.request_repaint_after(BUSY_FEEDBACK_DELAY.saturating_sub(started.elapsed()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redo_shortcut_never_falls_through_to_undo() {
        let ctx = egui::Context::default();
        let mut app = App::new(ctx.clone(), PathBuf::new());
        app.view.project_open = true;
        app.view.edit_history = clef_session::HistoryView {
            labels: vec!["Add Track"],
            cursor: 1,
            available: true,
            ..Default::default()
        };
        let press_z = |app: &mut App, modifiers| {
            let _ = ctx.run_ui(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key: Key::Z,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    }],
                    ..Default::default()
                },
                |_| app.shortcuts(&ctx),
            );
        };
        press_z(
            &mut app,
            Modifiers::MAC_CMD | Modifiers::COMMAND | Modifiers::SHIFT,
        );
        assert_eq!(app.actions_in_flight, 0);
        press_z(&mut app, Modifiers::MAC_CMD | Modifiers::COMMAND);
        assert_eq!(app.actions_in_flight, 1);
    }
    #[test]
    fn file_picker_lists_a_folder_once_until_navigation() {
        let folder = std::env::temp_dir().join(format!("clef-picker-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(folder.join("Sub")).unwrap();
        for name in ["b.clef", "a.clef", ".hidden.clef", "notes.txt"] {
            std::fs::write(folder.join(name), []).unwrap();
        }
        let names = |app: &App| match app.file.as_ref().unwrap().listing.as_ref() {
            Some(Ok(entries)) => entries.iter().map(|e| e.name.clone()).collect::<Vec<_>>(),
            _ => Vec::new(),
        };
        let context = egui::Context::default();
        let mut app = App::new(context.clone(), PathBuf::new());
        let mut picker = FilePicker::new(FileMode::Open, None);
        picker.folder.clone_from(&folder);
        app.file = Some(picker);
        let _ = context.run_ui(Default::default(), |ui| app.files(ui.ctx()));
        assert_eq!(names(&app), ["Sub", "a.clef", "b.clef"]);
        std::fs::write(folder.join("c.clef"), []).unwrap();
        let _ = context.run_ui(Default::default(), |ui| app.files(ui.ctx()));
        assert_eq!(
            names(&app),
            ["Sub", "a.clef", "b.clef"],
            "frames reuse the listing"
        );
        app.file.as_mut().unwrap().listing = None;
        let _ = context.run_ui(Default::default(), |ui| app.files(ui.ctx()));
        assert_eq!(names(&app), ["Sub", "a.clef", "b.clef", "c.clef"]);
        std::fs::remove_dir_all(&folder).unwrap();
    }
    #[test]
    fn dirty_project_transition_waits_for_confirmation_and_cancel_keeps_it() {
        let context = egui::Context::default();
        let mut app = App::new(context.clone(), PathBuf::new());
        app.view.dirty = true;
        app.transition(Transition::New, &context);
        assert!(matches!(
            app.confirm,
            Some(Confirm::Unsaved(Transition::New))
        ));
        assert!(app.busy_since.is_none());
        app.resolve_confirmation(2, &context);
        assert!(app.view.dirty);
        assert!(app.confirm.is_none());
        assert!(app.busy_since.is_none());
    }
    #[test]
    fn closing_a_clean_project_with_only_view_changes_saves_without_a_prompt() {
        let context = egui::Context::default();
        let mut app = App::new(context.clone(), PathBuf::new());
        app.view.view_unsaved = true;
        app.transition(Transition::Close, &context);
        assert!(app.confirm.is_none(), "no dialog for a view-only change");
        assert!(app.awaiting_close && !app.exiting);
        assert_eq!(app.actions_in_flight, 1);

        let mut app = App::new(context.clone(), PathBuf::new());
        app.transition(Transition::Close, &context);
        assert!(app.confirm.is_none() && !app.awaiting_close && app.exiting);
    }
    #[test]
    fn save_before_close_waits_for_success_and_failed_save_cancels_close() {
        let context = egui::Context::default();
        let mut app = App::new(context.clone(), PathBuf::new());
        app.view.dirty = true;
        app.transition(Transition::Close, &context);
        app.resolve_confirmation(0, &context);
        assert!(app.file.is_some());
        assert!(app.pending_after_save.is_some());
        assert!(!app.exiting);
        // An invalid destination exercises the real asynchronous save failure path.
        app.file = None;
        app.awaiting_save = true;
        app.send(Action::Save {
            path: "/missing-parent-dir-clef/project.clef".into(),
            replace: false,
            track_name: None,
            use_retained: Vec::new(),
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while app.busy_since.is_some() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
            app.poll(&context);
        }
        assert!(app.busy_since.is_none());
        assert!(!app.exiting);
        assert!(app.pending_after_save.is_none());
        assert!(app.view.dirty);
    }

    #[test]
    fn rejected_record_acknowledgement_clears_the_transport_lamp() {
        let context = egui::Context::default();
        let mut app = App::new(context.clone(), PathBuf::new());
        // The session has no audio engine, so it rejects the submitted command.
        app.send(Action::Transport(TransportAction::Record));
        assert!(app.transport_ui.record_submitted);
        let deadline = Instant::now() + Duration::from_secs(3);
        while app.busy_since.is_some() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
            app.poll(&context);
        }
        assert!(app.busy_since.is_none());
        assert!(!app.transport_ui.record_submitted);
        assert!(!app.view.recording);
    }
    #[test]
    fn discard_close_exits_and_commands_respect_session_state() {
        let context = egui::Context::default();
        let mut app = App::new(context.clone(), PathBuf::new());
        assert!(!app.enabled(Command::Editor));
        app.view.dirty = true;
        app.transition(Transition::Close, &context);
        app.resolve_confirmation(1, &context);
        assert!(!app.exiting);
        let deadline = Instant::now() + Duration::from_secs(3);
        while !app.exiting && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
            app.poll(&context);
        }
        assert!(app.exiting);
    }
    #[test]
    fn buttons_share_a_borderless_hover_background() {
        let context = egui::Context::default();
        let _app = App::new(context.clone(), PathBuf::new());
        let style = context.global_style();
        for widget in [
            &style.visuals.widgets.inactive,
            &style.visuals.widgets.hovered,
            &style.visuals.widgets.active,
            &style.visuals.widgets.open,
        ] {
            assert_eq!(widget.bg_stroke, egui::Stroke::NONE);
        }
        assert_eq!(style.visuals.widgets.inactive.bg_fill, colors::PANEL);
        assert_eq!(style.visuals.widgets.hovered.bg_fill, colors::FIELD);
        assert_eq!(style.visuals.widgets.active.bg_fill, colors::FIELD);
        assert!(!style.interaction.selectable_labels);
    }

    #[test]
    fn take_commands_need_a_lane_and_have_no_default_shortcut() {
        use clef_model::midi::{Event, Message, Mode};
        let context = egui::Context::default();
        let mut app = App::new(context, PathBuf::new());
        app.view.project_open = true;
        app.view.selected = Some(1);
        app.view.project.tracks.push(crate::project::Track {
            id: 1,
            name: "Strings".into(),
            plugin: None,
            instrument_bypassed: false,
            instrument_offline: false,
            record_mode: Default::default(),
            mix: Default::default(),
            inserts: Vec::new(),
            take_lanes: Default::default(),
            kind: Default::default(),
            output: None,
            sends: Vec::new(),
            state: Default::default(),
        });
        std::sync::Arc::make_mut(&mut app.view.project.midi)
            .record(
                1,
                Mode::NewTake,
                [0, 960],
                vec![
                    vec![Event {
                        tick: 100,
                        order: 0,
                        message: Message::Control([0xb0, 1, 10]),
                    }],
                    vec![Event {
                        tick: 500,
                        order: 0,
                        message: Message::Control([0xb0, 1, 20]),
                    }],
                ],
                false,
            )
            .unwrap();
        app.view.edit_history.available = true;
        for command in [
            Command::ShowTakeLanes,
            Command::RecordTrackGlobal,
            Command::RecordTrackMerge,
            Command::RecordTrackReplace,
            Command::RecordTrackNewTake,
        ] {
            assert!(app.enabled(command));
        }
        for command in [
            Command::UseWholeTake,
            Command::DeleteTake,
            Command::FlattenComp,
            Command::TakeToNewTrack,
            Command::RenameTake,
            Command::EditTake,
        ] {
            assert!(!app.enabled(command), "take command without a lane");
            assert!(
                app.bindings
                    .iter()
                    .find(|binding| binding.command == command)
                    .unwrap()
                    .shortcut
                    .is_none(),
                "take command gained a default shortcut"
            );
        }
        app.arrangement_ui.focused_lane = Some((1, 0));
        for command in [
            Command::UseWholeTake,
            Command::DeleteTake,
            Command::FlattenComp,
            Command::TakeToNewTrack,
            Command::RenameTake,
            Command::EditTake,
        ] {
            assert!(app.enabled(command), "take command with a lane");
        }
    }

    #[test]
    fn staged_name_requires_unsaved_confirmation() {
        let context = egui::Context::default();
        let mut app = App::new(context.clone(), PathBuf::new());
        app.view.project.next_id = 2;
        app.view.selected = Some(1);
        app.view.project.tracks = vec![crate::project::Track {
            id: 1,
            name: "Old".into(),
            plugin: None,
            instrument_bypassed: false,
            instrument_offline: false,
            record_mode: Default::default(),
            mix: Default::default(),
            inserts: Vec::new(),
            take_lanes: Default::default(),
            kind: Default::default(),
            output: None,
            sends: Vec::new(),
            state: Default::default(),
        }];
        app.rename = "New name".into();
        assert!(app.has_unsaved_changes());
        app.transition(Transition::Close, &context);
        assert!(matches!(
            app.confirm,
            Some(Confirm::Unsaved(Transition::Close))
        ));
        assert!(!app.exiting);
    }
    #[test]
    fn switching_arrangement_clip_does_not_flash_a_widget_rect_warning() {
        let context = egui::Context::default();
        let mut app = App::new(context.clone(), PathBuf::new());
        app.view.project_open = true;
        app.view.project.tracks = vec![
            crate::project::Track {
                id: 1,
                name: "One".into(),
                plugin: None,
                instrument_bypassed: false,
                instrument_offline: false,
                record_mode: Default::default(),
                mix: Default::default(),
                inserts: Vec::new(),
                take_lanes: Default::default(),
                kind: Default::default(),
                output: None,
                sends: Vec::new(),
                state: Default::default(),
            },
            crate::project::Track {
                id: 2,
                name: "Two".into(),
                plugin: None,
                instrument_bypassed: false,
                instrument_offline: false,
                record_mode: Default::default(),
                mix: Default::default(),
                inserts: Vec::new(),
                take_lanes: Default::default(),
                kind: Default::default(),
                output: None,
                sends: Vec::new(),
                state: Default::default(),
            },
        ];
        app.view.project.next_id = 3;
        for (track, tick) in [(1u64, 0u64), (2, 240)] {
            clef_model::midi::Arrangement::record(
                std::sync::Arc::make_mut(&mut app.view.project.midi),
                track,
                clef_model::midi::Mode::Merge,
                [0, 960],
                vec![vec![clef_model::midi::Event {
                    tick,
                    order: 1,
                    message: clef_model::midi::Message::Note {
                        channel: 2,
                        key: 60,
                        velocity: 90,
                        end: tick + 240,
                        release: 0,
                        release_order: 2,
                    },
                }]],
                false,
            )
            .unwrap();
        }
        let a = app.view.project.midi.clips(1)[0].id();
        app.piano_roll
            .open(a, app.view.project.midi.find_clip(a).unwrap().1);
        app.piano_roll_open = true;
        let render = |app: &mut App, events: Vec<egui::Event>| {
            context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1280.0, 800.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| app.main_surface(ui, &context),
            )
        };
        let first = render(&mut app, Vec::new());
        let mut clip_centers = first
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Rect(rect)
                    if rect.fill == colors::PEACOCK
                        && rect.rect.width() > 40.0
                        && rect.rect.height() > 30.0 =>
                {
                    Some(rect.rect.center())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        clip_centers.sort_by(|left, right| left.y.total_cmp(&right.y));
        assert_eq!(clip_centers.len(), 2, "both arrangement clips are visible");
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let mut warnings = Vec::new();
        for (frame, center) in [0usize, 1, 0]
            .into_iter()
            .enumerate()
            .map(|(frame, index)| (frame, clip_centers[index]))
        {
            for (step, events) in [
                ("move", vec![egui::Event::PointerMoved(center)]),
                ("press", vec![button(center, true)]),
                ("release", vec![button(center, false)]),
            ] {
                let output = render(&mut app, events);
                for clipped in output.shapes {
                    if let egui::Shape::Rect(rect) = clipped.shape
                        && rect.stroke.color == egui::Color32::RED
                        && rect.stroke.width >= 2.0
                    {
                        warnings.push((frame, step, rect.rect));
                    }
                }
            }
        }
        assert!(
            warnings.is_empty(),
            "switching clips painted widget-rect warnings: {warnings:?}"
        );
    }
}
