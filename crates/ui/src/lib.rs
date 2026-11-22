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
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};
use std::{
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};
mod primitives {
    pub mod accordion;
    pub mod checkbox;
    pub mod chip;
    pub mod empty_state;
    pub mod fader;
    pub mod glyph_toggle;
    pub mod history;
    pub mod icons;
    pub(crate) mod inserts;
    pub mod knob;
    pub mod level_meter;
    pub mod list_row;
    pub mod menu;
    pub mod menu_bar;
    pub mod menu_button;
    pub mod mixer;
    pub mod mute_solo;
    pub mod notice_box;
    pub mod panel_controls;
    pub mod piano_roll;
    pub mod power_button;
    pub mod project;
    pub mod range_overlay;
    pub mod scroll_zoom_bars;
    pub mod search_field;
    pub mod segment_display;
    pub mod segmented;
    pub mod slot_button;
    pub mod specimens;
    pub mod tabs;
    pub mod text;
    pub mod text_button;
    pub mod text_field;
    pub mod timeline;
    pub mod tool_button;
    pub mod track_state;
    pub mod transport;
    pub mod value_slider;
    pub mod window_frame;
}
mod arrangement;
mod commands;
mod menus;
mod palette;
use commands::{Command, Context, Keymap};
#[doc(hidden)]
pub mod gallery;
mod mixer;
mod paint;
mod piano_roll;
mod startup;
mod theme;
use primitives::{
    list_row::ListRow,
    menu_button::{choice, menu_button},
    notice_box::{NoticeBox, Outcome},
    search_field::SearchField,
    text,
    text_button::TextButton,
    text_field::TextField,
    window_frame::{ToolWindow, frame},
};
mod transport;

const BUSY_FEEDBACK_DELAY: Duration = Duration::from_millis(100);

#[derive(Clone)]
enum Transition {
    New,
    Open(PathBuf),
    Close,
}
/// T41 round 2 (R16): the Project Format dialog's choices.
#[derive(Clone, Copy)]
struct FormatDialog {
    rate: u32,
    bits: clef_model::project::BitDepth,
}

/// Sample rates the Project Format dialog offers.
const PROJECT_RATES: [u32; 6] = [44_100, 48_000, 88_200, 96_000, 176_400, 192_000];

/// T41 round 2 (R13): the New Tracks dialog's choices.
struct NewTracks {
    count: usize,
    bus: bool,
    name: String,
    plugin: Option<Plugin>,
}

impl Default for NewTracks {
    fn default() -> Self {
        Self {
            count: 1,
            bus: false,
            name: String::new(),
            plugin: None,
        }
    }
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
    /// T41: the Track Diagnostics window (Window menu) is open.
    diagnostics_open: bool,
    /// T41: the left drawer is hidden (window session only, not saved).
    left_drawer_hidden: bool,
    /// T41 round 3 (S2): the lower panel (Editor or Mixer) is detached or at full height
    /// (window session only, not saved).
    panel: primitives::panel_controls::PanelState,
    /// The editing column's width on the last frame: the default the track column steps from.
    track_area_width: f32,
    /// T41: the open New Tracks dialog.
    new_tracks: Option<NewTracks>,
    /// T41: the open Project Format dialog.
    project_format: Option<FormatDialog>,
    /// T41: the inspector's content height on the last frame; the left column sizes it.
    inspector_height: f32,
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
    keymap: Keymap,
    commands_open: bool,
    /// Debug builds only: the primitive gallery window (Window › Primitive Gallery). A cell, so
    /// the menu bar can open it while it draws through `&self`.
    primitive_gallery_open: std::cell::Cell<bool>,
    primitive_gallery: primitives::specimens::Specimens,
    history_window: primitives::history::HistoryWindow,
    command_search: String,
    rebinding: Option<Command>,
    /// The shortcut being assigned and the command that already uses it.
    rebind_conflict: Option<(KeyboardShortcut, Command)>,
    shortcuts_seen: std::sync::Arc<std::collections::BTreeMap<String, Option<String>>>,
    shortcuts_unsaved: bool,
    palette: Option<palette::Palette>,
    rename: String,
    status: String,
    retained_approved: Vec<(u64, u64)>,
    quarantine_open: bool,
    quarantine_notice: bool,
    last_notice: u64,
    track_focus: Option<u64>,
    keyboard_mode: bool,
}
impl App {
    fn new(context: egui::Context, executable: PathBuf) -> Self {
        theme::install_fonts(&context);
        let mut style = (*context.global_style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = theme::BACKGROUND;
        style.visuals.window_fill = theme::PANEL;
        style.visuals.extreme_bg_color = theme::FIELD;
        style.visuals.faint_bg_color = theme::PANEL;
        style.visuals.override_text_color = Some(theme::TEXT);
        style.visuals.selection.bg_fill = theme::PEACOCK;
        style.visuals.selection.stroke = egui::Stroke::new(1.0_f32, theme::ACCENT);
        for widget in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.open,
        ] {
            widget.bg_fill = theme::PANEL;
            widget.weak_bg_fill = theme::PANEL;
            widget.bg_stroke = egui::Stroke::NONE;
            widget.corner_radius = egui::CornerRadius::same(2);
        }
        style.visuals.widgets.inactive.bg_fill = theme::PANEL;
        style.visuals.widgets.inactive.weak_bg_fill = theme::PANEL;
        style.visuals.widgets.hovered.bg_fill = theme::FIELD;
        style.visuals.widgets.hovered.weak_bg_fill = theme::FIELD;
        style.visuals.widgets.active.bg_fill = theme::FIELD;
        style.visuals.widgets.active.weak_bg_fill = theme::FIELD;
        style.visuals.widgets.open.bg_fill = theme::FIELD;
        style.visuals.widgets.open.weak_bg_fill = theme::FIELD;
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
            diagnostics_open: false,
            left_drawer_hidden: false,
            panel: Default::default(),
            track_area_width: 1024.0,
            new_tracks: None,
            project_format: None,
            inspector_height: 0.0,
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
            keymap: Keymap::default(),
            commands_open: false,
            primitive_gallery_open: std::cell::Cell::new(false),
            primitive_gallery: Default::default(),
            history_window: primitives::history::HistoryWindow::default(),
            command_search: String::new(),
            rebinding: None,
            rebind_conflict: None,
            shortcuts_seen: Default::default(),
            shortcuts_unsaved: false,
            palette: None,
            rename: String::new(),
            status: "Create a project or open an existing .clef file.".into(),
            retained_approved: Vec::new(),
            quarantine_open: false,
            quarantine_notice: false,
            last_notice: 0,
            track_focus: None,
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
            // The arrangement stays usable beside a docked half-height mixer (T41 S1).
            command if command.is_clip() => {
                !(self.mixer_ui.open && self.panel.full && !self.panel.detached)
                    && self.clip_command_enabled(command)
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
            FitArrangementWidth | FitArrangementHeight => self.view.project_open,
            TogglePanelFull => {
                self.view.project_open
                    && (self.mixer_ui.open || self.piano_roll_open)
                    && !self.panel.detached
            }
            TogglePanelDetach => {
                self.view.project_open && (self.mixer_ui.open || self.piano_roll_open)
            }
            WiderTrackColumn | NarrowerTrackColumn => self.view.project_open,
            ShowEditor => self.piano_roll_open || self.editor_clip().is_some(),
            NewTracks => self.view.project.tracks.len() < crate::project::MAX_TRACKS,
            FitRollWidth | FitRollHeight => self.view.project_open && self.piano_roll_open,
            RollSelectAll | RollDelete | RollCopy | RollPaste | RollDuplicate | RollEnter
            | RollEarlier | RollLater | RollUp | RollDown | RollShorten | RollLengthen => {
                self.piano_roll_open
                    && (self.piano_roll.had_focus
                        || self.palette.as_ref().is_some_and(|palette| palette.roll))
            }
            _ => true,
        }
    }
    fn button(&self, ui: &mut egui::Ui, command: Command) -> bool {
        let hint = self.keymap.text(ui.ctx(), command);
        let title = match command {
            Command::ShowMixer if self.mixer_ui.open => "Hide Mixer",
            Command::ShowMixer => "Mixer",
            Command::Settings if self.mixer_ui.open => "Settings",
            Command::Commands if self.mixer_ui.open => "Commands",
            _ => command.title(),
        };
        let description = self.command_label(command);
        TextButton::new(title)
            .enabled(self.command_available(command, false))
            .tooltip(&description)
            .shortcut(&hint)
            .show(ui)
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
            FitArrangementWidth => self.arrangement_ui.fit = Some(crate::arrangement::Fit::Width),
            FitArrangementHeight => self.arrangement_ui.fit = Some(crate::arrangement::Fit::Height),
            FitRollWidth => self.piano_roll.fit = Some(crate::arrangement::Fit::Width),
            FitRollHeight => self.piano_roll.fit = Some(crate::arrangement::Fit::Height),
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
            TrackDiagnostics => self.diagnostics_open = !self.diagnostics_open,
            ShowEditor => self.toggle_editor(),
            ToggleLeftDrawer => self.left_drawer_hidden = !self.left_drawer_hidden,
            TogglePanelFull => self.panel.full = !self.panel.full,
            TogglePanelDetach => self.panel.detached = !self.panel.detached,
            WiderTrackColumn | NarrowerTrackColumn => {
                let step = if command == WiderTrackColumn {
                    20.0
                } else {
                    -20.0
                };
                let current = self
                    .arrangement_ui
                    .track_column
                    .unwrap_or(crate::transport::header_width(self.track_area_width, None));
                self.arrangement_ui.track_column = Some((current + step).clamp(
                    theme::metric::MINIMUM_HEADER_WIDTH,
                    theme::metric::MAXIMUM_HEADER_WIDTH,
                ));
            }
            NewTracks => self.open_new_tracks(ctx),
            ProjectFormat => {
                self.project_format = Some(FormatDialog {
                    rate: self.project_rate(),
                    bits: self.view.project.format.bits,
                });
            }
            Palette => self.open_palette(ctx),
            RollSelectAll | RollDelete | RollCopy | RollPaste | RollDuplicate | RollEnter
            | RollEarlier | RollLater | RollUp | RollDown | RollShorten | RollLengthen => {
                self.piano_roll.requested.set(Some(command));
            }
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
            // T41 round 2: Rename Track edits the name in its header.
            Rename => {
                if let Some(track) = self.view.track() {
                    self.arrangement_ui.renaming = Some(crate::arrangement::Renaming {
                        track: track.id,
                        text: track.name.clone(),
                        focus: true,
                    });
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
    /// Dispatch the one command whose live shortcut was pressed. Roll commands are consumed by
    /// the focused piano-roll lane itself; arrangement commands need arrangement focus.
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.rebinding.is_some() {
            self.capture_shortcut(ctx);
            return;
        }
        if self.palette.is_some() {
            return;
        }
        let text_has_focus = ctx
            .memory(|memory| memory.focused())
            .is_some_and(|id| egui::TextEdit::load_state(ctx, id).is_some());
        let composing = ctx.output(|output| output.ime.is_some());
        let arrangement = self.arrangement_ui.focused(ctx);
        let mixer_activation = self.mixer_ui.open && primitives::mixer::activation_has_focus(ctx);
        // The most specific context first, so a global binding never shadows a local one.
        let selected = commands::SPECS
            .iter()
            .filter(|spec| spec.context != Context::Global)
            .chain(
                commands::SPECS
                    .iter()
                    .filter(|spec| spec.context == Context::Global),
            )
            .map(|spec| spec.command)
            .filter(|&command| match command.context() {
                Context::PianoRoll => false,
                Context::Arrangement => arrangement && !text_has_focus,
                Context::Mixer => self.mixer_ui.open,
                Context::Global => true,
            })
            .filter(|&command| {
                self.keymap.shortcut(command).is_some_and(|shortcut| {
                    !commands::unmodified(shortcut) || !text_has_focus && !composing
                })
            })
            .filter(|&command| command != Command::PlayStop || !mixer_activation)
            .filter(|&command| !text_has_focus || !matches!(command, Command::Undo | Command::Redo))
            .filter(|&command| self.enabled(command))
            .find(|&command| ctx.input_mut(|input| self.keymap.consume(input, command)));
        if let Some(command) = selected {
            self.dispatch(command, ctx);
        }
    }
    fn open_palette(&mut self, ctx: &egui::Context) {
        self.palette = Some(palette::Palette::new(
            ctx.memory(|memory| memory.focused()),
            self.piano_roll.had_focus,
        ));
    }
    /// The open command palette: every registry command with its live name and shortcut.
    fn command_palette(&mut self, ctx: &egui::Context) {
        if self.palette.is_none() {
            return;
        }
        let rows: Vec<palette::Row> = commands::SPECS
            .iter()
            .map(|spec| palette::Row {
                command: spec.command,
                label: self.command_label(spec.command),
                shortcut: self.keymap.text(ctx, spec.command),
                enabled: self.enabled(spec.command),
            })
            .collect();
        let Some(outcome) = self
            .palette
            .as_mut()
            .and_then(|palette| palette.show(ctx, &rows))
        else {
            return;
        };
        // Give focus back first, so context commands act where the user was.
        match self.palette.as_ref().and_then(|palette| palette.origin) {
            Some(origin) => ctx.memory_mut(|memory| memory.request_focus(origin)),
            None => {
                if let Some(focused) = ctx.memory(|memory| memory.focused()) {
                    ctx.memory_mut(|memory| memory.surrender_focus(focused));
                }
            }
        }
        if let palette::Outcome::Run(command) = outcome {
            self.dispatch(command, ctx);
        }
        if !matches!(outcome, palette::Outcome::Run(Command::Palette)) {
            self.palette = None;
        }
    }
    /// The Commands window is waiting for a key for `rebinding`: Escape cancels, any other key
    /// is assigned, or asks first when it clashes.
    fn capture_shortcut(&mut self, ctx: &egui::Context) {
        let Some(command) = self.rebinding else {
            return;
        };
        if self.rebind_conflict.is_some() {
            return;
        }
        let pressed = ctx.input_mut(|input| {
            let found = input.events.iter().position(|event| {
                matches!(
                    event,
                    egui::Event::Key {
                        pressed: true,
                        repeat: false,
                        ..
                    }
                )
            })?;
            match input.events.remove(found) {
                egui::Event::Key { key, modifiers, .. } => Some((key, modifiers)),
                _ => None,
            }
        });
        let Some((key, modifiers)) = pressed else {
            return;
        };
        if key == Key::Escape {
            self.rebinding = None;
            return;
        }
        let shortcut = commands::captured(key, modifiers);
        match self.keymap.conflict(command, shortcut) {
            Some(other) => self.rebind_conflict = Some((shortcut, other)),
            None => self.assign_shortcut(command, Some(shortcut)),
        }
    }
    fn assign_shortcut(&mut self, command: Command, shortcut: Option<KeyboardShortcut>) {
        self.keymap.set(command, shortcut);
        self.rebinding = None;
        self.rebind_conflict = None;
        self.save_shortcuts();
    }
    /// Send the saved form to the session; retried from `poll` while the session is busy.
    fn save_shortcuts(&mut self) {
        self.piano_roll.keys = self.keymap.clone();
        let overrides = self.keymap.overrides();
        if *self.view.shortcuts == overrides {
            self.shortcuts_unsaved = false;
            return;
        }
        let sent = self.actions_in_flight;
        self.send(Action::SetShortcuts(overrides));
        self.shortcuts_unsaved = self.actions_in_flight == sent;
    }
    /// Apply the saved shortcuts when the session reports a new set (at launch, after a save).
    fn sync_shortcuts(&mut self) {
        if std::sync::Arc::ptr_eq(&self.shortcuts_seen, &self.view.shortcuts) {
            if self.shortcuts_unsaved && self.busy_since.is_none() {
                self.save_shortcuts();
            }
            return;
        }
        self.shortcuts_seen = self.view.shortcuts.clone();
        if self.shortcuts_unsaved {
            return;
        }
        let (keymap, problems) = Keymap::from_overrides(&self.view.shortcuts);
        self.piano_roll.keys = keymap.clone();
        self.keymap = keymap;
        if !problems.is_empty() {
            self.status = format!(
                "Some saved keyboard shortcuts were ignored: {}.",
                problems.join("; ")
            );
        }
    }
    /// T41: the clip the Editor opens: a selected clip, else the selected track's first clip.
    fn editor_clip(&self) -> Option<clef_model::midi::ClipId> {
        self.arrangement_ui.selected.first().copied().or_else(|| {
            let track = self.view.selected?;
            self.view
                .project
                .midi
                .tracks
                .get(&track)?
                .first()
                .map(|clip| clip.id())
        })
    }

    /// T41: the status-row Editor button and Show Editor command. Opening shows the arrangement
    /// with the piano roll of the selected clip or track, closing the mixer; a second press
    /// closes the roll.
    fn toggle_editor(&mut self) {
        if self.piano_roll_open && !self.mixer_ui.open {
            self.piano_roll_open = false;
            return;
        }
        if self.mixer_ui.open {
            self.dispatch_mixer(Command::ShowMixer);
        }
        if !self.piano_roll_open
            && let Some(clip) = self.editor_clip()
        {
            if !self.arrangement_ui.selected.contains(&clip) {
                self.arrangement_ui.selected = std::collections::BTreeSet::from([clip]);
            }
            self.arrangement_ui.open_roll = Some(clip);
        }
    }

    /// T41 round 2: opens the New Tracks dialog (R13).
    fn open_new_tracks(&mut self, ctx: &egui::Context) {
        self.new_tracks = Some(NewTracks::default());
        if self.catalog.plugins.is_empty() {
            self.scan(ctx.clone());
        }
    }

    /// T41 round 2 (R16): sample rate (the project's audio override, applied by restarting
    /// audio; a rate the device refuses leaves everything as it was) and recording bit depth.
    fn project_format_window(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.project_format.take() else {
            return;
        };
        if !self.view.project_open {
            return;
        }
        let base = self
            .view
            .project
            .audio
            .clone()
            .or_else(|| self.view.audio.clone())
            .or_else(|| self.view.audio_preferences.clone());
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        ToolWindow::new("Project Format")
            .width(360.0)
            .show(ctx, &mut open, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                text::caption(ui, "Sample rate");
                let label = |rate: u32| {
                    if rate.is_multiple_of(1_000) {
                        format!("{} kHz", rate / 1_000)
                    } else {
                        format!("{:.1} kHz", f64::from(rate) / 1_000.0)
                    }
                };
                let (_, chosen) = menu_button(
                    ui,
                    "project-rate",
                    "Sample rate",
                    None,
                    &label(dialog.rate),
                    base.is_some(),
                    |ui| {
                        PROJECT_RATES
                            .into_iter()
                            .find(|&rate| choice(ui, &label(rate), rate == dialog.rate))
                    },
                );
                if let Some(Some(rate)) = chosen {
                    dialog.rate = rate;
                }
                if base.is_none() {
                    text::caption(ui, "Choose an audio device in Audio/MIDI Settings first.");
                } else {
                    text::caption(
                        ui,
                        "Saved with the project; audio restarts at the new rate.",
                    );
                }
                text::caption(ui, "Recording bit depth");
                primitives::segmented::Segmented::new(
                    "Recording bit depth",
                    &mut dialog.bits,
                    &clef_model::project::BitDepth::ALL.map(|bits| (bits, bits.label())),
                )
                .show(ui);
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if TextButton::new("Apply")
                        .primary(true)
                        .enabled(self.busy_since.is_none())
                        .show(ui)
                        .clicked()
                    {
                        apply = true;
                    }
                    if TextButton::new("Cancel").show(ui).clicked() {
                        cancel = true;
                    }
                });
            });
        if apply {
            if dialog.bits != self.view.project.format.bits {
                self.send(Action::SetFormat(clef_model::project::ProjectFormat {
                    bits: dialog.bits,
                    ..self.view.project.format
                }));
            }
            if let Some(mut audio) = base
                && dialog.rate != self.project_rate()
            {
                audio.rate = dialog.rate;
                self.send(Action::Settings {
                    audio,
                    project: true,
                });
            }
        } else if open && !cancel {
            self.project_format = Some(dialog);
        }
    }

    /// T41 round 2 (R13): the New Tracks dialog.
    fn new_tracks_window(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.new_tracks.take() else {
            return;
        };
        if !self.view.project_open {
            return;
        }
        let room = crate::project::MAX_TRACKS.saturating_sub(self.view.project.tracks.len());
        let mut open = true;
        let mut create = false;
        let mut cancel = false;
        ToolWindow::new("New Tracks")
            .width(420.0)
            .show(ctx, &mut open, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                text::caption(ui, "Type");
                primitives::segmented::Segmented::new(
                    "Track type",
                    &mut dialog.bus,
                    &[(false, "Instrument"), (true, "Bus")],
                )
                .show(ui);
                text::caption(ui, "Number of tracks");
                ui.horizontal(|ui| {
                    if TextButton::new("−")
                        .accessible("Fewer tracks")
                        .enabled(dialog.count > 1)
                        .show(ui)
                        .clicked()
                    {
                        dialog.count -= 1;
                    }
                    text::body(ui, &dialog.count.to_string());
                    if TextButton::new("+")
                        .accessible("More tracks")
                        .enabled(dialog.count < room)
                        .show(ui)
                        .clicked()
                    {
                        dialog.count += 1;
                    }
                });
                text::caption(ui, "Name");
                TextField::new(&mut dialog.name, "Track name")
                    .hint(if dialog.bus {
                        "Bus 1, Bus 2…"
                    } else {
                        "Instrument 1, Instrument 2…"
                    })
                    .width(380.0)
                    .show(ui);
                if dialog.count > 1 && !dialog.name.trim().is_empty() {
                    text::caption(
                        ui,
                        &format!(
                            "{} 1 … {} {}",
                            dialog.name.trim(),
                            dialog.name.trim(),
                            dialog.count
                        ),
                    );
                }
                if !dialog.bus {
                    text::caption(ui, "Instrument");
                    let current = dialog.plugin.as_ref().map_or_else(
                        || "None".to_owned(),
                        |p| format!("{} · {}", p.name, p.format),
                    );
                    let plugins = &self.catalog.plugins;
                    let (_, chosen) = menu_button(
                        ui,
                        "new-tracks-instrument",
                        "Instrument",
                        None,
                        &current,
                        true,
                        |ui| {
                            if choice(ui, "None", dialog.plugin.is_none()) {
                                return Some(None);
                            }
                            plugins.iter().find_map(|plugin| {
                                let label = format!("{} · {}", plugin.name, plugin.format);
                                choice(ui, &label, dialog.plugin.as_ref() == Some(plugin))
                                    .then(|| Some(plugin.clone()))
                            })
                        },
                    );
                    if let Some(Some(plugin)) = chosen {
                        dialog.plugin = plugin;
                    }
                    if plugins.is_empty() {
                        text::caption(ui, "No instruments scanned yet: Window › Rescan Plugins.");
                    }
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if TextButton::new("Create")
                        .primary(true)
                        .enabled(room > 0 && self.busy_since.is_none())
                        .show(ui)
                        .clicked()
                    {
                        create = true;
                    }
                    if TextButton::new("Cancel").show(ui).clicked() {
                        cancel = true;
                    }
                });
            });
        if create {
            self.send(Action::AddTracks {
                kind: if dialog.bus {
                    clef_model::project::TrackKind::Bus
                } else {
                    clef_model::project::TrackKind::Instrument
                },
                count: dialog.count,
                name: dialog.name.clone(),
                plugin: dialog.plugin.clone().filter(|_| !dialog.bus),
            });
        } else if open && !cancel {
            self.new_tracks = Some(dialog);
        }
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
            let rows: Vec<[String; 3]> = self
                .view
                .project
                .tracks
                .iter()
                .filter_map(|track| {
                    let plugin = track.plugin.as_ref()?;
                    let entry = self
                        .view
                        .quarantine
                        .iter()
                        .find(|e| crate::quarantine::same(&e.plugin, plugin))?;
                    Some([
                        plugin.name.clone(),
                        track.name.clone(),
                        entry.reason.clone(),
                    ])
                })
                .collect();
            let outcome = NoticeBox::new(
                "quarantine-notice",
                "This project contains quarantined plugins",
                "Saved states are retained. These instruments will not load until explicitly reactivated.",
                "Continue ↩",
            )
            .rows(&["Plugin", "Track", "Reason"], &rows)
            .secondary("Show Quarantined Plugins…")
            .show(ctx);
            if outcome == Some(Outcome::Secondary) {
                show = true;
            }
            if outcome.is_some() {
                dismiss = true;
            }
            if dismiss {
                self.quarantine_notice = false;
            }
            if show {
                self.quarantine_open = true;
            }
        }
        if self.quarantine_open {
            let mut open = true;
            let mut reactivate = None;
            ToolWindow::new("Quarantined Plugins")
                .width(560.0)
                .show(ctx, &mut open, |ui| {
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
                                text::caption(ui, &entry.reason);
                                if TextButton::new("Reactivate Plugin…")
                                    .enabled(self.busy_since.is_none())
                                    .show(ui)
                                    .clicked()
                                {
                                    reactivate = Some(entry.plugin.clone());
                                }
                            }
                        });
                    if !self.view.quarantine_error.is_empty() {
                        text::danger(ui, &self.view.quarantine_error);
                    }
                });
            self.quarantine_open = open;
            if let Some(plugin) = reactivate {
                self.confirm = Some(Confirm::Reactivate(plugin));
            }
        }
    }
    /// T41 window layout, top to bottom: menu bar and project title; the full-width transport
    /// band with its region shadow; the workspace (tool row, inspector and channel strip on the
    /// left, arrangement and piano roll on the right, or the mixer); the status row.
    fn main_surface(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut selected = None;
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 12,
                right: 12,
                top: 12,
                bottom: 8,
            })
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                selected = self.menu_bar(ui);
                ui.horizontal_wrapped(|ui| self.project_title(ui));
            });
        // The transport keeps the window's side margins (T41 round 2).
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(12, 0))
            .show(ui, |ui| {
                selected = self.transport_bar(ui).or(selected.take());
            });
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 12,
                right: 12,
                top: 12,
                bottom: 8,
            })
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                selected = self.workspace(ui).or(selected.take());
            });
        self.finish_paint(ctx);
        if let Some(command) = selected {
            self.dispatch(command, ctx);
        }
    }

    /// The project name and its unsaved state; "Audio off" only when audio is not running.
    fn project_title(&self, ui: &mut egui::Ui) {
        text::heading(
            ui,
            &self
                .view
                .path
                .as_ref()
                .and_then(|p| p.file_stem())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled".into()),
        );
        if self.has_unsaved_changes() {
            text::caption(ui, "Unsaved changes");
        }
        if !self.view.active {
            text::caption(ui, "Audio off");
        }
        if self.busy_visible {
            ui.spinner();
            text::caption(ui, "Working…");
        }
    }

    fn workspace(&mut self, ui: &mut egui::Ui) -> Option<Command> {
        let mut selected = None;
        let width = ui.available_width();
        let status_height = theme::metric::CONTROL_HEIGHT;
        let body_height = (ui.available_height() - status_height - 8.0).max(150.0);
        let (body, _) =
            ui.allocate_exact_size(egui::vec2(width, body_height), egui::Sense::hover());
        // Each region is a child on its own rectangle, so its content never moves the
        // workspace cursor: the status row always follows the body.
        let region = |rect| {
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Min))
        };
        {
            let gap = 12.0;
            let left = egui::Rect::from_min_size(
                body.min,
                egui::vec2(theme::metric::INSPECTOR_WIDTH, body.height()),
            );
            // A hidden drawer gives its width to the arrangement.
            let right = if self.left_drawer_hidden {
                body
            } else {
                selected = self.left_column(&mut ui.new_child(region(left)), left);
                egui::Rect::from_min_max(egui::pos2(left.right() + gap, body.top()), body.max)
            };
            let editing = self.editing_column(&mut ui.new_child(region(right)), right);
            selected = editing.or(selected);
        }
        ui.add_space(8.0);
        self.status_row(ui, width).or(selected)
    }

    /// T41 left column: the selected-track inspector over the channel strips. The inspector
    /// takes the height of its content (measured on the previous frame, at most 60 % of the
    /// column); the strips take the rest, each part with its own scroll.
    fn left_column(&mut self, ui: &mut egui::Ui, rect: egui::Rect) -> Option<Command> {
        let gap = 8.0;
        let margin = 12.0;
        let wanted = self.inspector_height + 2.0 * margin;
        let top_height = wanted.min(rect.height() * 0.6).max(64.0).floor();
        let top = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), top_height));
        let bottom =
            egui::Rect::from_min_max(egui::pos2(rect.left(), top.bottom() + gap), rect.max);
        ui.painter().rect_filled(top, 0, theme::PANEL);
        let inner = top.shrink(margin);
        let mut column = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(inner)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        let output = egui::ScrollArea::vertical()
            .id_salt("track-details")
            .max_height(inner.height())
            .auto_shrink([false, true])
            .show(&mut column, |ui| {
                let start = ui.cursor().top();
                let selected = self.inspector(ui);
                (selected, ui.cursor().top() - start)
            });
        let (selected, content) = output.inner;
        if (content - self.inspector_height).abs() > 0.5 {
            self.inspector_height = content;
            ui.ctx().request_repaint();
        }
        ui.painter().rect_filled(bottom, 0, theme::PANEL);
        let mut strips = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(bottom)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        egui::ScrollArea::vertical()
            .id_salt("selected-strip")
            .max_height(bottom.height())
            .auto_shrink([false, false])
            .show(&mut strips, |ui| {
                ui.add_space(8.0);
                self.selected_strips(ui);
            });
        selected
    }

    /// The selected instrument track's inspector (no debug MIDI status line, T41).
    fn inspector(&mut self, ui: &mut egui::Ui) -> Option<Command> {
        let mut selected = None;
        let Some(track) = self.view.track().cloned() else {
            text::caption(
                ui,
                "Add an instrument track, choose a plugin, then play the selected track.",
            );
            return None;
        };
        ui.spacing_mut().item_spacing.y = 6.0;
        // T41 round 2: rename, move and remove live on the header (double-click, drag,
        // context menu) and in the Track menu, not here.
        text::section(ui, "Track");
        text::body(ui, &track.name);
        ui.add_space(8.0);
        text::section(ui, "Instrument");
        text::body(
            ui,
            &track
                .plugin
                .as_ref()
                .map(|p| format!("{} · {}", p.name, p.format))
                .unwrap_or_else(|| "No instrument".into()),
        );
        let state = self
            .view
            .state(track.id)
            .map(|s| s.status.as_str())
            .unwrap_or("Empty");
        if self.view.failed {
            text::danger(ui, state);
        } else {
            text::caption(ui, state);
        }
        ui.horizontal_wrapped(|ui| {
            // Plugin editors open from their slot in the channel strip.
            for command in [Command::Instrument] {
                if self.button(ui, command) {
                    selected = Some(command);
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            for command in [Command::Restart, Command::Reactivate, Command::Quarantine] {
                if self.button(ui, command) {
                    selected = Some(command);
                }
            }
        });
        ui.add_space(8.0);
        text::section(ui, "Recording");
        let mode_enabled = !self.view.recording
            && !self.view.transport.pending
            && self.busy_since.is_none()
            && self.view.tracks.iter().all(|track| !track.loading);
        let (_, chosen) = menu_button(
            ui,
            ("track-record-mode", track.id),
            "Record mode",
            Some("Record mode"),
            track.record_mode.label(),
            mode_enabled,
            |ui| {
                TrackMode::ALL
                    .into_iter()
                    .find(|&mode| choice(ui, mode.label(), mode == track.record_mode))
            },
        );
        if let Some(Some(mode)) = chosen
            && mode != track.record_mode
        {
            self.send(Action::Transport(TransportAction::SetTrackRecordMode(
                track.id, mode,
            )));
        }
        text::caption(ui, "Global follows the transport record mode.");
        selected
    }

    /// T41 right column: tool row, arrangement, and the lower panel (piano roll or mixer)
    /// under it when open and docked. At full height (S2) the lower panel takes the whole
    /// column and the arrangement is hidden.
    fn editing_column(&mut self, ui: &mut egui::Ui, rect: egui::Rect) -> Option<Command> {
        self.track_area_width = rect.width();
        let docked = (self.piano_roll_open || self.mixer_ui.open) && !self.panel.detached;
        let full = docked && self.panel.full;
        let mut selected = None;
        if !full {
            selected = self.tool_row(ui, rect.width());
            ui.add_space(8.0);
            let height = ui.available_height().max(150.0);
            let arrangement_height = if docked {
                self.piano_roll.split_height(height)
            } else {
                height
            };
            let (choose, transport_action) =
                self.timeline_surface(ui, arrangement_height, rect.width());
            if let Some(action) = transport_action {
                self.send(Action::Transport(action));
            }
            if let Some(id) = choose {
                self.select_track(id);
            }
            if docked {
                self.piano_roll.gutter(ui, height);
            }
        }
        if let Some(id) = self.arrangement_ui.open_roll.take()
            && self.view.project.midi.find_clip(id).is_some()
        {
            // Editor and Mixer share the lower panel: opening the editor closes the mixer.
            if self.mixer_ui.open {
                self.dispatch_mixer(Command::ShowMixer);
            }
            if let Some((_, clip)) = self.view.project.midi.find_clip(id) {
                self.piano_roll.open(id, clip);
            }
            self.piano_roll_open = true;
            ui.ctx().request_repaint();
        }
        if docked {
            selected = self.lower_panel(ui).or(selected);
        }
        selected
    }

    /// T41 round 3 (S1, S2): the lower panel's content, docked or in its own window: the
    /// mixer or the piano roll, with the panel controls at the right end of its tool row.
    fn lower_panel(&mut self, ui: &mut egui::Ui) -> Option<Command> {
        let mut control = None;
        if self.mixer_ui.open {
            control = self.show_mixer(ui);
        } else if self.piano_roll_open {
            let editor_height = ui.available_height();
            let enabled = self.view.edit_history.available
                && self.busy_since.is_none()
                && self.confirm.is_none()
                && self.file.is_none();
            self.piano_roll.panel = self.panel;
            self.piano_roll.keyboard = self.keyboard_mode;
            if let Some(action) = self.piano_roll.show(
                ui,
                editor_height,
                &self.view,
                &self.arrangement_ui.selected,
                enabled,
            ) {
                self.send(action);
            }
            control = self.piano_roll.panel_request.take();
        }
        use primitives::panel_controls::PanelControl;
        match control? {
            PanelControl::Detach => Some(Command::TogglePanelDetach),
            PanelControl::Full => Some(Command::TogglePanelFull),
            PanelControl::Close => {
                if self.mixer_ui.open {
                    Some(Command::ShowMixer)
                } else {
                    self.piano_roll_open = false;
                    None
                }
            }
        }
    }

    /// T41 round 3 (S2): a detached lower panel in its own OS window (an egui viewport),
    /// which can sit on another monitor. Closing the window closes the panel; Attach docks it.
    fn detached_panel(&mut self, ctx: &egui::Context) {
        if !self.view.project_open
            || !self.panel.detached
            || !(self.mixer_ui.open || self.piano_roll_open)
        {
            return;
        }
        let title = if self.mixer_ui.open {
            "Mixer — Clef"
        } else {
            "Editor — Clef"
        };
        let builder = egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size([1100.0, 680.0])
            .with_min_inner_size([520.0, 320.0]);
        let selected = ctx.show_viewport_immediate(panel_viewport(), builder, |ui, _| {
            let rect = ui.max_rect();
            ui.painter().rect_filled(rect, 0.0, theme::BACKGROUND);
            let closing = ui.input(|input| input.viewport().close_requested());
            let selected = egui::Frame::new()
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    self.lower_panel(ui)
                })
                .inner;
            if closing {
                Some(if self.mixer_ui.open {
                    Command::ShowMixer
                } else {
                    Command::ShowEditor
                })
            } else {
                selected
            }
        });
        self.finish_paint(ctx);
        if let Some(command) = selected {
            self.dispatch(command, ctx);
        }
    }

    /// T41 status row: engine diagnostics at the left, the Editor and Mixer toggles centred,
    /// the latest message or error at the right.
    fn status_row(&mut self, ui: &mut egui::Ui, width: f32) -> Option<Command> {
        let mut selected = None;
        let (row, _) = ui.allocate_exact_size(
            egui::vec2(width, theme::metric::CONTROL_HEIGHT),
            egui::Sense::hover(),
        );
        let centre_width = 160.0;
        let side = ((width - centre_width) / 2.0 - 12.0).max(0.0);
        let layout =
            |align| egui::Layout::left_to_right(egui::Align::Center).with_main_align(align);
        let left = egui::Rect::from_min_size(row.min, egui::vec2(side, row.height()));
        let centre =
            egui::Rect::from_center_size(row.center(), egui::vec2(centre_width, row.height()));
        let right = egui::Rect::from_min_max(egui::pos2(row.right() - side, row.top()), row.max);
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(left)
                .layout(layout(egui::Align::Min)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let name = self.keymap.tooltip(ui.ctx(), Command::ToggleLeftDrawer);
                if crate::primitives::tool_button::tool_button(
                    ui,
                    crate::primitives::icons::Icon::Drawer,
                    &name,
                    self.command_available(Command::ToggleLeftDrawer, false),
                    !self.left_drawer_hidden,
                    self.keyboard_mode,
                )
                .clicked()
                    && self.enabled(Command::ToggleLeftDrawer)
                {
                    selected = Some(Command::ToggleLeftDrawer);
                }
                text::status(ui, &self.view.actual, false);
            },
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(centre)
                .layout(layout(egui::Align::Center)),
            |ui| {
                // Flat texts with a divider (T41 round 2); a click on the lit one hides it.
                let commands = [Command::ShowEditor, Command::ShowMixer];
                let shown = if self.mixer_ui.open {
                    Some(1)
                } else if self.piano_roll_open {
                    Some(0)
                } else {
                    None
                };
                let enabled = commands.map(|command| self.command_available(command, false));
                if let Some(index) = crate::primitives::tabs::toggle_tabs(
                    ui,
                    "Lower views",
                    &["Editor", "Mixer"],
                    shown,
                    &enabled,
                ) && self.enabled(commands[index])
                {
                    selected = Some(commands[index]);
                }
            },
        );
        let (message, danger) = self.status_message();
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(right)
                .layout(layout(egui::Align::Max)),
            |ui| {
                text::status(ui, &message, danger);
            },
        );
        selected
    }

    /// The status row's message: errors first, then the recording state, then the latest
    /// session message, then why recording is unavailable.
    fn status_message(&self) -> (String, bool) {
        if !self.view.quarantine_error.is_empty() {
            return (self.view.quarantine_error.clone(), true);
        }
        if !self.view.history_message.is_empty() {
            return (self.view.history_message.clone(), true);
        }
        if self.view.recording {
            let label = match self.view.transport.phase {
                clef_session::TransportPhase::CountingIn => "Count-in",
                clef_session::TransportPhase::Playing if self.view.recording_region.is_some() => {
                    "Recording MIDI"
                }
                _ => "Preparing MIDI recording",
            };
            return (label.into(), true);
        }
        let message = if self.status.is_empty() {
            &self.view.message
        } else {
            &self.status
        };
        if !message.is_empty() {
            return (message.clone(), self.view.failed);
        }
        (
            self.view.record_unavailable.unwrap_or_default().into(),
            false,
        )
    }

    /// T41: per-track diagnostics, opened from Window → Track Diagnostics.
    fn diagnostics_window(&mut self, ctx: &egui::Context) {
        if !self.view.project_open || !self.diagnostics_open {
            return;
        }
        let mut open = true;
        ToolWindow::new("Track Diagnostics")
            .width(560.0)
            .show(ctx, &mut open, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("track-diagnostics")
                    .max_height(320.0)
                    .show(ui, |ui| {
                        for track in &self.view.project.tracks {
                            if let Some(state) = self.view.state(track.id) {
                                text::body(ui, &track.name);
                                text::caption(
                                    ui,
                                    &format!("{} · {}", state.status, state.diagnostics),
                                );
                                ui.add_space(6.0);
                            }
                        }
                    });
            });
        self.diagnostics_open = open;
    }

    fn plugin_picker(&mut self, ctx: &egui::Context) {
        if !self.picker_open {
            return;
        }
        let mut open = true;
        let mut picked = None;
        let mut rescan = false;
        ToolWindow::new("Choose Instrument").width(560.0).show(ctx, &mut open, |ui| {
            SearchField::new(&mut self.plugin_search, "Search instruments").hint("Search instruments by name or format").show(ui);
            ui.horizontal(|ui| {
                if self.button(ui,Command::Rescan) { rescan = true; }
                if TextButton::new("Show Quarantined Plugins…").show(ui).clicked() { self.quarantine_open=true; }
            });
            if self.scan_job.is_some() { ui.horizontal(|ui| { ui.spinner(); ui.label("Scanning plugins in isolated processes…"); }); }
            let search = self.plugin_search.to_lowercase();
            egui::ScrollArea::vertical().max_height(320.0).show(ui,|ui| {
                for plugin in &self.catalog.plugins {
                    let quarantined = self.view.quarantine.iter().any(|e| crate::quarantine::same(&e.plugin,plugin));
                    if !format!("{} {}",plugin.name,plugin.format).to_lowercase().contains(&search) { continue; }
                    let label = format!("{} · {}{}", plugin.name, plugin.format, if quarantined {" · Quarantined"} else {""});
                    let path = plugin.path.display().to_string();
                    let full = format!("{label}, {path}");
                    if ListRow::new(&label).accessible(&full).enabled(self.busy_since.is_none() && !quarantined).show(ui).clicked() { picked = Some(plugin.clone()); }
                }
                if self.catalog.plugins.is_empty() && self.scan_job.is_none() { ui.label("No instruments found. Install a VST3 or CLAP instrument, then Rescan."); }
            });
            if !self.catalog.failures.is_empty() { ui.collapsing(format!("{} scan failures",self.catalog.failures.len()),|ui| { egui::ScrollArea::vertical().id_salt("scan-failures").max_height(120.0).show(ui, |ui| { for failure in &self.catalog.failures { text::danger(ui, failure); } }); }); }
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
        ToolWindow::new("Audio/MIDI Settings").width(580.0).show(ctx, &mut open, |ui| {
            ui.horizontal(|ui| {
                ui.label("Applies to");
                primitives::segmented::Segmented::new(
                    "Applies to",
                    &mut self.settings_project,
                    &[(false, "All projects"), (true, "This project")],
                )
                .show(ui);
            });
            text::caption(ui, "All projects saves Clef's audio preferences. This project saves an override in the project file.");
            ui.label("Applying restarts audio.");
            if self.button(ui,Command::Refresh) { action = Some(Command::Refresh); }
            if self.device_job.is_some() { ui.spinner(); }
            ui.add_space(12.0);
            let name = self.devices.audio.iter().find(|d| d.id == self.settings.device).map(|d| d.name.as_str()).unwrap_or("Choose an audio output");
            let (_, device) = menu_button(ui, "audio-output", "Audio output", Some("Audio output"), name, true, |ui| {
                self.devices.audio.iter().find(|device| choice(ui, &device.name, device.id == self.settings.device)).map(|device| device.id.clone())
            });
            if let Some(Some(device)) = device { self.settings.device = device; }
            let selected = self.devices.audio.iter().find(|d| d.id == self.settings.device);
            let channels = selected.and_then(|d| d.configurations.iter().map(|c| c.channels).max()).unwrap_or(0);
            let pair = self.settings.pair;
            let (_, chosen) = menu_button(ui, "output-pair", "Stereo output pair", Some("Stereo output pair"), &format!("{}–{}",pair[0]+1,pair[1]+1), true, |ui| {
                (0..channels.saturating_sub(1) as usize).step_by(2).find(|&left| choice(ui, &format!("{}–{}",left+1,left+2), pair == [left,left+1]))
            });
            if let Some(Some(left)) = chosen { self.settings.pair = [left, left+1]; }
            let current = self.settings.rate;
            let (_, chosen) = menu_button(ui, "sample-rate", "Sample rate", Some("Sample rate"), &format!("{current} Hz"), true, |ui| {
                let mut rates = vec![8000,11025,16000,22050,32000,44100,48000,88200,96000,176400,192000,352800,384000];
                if let Some(device) = selected { for c in &device.configurations { rates.extend([c.min_rate,c.max_rate]); } } rates.sort();rates.dedup();
                rates.into_iter().filter(|rate| selected.is_some_and(|d| d.configurations.iter().any(|c| (c.min_rate..=c.max_rate).contains(rate)))).find(|&rate| choice(ui, &format!("{rate} Hz"), rate == current))
            });
            if let Some(Some(rate)) = chosen { self.settings.rate = rate; }
            let settings = self.settings.clone();
            let (_, chosen) = menu_button(ui, "buffer-size", "Buffer size", Some("Buffer size"), &format!("{} frames", settings.buffer), true, |ui| {
                let mut values = vec![16,32,64,128,256,512,1024,2048,4096];
                if let Some(device) = selected { for c in &device.configurations { if let clef_audio_io::SupportedBufferSize::Range { min,max } = c.buffer { values.extend([min,max]); } } } values.sort(); values.dedup();
                values.into_iter().filter(|&frames| { let mut candidate = settings.clone(); candidate.buffer = frames; selected.is_some_and(|d| d.configurations.iter().any(|c| c.supports(&candidate))) })
                    .find(|&frames| choice(ui, &format!("{frames} frames"), frames == settings.buffer))
            });
            if let Some(Some(frames)) = chosen { self.settings.buffer = frames; }
            ui.add_space(12.0);
            let midi = self.devices.midi.iter().find(|(id,_)| id == &self.settings.midi).map(|(_,n)| n.as_str()).unwrap_or("Choose a MIDI input");
            let (_, chosen) = menu_button(ui, "midi-input", "MIDI input", Some("MIDI input"), midi, true, |ui| {
                self.devices.midi.iter().find(|(id, name)| choice(ui, name, id == &self.settings.midi)).map(|(id, _)| id.clone())
            });
            if let Some(Some(id)) = chosen { self.settings.midi = id; }
            ui.add_space(12.0);
            match self.devices.validate(&self.settings) { Ok(()) => { if self.button(ui,Command::ApplySettings) { action = Some(Command::ApplySettings); } }, Err(e) => { text::caption(ui, &e.to_string()); } }
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
        ToolWindow::new(title)
            .width(620.0)
            .show(ctx, &mut open, |ui| {
                text::caption(ui, &picker.folder.display().to_string());
                if TextButton::new("Parent Folder").show(ui).clicked() {
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
                                let label = format!(
                                    "{}{}",
                                    entry.name,
                                    if entry.folder { "/" } else { "" }
                                );
                                if ListRow::new(&label)
                                    .selected(picker.name == entry.name)
                                    .show(ui)
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
                            text::danger(ui, error.as_str());
                        }
                    });
                text::caption(ui, "File name or absolute path");
                TextField::new(&mut picker.name, "File name or absolute path").show(ui);
                if !picker.error.is_empty() {
                    text::danger(ui, &picker.error);
                }
                if TextButton::new(title).primary(true).show(ui).clicked() {
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
        egui::Modal::new(egui::Id::new("confirmation"))
            .frame(frame().inner_margin(16))
            .show(ctx, |ui| {
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
                    let primary = if matches!(confirm, Confirm::Unsaved(_)) {
                        "Save"
                    } else if matches!(confirm, Confirm::Retained(_)) {
                        "Save Using Retained State"
                    } else if matches!(confirm, Confirm::SplitClips(_, _)) {
                        "Split Notes"
                    } else {
                        "Continue"
                    };
                    if TextButton::new(primary).primary(true).show(ui).clicked() {
                        choice = Some(0);
                    }
                    if matches!(confirm, Confirm::Unsaved(_))
                        && TextButton::new("Discard").show(ui).clicked()
                    {
                        choice = Some(1);
                    }
                    if TextButton::new("Cancel").show(ui).clicked() {
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

    /// The live name of a command: Undo and Redo name the edit, Show Mixer flips.
    fn command_label(&self, command: Command) -> String {
        let history = &self.view.edit_history;
        if command == Command::ShowMixer && self.mixer_ui.open {
            return "Hide Mixer".into();
        }
        let edit = match command {
            Command::Undo => history
                .cursor
                .checked_sub(1)
                .and_then(|index| history.labels.get(index)),
            Command::Redo => history.labels.get(history.cursor),
            _ => None,
        };
        edit.map_or_else(
            || command.title().into(),
            |edit| format!("{} {edit}", command.title()),
        )
    }

    /// Debug builds only: every T18 primitive on one scrolling page.
    fn primitive_gallery_window(&mut self, ctx: &egui::Context) {
        let mut open = self.primitive_gallery_open.get();
        if !open {
            return;
        }
        let height = ctx.content_rect().height() * 0.8;
        ToolWindow::new("Primitive Gallery (Debug)")
            .width(760.0)
            .show(ctx, &mut open, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.primitive_gallery.show_all(ui, ctx));
            });
        self.primitive_gallery_open.set(open);
    }

    /// Commands / Keyboard Shortcuts: remap only. Running commands belongs to the palette.
    fn command_window(&mut self, ctx: &egui::Context) {
        if !self.commands_open {
            return;
        }
        let mut open = true;
        let mut rebind = None;
        let mut clear = None;
        let mut restore = false;
        let search = self.command_search.to_lowercase();
        ToolWindow::new("Commands / Keyboard Shortcuts").width(560.0).show(ctx, &mut open, |ui| {
            SearchField::new(&mut self.command_search, "Search commands").show(ui);
            text::caption(ui, "Set, then press the new shortcut; Escape cancels. Changes apply to every project.");
            egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                for spec in commands::SPECS {
                    let command = spec.command;
                    if !spec.title.to_lowercase().contains(&search) { continue; }
                    let waiting = self.rebinding == Some(command);
                    ui.horizontal(|ui| {
                        ui.add_sized([300.0, ui.spacing().interact_size.y], egui::Label::new(spec.title).truncate());
                        let shortcut = self.keymap.text(ctx, command);
                        text::caption(ui, if waiting { "Press a shortcut…" } else if shortcut.is_empty() { "Unbound" } else { &shortcut });
                        if TextButton::new("Set").toggle(waiting).accessible(&format!("Set shortcut for {}", spec.title)).show(ui).clicked() { rebind = Some(command); }
                        if TextButton::new("Clear").enabled(!shortcut.is_empty()).accessible(&format!("Clear shortcut for {}", spec.title)).show(ui).clicked() { clear = Some(command); }
                    });
                }
            });
            if TextButton::new("Restore Defaults").show(ui).clicked() { restore = true; }
        });
        if let Some((shortcut, other)) = self.rebind_conflict
            && let Some(command) = self.rebinding
        {
            let body = format!(
                "{} is already used by {}. Replace it there, or cancel and keep both as they are.",
                ctx.format_shortcut(&shortcut),
                other.title()
            );
            match NoticeBox::new(
                "shortcut-conflict",
                "Shortcut already in use",
                &body,
                "Replace",
            )
            .question()
            .secondary("Cancel")
            .show(ctx)
            {
                Some(Outcome::Primary) => {
                    self.keymap.set(other, None);
                    self.assign_shortcut(command, Some(shortcut));
                }
                Some(_) => {
                    self.rebind_conflict = None;
                    self.rebinding = None;
                }
                None => {}
            }
        }
        self.commands_open = open;
        if !open {
            self.rebinding = None;
            self.rebind_conflict = None;
        }
        if let Some(command) = rebind {
            self.rebinding = (self.rebinding != Some(command)).then_some(command);
        }
        if let Some(command) = clear {
            self.assign_shortcut(command, None);
        }
        if restore {
            self.keymap = Keymap::default();
            self.rebinding = None;
            self.save_shortcuts();
        }
    }
}
/// T41 round 3 (S2): the viewport of the detached lower panel.
fn panel_viewport() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("clef-lower-panel")
}

impl eframe::App for App {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        theme::BACKGROUND.to_normalized_gamma_f32()
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
        paint::set_keyboard(ctx, self.keyboard_mode);
        self.poll(ctx);
        self.sync_shortcuts();
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
            self.main_surface(ui, &ctx);
        } else {
            self.startup_surface(ui, &ctx);
        }
        self.plugin_picker(&ctx);
        self.settings(&ctx);
        self.transport_windows(&ctx);
        self.command_window(&ctx);
        self.diagnostics_window(&ctx);
        self.detached_panel(&ctx);
        self.new_tracks_window(&ctx);
        self.project_format_window(&ctx);
        self.command_palette(&ctx);
        if cfg!(debug_assertions) {
            self.primitive_gallery_window(&ctx);
        }
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
        crate::theme::install_fonts(&ctx);
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
    fn press(key: Key, modifiers: Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn unmodified_shortcuts_wait_while_text_is_typed() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut app = App::new(ctx.clone(), PathBuf::new());
        app.view.project_open = true;
        app.keymap.set(
            Command::Commands,
            Some(KeyboardShortcut::new(Modifiers::NONE, Key::K)),
        );
        let field = egui::Id::new("typing");
        let mut text = String::new();
        let mut frame = |app: &mut App, events, focus: bool| {
            let _ = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    app.shortcuts(&ctx);
                    let response = ui.add(egui::TextEdit::singleline(&mut text).id(field));
                    if focus {
                        response.request_focus();
                    } else {
                        response.surrender_focus();
                    }
                },
            );
        };
        frame(&mut app, vec![], true);
        frame(&mut app, vec![press(Key::K, Modifiers::NONE)], true);
        assert!(!app.commands_open, "a plain key fired while typing");
        frame(&mut app, vec![], false);
        frame(&mut app, vec![press(Key::K, Modifiers::NONE)], false);
        assert!(app.commands_open);
    }

    #[test]
    fn palette_shortcut_opens_it_and_a_clashing_assignment_asks_first() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut app = App::new(ctx.clone(), PathBuf::new());
        app.view.project_open = true;
        let run = |app: &mut App, events| {
            let _ = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |_| app.shortcuts(&ctx),
            );
        };
        let command = Modifiers::MAC_CMD | Modifiers::COMMAND;
        run(&mut app, vec![press(Key::P, command | Modifiers::SHIFT)]);
        assert!(app.palette.is_some());
        app.palette = None;

        app.rebinding = Some(Command::Play);
        run(&mut app, vec![press(Key::N, command)]);
        let new = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
        assert_eq!(app.rebind_conflict, Some((new, Command::New)));
        assert_eq!(app.keymap.shortcut(Command::Play), None);
        assert_eq!(app.keymap.shortcut(Command::New), Some(new));

        app.rebind_conflict = None;
        app.rebinding = Some(Command::Play);
        // (P is still held from the palette shortcut, so egui would report a repeat.)
        run(&mut app, vec![press(Key::L, Modifiers::NONE)]);
        assert_eq!(
            app.keymap.shortcut(Command::Play),
            Some(KeyboardShortcut::new(Modifiers::NONE, Key::L))
        );
        assert!(app.rebinding.is_none());
        assert_eq!(
            app.piano_roll.keys, app.keymap,
            "the roll sees remapped shortcuts"
        );
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
        crate::theme::install_fonts(&context);
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
        crate::theme::install_fonts(&context);
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
        crate::theme::install_fonts(&context);
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
        crate::theme::install_fonts(&context);
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
        crate::theme::install_fonts(&context);
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
        crate::theme::install_fonts(&context);
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
        crate::theme::install_fonts(&context);
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
        assert_eq!(style.visuals.widgets.inactive.bg_fill, theme::PANEL);
        assert_eq!(style.visuals.widgets.hovered.bg_fill, theme::FIELD);
        assert_eq!(style.visuals.widgets.active.bg_fill, theme::FIELD);
        assert!(!style.interaction.selectable_labels);
    }

    #[test]
    fn take_commands_need_a_lane_and_have_no_default_shortcut() {
        use clef_model::midi::{Event, Message, Mode};
        let context = egui::Context::default();
        crate::theme::install_fonts(&context);
        let mut app = App::new(context, PathBuf::new());
        app.view.project_open = true;
        app.view.selected = Some(1);
        app.view.project.tracks.push(crate::project::Track {
            colour: None,
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
                app.keymap.shortcut(command).is_none(),
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
        crate::theme::install_fonts(&context);
        let mut app = App::new(context.clone(), PathBuf::new());
        app.view.project.next_id = 2;
        app.view.selected = Some(1);
        app.view.project.tracks = vec![crate::project::Track {
            colour: None,
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
        crate::theme::install_fonts(&context);
        let mut app = App::new(context.clone(), PathBuf::new());
        app.view.project_open = true;
        app.view.project.tracks = vec![
            crate::project::Track {
                colour: None,
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
                colour: None,
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
                    if rect.fill == theme::track_colour(clef_model::project::TRACK_COLOURS[0])
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
