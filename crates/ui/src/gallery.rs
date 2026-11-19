//! Snapshot registry for the startup, recovery and edit history surfaces.
use crate::{App, Snapshot, startup::Startup};
use clef_model::{
    Result,
    midi::{ClipEdit, Event, Message, Mode, Snap},
};
use clef_session::{HistoryView, RecentProject, RecentStatus, RecoveryEntry};
use eframe::{egui, icon_data::IconDataExt};
use std::{
    path::PathBuf,
    time::{Duration, UNIX_EPOCH},
};

const CASES: &[&str] = &[
    "startup-loading",
    "startup-empty",
    "startup-recent",
    "startup-missing",
    "startup-incompatible",
    "startup-read-only",
    "recovery-ready",
    "recovery-fallback",
    "recovery-unavailable",
    "recovery-discard",
    "recovery-later",
];
const HISTORY_CASES: &[&str] = &["history-empty", "history-ready", "history-locked"];
const ARRANGEMENT_CASES: &[&str] = &[
    "arrangement-selected",
    "arrangement-overlap",
    "arrangement-repeat",
    "arrangement-snap-preview",
    "arrangement-free-preview",
    "arrangement-zoomed",
    "arrangement-menu",
    "arrangement-split-confirm",
    "arrangement-refused",
];
const ROLL_CASES: &[&str] = &[
    "dock",
    "single",
    "multiple",
    "selection",
    "snap-preview",
    "velocity",
    "linear-cc",
    "curved-cc",
    "switch-cc",
    "empty",
    "unavailable",
];

fn piano_roll_fixture(app: &mut App, case: &str) {
    use clef_model::midi::EventChange;
    arrangement_fixture(
        app,
        if matches!(case, "dock" | "multiple") {
            "arrangement-selected"
        } else {
            ""
        },
    );
    app.piano_roll = Default::default();
    app.piano_roll_open = true;
    let model = std::sync::Arc::make_mut(&mut app.view.project.midi);
    let id = model.clips(1)[0].id();
    let cc = if case == "switch-cc" { 64 } else { 11 };
    let mut changes = Vec::new();
    for (index, value) in [12, 100, 55, 124].into_iter().enumerate() {
        let bytes = [0xb0, cc, value];
        let message = if cc == 64 {
            Message::Control(bytes)
        } else {
            Message::CurvedControl {
                bytes,
                bend: if case == "curved-cc" { 66 } else { 0 },
            }
        };
        changes.push(EventChange::Insert(Event {
            tick: index as u64 * 720,
            order: 0,
            message,
        }));
    }
    if case == "empty" {
        changes = model.clips(1)[0]
            .roll_events([0, 2_880])
            .into_iter()
            .map(|event| EventChange::Remove(event.id))
            .collect();
    }
    model
        .edit(
            &ClipEdit::EditEvents {
                clip: id,
                label: "Edit MIDI Events",
                changes,
            },
            [1, 2].into_iter(),
        )
        .unwrap();
    app.view.edit_history.available = case != "unavailable";
    app.piano_roll.gallery_state(case, &model.clips(1)[0]);
}

fn arrangement_fixture(app: &mut App, case: &str) {
    let mut view = history_fixture("history-empty");
    view.selected = Some(1);
    view.project.tracks.push(clef_model::project::Track {
        id: 2,
        name: "Piano".into(),
        plugin: None,
        state: Vec::new(),
    });
    view.project.next_id = 3;
    view.transport.audible_tick = 1_200;
    let arrangement = std::sync::Arc::make_mut(&mut view.project.midi);
    let notes = |start, pitches: &[u8]| {
        pitches
            .iter()
            .enumerate()
            .map(|(i, &key)| Event {
                tick: start + i as u64 * 360,
                order: i as u32,
                message: Message::Note {
                    channel: 0,
                    key,
                    velocity: 96,
                    end: start + i as u64 * 360 + 480,
                    release: 0,
                    release_order: 12 + i as u32,
                },
            })
            .collect::<Vec<_>>()
    };
    arrangement
        .record(
            1,
            Mode::Merge,
            [0, 2_880],
            vec![notes(0, &[55, 57, 62, 60, 65, 62, 57])],
            false,
        )
        .unwrap();
    arrangement
        .record(
            2,
            Mode::Merge,
            [960, 3_840],
            vec![notes(960, &[60, 64, 67, 64, 69, 64, 60])],
            false,
        )
        .unwrap();
    let lower = arrangement.clips(1)[0].id();
    let upper = arrangement.clips(2)[0].id();
    app.arrangement_ui = Default::default();
    if matches!(case, "arrangement-overlap" | "arrangement-menu") {
        arrangement
            .edit(
                &ClipEdit::Move {
                    destinations: vec![(upper, 1)],
                    delta: 0,
                },
                [1, 2].into_iter(),
            )
            .unwrap();
        app.arrangement_ui.selected.insert(upper);
    } else {
        app.arrangement_ui.selected.insert(lower);
    }
    if case == "arrangement-selected" {
        app.arrangement_ui.selected.insert(upper);
    }
    if case == "arrangement-repeat" {
        arrangement
            .edit(
                &ClipEdit::Resize {
                    clip: lower,
                    end: 5_760,
                    repeat: true,
                },
                [1, 2].into_iter(),
            )
            .unwrap();
    }
    if matches!(
        case,
        "arrangement-snap-preview" | "arrangement-free-preview"
    ) {
        view.project.arrangement_view.snap = if case == "arrangement-free-preview" {
            Snap::Off
        } else {
            Snap::Eighth
        };
        app.arrangement_ui.pending = Some(ClipEdit::Resize {
            clip: lower,
            end: if case == "arrangement-free-preview" {
                4_560
            } else {
                4_320
            },
            repeat: false,
        });
    }
    if case == "arrangement-zoomed" {
        view.project.arrangement_view.offset = 900;
        view.project.arrangement_view.zoom_milli = 150_000;
    }
    if case == "arrangement-menu" {
        app.arrangement_ui.gallery_menu = Some(1);
    }
    app.confirm = (case == "arrangement-split-confirm").then(|| {
        crate::Confirm::SplitClips(
            view.project.midi.clone(),
            ClipEdit::Split {
                clips: vec![lower],
                tick: 1_200,
                confirmed: true,
            },
        )
    });
    app.arrangement_ui.settings = view.project.arrangement_view;
    app.status.clear();
    app.rename = "Strings".into();
    if case == "arrangement-refused" {
        let reason = std::sync::Arc::make_mut(&mut view.project.midi)
            .edit(
                &ClipEdit::Move {
                    destinations: vec![(lower, 1)],
                    delta: -960,
                },
                [1, 2].into_iter(),
            )
            .unwrap_err();
        view.message = reason.to_string();
    }
    app.view = view;
}

fn history_fixture(case: &str) -> Snapshot {
    let mut project = clef_model::project::Project::default();
    project.tracks.push(clef_model::project::Track {
        id: 1,
        name: "Strings".into(),
        plugin: None,
        state: Vec::new(),
    });
    project.next_id = 2;
    Snapshot {
        project,
        project_open: true,
        edit_history: if case == "history-empty" {
            HistoryView {
                available: true,
                ..Default::default()
            }
        } else {
            HistoryView {
                labels: vec![
                    "Add Track",
                    "Rename Track",
                    "Edit Loop Range",
                    "Record MIDI",
                ],
                cursor: 2,
                version: 4,
                available: case != "history-locked",
            }
        },
        ..Default::default()
    }
}

fn fixture(case: &str) -> Snapshot {
    let mut view = Snapshot {
        history_ready: case != "startup-loading",
        ..Default::default()
    };
    let time = UNIX_EPOCH + Duration::from_secs(1_795_081_600);
    let original = PathBuf::from("/Projects/Autumn/Evening Sketch No. 4.clef");
    if !matches!(case, "startup-loading" | "startup-empty") {
        for (path, status) in [
            (
                original.clone(),
                match case {
                    "startup-missing" => RecentStatus::Missing,
                    "startup-incompatible" => {
                        RecentStatus::Unavailable("Unsupported Clef project version.".into())
                    }
                    "startup-read-only" => RecentStatus::ReadOnly,
                    _ => RecentStatus::Ready,
                },
            ),
            (
                PathBuf::from(
                    "/Projects/Autumn/Revisions/Concerto for Cello and Chamber Orchestra (Piano Reduction).clef",
                ),
                RecentStatus::Ready,
            ),
        ] {
            view.recents.push(RecentProject {
                path,
                status,
                last_used: time,
            });
        }
    }
    if case.starts_with("recovery-") {
        view.recoveries.push(RecoveryEntry {
            id: "1-1".into(),
            original: Some(original),
            checkpoint: (case != "recovery-unavailable").then_some(time),
            error: matches!(case, "recovery-unavailable" | "recovery-fallback")
                .then(|| "Latest checkpoint cannot be read: unsupported project version.".into()),
        });
        if case == "recovery-ready" {
            view.recoveries.push(RecoveryEntry {
                id: "2-1".into(),
                original: None,
                checkpoint: Some(time - Duration::from_secs(180)),
                error: None,
            });
        }
    }
    view
}

struct Gallery {
    app: App,
    output: PathBuf,
    width: u32,
    cases: &'static [&'static str],
    prefix: &'static str,
    case: usize,
    frames: usize,
    result: std::sync::Arc<std::sync::Mutex<Option<Result<()>>>>,
}
impl Gallery {
    fn install(&mut self, ctx: &egui::Context) {
        let case = self.cases[self.case];
        if self.prefix == "t10" {
            piano_roll_fixture(&mut self.app, case);
            self.frames = 0;
            return;
        }
        if self.prefix == "t09" {
            egui::Popup::close_all(ctx);
            arrangement_fixture(&mut self.app, case);
            self.frames = 0;
            return;
        }
        if self.prefix == "t08" {
            self.app.view = history_fixture(case);
            self.app.history_window.open = true;
            self.frames = 0;
            return;
        }
        self.app.view = fixture(case);
        self.app.startup = Startup::default();
        self.app.startup.update(&self.app.view);
        match case {
            "recovery-discard" => self
                .app
                .dispatch_startup(crate::Command::DiscardRecovery, ctx),
            "recovery-later" => self
                .app
                .dispatch_startup(crate::Command::RecoveryLater, ctx),
            _ => {}
        }
        self.frames = 0;
    }
}

impl eframe::App for Gallery {
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        self.app.clear_color(visuals)
    }
    fn logic(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if self.case == self.cases.len() {
            return;
        }
        let screenshot = ctx.input(|i| {
            i.events.iter().find_map(|event| {
                if let egui::Event::Screenshot { image, .. } = event {
                    Some(image.clone())
                } else {
                    None
                }
            })
        });
        if let Some(image) = screenshot {
            let icon = egui::IconData {
                rgba: image
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_srgba_unmultiplied())
                    .collect(),
                width: image.size[0] as u32,
                height: image.size[1] as u32,
            };
            let result = icon.to_png_bytes().map_err(Into::into).and_then(|bytes| {
                std::fs::write(
                    self.output.join(format!(
                        "{}-{}-{}.png",
                        self.prefix, self.cases[self.case], self.width
                    )),
                    bytes,
                )?;
                Ok(())
            });
            if result.is_err() || self.case + 1 == self.cases.len() {
                *self.result.lock().unwrap() = Some(result);
                self.case = self.cases.len();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                self.case += 1;
                self.install(ctx);
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        if self.case == self.cases.len() {
            return;
        }
        let ctx = ui.ctx().clone();
        if self.prefix == "t10" && self.cases[self.case] != "dock" {
            egui::Frame::new().inner_margin(12.0).show(ui, |ui| {
                let height = ui.available_height() - 100.0;
                let app = &mut self.app;
                app.piano_roll.show(
                    ui,
                    height,
                    &app.view,
                    &app.arrangement_ui.selected,
                    app.view.edit_history.available,
                    &mut app.piano_roll_open,
                );
            });
        } else if matches!(self.prefix, "t08" | "t09" | "t10") {
            egui::Frame::new()
                .inner_margin(12.0)
                .show(ui, |ui| self.app.main_surface(ui, &ctx));
            if self.prefix == "t08" {
                self.app.history_window.show(
                    &ctx,
                    &self.app.view.edit_history,
                    self.app.view.edit_history.available,
                );
            } else {
                self.app.confirmation(&ctx);
            }
        } else {
            self.app.startup_surface(ui, &ctx);
        }
        self.frames += 1;
        if self.frames == 4 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        ctx.request_repaint_after(Duration::from_millis(40));
    }
}

pub fn render_startup(output: PathBuf, width: u32) -> Result<()> {
    render(output, width, "t07", CASES)
}

pub fn render_history(output: PathBuf, width: u32) -> Result<()> {
    render(output, width, "t08", HISTORY_CASES)
}

pub fn render_arrangement(output: PathBuf, width: u32) -> Result<()> {
    render(output, width, "t09", ARRANGEMENT_CASES)
}

pub fn render_piano_roll(output: PathBuf, width: u32) -> Result<()> {
    render(output, width, "t10", ROLL_CASES)
}

fn render(
    output: PathBuf,
    width: u32,
    prefix: &'static str,
    cases: &'static [&'static str],
) -> Result<()> {
    if !matches!(width, 1024 | 760) {
        return Err("gallery width must be 1024 or 760 points".into());
    }
    std::fs::create_dir_all(&output)?;
    let result = std::sync::Arc::new(std::sync::Mutex::new(None));
    let completion = result.clone();
    eframe::run_native(
        "Clef Gallery",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([width as f32, 640.0]),
            ..Default::default()
        },
        Box::new(move |cc| {
            let mut gallery = Gallery {
                app: App::new(cc.egui_ctx.clone(), PathBuf::new()),
                output,
                width,
                prefix,
                cases,
                frames: 0,
                case: 0,
                result: completion,
            };
            gallery.install(&cc.egui_ctx);
            Ok(Box::new(gallery))
        }),
    )
    .map_err(|error| error.to_string())?;
    result
        .lock()
        .unwrap()
        .take()
        .ok_or("gallery closed before all screenshots")?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_accessible_controls_fit_both_review_widths() {
        for width in [760.0, 1024.0] {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let mut app = App::new(ctx.clone(), PathBuf::new());
            for (case, labels) in [
                (
                    "startup-missing",
                    &["Not found", "Locate…", "Open", "New Project"][..],
                ),
                (
                    "recovery-ready",
                    &["Recover Unsaved Work?", "Later", "Discard…", "Recover"][..],
                ),
            ] {
                app.view = fixture(case);
                app.startup = Startup::default();
                app.startup.update(&app.view);
                let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 640.0));
                let render = || egui::RawInput {
                    screen_rect: Some(bounds),
                    ..Default::default()
                };
                let _ = ctx.run_ui(render(), |ui| app.startup_surface(ui, &ctx));
                let frame = ctx.run_ui(render(), |ui| app.startup_surface(ui, &ctx));
                let tree = frame.platform_output.accesskit_update.unwrap();
                for label in labels {
                    let node = tree
                        .nodes
                        .iter()
                        .map(|(_, n)| n)
                        .find(|node| {
                            node.label()
                                .or_else(|| node.value())
                                .is_some_and(|name| name == *label)
                        })
                        .unwrap_or_else(|| {
                            panic!("missing accessible control {label} at {width}: {case}")
                        });
                    let rect = node.bounds().unwrap();
                    assert!(
                        rect.x0 >= 0.0
                            && rect.y0 >= 0.0
                            && rect.x1 <= width as f64
                            && rect.y1 <= 640.0,
                        "{label} overflows {width}"
                    );
                    if *label == "Open" {
                        assert!(node.is_disabled());
                    }
                }
            }
        }
    }

    #[test]
    fn later_preserves_recovery_and_recovered_and_read_only_projects_offer_save_as() {
        let ctx = egui::Context::default();
        let mut app = App::new(ctx.clone(), PathBuf::new());
        app.view = fixture("recovery-ready");
        app.startup.update(&app.view);
        assert!(app.enabled(crate::Command::RecoverProject));
        app.dispatch_startup(crate::Command::RecoveryLater, &ctx);
        assert!(app.enabled(crate::Command::ShowRecovery));
        assert_eq!(app.view.recoveries.len(), 2);
        let mut relaunched = Startup::default();
        relaunched.update(&app.view);
        assert!(relaunched.command_available(crate::Command::RecoverProject, &app.view));

        let original = app.view.recoveries[0].original.clone().unwrap();
        app.view.project_open = true;
        app.view.recovered_from = Some(original.clone());
        app.save(false);
        let picker = app.file.take().unwrap();
        assert!(matches!(picker.mode, crate::FileMode::Save));
        assert_eq!(picker.name, "Evening Sketch No. 4-recovered.clef");

        app.view.recovered_from = None;
        app.view.path = Some(original);
        app.view.read_only = true;
        app.save(false);
        let picker = app.file.take().unwrap();
        assert!(matches!(picker.mode, crate::FileMode::Save));
        assert_eq!(picker.name, "Evening Sketch No. 4-copy.clef");
    }

    #[test]
    fn chooser_keyboard_selects_and_opens_the_selected_recent_project() {
        let ctx = egui::Context::default();
        let mut app = App::new(ctx.clone(), PathBuf::new());
        app.view = fixture("startup-missing");
        app.startup.update(&app.view);
        assert!(!app.enabled(crate::Command::OpenRecent));
        for key in [egui::Key::ArrowDown, egui::Key::Enter] {
            let _ = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(760.0, 640.0),
                    )),
                    events: vec![egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |ui| app.startup_surface(ui, &ctx),
            );
        }
        assert_eq!(
            app.startup.selected.as_ref(),
            Some(&app.view.recents[1].path)
        );
        assert_eq!(app.actions_in_flight, 1);
    }
}
