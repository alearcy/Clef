use crate::{
    Action, App, Command, FileMode, FilePicker, Snapshot, Transition, primitives::project,
};
use clef_session::{RecentStatus, RecoveryEntry};
use eframe::egui::{self, Key, Modifiers};
use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Default)]
pub(super) struct Startup {
    pub selected: Option<PathBuf>,
    recovery: Option<String>,
    modal: bool,
    prompted: bool,
    confirm_discard: bool,
    scroll_recent: bool,
    restore_recovery_focus: bool,
}

impl Startup {
    pub(super) fn update(&mut self, view: &Snapshot) {
        if !view.history_ready || view.project_open {
            return;
        }
        if !view
            .recents
            .iter()
            .any(|entry| Some(&entry.path) == self.selected.as_ref())
        {
            self.selected = view.recents.first().map(|entry| entry.path.clone());
        }
        if self.selected_recovery(view).is_none() {
            self.recovery = view.recoveries.first().map(|entry| entry.id.clone());
            self.confirm_discard = false;
        }
        if !self.prompted {
            self.prompted = true;
            self.modal = self.recovery.is_some();
        }
        self.modal &= self.recovery.is_some();
    }

    fn selected_recovery<'a>(&self, view: &'a Snapshot) -> Option<&'a RecoveryEntry> {
        view.recoveries
            .iter()
            .find(|entry| Some(&entry.id) == self.recovery.as_ref())
    }

    pub(super) fn command_available(&self, command: Command, view: &Snapshot) -> bool {
        use Command::*;
        if self.modal {
            return match command {
                RecoveryLater | Close => true,
                RecoverProject => {
                    !self.confirm_discard
                        && self
                            .selected_recovery(view)
                            .is_some_and(|entry| entry.checkpoint.is_some())
                }
                DiscardRecovery => self.selected_recovery(view).is_some(),
                _ => false,
            };
        }
        let recent = view
            .recents
            .iter()
            .find(|entry| Some(&entry.path) == self.selected.as_ref());
        match command {
            Close => true,
            New | Open | Commands => view.history_ready,
            ShowRecovery => !view.recoveries.is_empty(),
            OpenRecent => recent.is_some_and(|entry| entry.status.can_open()),
            LocateRecent => recent.is_some_and(|entry| entry.status == RecentStatus::Missing),
            RemoveRecent => recent.is_some(),
            _ => false,
        }
    }
}

fn name(path: Option<&Path>) -> String {
    path.and_then(Path::file_stem).map_or_else(
        || "Untitled".into(),
        |stem| stem.to_string_lossy().into_owned(),
    )
}

fn date(time: SystemTime) -> String {
    jiff::Timestamp::try_from(time).map_or_else(
        |_| "Unknown time".into(),
        |stamp| {
            stamp
                .to_zoned(jiff::tz::TimeZone::UTC)
                .strftime("%Y-%m-%d %H:%M UTC")
                .to_string()
        },
    )
}

fn status(status: &RecentStatus) -> &str {
    match status {
        RecentStatus::Ready => "",
        RecentStatus::Missing => "Not found",
        RecentStatus::ReadOnly => "Read-only. Use Save As to save changes.",
        RecentStatus::Unavailable(error) => error,
    }
}

impl App {
    pub(super) fn dispatch_startup(&mut self, command: Command, ctx: &egui::Context) {
        match command {
            Command::OpenRecent => {
                if let Some(path) = self.startup.selected.clone() {
                    self.transition(Transition::Open(path), ctx);
                }
            }
            Command::LocateRecent => {
                self.file = Some(FilePicker::new(
                    FileMode::Locate,
                    self.startup.selected.as_ref(),
                ));
            }
            Command::RemoveRecent => {
                if let Some(path) = self.startup.selected.clone() {
                    self.send(Action::RemoveRecent(path));
                }
            }
            Command::ShowRecovery => self.startup.modal = true,
            Command::RecoverProject => {
                if let Some(id) = self.startup.recovery.clone() {
                    self.send(Action::Recover(id));
                }
            }
            Command::DiscardRecovery => {
                if self.startup.confirm_discard {
                    if let Some(id) = self.startup.recovery.clone() {
                        self.send(Action::DiscardRecovery(id));
                    }
                } else {
                    self.startup.confirm_discard = true;
                }
            }
            Command::RecoveryLater => {
                if self.startup.confirm_discard {
                    self.startup.confirm_discard = false;
                } else {
                    self.startup.modal = false;
                    self.startup.restore_recovery_focus = true;
                }
            }
            _ => unreachable!(),
        }
    }

    pub(super) fn startup_surface(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut action = None;
        let chooser_enabled = !self.startup.modal
            && self.file.is_none()
            && self.confirm.is_none()
            && self.busy_since.is_none();
        if chooser_enabled {
            self.recent_keys(ctx);
        }
        project::panel(ui, |ui| {
            project::heading(ui, "Clef");
            project::caption(ui, "Recent Projects");
            if !self.view.history_ready {
                project::body(ui, "Loading project history…");
                return;
            }
            if self.view.recents.is_empty() {
                project::body(ui, "No recent projects.");
            } else if ui.available_width() >= 690.0 {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(380.0);
                        if self.recent_rows(ui) {
                            action = Some(Command::OpenRecent);
                        }
                    });
                    ui.vertical(|ui| self.recent_details(ui));
                });
            } else {
                if self.recent_rows(ui) {
                    action = Some(Command::OpenRecent);
                }
                self.recent_details(ui);
            }
            ui.horizontal_wrapped(|ui| {
                for (command, label, primary, visible) in [
                    (
                        Command::ShowRecovery,
                        "Recovery…",
                        false,
                        !self.view.recoveries.is_empty(),
                    ),
                    (
                        Command::RemoveRecent,
                        "Remove",
                        false,
                        self.startup.selected.is_some(),
                    ),
                    (
                        Command::LocateRecent,
                        "Locate…",
                        false,
                        self.startup
                            .command_available(Command::LocateRecent, &self.view),
                    ),
                    (
                        Command::OpenRecent,
                        "Open",
                        self.enabled(Command::OpenRecent),
                        self.startup.selected.is_some(),
                    ),
                    (Command::Open, "Open…", false, true),
                    (
                        Command::New,
                        "New Project",
                        self.startup.selected.is_none(),
                        true,
                    ),
                ] {
                    if visible {
                        let response = project::button(ui, label, self.enabled(command), primary);
                        if command == Command::ShowRecovery && self.startup.restore_recovery_focus {
                            self.startup.restore_recovery_focus = false;
                            response.request_focus();
                        }
                        if response.clicked() {
                            action = Some(command);
                        }
                    }
                }
            });
            if self.busy_visible {
                project::caption(ui, "Working…");
            }
            if !self.view.history_message.is_empty() {
                project::body(ui, &self.view.history_message);
            }
            if !self.view.message.is_empty() && !self.startup.modal {
                project::caption(ui, &self.view.message);
            }
        });
        if self.startup.modal {
            self.recovery_modal(ctx);
        } else if chooser_enabled {
            if !ctx.egui_wants_keyboard_input()
                && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
            {
                action = Some(Command::OpenRecent);
            }
            if let Some(command) = action {
                self.dispatch(command, ctx);
            }
        }
    }

    fn recent_rows(&mut self, ui: &mut egui::Ui) -> bool {
        let mut open = false;
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                for entry in &self.view.recents {
                    let path = entry.path.display().to_string();
                    let detail = if entry.status.can_open() {
                        path.as_str()
                    } else {
                        status(&entry.status)
                    };
                    let accessible = if detail == path {
                        path.clone()
                    } else {
                        format!("{path}, {detail}")
                    };
                    let response = project::row(
                        ui,
                        &name(Some(&entry.path)),
                        detail,
                        &accessible,
                        Some(&entry.path) == self.startup.selected.as_ref(),
                        entry.status.can_open(),
                    );
                    if self.startup.scroll_recent
                        && Some(&entry.path) == self.startup.selected.as_ref()
                    {
                        self.startup.scroll_recent = false;
                        response.scroll_to_me(None);
                    }
                    if (response.clicked() || (self.keyboard_mode && response.gained_focus()))
                        && !self.startup.modal
                        && self.file.is_none()
                    {
                        self.startup.selected = Some(entry.path.clone());
                        open = response.double_clicked()
                            || (response.clicked()
                                && !response.clicked_by(egui::PointerButton::Primary));
                    }
                }
            });
        open
    }

    fn recent_details(&self, ui: &mut egui::Ui) {
        if let Some(entry) = self
            .view
            .recents
            .iter()
            .find(|entry| Some(&entry.path) == self.startup.selected.as_ref())
        {
            project::body(ui, &name(Some(&entry.path)));
            project::caption(ui, &entry.path.display().to_string());
            project::caption(ui, &format!("Last used {}", date(entry.last_used)));
            if !status(&entry.status).is_empty() {
                project::body(ui, status(&entry.status));
            }
        }
    }

    fn recent_keys(&mut self, ctx: &egui::Context) {
        let step = ctx.input_mut(|input| {
            if input.consume_key(Modifiers::NONE, Key::ArrowDown) {
                Some(1)
            } else if input.consume_key(Modifiers::NONE, Key::ArrowUp) {
                Some(-1)
            } else {
                None
            }
        });
        if let Some(step) = step
            && let Some(index) = self
                .view
                .recents
                .iter()
                .position(|entry| Some(&entry.path) == self.startup.selected.as_ref())
        {
            let next = index
                .saturating_add_signed(step)
                .min(self.view.recents.len() - 1);
            self.startup.selected = Some(self.view.recents[next].path.clone());
            self.startup.scroll_recent = true;
        }
    }

    fn recovery_modal(&mut self, ctx: &egui::Context) {
        let mut action = None;
        let dismissed = project::modal(ctx, |ui| {
            if self.startup.confirm_discard {
                project::heading(ui, "Discard this recovery?");
                if let Some(entry) = self.startup.selected_recovery(&self.view) {
                    project::body(ui, &name(entry.original.as_deref()));
                    if let Some(path) = &entry.original {
                        project::caption(ui, &path.display().to_string());
                    }
                }
                project::body(ui, "Clef will remove the autosave copies for this session.");
            } else {
                project::heading(ui, "Recover Unsaved Work?");
                if self.view.recoveries.len() > 1 {
                    egui::ScrollArea::vertical()
                        .max_height(180.0)
                        .show(ui, |ui| {
                            for entry in &self.view.recoveries {
                                let selected = Some(&entry.id) == self.startup.recovery.as_ref();
                                let time = entry
                                    .checkpoint
                                    .map_or_else(|| "Checkpoint unavailable".into(), date);
                                if project::row(
                                    ui,
                                    &name(entry.original.as_deref()),
                                    &time,
                                    &time,
                                    selected,
                                    entry.checkpoint.is_some(),
                                )
                                .clicked()
                                {
                                    self.startup.recovery = Some(entry.id.clone());
                                }
                            }
                        });
                }
                if let Some(entry) = self.startup.selected_recovery(&self.view) {
                    project::body(ui, &name(entry.original.as_deref()));
                    if let Some(time) = entry.checkpoint {
                        project::caption(ui, &format!("Checkpoint {}", date(time)));
                        project::body(
                            ui,
                            "Recover opens an unsaved copy. Use Save As to keep it. Plugin changes since the last captured state and an unfinished take may be missing.",
                        );
                    }
                    if let Some(path) = &entry.original {
                        project::caption(ui, &path.display().to_string());
                    }
                    if let Some(error) = &entry.error {
                        project::body(ui, error);
                    }
                }
            }
            if !self.view.message.is_empty() {
                project::caption(ui, &self.view.message);
            }
            ui.horizontal_wrapped(|ui| {
                let confirming = self.startup.confirm_discard;
                for (command, label, primary) in [
                    (
                        Command::RecoveryLater,
                        if confirming { "Cancel" } else { "Later" },
                        false,
                    ),
                    (
                        Command::DiscardRecovery,
                        if confirming {
                            "Discard Recovery"
                        } else {
                            "Discard…"
                        },
                        false,
                    ),
                    (Command::RecoverProject, "Recover", true),
                ] {
                    if (!confirming || command != Command::RecoverProject)
                        && project::button(ui, label, self.enabled(command), primary).clicked()
                    {
                        action = Some(command);
                    }
                }
            });
        });
        if let Some(command) = action.or(dismissed.then_some(Command::RecoveryLater)) {
            self.dispatch(command, ctx);
        }
    }
}
