use clef_model::{
    Plugin, Result,
    project::{self, AudioSettings},
};
use clef_plugin_host::{
    catalog::{self, Catalog},
    quarantine,
};
use clef_session::{Action, Backend, Devices, Snapshot};
use eframe::egui::{self, Color32, Key, KeyboardShortcut, Modifiers};
use std::{path::PathBuf, sync::mpsc};

mod colors {
    use super::Color32;
    pub const BACKGROUND: Color32 = Color32::from_rgb(0x10, 0x1a, 0x1c);
    pub const PANEL: Color32 = Color32::from_rgb(0x17, 0x23, 0x26);
    pub const FIELD: Color32 = Color32::from_rgb(0x1f, 0x2f, 0x32);
    pub const TEXT: Color32 = Color32::from_rgb(0xe3, 0xe8, 0xe5);
    pub const SECONDARY: Color32 = Color32::from_rgb(0xa4, 0xb5, 0xae);
    pub const ACCENT: Color32 = Color32::from_rgb(0x78, 0xb5, 0xaa);
    pub const PEACOCK: Color32 = Color32::from_rgb(0x33, 0x6a, 0x64);
    pub const DISABLED: Color32 = Color32::from_rgb(0x74, 0x84, 0x7d);
    pub const DANGER: Color32 = Color32::from_rgb(0xc7, 0x84, 0x7c);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Command {
    New,
    Open,
    Save,
    SaveAs,
    Close,
    Settings,
    Commands,
    AddTrack,
    Instrument,
    Editor,
    CloseEditor,
    Enable,
    Disable,
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
        (Close, "Close Clef", Some(Key::Q)),
        (Settings, "Audio/MIDI Settings", None),
        (Commands, "Commands / Keyboard Shortcuts", Some(Key::K)),
        (AddTrack, "Add Instrument Track", None),
        (Instrument, "Choose Instrument", None),
        (Editor, "Open Editor", None),
        (CloseEditor, "Close Editor", None),
        (Enable, "Enable Audio", None),
        (Disable, "Disable Audio", None),
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
    ]
    .into_iter()
    .map(|(command, title, key)| Binding {
        command,
        title,
        shortcut: key.map(|key| KeyboardShortcut::new(Modifiers::COMMAND, key)),
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
}
#[derive(Clone, Copy)]
enum FileMode {
    Open,
    Save,
}
struct FilePicker {
    mode: FileMode,
    folder: PathBuf,
    name: String,
    error: String,
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
    executable: PathBuf,
    quit_gate: Option<QuitGate>,
    backend: Backend,
    view: Snapshot,
    busy: bool,
    pending_after_save: Option<Transition>,
    awaiting_save: bool,
    exiting: bool,
    confirm: Option<Confirm>,
    file: Option<FilePicker>,
    settings_open: bool,
    settings: AudioSettings,
    devices: Devices,
    device_job: Option<mpsc::Receiver<std::result::Result<Devices, String>>>,
    picker_open: bool,
    catalog: Catalog,
    scan_job: Option<crate::catalog::ScanJob>,
    plugin_search: String,
    bindings: Vec<Binding>,
    commands_open: bool,
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
        ] {
            widget.bg_fill = colors::FIELD;
            widget.weak_bg_fill = colors::FIELD;
            widget.corner_radius = egui::CornerRadius::same(2);
        }
        style.spacing.item_spacing = egui::vec2(12.0, 8.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.spacing.interact_size.y = 24.0;
        context.set_global_style(style);
        Self {
            quit_gate: None,
            backend: Backend::spawn(executable.clone(), move || context.request_repaint()),
            executable,
            view: Snapshot::default(),
            busy: false,
            pending_after_save: None,
            awaiting_save: false,
            exiting: false,
            confirm: None,
            file: None,
            settings_open: false,
            settings: AudioSettings::default(),
            devices: Devices::default(),
            device_job: None,
            picker_open: false,
            catalog: Catalog::default(),
            scan_job: None,
            plugin_search: String::new(),
            bindings: commands(),
            commands_open: false,
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
        if matches!(action, Action::Disable) {
            self.backend
                .silence
                .store(true, std::sync::atomic::Ordering::Release);
        }
        if self.busy && !matches!(action, Action::Disable) {
            return;
        }
        match self.backend.send.send(action) {
            Ok(()) => {
                self.busy = true;
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
        if (self.busy && command != Command::Disable)
            || self.confirm.is_some()
            || self.file.is_some()
        {
            return false;
        }
        use Command::*;
        match command {
            AddTrack => self.view.project.tracks.len() < crate::project::MAX_TRACKS,
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
            Instrument | Rename => self.view.track().is_some(),
            Editor => self.view.track().is_some_and(|t| t.plugin.is_some()) && !self.view.failed,
            CloseEditor => self.view.editor,
            Enable => {
                self.view.track().is_some_and(|t| t.plugin.is_some())
                    && !self.view.active
                    && !self.view.failed
            }
            Disable => self.view.active,
            Restart => self.view.track().is_some_and(|t| t.plugin.is_some()),
            Rescan => self.scan_job.is_none(),
            Refresh => self.device_job.is_none(),
            ApplySettings => self.settings_open,
            _ => true,
        }
    }
    fn button(&self, ui: &mut egui::Ui, command: Command) -> bool {
        let binding = self.bindings.iter().find(|b| b.command == command).unwrap();
        let hint = binding
            .shortcut
            .map(|s| ui.ctx().format_shortcut(&s))
            .unwrap_or_default();
        ui.add_enabled(self.enabled(command), egui::Button::new(binding.title))
            .on_hover_text(if hint.is_empty() {
                binding.title.into()
            } else {
                format!("{} ({hint})", binding.title)
            })
            .clicked()
    }
    fn has_unsaved_changes(&self) -> bool {
        self.view.dirty || self.view.track().is_some_and(|t| t.name != self.rename)
    }
    fn transition(&mut self, next: Transition, ctx: &egui::Context) {
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
                self.exiting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
    fn save(&mut self, as_new: bool) {
        if self
            .view
            .tracks
            .iter()
            .any(|t| t.lost && !self.retained_approved.contains(&(t.id, t.loss_version)))
        {
            self.confirm = Some(Confirm::Retained(as_new));
            return;
        }
        if !as_new && let Some(path) = self.view.path.clone() {
            self.awaiting_save = true;
            self.send(Action::Save {
                path,
                replace: true,
                track_name: self.view.selected.map(|id| (id, self.rename.clone())),
                use_retained: self.retained_approved.clone(),
            });
        } else {
            self.file = Some(FilePicker::new(FileMode::Save, self.view.path.as_ref()));
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
            New => self.transition(Transition::New, ctx),
            Open => self.file = Some(FilePicker::new(FileMode::Open, self.view.path.as_ref())),
            Save => self.save(false),
            SaveAs => self.save(true),
            Close => self.transition(Transition::Close, ctx),
            Settings => {
                self.settings = self.view.project.audio.clone();
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
            Enable => self.send(Action::Enable),
            Disable => self.send(Action::Disable),
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
            ApplySettings => self.send(Action::Settings(self.settings.clone())),
        }
    }
    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok((view, completed)) = self.backend.receive.try_recv() {
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
            self.view = view;
            if let Some(success) = completed {
                self.busy = false;
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
        let selected = self
            .bindings
            .iter()
            .filter(|b| self.enabled(b.command))
            .find_map(|b| {
                b.shortcut
                    .filter(|s| ctx.input_mut(|i| i.consume_shortcut(s)))
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
                .min_size(egui::vec2(width, 44.0)),
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
                                        !self.busy,
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
            for command in [
                Command::New,
                Command::Open,
                Command::Save,
                Command::SaveAs,
                Command::Settings,
                Command::Commands,
            ] {
                if self.button(ui, command) {
                    selected = Some(command);
                }
            }
        });
        ui.add_space(20.0);
        ui.horizontal(|ui| {
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
        });
        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            let command = if self.view.active {
                Command::Disable
            } else {
                Command::Enable
            };
            if self.button(ui, command) {
                selected = Some(command);
            }
            ui.colored_label(
                if self.view.active {
                    colors::ACCENT
                } else {
                    colors::SECONDARY
                },
                if self.view.active {
                    "Audio active"
                } else {
                    "Audio stopped"
                },
            );
            if self.busy {
                ui.spinner();
                ui.label("Working…");
            }
        });
        ui.add_space(12.0);
        let mut choose = None;
        let height = (ui.available_height() - 140.0).max(110.0);
        let left_width = if ui.available_width() >= 920.0 {
            320.0
        } else {
            240.0
        };
        ui.horizontal_top(|ui| {
            egui::Frame::new().fill(colors::PANEL).inner_margin(12.0).show(ui, |ui| { ui.vertical(|ui| {
                ui.set_width(left_width - 24.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(format!("Tracks ({})",self.view.project.tracks.len())).size(16.0));
                    if self.button(ui,Command::AddTrack) { selected = Some(Command::AddTrack); }
                });
                egui::ScrollArea::vertical().id_salt("tracks").max_height(height-45.0).show(ui,|ui| {
                    for (index, track) in self.view.project.tracks.iter().enumerate() {
                        let state = self.view.state(track.id).map(|s|s.status.as_str()).unwrap_or("Empty");
                        let plugin = track.plugin.as_ref().map(|p|format!("{} · {}",p.name,p.format)).unwrap_or_else(||"No instrument".into());
                        let is_selected = self.view.selected == Some(track.id);
                        let response=Self::track_row(ui,&track.name,&format!("{plugin} · {state}"),left_width-24.0,is_selected,!self.busy && self.confirm.is_none(),self.keyboard_mode);
                        if self.track_focus == Some(track.id) { response.request_focus(); response.scroll_to_me(Some(egui::Align::Center)); self.track_focus = None; }
                        if response.clicked() { choose = Some(track.id); }
                        if response.double_clicked() { choose = Some(track.id); self.rename_focus = true; }
                        if response.has_focus() {
                            if ui.input_mut(|i|i.consume_key(Modifiers::NONE,Key::ArrowDown)) && index+1 < self.view.project.tracks.len() {choose=Some(self.view.project.tracks[index+1].id);}
                            if ui.input_mut(|i|i.consume_key(Modifiers::NONE,Key::ArrowUp)) && index>0 {choose=Some(self.view.project.tracks[index-1].id);}
                        }
                    }
                });
            }); });
            egui::Frame::new().fill(colors::PANEL).inner_margin(12.0).show(ui, |ui| { ui.vertical(|ui| {
                ui.set_width((ui.available_width()-24.0).max(240.0));
                egui::ScrollArea::vertical().id_salt("track-details").max_height(height).show(ui,|ui| {
                    if let Some(track) = self.view.track().cloned() {
                        ui.label(egui::RichText::new("Selected instrument track").size(16.0));
                        let label = ui.label("Track name");
                        let edit = ui.add_enabled(!self.busy,egui::TextEdit::singleline(&mut self.rename).id_salt(("track-name",track.id)).desired_width(f32::INFINITY)).labelled_by(label.id);
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
                        ui.horizontal_wrapped(|ui| { for command in [Command::Instrument,Command::Editor,Command::CloseEditor] {if self.button(ui,command) {selected=Some(command);}} });
                        ui.horizontal_wrapped(|ui| { for command in [Command::Restart,Command::Reactivate,Command::Quarantine] {if self.button(ui,command) {selected=Some(command);}} });
                        ui.add_space(12.0);
                        let audio = &self.view.project.audio;
                        let midi = self.devices.midi.iter().find(|(id,_)| id == &audio.midi).map(|(_,n)| n.as_str()).unwrap_or("MIDI input unavailable / not selected");
                        ui.label(format!("MIDI: {midi} / selected track"));
                        ui.label(format!("Output: stereo {}–{}",audio.pair[0]+1,audio.pair[1]+1));
                    } else {
                        ui.label("Add an instrument track, choose a plugin, then play the selected track.");
                    }
                });
            }); });
        });
        if let Some(id) = choose {
            self.select_track(id);
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
                ui.add_space(8.0);
                if !self.view.actual.is_empty() {
                    ui.colored_label(colors::SECONDARY, &self.view.actual);
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
                    if format!("{} {}",plugin.name,plugin.format).to_lowercase().contains(&search) && ui.add_enabled(!self.busy && !self.view.quarantine.iter().any(|e| crate::quarantine::same(&e.plugin,plugin)),egui::Button::new(format!("{} · {}{}",plugin.name,plugin.format,if self.view.quarantine.iter().any(|e| crate::quarantine::same(&e.plugin,plugin)) {" · Quarantined"} else {""})).frame(false)).on_hover_text(plugin.path.display().to_string()).clicked() { picked = Some(plugin.clone()); }
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
            ui.label("Changes stop audio. Enable Audio explicitly after applying.");
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
                egui::ScrollArea::vertical().max_height(260.0).show(
                    ui,
                    |ui| match std::fs::read_dir(&picker.folder) {
                        Ok(entries) => {
                            let mut paths: Vec<_> = entries
                                .filter_map(|e| e.ok().map(|e| e.path()))
                                .filter(|p| {
                                    p.is_dir() || p.extension().is_some_and(|e| e == "clef")
                                })
                                .collect();
                            paths.sort_by_key(|p| {
                                (!p.is_dir(), p.file_name().map(|s| s.to_owned()))
                            });
                            for path in paths {
                                let folder = path.is_dir();
                                let name = path.file_name().unwrap_or_default().to_string_lossy();
                                if name.starts_with('.') {
                                    continue;
                                }
                                if ui
                                    .selectable_label(
                                        picker.name == name,
                                        format!("{name}{}", if folder { "/" } else { "" }),
                                    )
                                    .clicked()
                                {
                                    if folder {
                                        navigate = Some(path);
                                    } else {
                                        picker.name = name.into_owned();
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            ui.colored_label(colors::DANGER, e.to_string());
                        }
                    },
                );
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
            Confirm::Retained(as_new) => {
                self.retained_approved = self
                    .view
                    .tracks
                    .iter()
                    .filter(|t| t.lost)
                    .map(|t| (t.id, t.loss_version))
                    .collect();
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
                    if !binding.title.to_lowercase().contains(&self.command_search.to_lowercase()) { continue; }
                    ui.horizontal(|ui| {
                        if ui.add_enabled(self.enabled(binding.command),egui::Button::new(binding.title)).clicked() { selected = Some(binding.command); }
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
        self.shortcuts(ctx);
        let name = self
            .view
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into());
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "{}{} — Clef",
            name,
            if self.has_unsaved_changes() { " *" } else { "" }
        )));
        let os_quit = self.quit_gate.is_some() && unsafe { clef_take_quit_request() };
        if (os_quit || ctx.input(|i| i.viewport().close_requested())) && !self.exiting {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if !self.busy && self.confirm.is_none() {
                self.transition(Transition::Close, ctx);
            }
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        egui::Frame::new().inner_margin(12.0).show(ui, |ui| {
            self.main_surface(ui, &ctx);
        });
        self.plugin_picker(&ctx);
        self.settings(&ctx);
        self.command_window(&ctx);
        self.files(&ctx);
        self.quarantine_windows(&ctx);
        self.confirmation(&ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(!app.busy);
        app.resolve_confirmation(2, &context);
        assert!(app.view.dirty);
        assert!(app.confirm.is_none());
        assert!(!app.busy);
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
        while app.busy && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
            app.poll(&context);
        }
        assert!(!app.busy);
        assert!(!app.exiting);
        assert!(app.pending_after_save.is_none());
        assert!(app.view.dirty);
    }
    #[test]
    fn discard_close_exits_and_commands_respect_session_state() {
        let context = egui::Context::default();
        let mut app = App::new(context.clone(), PathBuf::new());
        assert!(!app.enabled(Command::Enable));
        assert!(!app.enabled(Command::Editor));
        app.view.dirty = true;
        app.transition(Transition::Close, &context);
        app.resolve_confirmation(1, &context);
        assert!(app.exiting);
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
            state: Vec::new(),
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
}
