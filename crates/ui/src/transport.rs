use crate::{
    Action, App, Command,
    arrangement::{
        ArrangementUi, Fit, Lane, LaneInput, TakeLaneInput, TakeLaneRow, clamp_height, paint_kebab,
    },
    primitives::{
        list_row::ListRow,
        scroll_zoom_bars,
        timeline::{self as timeline_ui, LOOP_HEIGHT, LoopGesture, RULER_HEIGHT, Viewport},
        transport::{self as controls, Icon},
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
                (Command::ReturnToStart, Icon::Start, false),
                (
                    Command::Play,
                    Icon::Play,
                    transport.phase != TransportPhase::Stopped,
                ),
                (Command::Stop, Icon::Stop, false),
                (
                    Command::Record,
                    Icon::Record,
                    self.view.recording || self.transport_ui.record_submitted,
                ),
            ] {
                if self.transport_tool(ui, command, glyph, on) {
                    chosen = Some(command);
                }
            }
            let mode = self.view.project.record_mode;
            ui.add_enabled_ui(
                !self.view.recording && self.confirm.is_none() && self.file.is_none(),
                |ui| {
                    let button = ui.button(format!("REC {}   ", mode.label()));
                    crate::primitives::menu::show(crate::primitives::menu::under(&button), |ui| {
                        for (mode, command) in Mode::ALL.into_iter().zip([
                            Command::RecordMerge,
                            Command::RecordReplace,
                            Command::RecordNewTake,
                        ]) {
                            if self.menu_command(
                                ui,
                                command,
                                Some(mode == self.view.project.record_mode),
                            ) {
                                chosen = Some(command);
                            }
                        }
                    });
                    let menu = egui::InnerResponse::new((), button);
                    paint_kebab(
                        ui,
                        egui::Rect::from_center_size(
                            egui::pos2(
                                menu.response.rect.right() - 9.0,
                                menu.response.rect.center().y,
                            ),
                            egui::vec2(10.0, 18.0),
                        ),
                    );
                    menu.response.on_hover_text("MIDI record mode");
                },
            );
            if self.transport_tool(ui, Command::ToggleLoop, Icon::Loop, timeline.looping) {
                chosen = Some(Command::ToggleLoop);
            }
            if !compact {
                if self.transport_tool(ui, Command::ToggleClick, Icon::Metronome, timeline.click) {
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
            controls::counter(
                ui,
                position,
                transport.phase == TransportPhase::CountingIn,
                self.view.recording && transport.phase == TransportPhase::Playing,
            );
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

    fn transport_tool(&self, ui: &mut egui::Ui, command: Command, glyph: Icon, on: bool) -> bool {
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
        self.keymap.tooltip(ui.ctx(), command)
    }

    pub fn timeline_surface(
        &mut self,
        ui: &mut egui::Ui,
        height: f32,
        width: f32,
    ) -> (Option<u64>, bool, Option<TransportAction>) {
        let start_top = ui.cursor().top();
        let header_width = if width >= 550.0 { 220.0 } else { 176.0 };
        let lane_width = (width - header_width - crate::theme::metric::SCROLL_BAR)
            .max(crate::theme::metric::SCROLL_BAR);
        let timeline = &self.view.project.timeline;
        let transport = self.view.transport;
        let locate_enabled = self.enabled(Command::ReturnToStart);
        let edit_enabled = self.enabled(Command::TempoMap);
        let clip_enabled = self.view.edit_history.available
            && self.busy_since.is_none()
            && self.confirm.is_none()
            && self.file.is_none();
        let take_enabled = self.view.project_open
            && !self.view.recording
            && !transport.pending
            && self.busy_since.is_none()
            && self.confirm.is_none()
            && self.file.is_none()
            && self.view.tracks.iter().all(|track| !track.loading);
        let mut fit = self.arrangement_ui.fit.take();
        let mut snap = self.arrangement_ui.settings.snap;
        ui.horizontal(|ui| {
            timeline_ui::snap_control(ui, &mut snap);
            for (command, glyph) in [
                (Command::FitArrangementWidth, Icon::FitWidth),
                (Command::FitArrangementHeight, Icon::FitHeight),
            ] {
                if self.transport_tool(ui, command, glyph, false) {
                    fit = Some(if command == Command::FitArrangementWidth {
                        Fit::Width
                    } else {
                        Fit::Height
                    });
                }
            }
        });
        self.arrangement_ui.settings.snap = snap;
        let mut settings = self.arrangement_ui.settings;
        let mut v_offset = self.arrangement_ui.v_offset;
        let surface_top = ui.cursor().top();
        let surface_left = ui.cursor().left();
        let lane_left = surface_left + header_width;
        let row_height = f32::from(settings.track_height);
        let master_height = crate::theme::ARRANGEMENT_TRACK_HEIGHT;
        let bar = crate::theme::metric::SCROLL_BAR;
        let tracks_height =
            (height - (surface_top - start_top) - RULER_HEIGHT - LOOP_HEIGHT - master_height - bar)
                .max(row_height);
        let instruments: Vec<_> = self
            .view
            .project
            .tracks
            .iter()
            .filter(|track| track.is_instrument())
            .collect();
        let height_preview = self.arrangement_ui.lane_height_preview;
        let midi = &self.view.project.midi;
        let take_rows = |track: &clef_model::project::Track| -> Vec<f32> {
            let count = ArrangementUi::take_count(midi, track.id);
            if !(track.take_lanes.visible && count >= 2) {
                return Vec::new();
            }
            (0..count)
                .map(|lane| {
                    height_preview
                        .filter(|(id, index, _)| *id == track.id && *index == lane)
                        .map_or_else(
                            || f32::from(track.take_lanes.height(lane)),
                            |(_, _, height)| f32::from(height),
                        )
                })
                .collect()
        };
        let content_height = |row: f32| -> f32 {
            instruments
                .iter()
                .map(|track| row + take_rows(track).iter().sum::<f32>())
                .sum::<f32>()
                .max(row)
        };
        // Wheel and trackpad over the whole surface: scroll, ⌘-wheel and ⌘⇧-wheel zoom.
        let surface = egui::Rect::from_min_size(
            egui::pos2(surface_left, surface_top),
            egui::vec2(
                width,
                RULER_HEIGHT + LOOP_HEIGHT + tracks_height + master_height + bar,
            ),
        );
        let wheel = scroll_zoom_bars::wheel(ui, surface);
        let tracks_top = surface_top + RULER_HEIGHT + LOOP_HEIGHT;
        let zoom = settings.zoom_milli as f32 / 1_000.0;
        let before =
            Viewport::with_snap(timeline, settings.offset, zoom, lane_width, settings.snap);
        if let Some(factor) = wheel.zoom_horizontal {
            let next =
                (zoom * factor).clamp(*controls::ZOOM_RANGE.start(), *controls::ZOOM_RANGE.end());
            let anchor = wheel.pointer.map_or(lane_width / 2.0, |pointer| {
                (pointer.x - lane_left).clamp(0.0, lane_width)
            });
            settings.offset = before.offset_for_zoom(next, anchor);
            settings.zoom_milli = (next * 1_000.0).round() as u32;
        }
        if wheel.scroll.x != 0.0 {
            settings.offset = before.offset_scrolled(-wheel.scroll.x);
        }
        let mut height_steps = wheel.zoom_vertical;
        let height_anchor = wheel.pointer.map_or(tracks_height / 2.0, |pointer| {
            (pointer.y - tracks_top).clamp(0.0, tracks_height)
        });
        v_offset -= wheel.scroll.y;
        // Fit buttons.
        match fit {
            Some(Fit::Width) => {
                let end = self
                    .view
                    .project
                    .midi
                    .tracks
                    .values()
                    .flatten()
                    .map(|clip| clip.end)
                    .max()
                    .unwrap_or(0)
                    .max(timeline.loop_range[1])
                    .max(timeline.bar_start(2));
                let next = (lane_width * clef_model::timeline::QUARTER as f32 / end as f32)
                    .clamp(*controls::ZOOM_RANGE.start(), *controls::ZOOM_RANGE.end());
                settings.zoom_milli = (next * 1_000.0).round() as u32;
                settings.offset = 0;
            }
            Some(Fit::Height) => {
                let takes: f32 = instruments
                    .iter()
                    .map(|track| take_rows(track).iter().sum::<f32>())
                    .sum();
                let count = instruments.len().max(1) as f32;
                let fitted = ((tracks_height - takes) / count).floor();
                settings.track_height = clamp_height(fitted);
                v_offset = 0.0;
                height_steps = 0;
            }
            None => {}
        }
        let vertical_zoom = |settings: &mut clef_model::project::ArrangementView,
                             v_offset: &mut f32,
                             steps: i32,
                             anchor: f32| {
            if steps == 0 {
                return;
            }
            let old = f32::from(settings.track_height);
            let next = clamp_height(old * 1.25_f32.powi(steps));
            settings.track_height = next;
            let ratio = f32::from(next) / old;
            *v_offset = (*v_offset + anchor) * ratio - anchor;
        };
        vertical_zoom(&mut settings, &mut v_offset, height_steps, height_anchor);
        let row_height = f32::from(settings.track_height);
        let max_v_offset = (content_height(row_height) - tracks_height).max(0.0);
        v_offset = v_offset.clamp(0.0, max_v_offset);
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
        let mut take_action = None;
        let mut take_toggle = None;
        let mut loop_gesture = self.transport_ui.loop_gesture.take();
        let mut shown_playhead = transport.audible_tick;
        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(header_width);
                add = self.button(ui, Command::AddTrack);
            });
            ui.vertical(|ui| {
                let (locate, shown) = viewport.ruler(
                    ui,
                    lane_width,
                    transport.audible_tick,
                    timeline,
                    locate_enabled,
                );
                shown_playhead = shown;
                if let Some(tick) = locate {
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
        // The scrolled track rows: header column and lane column on one row geometry.
        let (tracks, _) = ui.allocate_exact_size(
            egui::vec2(header_width + lane_width + bar, tracks_height),
            egui::Sense::hover(),
        );
        let rows_area = egui::Rect::from_min_size(
            tracks.min,
            egui::vec2(header_width + lane_width, tracks_height),
        );
        let mut focus_row = None;
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(egui::Rect::from_min_size(
                    rows_area.min - egui::vec2(0.0, v_offset),
                    egui::vec2(rows_area.width(), f32::INFINITY),
                ))
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                ui.set_clip_rect(rows_area.intersect(ui.clip_rect()));
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                if instruments.is_empty() {
                    let (row, _) = ui.allocate_exact_size(
                        egui::vec2(rows_area.width(), row_height),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(row.min, egui::vec2(header_width, row_height)),
                        0,
                        crate::theme::PANEL,
                    );
                    viewport.lane_grid(
                        ui,
                        egui::Rect::from_min_max(egui::pos2(lane_left, row.top()), row.max),
                    );
                }
                for (index, track) in instruments.iter().copied().enumerate() {
                    let state = self
                        .view
                        .state(track.id)
                        .map_or("Empty", |state| state.status.as_str());
                    let plugin = track.plugin.as_ref().map_or_else(
                        || "No instrument".to_owned(),
                        |p| format!("{} · {}", p.name, p.format),
                    );
                    let take_count = ArrangementUi::take_count(&self.view.project.midi, track.id);
                    let chip = take_count >= 2;
                    let colour = crate::theme::track_colour(track.colour());
                    let (row, _) = ui.allocate_exact_size(
                        egui::vec2(rows_area.width(), row_height),
                        egui::Sense::hover(),
                    );
                    let header =
                        egui::Rect::from_min_size(row.min, egui::vec2(header_width, row_height));
                    let rect = egui::Rect::from_min_max(egui::pos2(lane_left, row.top()), row.max);
                    ui.painter().rect_filled(header, 0, crate::theme::PANEL);
                    let mut response = None;
                    ui.scope_builder(
                        egui::UiBuilder::new()
                            .max_rect(header)
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                        |ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);
                            ui.add_space(crate::theme::metric::STRIPE + 1.0);
                            let detail = format!("{plugin} · {state}");
                            let full = format!("{}\n{detail}", track.name);
                            // Visual (and focus) order: name, take chip, arm, meter at the edge.
                            let meter_left = header.right() - crate::theme::HEADER_METER;
                            let arm_left = meter_left - 2.0 - 28.0;
                            let chip_left = if chip {
                                arm_left - 2.0 - 34.0
                            } else {
                                arm_left
                            };
                            let slot = |left: f32, right: f32| {
                                egui::UiBuilder::new()
                                    .max_rect(egui::Rect::from_x_y_ranges(
                                        left..=right,
                                        header.y_range(),
                                    ))
                                    .layout(egui::Layout::left_to_right(egui::Align::Center))
                            };
                            let name_left = ui.cursor().left();
                            let header_response = ui
                                .scope_builder(slot(name_left, chip_left - 2.0), |ui| {
                                    let mut row_widget = ListRow::new(&track.name)
                                        .accessible(&full)
                                        .height(row_height)
                                        .width((chip_left - 2.0 - name_left).max(24.0))
                                        .selected(self.view.selected == Some(track.id))
                                        .enabled(self.confirm.is_none())
                                        .header();
                                    if row_height >= crate::theme::ARRANGEMENT_TRACK_HEIGHT {
                                        row_widget = row_widget.detail(&detail);
                                    }
                                    row_widget.show(ui)
                                })
                                .inner;
                            if chip {
                                ui.scope_builder(slot(chip_left, arm_left - 2.0), |ui| {
                                    let response = crate::primitives::chip::chip(
                                        ui,
                                        &format!("T {take_count}"),
                                        &format!("Take lanes, {take_count} takes"),
                                        track.take_lanes.visible,
                                        !self.view.recording,
                                    );
                                    if response.clicked() {
                                        let mut lanes = track.take_lanes.clone();
                                        lanes.visible = !lanes.visible;
                                        take_toggle = Some((track.id, lanes));
                                    }
                                });
                            }
                            ui.scope_builder(slot(arm_left, meter_left - 2.0), |ui| {
                                let armed = self.view.armed == Some(track.id);
                                let arm_enabled = !self.view.recording
                                    && !transport.pending
                                    && self.confirm.is_none()
                                    && self.file.is_none()
                                    && self.busy_since.is_none()
                                    && (armed || track.plugin.is_some());
                                if crate::primitives::track_state::record_arm(
                                    ui,
                                    &track.name,
                                    arm_enabled,
                                    armed,
                                    armed && self.view.recording_region.is_some(),
                                    self.keyboard_mode,
                                ) {
                                    action = Some(TransportAction::Arm(track.id));
                                }
                            });
                            ui.scope_builder(slot(meter_left, header.right()), |ui| {
                                crate::primitives::mixer::header_meter(
                                    ui,
                                    row_height,
                                    &track.name,
                                    self.view
                                        .track_meters
                                        .get(&track.id)
                                        .copied()
                                        .unwrap_or_default(),
                                );
                            });
                            response = Some(header_response);
                        },
                    );
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(
                            header.min,
                            egui::vec2(crate::theme::metric::STRIPE + 1.0, row_height),
                        ),
                        0,
                        colour,
                    );
                    let response = response.expect("header row drawn");
                    viewport.lane_grid(ui, rect);
                    let lane = Lane {
                        track: track.id,
                        rect,
                        visible: rect.intersect(ui.clip_rect()),
                        colour,
                    };
                    lanes.push(lane);
                    if lane.visible.is_positive() {
                        let binding = |command: Command| {
                            (command.title(), self.keymap.text(ui.ctx(), command))
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
                                    enabled: take_enabled,
                                    playhead: transport.audible_tick,
                                },
                                binding,
                            )
                            .or(clip_command);
                    }
                    let armed = self.view.armed == Some(track.id);
                    if armed && let Some(range) = self.view.recording_region {
                        viewport.recording(
                            ui,
                            rect,
                            range,
                            timeline.loop_range,
                            self.view.recording_notes.as_deref(),
                            colour,
                        );
                    }
                    if self.track_focus == Some(track.id) {
                        response.request_focus();
                        focus_row = Some(row.y_range());
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
                        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown))
                            && index + 1 < instruments.len()
                        {
                            choose = Some(instruments[index + 1].id);
                        }
                        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp))
                            && index > 0
                        {
                            choose = Some(instruments[index - 1].id);
                        }
                    }
                    for (take_lane, height) in take_rows(track).into_iter().enumerate() {
                        let (row, _) = ui.allocate_exact_size(
                            egui::vec2(rows_area.width(), height),
                            egui::Sense::hover(),
                        );
                        let header =
                            egui::Rect::from_min_size(row.min, egui::vec2(header_width, height));
                        let band =
                            egui::Rect::from_min_max(egui::pos2(lane_left, row.top()), row.max);
                        viewport.lane_grid(ui, band);
                        let row = TakeLaneRow {
                            track: track.id,
                            lane: take_lane,
                            header,
                            band,
                        };
                        if let Some(action) = self.arrangement_ui.take_lane(
                            ui,
                            TakeLaneInput {
                                viewport: &viewport,
                                timeline,
                                arrangement: &self.view.project.midi,
                                track,
                                row,
                                enabled: take_enabled,
                            },
                        ) {
                            take_action = Some(action);
                        }
                    }
                }
            },
        );
        // A row focused from the keyboard or the details panel scrolls into view.
        if let Some(range) = focus_row {
            let top = range.min - rows_area.top() + v_offset;
            let bottom = range.max - rows_area.top() + v_offset;
            if top < v_offset {
                v_offset = top;
            } else if bottom > v_offset + tracks_height {
                v_offset = bottom - tracks_height;
            }
            ui.ctx().request_repaint();
        }
        // The vertical bar beside the rows: track scroll and track-height zoom.
        let bar_rect =
            egui::Rect::from_min_max(egui::pos2(rows_area.right(), tracks.top()), tracks.max);
        let moved = ui
            .scope_builder(egui::UiBuilder::new().max_rect(bar_rect), |ui| {
                scroll_zoom_bars::vertical(
                    ui,
                    "Arrangement tracks",
                    scroll_zoom_bars::Axis {
                        offset: v_offset,
                        visible: tracks_height,
                        total: content_height(row_height).max(tracks_height),
                    },
                    true,
                )
            })
            .inner;
        if let Some(next) = moved.offset {
            v_offset = next;
        }
        match moved.zoom {
            Some(scroll_zoom_bars::Zoom::In) => {
                vertical_zoom(&mut settings, &mut v_offset, 1, tracks_height / 2.0)
            }
            Some(scroll_zoom_bars::Zoom::Out) => {
                vertical_zoom(&mut settings, &mut v_offset, -1, tracks_height / 2.0)
            }
            None => {}
        }
        let master = ui
            .horizontal_top(|ui| {
                crate::primitives::mixer::master_header(ui, header_width, self.view.master_meter);
                let (_, lane) = ui.allocate_space(egui::vec2(lane_width, master_height));
                viewport.lane_grid(ui, lane);
                lane
            })
            .inner;
        // The one playhead: from the ruler top through every track row to the Master row.
        viewport.playhead(
            ui,
            egui::Rect::from_min_max(
                egui::pos2(lane_left, surface_top),
                egui::pos2(lane_left + lane_width, master.bottom()),
            ),
            shown_playhead,
        );
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
        ui.horizontal_top(|ui| {
            ui.add_space(header_width);
            controls::navigation(
                ui,
                "Arrangement time",
                &mut offset,
                &mut zoom,
                max_offset,
                lane_width,
                lane_width,
            );
        });
        let zoom_milli = (zoom * 1_000.0).round() as u32;
        if zoom_milli != settings.zoom_milli {
            let playhead = viewport.x(transport.audible_tick);
            let anchor = if (0.0..=lane_width).contains(&playhead) {
                playhead
            } else {
                lane_width / 2.0
            };
            offset = viewport.offset_for_zoom(zoom, anchor);
        }
        self.arrangement_ui.v_offset = v_offset;
        let next = clef_model::project::ArrangementView {
            offset: offset.min(max_offset),
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
        if let Some((track, lanes)) = take_toggle {
            if !lanes.visible {
                self.arrangement_ui.focused_lane = None;
            }
            if self.view.selected != Some(track) {
                self.select_track(track);
            }
            self.send(Action::TrackView { track, lanes });
        }
        if let Some(action) = take_action {
            self.apply_take_action(action);
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
    fn take_lanes_and_chip_render_at_both_widths() {
        use clef_model::{
            midi::{Event, Message},
            project::Track,
        };
        for width in [760.0, 1024.0] {
            let context = egui::Context::default();
            context.enable_accesskit();
            let mut app = App::new(context.clone(), PathBuf::new());
            app.view.project_open = true;
            app.view.selected = Some(1);
            app.view.project.tracks.push(Track {
                colour: None,
                id: 1,
                name: "Strings".into(),
                plugin: None,
                instrument_bypassed: false,
                instrument_offline: false,
                record_mode: Default::default(),
                mix: Default::default(),
                inserts: Vec::new(),
                take_lanes: clef_model::project::TakeLanes {
                    visible: true,
                    lanes: vec![Default::default(), Default::default()],
                },
                kind: Default::default(),
                output: None,
                sends: Vec::new(),
                state: Default::default(),
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
                            message: Message::Note {
                                channel: 0,
                                key: 60,
                                velocity: 90,
                                end: 200,
                                release: 10,
                                release_order: 1,
                            },
                        }],
                        vec![Event {
                            tick: 900,
                            order: 0,
                            message: Message::Note {
                                channel: 0,
                                key: 62,
                                velocity: 90,
                                end: 1100,
                                release: 10,
                                release_order: 2,
                            },
                        }],
                    ],
                    false,
                )
                .unwrap();
            let output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 720.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    egui::Frame::new().inner_margin(12.0).show(ui, |ui| {
                        app.main_surface(ui, &context);
                    });
                },
            );
            let tree = output.platform_output.accesskit_update.unwrap();
            let labels: Vec<String> = tree
                .nodes
                .iter()
                .filter_map(|(_, node)| {
                    node.label()
                        .or_else(|| node.value())
                        .map(|label| label.to_owned())
                })
                .collect();
            assert!(
                labels
                    .iter()
                    .any(|label| label.contains("Take lanes, 2 takes")),
                "missing take chip at {width}: {labels:?}"
            );
            assert!(
                labels.iter().any(|label| label.starts_with("Take 1,")),
                "missing first take lane at {width}: {labels:?}"
            );
            assert!(
                labels.iter().any(|label| label.starts_with("Take 2,")),
                "missing second take lane at {width}: {labels:?}"
            );
        }
    }

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
                colour: None,
                id: 1,
                name: "Lead".into(),
                plugin: Some(Plugin {
                    id: "test".into(),
                    name: "Test instrument".into(),
                    format: "VST3".into(),
                    path: "/fixture.vst3".into(),
                }),
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
                    crate::theme::TEXT
                } else {
                    crate::theme::DANGER
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
                    app.transport_tool(ui, Command::ToggleClick, Icon::Metronome, false);
                    app.transport_tool(ui, Command::Stop, Icon::Stop, false);
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
