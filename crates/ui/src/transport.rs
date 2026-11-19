use crate::{
    Action, App, Command,
    arrangement::{Lane, LaneInput},
    primitives::{
        timeline::{self as timeline_ui, LOOP_HEIGHT, LoopGesture, RULER_HEIGHT, Viewport},
        transport::{self as controls, Glyph},
    },
};
use clef_model::{
    midi::Mode,
    timeline::{MAX_TICK, StopMode, Tempo},
};
use clef_session::{TransportAction, TransportPhase};
use eframe::egui::{self, Key, Modifiers};

#[derive(Default)]
pub struct TransportUi {
    loop_gesture: Option<LoopGesture>,
    pub(super) pending_loop_range: Option<[u64; 2]>,
    pub(super) record_submitted: bool,
    tempo: Option<Tempo>,
    settings: bool,
    locate: u64,
    loop_range: [u64; 2],
}

impl App {
    pub(super) fn play_stop_command(&self) -> Command {
        let transport = self.view.transport;
        if transport.phase != TransportPhase::Stopped || transport.play_pending {
            Command::Stop
        } else {
            Command::Play
        }
    }

    pub fn transport_enabled(&self, command: Command, wait_for_transport: bool) -> bool {
        let transport = self.view.transport;
        let stopped = transport.phase == TransportPhase::Stopped;
        let waiting = wait_for_transport && transport.pending;
        use Command::*;
        match command {
            Stop => self.view.active && self.play_stop_command() == Stop,
            Play => {
                self.view.active
                    && stopped
                    && !waiting
                    && !self.view.recording
                    && (transport.tick < MAX_TICK || self.view.project.timeline.looping)
            }
            Record => {
                self.view.active
                    && stopped
                    && !waiting
                    && !self.view.recording
                    && self.view.armed.is_some()
                    && self.view.record_unavailable.is_none()
                    && (transport.tick < MAX_TICK || self.view.project.timeline.looping)
            }
            ArmTrack => {
                !self.view.recording
                    && !waiting
                    && self.view.track().is_some_and(|track| {
                        track.plugin.is_some() || self.view.armed == Some(track.id)
                    })
            }
            RecordMerge | RecordReplace | RecordNewTake | ReturnToStart | ToggleLoop
            | ToggleCountIn | KeepStopPosition | ReturnStopPosition => {
                !self.view.recording && !waiting
            }
            PlayStop => self.transport_enabled(self.play_stop_command(), wait_for_transport),
            TempoMap => stopped && !self.view.recording && !waiting,
            _ => !waiting,
        }
    }

    pub fn dispatch_transport(&mut self, command: Command) {
        use Command::*;
        let action = match command {
            Play => TransportAction::Play,
            Record => TransportAction::Record,
            ArmTrack => {
                let Some(track) = self.view.track() else {
                    return;
                };
                TransportAction::Arm(track.id)
            }
            RecordMerge => TransportAction::SetRecordMode(Mode::Merge),
            RecordReplace => TransportAction::SetRecordMode(Mode::Replace),
            RecordNewTake => TransportAction::SetRecordMode(Mode::NewTake),
            Stop => TransportAction::Stop,
            PlayStop => {
                self.dispatch_transport(self.play_stop_command());
                return;
            }
            ReturnToStart => {
                self.arrangement_ui.settings.offset = 0;
                TransportAction::Locate(0)
            }
            ToggleLoop => TransportAction::ToggleLoop,
            ToggleClick => TransportAction::ToggleClick,
            ToggleCountIn => TransportAction::ToggleCountIn,
            KeepStopPosition => TransportAction::SetStopMode(StopMode::KeepPosition),
            ReturnStopPosition => TransportAction::SetStopMode(StopMode::ReturnToPlayStart),
            TempoMap => {
                let timeline = &self.view.project.timeline;
                let tick = self.view.transport.tick;
                let mut tempo = timeline.section_at(tick).1;
                tempo.bar = timeline
                    .position(tick)
                    .bar
                    .min(clef_model::timeline::MAX_BAR);
                self.transport_ui.tempo = Some(tempo);
                return;
            }
            TransportSettings => {
                self.transport_ui.locate = self.view.transport.tick;
                self.transport_ui.loop_range = self.view.project.timeline.loop_range;
                self.transport_ui.settings = true;
                return;
            }
            _ => unreachable!(),
        };
        self.send(Action::Transport(action));
    }

    pub fn transport_bar(&self, ui: &mut egui::Ui) -> Option<Command> {
        let transport = self.view.transport;
        let timeline = &self.view.project.timeline;
        let position = timeline.position(transport.audible_tick);
        let tempo = timeline.section_at(transport.audible_tick).1;
        let mut chosen = None;
        let compact = ui.available_width() < 860.0;
        controls::bar(ui, |ui| {
            for (command, glyph, on) in [
                (Command::ReturnToStart, Glyph::Start, false),
                (
                    Command::Play,
                    Glyph::Play,
                    transport.phase != TransportPhase::Stopped,
                ),
                (Command::Stop, Glyph::Stop, false),
                (
                    Command::Record,
                    Glyph::Record,
                    self.view.recording || self.transport_ui.record_submitted,
                ),
            ] {
                if self.transport_tool(ui, command, glyph, on) {
                    chosen = Some(command);
                }
            }
            let mode = self.view.project.midi.mode;
            ui.add_enabled_ui(
                !self.view.recording && self.confirm.is_none() && self.file.is_none(),
                |ui| {
                    let menu = ui.menu_button(format!("REC {} ▾", mode.label()), |ui| {
                        for (mode, command) in Mode::ALL.into_iter().zip([
                            Command::RecordMerge,
                            Command::RecordReplace,
                            Command::RecordNewTake,
                        ]) {
                            if self.transport_button(
                                ui,
                                command,
                                mode.label(),
                                mode == self.view.project.midi.mode,
                            ) {
                                chosen = Some(command);
                                ui.close();
                            }
                        }
                    });
                    menu.response.on_hover_text("MIDI record mode");
                },
            );
            if self.transport_tool(ui, Command::ToggleLoop, Glyph::Loop, timeline.looping) {
                chosen = Some(Command::ToggleLoop);
            }
            if !compact {
                if self.transport_tool(ui, Command::ToggleClick, Glyph::Metronome, timeline.click) {
                    chosen = Some(Command::ToggleClick);
                }
                if self.transport_button(ui, Command::ToggleCountIn, "1 Bar", timeline.count_in) {
                    chosen = Some(Command::ToggleCountIn);
                }
                if self.transport_button(
                    ui,
                    Command::TempoMap,
                    &format!("{} BPM  {}/{}", tempo.bpm, tempo.beats, tempo.unit),
                    false,
                ) {
                    chosen = Some(Command::TempoMap);
                }
            }
            controls::counter(ui, position, transport.phase == TransportPhase::CountingIn);
            if self.transport_button(ui, Command::TransportSettings, "Transport…", false) {
                chosen = Some(Command::TransportSettings);
            }
        });
        chosen
    }

    fn transport_button(&self, ui: &mut egui::Ui, command: Command, label: &str, on: bool) -> bool {
        let name = self.transport_tooltip(ui, command);
        controls::button(ui, label, &name, self.command_available(command, false), on)
            && self.enabled(command)
    }

    fn transport_tool(&self, ui: &mut egui::Ui, command: Command, glyph: Glyph, on: bool) -> bool {
        let name = self.transport_tooltip(ui, command);
        let mut response = controls::tool_button(
            ui,
            glyph,
            &name,
            self.command_available(command, false),
            on,
            self.keyboard_mode,
        );
        if command == Command::Record
            && let Some(reason) = self.view.record_unavailable
        {
            response = response.on_disabled_hover_text(reason);
        }
        response.clicked() && self.enabled(command)
    }

    fn transport_tooltip(&self, ui: &egui::Ui, command: Command) -> String {
        let binding = self
            .bindings
            .iter()
            .find(|binding| binding.command == command)
            .unwrap();
        binding.shortcut.map_or_else(
            || binding.title.to_string(),
            |key| format!("{} ({})", binding.title, ui.ctx().format_shortcut(&key)),
        )
    }

    pub fn timeline_surface(
        &mut self,
        ui: &mut egui::Ui,
        height: f32,
        width: f32,
    ) -> (Option<u64>, bool, Option<TransportAction>) {
        let header_width = if width >= 550.0 { 220.0 } else { 176.0 };
        let lane_width = width - header_width - ui.spacing().item_spacing.x;
        let timeline = &self.view.project.timeline;
        let transport = self.view.transport;
        let locate_enabled = self.enabled(Command::ReturnToStart);
        let edit_enabled = self.enabled(Command::TempoMap);
        let clip_enabled = self.view.edit_history.available
            && self.busy_since.is_none()
            && self.confirm.is_none()
            && self.file.is_none();
        ui.horizontal(|ui| {
            timeline_ui::snap_control(ui, &mut self.arrangement_ui.settings.snap);
        });
        let settings = self.arrangement_ui.settings;
        let viewport = Viewport::with_snap(
            timeline,
            settings.offset,
            settings.zoom_milli as f32 / 1_000.0,
            lane_width,
            settings.snap,
        );
        let mut action = None;
        let mut choose = None;
        let mut add = false;
        let mut clip_command = None;
        let mut lanes = Vec::new();
        let mut loop_gesture = self.transport_ui.loop_gesture.take();
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(header_width);
                add = self.button(ui, Command::AddTrack);
            });
            ui.vertical(|ui| {
                if let Some(tick) = viewport.ruler(
                    ui,
                    lane_width,
                    transport.audible_tick,
                    timeline,
                    locate_enabled,
                ) {
                    action = Some(TransportAction::Locate(tick));
                }
                if let Some(range) = viewport.loop_band(
                    ui,
                    lane_width,
                    timeline,
                    edit_enabled,
                    &mut loop_gesture,
                    self.transport_ui.pending_loop_range,
                ) {
                    action = Some(TransportAction::SetLoopRange(range));
                }
            });
        });
        self.transport_ui.loop_gesture = loop_gesture;
        egui::ScrollArea::vertical()
            .id_salt("arrangement-tracks")
            .max_height((height - RULER_HEIGHT - LOOP_HEIGHT - 50.0).max(50.0))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.view.project.tracks.is_empty() {
                    ui.horizontal(|ui| {
                        ui.allocate_space(egui::vec2(header_width, 44.0));
                        let (_, rect) = ui.allocate_space(egui::vec2(lane_width, 44.0));
                        viewport.lane(ui, rect, transport.audible_tick);
                    });
                }
                for (index, track) in self.view.project.tracks.iter().enumerate() {
                    let state = self
                        .view
                        .state(track.id)
                        .map_or("Empty", |state| state.status.as_str());
                    let plugin = track.plugin.as_ref().map_or_else(
                        || "No instrument".to_owned(),
                        |p| format!("{} · {}", p.name, p.format),
                    );
                    ui.horizontal_top(|ui| {
                        let response = Self::track_row(
                            ui,
                            &track.name,
                            &format!("{plugin} · {state}"),
                            header_width - 28.0 - ui.spacing().item_spacing.x,
                            self.view.selected == Some(track.id),
                            self.confirm.is_none(),
                            self.keyboard_mode,
                        );
                        let armed = self.view.armed == Some(track.id);
                        let arm_enabled = !self.view.recording
                            && !transport.pending
                            && self.confirm.is_none()
                            && self.file.is_none()
                            && self.busy_since.is_none()
                            && (armed || track.plugin.is_some());
                        if controls::arm(
                            ui,
                            &track.name,
                            arm_enabled,
                            armed,
                            armed && self.view.recording_region.is_some(),
                            self.keyboard_mode,
                        ) {
                            action = Some(TransportAction::Arm(track.id));
                        }
                        let (_, rect) =
                            ui.allocate_space(egui::vec2(lane_width, response.rect.height()));
                        viewport.lane_grid(ui, rect);
                        let lane = Lane {
                            track: track.id,
                            rect,
                            visible: rect.intersect(ui.clip_rect()),
                        };
                        lanes.push(lane);
                        if lane.visible.is_positive() {
                            let binding = |command| {
                                let binding = self
                                    .bindings
                                    .iter()
                                    .find(|binding| binding.command == command)
                                    .unwrap();
                                (
                                    binding.title,
                                    binding
                                        .shortcut
                                        .map(|shortcut| ui.ctx().format_shortcut(&shortcut))
                                        .unwrap_or_default(),
                                )
                            };
                            clip_command = self
                                .arrangement_ui
                                .lane(
                                    ui,
                                    lane,
                                    LaneInput {
                                        viewport: &viewport,
                                        timeline,
                                        arrangement: &self.view.project.midi,
                                        track,
                                        enabled: clip_enabled,
                                        playhead: transport.audible_tick,
                                    },
                                    binding,
                                )
                                .or(clip_command);
                        }
                        if armed && let Some(range) = self.view.recording_region {
                            viewport.recording(
                                ui,
                                rect,
                                range,
                                timeline.loop_range,
                                self.view.recording_notes.as_deref(),
                            );
                        }
                        viewport.paint_playhead(ui, rect, transport.audible_tick);
                        if self.track_focus == Some(track.id) {
                            response.request_focus();
                            response.scroll_to_me(Some(egui::Align::Center));
                            self.track_focus = None;
                        }
                        if response.clicked() {
                            choose = Some(track.id);
                        }
                        if response.double_clicked() {
                            choose = Some(track.id);
                            self.rename_focus = true;
                        }
                        if response.has_focus() {
                            let tracks = &self.view.project.tracks;
                            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown))
                                && index + 1 < tracks.len()
                            {
                                choose = Some(tracks[index + 1].id);
                            }
                            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp))
                                && index > 0
                            {
                                choose = Some(tracks[index - 1].id);
                            }
                        }
                    });
                }
            });
        let last_bar = timeline.changes.last().map_or(1, |tempo| tempo.bar);
        let max_offset = timeline
            .bar_start(last_bar.saturating_add(64))
            .max(timeline.loop_range[1])
            .max(
                timeline.bar_start(
                    timeline
                        .position(transport.audible_tick)
                        .bar
                        .saturating_add(16),
                ),
            )
            .max(
                self.view
                    .project
                    .midi
                    .tracks
                    .values()
                    .flatten()
                    .map(|clip| clip.end)
                    .max()
                    .unwrap_or(0),
            )
            .max(settings.offset)
            .min(MAX_TICK);
        let edit = match self.arrangement_ui.finish_frame(
            ui,
            &lanes,
            &viewport,
            timeline,
            &self.view.project.midi,
            clip_enabled,
        ) {
            Ok(edit) => edit,
            Err(reason) => {
                self.status = reason.into();
                None
            }
        };
        let mut offset = settings.offset;
        let mut zoom = settings.zoom_milli as f32 / 1_000.0;
        controls::navigation(ui, &mut offset, &mut zoom, max_offset);
        let zoom_milli = (zoom * 1_000.0).round() as u32;
        if zoom_milli != settings.zoom_milli {
            let playhead = viewport.x(transport.audible_tick);
            let anchor = if (0.0..=lane_width).contains(&playhead) {
                playhead
            } else {
                lane_width / 2.0
            };
            let offset_at_zoom = (f64::from(anchor) * clef_model::timeline::QUARTER as f64
                / f64::from(zoom))
            .round() as u64;
            offset = viewport.tick_at(anchor).saturating_sub(offset_at_zoom);
        }
        let next = clef_model::project::ArrangementView {
            offset,
            zoom_milli,
            ..settings
        };
        self.arrangement_ui.settings = next;
        if let Some(command) = clip_command {
            self.dispatch(command, ui.ctx());
        } else if let Some(edit) = edit {
            self.commit_clip_edit(edit);
        } else if next != self.view.project.arrangement_view
            && action.is_none()
            && !add
            && choose.is_none()
            && self.confirm.is_none()
            && self.file.is_none()
            && !ui.input(|input| input.pointer.any_down())
        {
            self.send(Action::ArrangementView(next));
        }
        (choose, add, action)
    }

    pub fn transport_windows(&mut self, ctx: &egui::Context) {
        if let Some(mut tempo) = self.transport_ui.tempo {
            let mut open = true;
            let mut action = None;
            let enabled = self.enabled(Command::TempoMap);
            controls::window(ctx, "Tempo and Meter Map", &mut open, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(144.0)
                    .show(ui, |ui| {
                        for &change in &self.view.project.timeline.changes {
                            let label = format!(
                                "Bar {}    {} BPM    {}/{}",
                                change.bar, change.bpm, change.beats, change.unit
                            );
                            if controls::button(ui, &label, &label, true, tempo.bar == change.bar) {
                                tempo = change;
                            }
                        }
                    });
                controls::tempo_fields(ui, &mut tempo);
                ui.horizontal(|ui| {
                    if controls::button(
                        ui,
                        "Set Change",
                        "Set tempo/meter at this bar",
                        enabled,
                        false,
                    ) {
                        action = Some(TransportAction::SetTempo(tempo));
                    }
                    let can_remove = enabled
                        && tempo.bar != 1
                        && self
                            .view
                            .project
                            .timeline
                            .changes
                            .iter()
                            .any(|t| t.bar == tempo.bar);
                    if controls::button(
                        ui,
                        "Remove Change",
                        "Remove the change at this bar",
                        can_remove,
                        false,
                    ) {
                        action = Some(TransportAction::RemoveTempo(tempo.bar));
                    }
                });
            });
            self.transport_ui.tempo = open.then_some(tempo);
            if let Some(action) = action {
                self.send(Action::Transport(action));
            }
        }
        if self.transport_ui.settings {
            let mut open = true;
            let mut command = None;
            let mut action = None;
            let mut locate = self.transport_ui.locate;
            let [mut left, mut right] = self.transport_ui.loop_range;
            let mode = self.view.project.timeline.stop_mode;
            controls::window(ctx, "Transport Settings", &mut open, |ui| {
                let timeline = &self.view.project.timeline;
                ui.horizontal_wrapped(|ui| {
                    for (choice, label, on) in [
                        (Command::ToggleClick, "Metronome", timeline.click),
                        (
                            Command::ToggleCountIn,
                            "One-Bar Count-In",
                            timeline.count_in,
                        ),
                        (Command::TempoMap, "Tempo and Meter Map…", false),
                    ] {
                        if self.transport_button(ui, choice, label, on) {
                            command = Some(choice);
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("On Stop");
                    for (choice, value, label) in [
                        (
                            Command::KeepStopPosition,
                            StopMode::KeepPosition,
                            "Keep Position",
                        ),
                        (
                            Command::ReturnStopPosition,
                            StopMode::ReturnToPlayStart,
                            "Return to Play Start",
                        ),
                    ] {
                        if self.transport_button(ui, choice, label, mode == value) {
                            command = Some(choice);
                        }
                    }
                });
                controls::tick_field(ui, "Position", &mut locate);
                if controls::button(
                    ui,
                    "Locate",
                    "Locate to the specified tick",
                    self.enabled(Command::ReturnToStart),
                    false,
                ) {
                    action = Some(TransportAction::Locate(locate));
                }
                controls::tick_field(ui, "Loop Start", &mut left);
                controls::tick_field(ui, "Loop End", &mut right);
                if controls::button(
                    ui,
                    "Set Loop Range",
                    "Set both loop locators",
                    self.enabled(Command::TempoMap) && left < right,
                    false,
                ) {
                    action = Some(TransportAction::SetLoopRange([left, right]));
                }
            });
            self.transport_ui.settings = open;
            self.transport_ui.locate = locate;
            self.transport_ui.loop_range = [left, right];
            if let Some(command) = command {
                self.dispatch(command, ctx);
            } else if let Some(action) = action {
                self.send(Action::Transport(action));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, time::Instant};

    #[test]
    fn record_controls_and_clips_fit_both_workspace_widths() {
        use clef_model::{
            Plugin,
            midi::{Event, Message},
            project::Track,
        };
        for width in [760.0, 1024.0] {
            let context = egui::Context::default();
            context.enable_accesskit();
            let mut app = App::new(context.clone(), PathBuf::new());
            app.view.project_open = true;
            app.view.active = true;
            app.view.selected = Some(1);
            app.view.armed = Some(1);
            app.view.project.tracks.push(Track {
                id: 1,
                name: "Lead".into(),
                plugin: Some(Plugin {
                    id: "test".into(),
                    name: "Test instrument".into(),
                    format: "VST3".into(),
                    path: "/fixture.vst3".into(),
                }),
                state: Vec::new(),
            });
            std::sync::Arc::make_mut(&mut app.view.project.midi)
                .record(
                    1,
                    Mode::NewTake,
                    [0, 1920],
                    vec![
                        vec![Event {
                            tick: 30,
                            order: 0,
                            message: Message::Control([0xb0, 1, 21]),
                        }],
                        vec![Event {
                            tick: 70,
                            order: 0,
                            message: Message::Control([0xb0, 1, 29]),
                        }],
                    ],
                    false,
                )
                .unwrap();
            for phase in [
                TransportPhase::Stopped,
                TransportPhase::CountingIn,
                TransportPhase::Playing,
            ] {
                app.view.transport.phase = phase;
                app.view.recording = phase != TransportPhase::Stopped;
                app.view.recording_region = (phase == TransportPhase::Playing).then_some([0, 960]);
                let output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 640.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        egui::Frame::new().inner_margin(12.0).show(ui, |ui| {
                            app.main_surface(ui, &context);
                        });
                    },
                );
                let record_color = if phase == TransportPhase::Stopped {
                    crate::colors::TEXT
                } else {
                    crate::colors::DANGER
                };
                assert!(output.shapes.iter().any(|shape| matches!(
                    &shape.shape,
                    egui::Shape::Circle(circle)
                        if circle.radius == crate::theme::TOOL_GLYPH * 0.4
                            && circle.fill == record_color
                )));
                let tree = output.platform_output.accesskit_update.unwrap();
                let find = |label: &str| {
                    tree.nodes
                        .iter()
                        .find_map(|(_, node)| {
                            node.label()
                                .or_else(|| node.value())
                                .filter(|name| name.starts_with(label))
                                .map(|_| node)
                        })
                        .unwrap_or_else(|| {
                            panic!("missing accessible label {label} at {width}: {phase:?}")
                        })
                };
                let stop = find("Stop");
                let record = find("Record MIDI");
                assert_eq!(stop.is_disabled(), phase == TransportPhase::Stopped);
                assert_eq!(record.is_disabled(), phase != TransportPhase::Stopped);
                let mode = find("REC Merge");
                let labels = [
                    stop,
                    record,
                    mode,
                    find("Transport Settings"),
                    find("Lead, MIDI clip"),
                ];
                for node in labels {
                    let bounds = node.bounds().unwrap();
                    assert!(
                        bounds.x0 >= 0.0 && bounds.x1 <= f64::from(width),
                        "out of frame: {:?}",
                        node.label()
                    );
                }
                let arm = find(if phase == TransportPhase::Playing {
                    "Recording MIDI on Lead"
                } else {
                    "Disarm Lead"
                });
                assert_eq!(arm.is_disabled(), app.view.recording);
                let focus = egui::accesskit::Action::Focus;
                let click = egui::accesskit::Action::Click;
                for control in if phase == TransportPhase::Stopped {
                    vec![record, mode, arm]
                } else {
                    vec![stop]
                } {
                    assert!(control.supports_action(focus));
                    assert!(control.supports_action(click));
                }
                assert!(find("Lead, MIDI clip").supports_action(focus));
                let (stop, record, mode) = (
                    stop.bounds().unwrap(),
                    record.bounds().unwrap(),
                    mode.bounds().unwrap(),
                );
                assert!(stop.x1 <= record.x0 && record.x1 <= mode.x0);
                assert_eq!(stop.y0, record.y0);
                assert_eq!(record.y0 + record.y1, mode.y0 + mode.y1);
            }
        }
    }

    #[test]
    fn unrelated_buttons_keep_their_paint_while_commands_are_waiting() {
        let context = egui::Context::default();
        let mut app = App::new(context.clone(), PathBuf::new());
        app.view.project_open = true;
        app.view.active = true;
        let frame = |app: &App| {
            context
                .run_ui(egui::RawInput::default(), |ui| {
                    app.button(ui, Command::AddTrack);
                    app.transport_tool(ui, Command::ToggleClick, Glyph::Metronome, false);
                    app.transport_tool(ui, Command::Stop, Glyph::Stop, false);
                })
                .shapes
        };
        let idle = frame(&app);
        app.busy_since = Some(Instant::now());
        app.busy_visible = true;
        assert!(!app.enabled(Command::AddTrack));
        assert_eq!(frame(&app), idle);

        app.busy_since = None;
        app.busy_visible = false;
        app.view.transport.pending = true;
        assert!(!app.enabled(Command::ToggleClick));
        assert!(!app.enabled(Command::Stop));
        assert_eq!(frame(&app), idle);

        app.view.transport.play_pending = true;
        assert!(app.enabled(Command::Stop));
        assert!(matches!(app.play_stop_command(), Command::Stop));
    }
}
