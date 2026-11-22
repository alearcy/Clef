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
/// T41 session-window cases: the default layout, the Editor and Mixer toggles, a track search,
/// and an active Fit Height.
const SESSION_CASES: &[&str] = &[
    "session",
    "editor",
    "mixer",
    "search",
    "fit",
    "mixer-full",
    "functions",
    "no-clip",
    "rulers",
    "empty",
];
const EDITING_CASES: &[&str] = &[
    "arrangement-five",
    "arrangement-height-24",
    "arrangement-height-160",
    "roll-empty",
    "roll-rows-6",
    "roll-rows-24",
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

/// T40 editing-surface cases: five adjacent tracks at three heights, and the one-grid roll.
fn editing_fixture(app: &mut App, case: &str) {
    if let Some(rows) = case.strip_prefix("roll-") {
        piano_roll_fixture(app, if rows == "empty" { "empty" } else { "single" });
        app.view.project.piano_roll_view.row_height = match rows {
            "rows-6" => 6,
            "rows-24" => 24,
            _ => clef_model::project::DEFAULT_ROW_HEIGHT,
        };
        return;
    }
    arrangement_fixture(app, "");
    let names = ["Bass", "Pads", "Drums"];
    for (index, name) in names.into_iter().enumerate() {
        let id = 3 + index as u64;
        let mut track = app.view.project.tracks[1].clone();
        track.id = id;
        track.name = name.into();
        track.colour = Some(clef_model::project::next_track_colour(id as usize - 1));
        app.view.project.tracks.push(track);
        std::sync::Arc::make_mut(&mut app.view.project.midi)
            .record(
                id,
                Mode::Merge,
                [index as u64 * 960, index as u64 * 960 + 1_920],
                vec![vec![Event {
                    tick: index as u64 * 960,
                    order: 0,
                    message: Message::Note {
                        channel: 0,
                        key: 48 + index as u8 * 5,
                        velocity: 96,
                        end: index as u64 * 960 + 960,
                        release: 0,
                        release_order: 1,
                    },
                }]],
                false,
            )
            .unwrap();
    }
    app.view.project.next_id = 6;
    app.view.project.arrangement_view.track_height = match case {
        "arrangement-height-24" => 24,
        "arrangement-height-160" => 160,
        _ => clef_model::project::DEFAULT_TRACK_HEIGHT,
    };
    app.arrangement_ui.settings = app.view.project.arrangement_view;
}

/// T41: five tracks with audio running, plus the state each session case shows.
fn session_fixture(app: &mut App, case: &str) {
    app.arrangement_ui = Default::default();
    app.piano_roll_open = false;
    editing_fixture(app, "");
    app.view.active = true;
    app.view.rate = 48_000;
    app.view.actual = "requested 256 frames · actual 256–256 frames · lead 1024 samples · \
        plugin latency 0 samples · clipped samples 0"
        .into();
    app.panel = Default::default();
    app.left_drawer_hidden = false;
    app.mixer_ui.open = matches!(case, "mixer" | "mixer-full" | "empty");
    app.panel.full = case == "mixer-full";
    if case == "search" {
        app.arrangement_ui.track_search = "pa".into();
    }
    if matches!(case, "editor" | "functions") {
        app.toggle_editor();
    }
    app.piano_roll
        .gallery_functions((case == "functions").then_some([true, false, true]));
    if case == "no-clip" {
        app.arrangement_ui.selected.clear();
        app.piano_roll_open = true;
    }
    if case == "rulers" {
        app.view.project.format.second_ruler = Some(clef_model::project::SecondCounter::Time);
        app.arrangement_ui.track_column = Some(300.0);
    }
    if case == "empty" {
        app.view
            .project
            .tracks
            .retain(|track| !track.is_instrument());
        app.view.selected = None;
        app.arrangement_ui.selected.clear();
        std::sync::Arc::make_mut(&mut app.view.project.midi)
            .tracks
            .clear();
    }
    if case == "fit" {
        app.arrangement_ui.fit = Some(crate::arrangement::Fit::Height);
    }
}

fn arrangement_fixture(app: &mut App, case: &str) {
    let mut view = history_fixture("history-empty");
    view.selected = Some(1);
    view.project.tracks.push(clef_model::project::Track {
        colour: Some(clef_model::project::TRACK_COLOURS[2]),
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
    // The mixer at full height without the drawer: the whole workspace (T41 S2).
    app.panel.full = app.mixer_ui.open;
    app.left_drawer_hidden = app.mixer_ui.open;
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
    // The mixer at full height without the drawer: the whole workspace (T41 S2).
    app.panel.full = true;
    app.left_drawer_hidden = true;
    for track in &mut app.view.project.tracks {
        track.mix.solo = false;
        track.mix.mute = false;
    }
    app.view.project.tracks[0].output = Some(3);
    app.view.project.tracks[1].output = Some(4);
    let bus = |id, name: &str| Track {
        colour: Some(clef_model::project::TRACK_COLOURS[4]),
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
        colour: Some(clef_model::project::TRACK_COLOURS[0]),
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
    scale: u32,
    cases: &'static [&'static str],
    prefix: &'static str,
    case: usize,
    frames: usize,
    specimens: crate::primitives::specimens::Specimens,
    result: std::sync::Arc<std::sync::Mutex<Option<Result<()>>>>,
}
impl Gallery {
    fn install(&mut self, ctx: &egui::Context) {
        let case = self.cases[self.case];
        if self.prefix == "t18" {
            egui::Popup::close_all(ctx);
            self.frames = 0;
            return;
        }
        if self.prefix == "t41" {
            egui::Popup::close_all(ctx);
            session_fixture(&mut self.app, case);
            self.frames = 0;
            return;
        }
        if self.prefix == "t40" {
            egui::Popup::close_all(ctx);
            editing_fixture(&mut self.app, case);
            self.frames = 0;
            return;
        }
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
                        "{}-{}-{}{}.png",
                        self.prefix,
                        self.cases[self.case],
                        self.width,
                        if self.scale == 2 { "@2x" } else { "" }
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
        self.draw(ui, &ctx);
        self.frames += 1;
        if self.frames == 4 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        ctx.request_repaint_after(Duration::from_millis(40));
    }
}

impl Gallery {
    /// Draws the current case; shared by the screenshot run and the accessibility walk.
    fn draw(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let ctx = ctx.clone();
        if self.prefix == "t18" {
            crate::paint::set_keyboard(&ctx, true);
            self.specimens.show(ui, &ctx, self.cases[self.case]);
        } else if self.prefix == "t40" && self.cases[self.case].starts_with("roll-") {
            egui::Frame::new().inner_margin(12.0).show(ui, |ui| {
                let height = ui.available_height() - 24.0;
                let app = &mut self.app;
                app.piano_roll.show(
                    ui,
                    height,
                    &app.view,
                    &app.arrangement_ui.selected,
                    app.view.edit_history.available,
                );
            });
        } else if self.prefix == "t10" && self.cases[self.case] != "dock" {
            egui::Frame::new().inner_margin(12.0).show(ui, |ui| {
                let height = ui.available_height() - 100.0;
                let app = &mut self.app;
                app.piano_roll.show(
                    ui,
                    height,
                    &app.view,
                    &app.arrangement_ui.selected,
                    app.view.edit_history.available,
                );
            });
        } else if matches!(
            self.prefix,
            "t08" | "t09" | "t10" | "t11" | "t12" | "t15" | "t34" | "t40" | "t41"
        ) {
            // main_surface lays out its own margins (T41).
            self.app.main_surface(ui, &ctx);
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
    }
}

pub fn render_startup(output: PathBuf, width: u32, scale: u32) -> Result<()> {
    render(output, width, scale, "t07", CASES)
}

pub fn render_history(output: PathBuf, width: u32, scale: u32) -> Result<()> {
    render(output, width, scale, "t08", HISTORY_CASES)
}

pub fn render_arrangement(output: PathBuf, width: u32, scale: u32) -> Result<()> {
    render(output, width, scale, "t09", ARRANGEMENT_CASES)
}

pub fn render_piano_roll(output: PathBuf, width: u32, scale: u32) -> Result<()> {
    render(output, width, scale, "t10", ROLL_CASES)
}

pub fn render_mixer(output: PathBuf, width: u32, scale: u32) -> Result<()> {
    render(output, width, scale, "t11", MIXER_CASES)
}

pub fn render_inserts(output: PathBuf, width: u32, scale: u32) -> Result<()> {
    render(output, width, scale, "t12", INSERT_CASES)
}

pub fn render_routing(output: PathBuf, width: u32, scale: u32) -> Result<()> {
    render(output, width, scale, "t15", ROUTING_CASES)
}

pub fn render_primitives(output: PathBuf, width: u32, scale: u32) -> Result<()> {
    render(
        output,
        width,
        scale,
        "t18",
        crate::primitives::specimens::CASES,
    )
}

pub fn render_editing(output: PathBuf, width: u32, scale: u32) -> Result<()> {
    render(output, width, scale, "t40", EDITING_CASES)
}

pub fn render_session(output: PathBuf, width: u32, scale: u32) -> Result<()> {
    render(output, width, scale, "t41", SESSION_CASES)
}

pub fn render_transport(output: PathBuf, width: u32, scale: u32) -> Result<()> {
    render(output, width, scale, "t34", TRANSPORT_CASES)
}

/// Renders every case of a stage. `scale` 2 renders at `pixels_per_point = 2.0` (Retina) and
/// names the files `…-1024@2x.png`; the window is then twice the size in physical pixels.
fn render(
    output: PathBuf,
    width: u32,
    scale: u32,
    prefix: &'static str,
    cases: &'static [&'static str],
) -> Result<()> {
    if !matches!(width, 1024 | 760) {
        return Err("gallery width must be 1024 or 760 points".into());
    }
    if !matches!(scale, 1 | 2) {
        return Err("gallery scale must be 1 or 2".into());
    }
    std::fs::create_dir_all(&output)?;
    let height = if prefix == "t15" {
        900.0
    } else if matches!(prefix, "t11" | "t12" | "t41") {
        768.0
    } else {
        640.0
    };
    let result = std::sync::Arc::new(std::sync::Mutex::new(None));
    let completion = result.clone();
    eframe::run_native(
        "Clef Gallery",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([(width * scale) as f32, height * scale as f32]),
            ..Default::default()
        },
        Box::new(move |cc| {
            // The gallery display reports 1.0; the zoom factor gives the requested scale.
            cc.egui_ctx.set_zoom_factor(
                scale as f32 / cc.egui_ctx.native_pixels_per_point().unwrap_or(1.0),
            );
            let mut gallery = Gallery {
                app: App::new(cc.egui_ctx.clone(), PathBuf::new()),
                output,
                width,
                scale,
                prefix,
                cases,
                frames: 0,
                case: 0,
                specimens: crate::primitives::specimens::Specimens::for_screenshots(),
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

    /// Every gallery stage and case, as `render` runs them.
    const STAGES: &[(&str, &[&str])] = &[
        ("t07", CASES),
        ("t08", HISTORY_CASES),
        ("t09", ARRANGEMENT_CASES),
        ("t10", ROLL_CASES),
        ("t11", MIXER_CASES),
        ("t12", INSERT_CASES),
        ("t15", ROUTING_CASES),
        ("t18", crate::primitives::specimens::CASES),
        ("t34", TRANSPORT_CASES),
        ("t40", EDITING_CASES),
        ("t41", SESSION_CASES),
    ];

    /// Interactive accesskit nodes without a name, or sliders without a value, in one case.
    fn unnamed(prefix: &'static str, cases: &'static [&'static str], case: usize) -> Vec<String> {
        use egui::accesskit::{Action, Role};
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut gallery = Gallery {
            app: App::new(ctx.clone(), PathBuf::new()),
            output: PathBuf::new(),
            width: 1024,
            scale: 1,
            prefix,
            cases,
            frames: 0,
            case,
            specimens: crate::primitives::specimens::Specimens::for_screenshots(),
            result: Default::default(),
        };
        gallery.install(&ctx);
        let mut tree = None;
        for _ in 0..3 {
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1024.0, 900.0),
                    )),
                    ..Default::default()
                },
                |ui| gallery.draw(ui, &ctx),
            );
            tree = output.platform_output.accesskit_update;
        }
        let tree = tree.expect("accesskit tree");
        let mut problems = Vec::new();
        for (id, node) in &tree.nodes {
            // egui's own ScrollArea bars carry no name; the scrolled content is what is read.
            let interactive = node.role() != Role::ScrollBar
                && (node.supports_action(Action::Click) || node.supports_action(Action::Focus));
            let named = node.label().is_some_and(|label| !label.trim().is_empty())
                || node
                    .value()
                    .is_some_and(|value| !value.trim().is_empty() && node.role() != Role::Slider);
            if interactive && !named {
                problems.push(format!(
                    "{prefix}-{}: {:?} {id:?} without a name at {:?}",
                    cases[case],
                    node.role(),
                    node.bounds()
                ));
            }
            if node.role() == Role::Slider
                && node.numeric_value().is_none()
                && node.value().is_none()
            {
                problems.push(format!(
                    "{prefix}-{}: slider {:?} without a value",
                    cases[case],
                    node.label()
                ));
            }
        }
        problems
    }

    #[test]
    fn resizing_to_the_minimum_width_and_back_keeps_controls_in_the_window() {
        use egui::accesskit::Action;
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = App::new(ctx.clone(), PathBuf::new());
        let mut outside = Vec::new();
        // The arrangement fits its window; mixer strips scroll, and its docked Master is covered by
        // `mixer_master_stays_docked_and_track_meters_line_up_at_both_widths`.
        arrangement_fixture(&mut app, "arrangement-selected");
        for width in [1024.0, 760.0, 1024.0, 760.0] {
            let mut tree = None;
            for _ in 0..3 {
                let output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 480.0),
                        )),
                        ..Default::default()
                    },
                    |ui| app.main_surface(ui, &ctx),
                );
                tree = output.platform_output.accesskit_update;
            }
            for (_, node) in tree.unwrap().nodes {
                let interactive =
                    node.supports_action(Action::Click) || node.supports_action(Action::Focus);
                if let Some(bounds) = node.bounds().filter(|_| interactive)
                    && (bounds.x0 < -0.5 || bounds.x1 > f64::from(width) + 0.5)
                {
                    outside.push(format!(
                        "{width}: {:?} {:?} {:?} {:?}",
                        node.role(),
                        node.label(),
                        node.value(),
                        bounds
                    ));
                }
            }
        }
        assert!(
            outside.is_empty(),
            "controls outside the window: {outside:#?}"
        );
    }

    #[test]
    fn every_gallery_control_has_an_accessible_name() {
        let mut problems = Vec::new();
        for (prefix, cases) in STAGES {
            for case in 0..cases.len() {
                problems.extend(unnamed(prefix, cases, case));
            }
        }
        assert!(
            problems.is_empty(),
            "{} unnamed controls:\n{}",
            problems.len(),
            problems.join("\n")
        );
    }

    #[test]
    fn arrangement_and_roll_each_paint_one_continuous_playhead() {
        let ctx = egui::Context::default();
        let mut app = App::new(ctx.clone(), PathBuf::new());
        editing_fixture(&mut app, "arrangement-five");
        let lower = app.view.project.midi.clips(1)[0].id();
        app.piano_roll
            .open(lower, app.view.project.midi.find_clip(lower).unwrap().1);
        app.piano_roll_open = true;
        let mut output = None;
        for _ in 0..3 {
            output = Some(ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1024.0, 1000.0),
                    )),
                    ..Default::default()
                },
                |ui| app.main_surface(ui, &ctx),
            ));
        }
        let playheads: Vec<_> = output
            .unwrap()
            .shapes
            .into_iter()
            .filter_map(|clipped| match clipped.shape {
                egui::Shape::LineSegment { points, stroke }
                    if stroke.color == crate::theme::ACCENT
                        && points[0].x == points[1].x
                        && (points[1].y - points[0].y).abs() > 60.0 =>
                {
                    Some((points[0].y.min(points[1].y), points[0].y.max(points[1].y)))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            playheads.len(),
            2,
            "one playhead per surface: {playheads:?}"
        );
        let tracks = 5.0 * f32::from(clef_model::project::DEFAULT_TRACK_HEIGHT);
        assert!(
            playheads.iter().any(|(top, bottom)| bottom - top > tracks),
            "the arrangement playhead runs through every track to the Master row"
        );
    }

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
                    if stroke.color == crate::theme::ACCENT && stroke.width == 3.0
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
    fn the_editor_toggle_opens_the_selected_roll_from_the_mixer_and_closes_it_again() {
        let mut app = App::new(egui::Context::default(), PathBuf::new());
        session_fixture(&mut app, "mixer");
        assert!(app.mixer_ui.open);
        assert!(app.command_available(crate::Command::ShowEditor, false));
        app.toggle_editor();
        assert!(!app.mixer_ui.open, "Editor leaves the mixer");
        let clip = app
            .arrangement_ui
            .open_roll
            .take()
            .expect("a roll is requested");
        assert!(app.arrangement_ui.selected.contains(&clip));
        app.piano_roll_open = true;
        app.toggle_editor();
        assert!(!app.piano_roll_open, "a second press closes the roll");
        assert_eq!(app.arrangement_ui.open_roll, None);
    }

    #[test]
    fn the_lower_panel_docks_one_view_and_full_height_hides_the_arrangement() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = App::new(ctx.clone(), PathBuf::new());
        session_fixture(&mut app, "editor");
        let clip = app.arrangement_ui.open_roll.take().unwrap();
        app.arrangement_ui.open_roll = Some(clip);
        let labels = |app: &mut App, events: Vec<egui::Event>| {
            let render = |app: &mut App, events| {
                ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1024.0, 768.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| app.main_surface(ui, &ctx),
                )
            };
            let _ = render(app, events);
            let tree = render(app, vec![])
                .platform_output
                .accesskit_update
                .unwrap();
            tree.nodes
                .iter()
                .filter_map(|(_, node)| {
                    let label = node.label()?.to_owned();
                    Some((label, node.bounds()?))
                })
                .collect::<Vec<_>>()
        };
        let has = |labels: &[(String, egui::accesskit::Rect)], label: &str| {
            labels.iter().any(|(item, _)| item == label)
        };
        let docked = labels(&mut app, vec![]);
        assert!(app.piano_roll_open);
        assert!(
            has(&docked, "Find tracks"),
            "half height keeps the arrangement"
        );
        assert!(has(&docked, "Detach into Own Window"));
        assert!(has(&docked, "Full Height"));
        // Opening the mixer takes the editor's place (S1).
        app.dispatch_mixer(crate::Command::ShowMixer);
        assert!(app.mixer_ui.open && !app.piano_roll_open);
        // Full height hides the tool row and the arrangement (S2).
        app.dispatch(crate::Command::TogglePanelFull, &ctx);
        let full = labels(&mut app, vec![]);
        assert!(!has(&full, "Find tracks"));
        assert!(has(&full, "Half Height"));
        assert!(
            !app.enabled(crate::Command::SelectAllClips),
            "clip commands wait while the arrangement is hidden"
        );
        app.dispatch(crate::Command::TogglePanelFull, &ctx);
        assert!(app.enabled(crate::Command::SelectAllClips));
        // The panel's Close closes the mixer.
        let close = docked_close(&labels(&mut app, vec![]));
        let click = |pressed| egui::Event::PointerButton {
            pos: close,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let _ = labels(&mut app, vec![click(true)]);
        let _ = labels(&mut app, vec![click(false)]);
        assert!(!app.mixer_ui.open);
    }

    /// The centre of the lower panel's Close control.
    fn docked_close(labels: &[(String, egui::accesskit::Rect)]) -> egui::Pos2 {
        let (_, rect) = labels
            .iter()
            .filter(|(label, _)| label == "Close")
            .max_by(|a, b| a.1.x0.total_cmp(&b.1.x0))
            .expect("a Close control");
        egui::pos2(
            ((rect.x0 + rect.x1) / 2.0) as f32,
            ((rect.y0 + rect.y1) / 2.0) as f32,
        )
    }

    #[test]
    fn the_status_row_puts_errors_before_recording_and_messages() {
        let mut app = App::new(egui::Context::default(), PathBuf::new());
        session_fixture(&mut app, "session");
        app.view.message = "Saved".into();
        app.view.record_unavailable = Some("Arm an instrument track to record");
        assert_eq!(app.status_message(), ("Saved".into(), false));
        app.view.message.clear();
        app.status.clear();
        assert_eq!(
            app.status_message(),
            ("Arm an instrument track to record".into(), false)
        );
        app.view.recording = true;
        assert_eq!(
            app.status_message(),
            ("Preparing MIDI recording".into(), true)
        );
        app.view.history_message = "History unavailable".into();
        assert_eq!(app.status_message(), ("History unavailable".into(), true));
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
                        app.main_surface(ui, &ctx);
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
                    let hover = egui::Event::PointerMoved(egui::pos2(
                        before.x0 as f32 - 20.0,
                        before.y0 as f32 + 10.0,
                    ));
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
    fn t15_mixer_exposes_bus_sends_output_and_panel_controls_at_both_widths() {
        for width in [760.0, 1024.0] {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let mut app = App::new(ctx.clone(), PathBuf::new());
            routing_fixture(&mut app, "buses");
            app.mixer_ui.open = true;
            // The mixer at full height without the drawer: the whole workspace (T41 S2).
            app.panel.full = true;
            app.left_drawer_hidden = true;
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
            // T41 round 3: "+ Bus" left the mixer (S4); its panel controls sit in view (S2).
            let add_bus = find("Detach into Own Window");
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
        // The mixer at full height without the drawer: the whole workspace (T41 S2).
        app.panel.full = true;
        app.left_drawer_hidden = true;
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
        // The mixer at full height without the drawer: the whole workspace (T41 S2).
        app.panel.full = true;
        app.left_drawer_hidden = true;
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
    fn t15_send_bypass_glyph_click_reaches_the_session() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = App::new(ctx.clone(), PathBuf::new());
        routing_fixture(&mut app, "buses");
        app.mixer_ui.open = true;
        // The mixer at full height without the drawer: the whole workspace (T41 S2).
        app.panel.full = true;
        app.left_drawer_hidden = true;
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
        let glyph = tree
            .nodes
            .iter()
            .find_map(|(_, node)| {
                node.label()
                    .is_some_and(|label| label.contains("Bypass Send to"))
                    .then(|| node.bounds())
                    .flatten()
            })
            .expect("send bypass glyph");
        let pos = egui::pos2(
            (glyph.x0 + glyph.x1) as f32 / 2.0,
            (glyph.y0 + glyph.y1) as f32 / 2.0,
        );
        app.actions_in_flight = 0;
        app.busy_since = None;
        let _ = render(&mut app, vec![egui::Event::PointerMoved(pos)]);
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
        assert_eq!(app.actions_in_flight, 1, "the bypass click sends one edit");
    }

    #[test]
    fn t15_add_send_menu_can_create_a_bus_or_pick_an_existing_one() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = App::new(ctx.clone(), PathBuf::new());
        routing_fixture(&mut app, "buses");
        app.mixer_ui.open = true;
        // The mixer at full height without the drawer: the whole workspace (T41 S2).
        app.panel.full = true;
        app.left_drawer_hidden = true;
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
