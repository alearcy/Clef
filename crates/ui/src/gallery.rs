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
const MIXER_CASES: &[&str] = &[
    "empty",
    "stopped",
    "playing",
    "long-bank",
    "header",
    "header-long",
    "header-dock",
];
const ROUTING_CASES: &[&str] = &["buses", "solo", "long-sends", "empty"];
const INSERT_CASES: &[&str] = &[
    "empty",
    "populated",
    "insertion",
    "bypassed",
    "missing",
    "failed",
    "late",
    "long-chain",
    "vi-bypassed",
    "vi-offline",
    "vi-empty",
];
const TRANSPORT_CASES: &[&str] = &["stopped", "looping", "count-in", "recording"];
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
                take: None,
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
        instrument_bypassed: false,
        instrument_offline: false,
        record_mode: Default::default(),
        kind: clef_model::project::TrackKind::Instrument,
        output: None,
        sends: Vec::new(),
        mix: Default::default(),
        inserts: Vec::new(),
        take_lanes: Default::default(),
        state: Default::default(),
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

fn mixer_fixture(app: &mut App, case: &str) {
    use clef_session::Meter;
    arrangement_fixture(app, "arrangement-selected");
    app.mixer_ui.open = !case.starts_with("header");
    app.piano_roll_open = case == "header-dock";
    let view = &mut app.view;
    view.active = !matches!(case, "empty" | "stopped" | "header-empty");
    if matches!(case, "empty" | "header-empty") {
        view.project.tracks.clear();
        view.project.midi = Default::default();
        view.selected = None;
    }
    if view.active {
        view.transport.phase = clef_session::TransportPhase::Playing;
        view.project.tracks[0].mix.solo = true;
        view.project.tracks[0].mix.trim = 25;
        view.project.tracks[0].mix.level.pan = -23;
        view.project.tracks[0].mix.width = 142;
        view.project.tracks[1].mix.mute = true;
        view.track_meters.insert(
            1,
            Meter {
                level: [-35, -97],
                hold: [12, -72],
                clipped: true,
            },
        );
        view.master_meter = Meter {
            level: [-9, -60],
            hold: [-4, -50],
            clipped: false,
        };
    }
    if let Some(track) = view.project.tracks.get_mut(0) {
        track
            .plugin
            .get_or_insert_with(|| gallery_plugin("Glass Strings", "VST3"));
    }
    if let Some(track) = view.project.tracks.get_mut(1) {
        track
            .plugin
            .get_or_insert_with(|| gallery_plugin("Zebra2", "VST3"));
    }
    if matches!(case, "long-bank" | "header-long" | "header-dock") {
        let template = view.project.tracks[1].clone();
        for id in 3..=16 {
            let mut track = template.clone();
            track.id = id;
            track.name = format!("Very long instrument track name {id}");
            view.project.tracks.push(track);
        }
        view.project.next_id = 17;
    }
}

fn gallery_plugin(name: &str, format: &str) -> clef_model::Plugin {
    clef_model::Plugin {
        path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../vendor/rust-vst3-host/test_plugins/TestSynth.vst3"),
        id: format!("fixture-{name}"),
        name: name.into(),
        format: format.into(),
    }
}

fn insert_fixture(app: &mut App, case: &str) {
    use clef_model::{Plugin, project::Insert};
    use clef_session::InsertView;
    mixer_fixture(app, "stopped");
    app.mixer_ui.gallery_insert_focus = None;
    if case == "empty" {
        return;
    }
    let mut make = |name: &str, format: &str| {
        let id = app.view.project.next_id;
        app.view.project.next_id += 1;
        Insert {
            id,
            plugin: Plugin {
                path: PathBuf::from(format!("/Fixtures/{name}.{}", format.to_lowercase())),
                id: format!("fixture-{id}"),
                name: name.into(),
                format: format.into(),
            },
            bypassed: false,
            state: Default::default(),
        }
    };
    let mut eq = make("Tape EQ", "VST3");
    let delay = make("Stereo Delay", "CLAP");
    let master = make("Limiter", "VST3");
    let long: Vec<_> = if case == "long-chain" {
        (1..=9)
            .map(|i| make(&format!("Long Insert Effect Name {i}"), "CLAP"))
            .collect()
    } else {
        Vec::new()
    };
    match case {
        "vi-bypassed" => app.view.project.tracks[0].instrument_bypassed = true,
        "vi-offline" => app.view.project.tracks[0].instrument_offline = true,
        "vi-empty" => {
            app.view.project.tracks[0].plugin = None;
            app.view.project.tracks[0].inserts.clear();
            app.view.project.master_inserts.clear();
            return;
        }
        "bypassed" => eq.bypassed = true,
        "missing" => {
            app.view.inserts.insert(
                eq.id,
                InsertView {
                    missing: true,
                    ..Default::default()
                },
            );
        }
        "failed" => {
            app.view.inserts.insert(
                eq.id,
                InsertView {
                    error: "Worker exited".into(),
                    ..Default::default()
                },
            );
        }
        "late" => {
            app.view.inserts.insert(
                eq.id,
                InsertView {
                    late: Some(25),
                    ..Default::default()
                },
            );
        }
        "insertion" => app.mixer_ui.gallery_insert_focus = Some(delay.id),
        _ => {}
    }
    app.view.project.tracks[0].inserts = vec![eq, delay];
    app.view.project.tracks[0].inserts.extend(long);
    app.view.project.master_inserts.push(master);
}

fn routing_fixture(app: &mut App, case: &str) {
    use clef_model::project::{Send, Track, TrackKind};
    use clef_session::Meter;
    mixer_fixture(app, "playing");
    app.mixer_ui.open = true;
    for track in &mut app.view.project.tracks {
        track.mix.solo = false;
        track.mix.mute = false;
    }
    app.view.project.tracks[0].output = Some(3);
    app.view.project.tracks[1].output = Some(4);
    let bus = |id, name: &str| Track {
        id,
        name: name.into(),
        plugin: None,
        instrument_bypassed: false,
        instrument_offline: false,
        record_mode: Default::default(),
        kind: TrackKind::Bus,
        output: None,
        sends: Vec::new(),
        mix: Default::default(),
        inserts: Vec::new(),
        take_lanes: Default::default(),
        state: Default::default(),
    };
    app.view.project.tracks.push(bus(3, "Reverb"));
    app.view.project.tracks.push(bus(4, "Drum Bus"));
    app.view.project.next_id = 7;
    app.view.project.tracks[0].sends = vec![Send {
        id: 5,
        target: 4,
        level: -60,
        pre: true,
        enabled: true,
    }];
    app.view.project.tracks[1].sends = vec![Send {
        id: 6,
        target: 3,
        level: -30,
        pre: false,
        enabled: true,
    }];
    app.view.track_meters.insert(
        3,
        Meter {
            level: [-18, -21],
            hold: [-12, -16],
            clipped: false,
        },
    );
    app.view.track_meters.insert(
        4,
        Meter {
            level: [-24, -26],
            hold: [-18, -21],
            clipped: false,
        },
    );
    if case == "long-sends" {
        let mut id = 7;
        for target in [3, 4] {
            app.view.project.tracks[0].sends.push(Send {
                id,
                target,
                level: -40,
                pre: id % 2 == 0,
                enabled: true,
            });
            id += 1;
        }
        app.view.project.next_id = id;
    }
    if case == "solo" {
        app.view.project.tracks[3].mix.solo = true;
    }
    if case == "empty" {
        app.view.project.tracks[0].output = None;
        app.view.project.tracks[1].output = None;
        app.view.project.tracks[0].sends.clear();
        app.view.project.tracks[1].sends.clear();
    }
}

fn history_fixture(case: &str) -> Snapshot {
    let mut project = clef_model::project::Project::default();
    project.tracks.push(clef_model::project::Track {
        id: 1,
        name: "Strings".into(),
        plugin: None,
        instrument_bypassed: false,
        instrument_offline: false,
        record_mode: Default::default(),
        kind: clef_model::project::TrackKind::Instrument,
        output: None,
        sends: Vec::new(),
        mix: Default::default(),
        inserts: Vec::new(),
        take_lanes: Default::default(),
        state: Default::default(),
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
        if self.prefix == "t34" {
            arrangement_fixture(&mut self.app, "");
            let view = &mut self.app.view;
            view.active = true;
            view.armed = Some(1);
            view.record_unavailable = None;
            view.recording = matches!(case, "count-in" | "recording");
            view.transport.phase = match case {
                "count-in" => clef_session::TransportPhase::CountingIn,
                "recording" => clef_session::TransportPhase::Playing,
                _ => clef_session::TransportPhase::Stopped,
            };
            view.recording_region = (case == "recording").then_some([0, 1_920]);
            view.project.timeline.looping = case != "stopped";
            view.project.timeline.loop_range = [0, 1_920];
            view.project.timeline.click = case != "stopped";
            self.frames = 0;
            return;
        }
        if self.prefix == "t11" {
            mixer_fixture(&mut self.app, case);
            self.frames = 0;
            return;
        }
        if self.prefix == "t12" {
            insert_fixture(&mut self.app, case);
            self.frames = 0;
            return;
        }
        if self.prefix == "t15" {
            routing_fixture(&mut self.app, case);
            self.frames = 0;
            return;
        }
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
        } else if matches!(
            self.prefix,
            "t08" | "t09" | "t10" | "t11" | "t12" | "t15" | "t34"
        ) {
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

pub fn render_mixer(output: PathBuf, width: u32) -> Result<()> {
    render(output, width, "t11", MIXER_CASES)
}

pub fn render_inserts(output: PathBuf, width: u32) -> Result<()> {
    render(output, width, "t12", INSERT_CASES)
}

pub fn render_routing(output: PathBuf, width: u32) -> Result<()> {
    render(output, width, "t15", ROUTING_CASES)
}

pub fn render_transport(output: PathBuf, width: u32) -> Result<()> {
    render(output, width, "t34", TRANSPORT_CASES)
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
    let height = if prefix == "t15" {
        900.0
    } else if matches!(prefix, "t11" | "t12") {
        768.0
    } else {
        640.0
    };
    let result = std::sync::Arc::new(std::sync::Mutex::new(None));
    let completion = result.clone();
    eframe::run_native(
        "Clef Gallery",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([width as f32, height]),
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
    fn pointer_hover_and_click_open_the_effect_picker_at_the_visible_gap() {
        for width in [760.0, 1024.0] {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let mut app = App::new(ctx.clone(), PathBuf::new());
            insert_fixture(&mut app, "populated");
            app.catalog.effects = vec![app.view.project.tracks[0].inserts[0].plugin.clone()];
            let original = app.view.project.clone();
            let render = |app: &mut App, events| {
                ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 768.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| app.main_surface(ui, &ctx),
                )
            };
            let _ = render(&mut app, vec![]);
            let tree = render(&mut app, vec![])
                .platform_output
                .accesskit_update
                .unwrap();
            assert!(tree.nodes.iter().any(|(_, node)| {
                node.label() == Some("Strings, Plugin Glass Strings, Open instrument editor")
            }));
            let gap = tree
                .nodes
                .iter()
                .find_map(|(_, node)| {
                    (node.label() == Some("Strings, insert effect before Stereo Delay"))
                        .then(|| node.bounds())
                        .flatten()
                })
                .expect("insertion gap is accessible");
            let pos = egui::pos2(
                ((gap.x0 + gap.x1) / 2.0) as f32,
                ((gap.y0 + gap.y1) / 2.0) as f32,
            );
            let hovered = render(&mut app, vec![egui::Event::PointerMoved(pos)]);
            assert!(hovered.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::LineSegment { points, stroke }
                    if stroke.color == crate::colors::ACCENT && stroke.width == 3.0
                        && (points[0].y - pos.y).abs() < 1.0
            )));
            for pressed in [true, false] {
                let clicked = render(
                    &mut app,
                    vec![egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    }],
                );
                if !pressed {
                    assert!(
                        clicked
                            .platform_output
                            .accesskit_update
                            .unwrap()
                            .nodes
                            .iter()
                            .any(|(_, node)| node.label() == Some("Tape EQ · VST3"))
                    );
                }
            }
            let _ = render(
                &mut app,
                vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            assert_eq!(app.view.project, original);
        }
    }

    #[test]
    fn mixer_master_stays_docked_and_track_meters_line_up_at_both_widths() {
        for width in [760.0, 1024.0] {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let mut app = App::new(ctx.clone(), PathBuf::new());
            for case in [
                "long-bank",
                "header",
                "header-empty",
                "header-long",
                "header-dock",
            ] {
                mixer_fixture(&mut app, case);
                let height = if case == "header-dock" { 480.0 } else { 640.0 };
                let render = |app: &mut App, events| {
                    let input = egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, height),
                        )),
                        events,
                        ..Default::default()
                    };
                    ctx.run_ui(input, |ui| {
                        egui::Frame::new()
                            .inner_margin(12.0)
                            .show(ui, |ui| app.main_surface(ui, &ctx));
                    })
                };
                let _ = render(&mut app, vec![]);
                let tree = render(&mut app, vec![])
                    .platform_output
                    .accesskit_update
                    .unwrap();
                let bounds = |label| {
                    tree.nodes
                        .iter()
                        .find_map(|(_, node)| {
                            (node.label() == Some(label) || node.value() == Some(label))
                                .then(|| node.bounds())
                                .flatten()
                        })
                        .unwrap_or_else(|| panic!("missing {label} at {width} points in {case}"))
                };
                if case == "long-bank" {
                    let master = bounds("Master Fader");
                    let pan = bounds("Strings Pan");
                    let width_knob = bounds("Strings Width");
                    let master_name = bounds("Master");
                    assert!(master.x0 > f64::from(width) - 110.0 && master.x1 < f64::from(width));
                    // T12 insert sections make the strip taller than the 640-point stage;
                    // the mixer scrolls vertically, so compare the docked rows instead.

                    assert!(master_name.y1 > master.y1);
                    assert_eq!(pan.y0, width_knob.y0);
                    assert!(pan.x1 <= width_knob.x0);
                    assert_eq!(bounds("Strings Fader").y0, master.y0);
                } else if case == "header" {
                    let strings = bounds("Strings stereo level · Clipped");
                    let piano = bounds("Piano stereo level");
                    assert_eq!(strings.x1, piano.x1);
                    assert_eq!(
                        strings.x1 - strings.x0,
                        f64::from(crate::theme::HEADER_METER)
                    );
                    assert_eq!(strings.y1 - strings.y0, 44.0);
                    assert!(bounds("Arm Strings for MIDI recording").x1 < strings.x0);
                    let master = bounds("Master stereo level");
                    assert_eq!(master.x1, strings.x1);
                    assert!(master.y0 > piano.y1 && master.y1 < 640.0);
                } else {
                    let master = bounds("Master stereo level");
                    assert_eq!(master.y1 - master.y0, 44.0);
                    assert!(master.y0 > 0.0 && master.y1 < f64::from(height));
                    if case != "header-long" {
                        continue;
                    }
                    let before = bounds("Strings stereo level · Clipped");
                    let hover =
                        egui::Event::PointerMoved(egui::pos2(210.0, before.y0 as f32 + 10.0));
                    let wheel = egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, -100.0),
                        phase: egui::TouchPhase::Move,
                        modifiers: egui::Modifiers::NONE,
                    };
                    let _ = render(&mut app, vec![hover.clone()]);
                    let _ = render(&mut app, vec![hover, wheel]);
                    let tree = render(&mut app, vec![])
                        .platform_output
                        .accesskit_update
                        .unwrap();
                    let find = |label| {
                        tree.nodes.iter().find_map(|(_, node)| {
                            (node.label() == Some(label))
                                .then(|| node.bounds())
                                .flatten()
                        })
                    };
                    assert_eq!(find("Master stereo level"), Some(master));
                    assert!(
                        find("Strings stereo level · Clipped")
                            .is_none_or(|after| after.y0 < before.y0)
                    );
                }
            }
        }
    }

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
    fn t15_mixer_exposes_bus_sends_output_and_add_bus_at_both_widths() {
        for width in [760.0, 1024.0] {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let mut app = App::new(ctx.clone(), PathBuf::new());
            routing_fixture(&mut app, "buses");
            app.mixer_ui.open = true;
            let render = |app: &mut App, events| {
                ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 900.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| app.main_surface(ui, &ctx),
                )
            };
            let _ = render(&mut app, vec![]);
            let tree = render(&mut app, vec![])
                .platform_output
                .accesskit_update
                .unwrap();
            let find = |label: &str| {
                tree.nodes
                    .iter()
                    .find_map(|(_, node)| {
                        (node.label() == Some(label) || node.value() == Some(label))
                            .then(|| node.bounds())
                            .flatten()
                    })
                    .unwrap_or_else(|| panic!("missing {label} at {width} points"))
            };
            let bus = find("Reverb");
            let send = find("Strings Send");
            let add_send = find("Strings, Add send…");
            let add_effect = find("Strings, Add audio effect…");
            let add_bus = find("+ Bus");
            assert!(bus.x0 >= 0.0 && bus.x1 <= f64::from(width));
            assert!(send.x0 >= bus.x1.min(send.x0) && send.x1 <= f64::from(width));
            assert!(add_send.x0 >= 0.0 && add_send.x1 <= f64::from(width));
            assert!(add_bus.x0 >= 0.0 && add_bus.x1 <= f64::from(width));
            assert!(
                (add_send.height() - add_effect.height()).abs() < 1.0,
                "Add send is {:.1} high, Add effect is {:.1} high at {width} points",
                add_send.height(),
                add_effect.height()
            );
            let find_prefix = |prefixes: &[&str]| {
                tree.nodes
                    .iter()
                    .find_map(|(_, node)| {
                        let label = node.label()?;
                        prefixes
                            .iter()
                            .any(|prefix| label.starts_with(prefix))
                            .then(|| node.bounds())
                            .flatten()
                    })
                    .unwrap_or_else(|| panic!("missing {prefixes:?} at {width} points"))
            };
            let slot = find_prefix(&["Strings, Plugin "]);
            let power = find_prefix(&["Strings, Bypass Plugin ", "Strings, Enable Plugin "]);
            let power_center = (power.x0 + power.x1) / 2.0;
            let frame_right = add_send.x1 + f64::from(crate::theme::MIX_PADDING[1]);
            let slot_end = (slot.x1 + frame_right) / 2.0;
            assert!(
                (power_center - slot_end).abs() < 1.0,
                "bypass is not centred between slot and strip edge at {width} points: slot={slot:?} power={power:?} add_send={add_send:?} power_center={power_center} slot_end={slot_end}"
            );
            assert!(
                (power.width() - 14.0).abs() < 1.0,
                "insert bypass keeps too much horizontal padding: {power:?}"
            );
            let send_bypass =
                find_prefix(&["Strings, Bypass Send to ", "Strings, Enable Send to "]);
            assert!(
                (send_bypass.width() - 14.0).abs() < 1.0
                    && (send_bypass.height() - 16.0).abs() < 1.0,
                "send bypass is not the compact glyph: {send_bypass:?}"
            );
        }
    }

    #[test]
    fn t15_buses_dock_right_and_strips_keep_one_width_and_height() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = App::new(ctx.clone(), PathBuf::new());
        routing_fixture(&mut app, "buses");
        app.mixer_ui.open = true;
        let render = |app: &mut App, events| {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1024.0, 900.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| app.main_surface(ui, &ctx),
            )
        };
        let _ = render(&mut app, vec![]);
        let tree = render(&mut app, vec![])
            .platform_output
            .accesskit_update
            .unwrap();
        let bounds = |label: &str| {
            tree.nodes
                .iter()
                .filter_map(|(_, node)| {
                    (node.label() == Some(label) || node.value() == Some(label))
                        .then(|| node.bounds())
                        .flatten()
                })
                .collect::<Vec<_>>()
        };
        let strings = bounds("Strings Fader")[0];
        let piano = bounds("Piano Fader")[0];
        let drum_bus = bounds("Reverb Fader")[0];
        let master = bounds("Master Fader")[0];
        assert!((piano.x0 - strings.x0 - 93.0).abs() < 1.0);
        assert!(drum_bus.x0 > piano.x1);
        assert!(master.x0 > drum_bus.x0);
        assert!((drum_bus.y0 - strings.y0).abs() < 1.0);
        assert!((master.y0 - strings.y0).abs() < 1.0);
    }

    #[test]
    fn t15_mixer_strip_click_selects_the_channel_and_reveals_it() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = App::new(ctx.clone(), PathBuf::new());
        routing_fixture(&mut app, "buses");
        app.mixer_ui.open = true;
        app.view.selected = Some(2);
        app.mixer_ui.follow_instrument_selection(Some(2));
        let render = |app: &mut App, events| {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1024.0, 900.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| app.main_surface(ui, &ctx),
            )
        };
        let _ = render(&mut app, vec![]);
        let tree = render(&mut app, vec![])
            .platform_output
            .accesskit_update
            .unwrap();
        let send = tree
            .nodes
            .iter()
            .find_map(|(_, node)| {
                (node.label() == Some("Strings Send") || node.value() == Some("Strings Send"))
                    .then(|| node.bounds())
                    .flatten()
            })
            .unwrap();
        let pos = egui::pos2(
            (send.x0 + send.x1) as f32 / 2.0,
            (send.y0 + send.y1) as f32 / 2.0,
        );
        for pressed in [true, false] {
            let _ = render(
                &mut app,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
        assert_eq!(app.track_focus, Some(1));
        assert_eq!(app.mixer_ui.reveal, None);
    }

    #[test]
    fn t15_add_send_menu_can_create_a_bus_or_pick_an_existing_one() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = App::new(ctx.clone(), PathBuf::new());
        routing_fixture(&mut app, "buses");
        app.mixer_ui.open = true;
        let render = |app: &mut App, events| {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1024.0, 900.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| app.main_surface(ui, &ctx),
            )
        };
        let _ = render(&mut app, vec![]);
        let tree = render(&mut app, vec![])
            .platform_output
            .accesskit_update
            .unwrap();
        let add_send = tree
            .nodes
            .iter()
            .find_map(|(_, node)| {
                (node.label() == Some("Strings, Add send…")
                    || node.value() == Some("Strings, Add send…"))
                .then(|| node.bounds())
                .flatten()
            })
            .unwrap();
        let pos = egui::pos2(
            (add_send.x0 + add_send.x1) as f32 / 2.0,
            (add_send.y0 + add_send.y1) as f32 / 2.0,
        );
        for pressed in [true, false] {
            let _ = render(
                &mut app,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
        let _ = render(&mut app, vec![]);
        let tree = render(&mut app, vec![])
            .platform_output
            .accesskit_update
            .unwrap();
        let mut labels = Vec::new();
        for (_, node) in &tree.nodes {
            labels.extend(node.label().map(str::to_owned));
            labels.extend(node.value().map(str::to_owned));
        }
        assert!(
            labels.iter().any(|label| label == "New Bus Send…"),
            "new-send action missing: {labels:?}"
        );
        assert!(
            labels.iter().any(|label| label == "Reverb"),
            "existing bus entries missing: {labels:?}"
        );
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
