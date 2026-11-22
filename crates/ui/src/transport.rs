use crate::{
    Action, App, Command,
    arrangement::{
        ArrangementUi, Fit, Lane, LaneInput, RenameEnd, Renaming, TakeLaneInput, TakeLaneRow,
        clamp_height, paint_kebab,
    },
    primitives::{
        list_row::ListRow,
        scroll_zoom_bars,
        search_field::SearchField,
        text_field::TextField,
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

    pub fn transport_bar(&mut self, ui: &mut egui::Ui) -> Option<Command> {
        let mut snap = self.arrangement_ui.settings.snap;
        let mut format = None;
        let chosen = self.transport_controls(ui, &mut snap, &mut format);
        self.arrangement_ui.settings.snap = snap;
        if let Some(format) = format {
            self.send(Action::SetFormat(format));
        }
        chosen
    }

    /// T41 round 2: the project's sample rate: the running rate, else the project override's,
    /// else the global preference's (0 when none is known).
    pub(crate) fn project_rate(&self) -> u32 {
        if self.view.rate > 0 {
            return self.view.rate;
        }
        self.view
            .project
            .audio
            .as_ref()
            .or(self.view.audio_preferences.as_ref())
            .map_or(0, |audio| audio.rate)
    }

    fn transport_controls(
        &self,
        ui: &mut egui::Ui,
        snap: &mut clef_model::midi::Snap,
        format: &mut Option<clef_model::project::ProjectFormat>,
    ) -> Option<Command> {
        let transport = self.view.transport;
        let timeline = &self.view.project.timeline;
        let position = timeline.position(transport.audible_tick);
        let tempo = timeline.section_at(transport.audible_tick).1;
        let mut chosen = None;
        let compact = ui.available_width() < 860.0;
        controls::band(ui, |ui| {
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
            let recording = self.view.recording && transport.phase == TransportPhase::Playing;
            controls::counter(
                ui,
                position,
                transport.phase == TransportPhase::CountingIn,
                recording,
            );
            // T41 round 2: the second counter; a click picks what it reads. At compact widths it
            // yields first, so the transport stays on one row.
            let project = self.view.project.format;
            if !compact {
                let text = controls::second_counter_text(
                    project.counter,
                    project.smpte_fps,
                    timeline.nanos(transport.audible_tick),
                    self.project_rate(),
                );
                let label = controls::second_counter_label(project.counter, project.smpte_fps);
                let display = crate::primitives::segment_display::segment_display(
                    ui, &label, &text, recording, 120.0,
                );
                let display = ui
                    .interact(display.rect, display.id.with("pick"), egui::Sense::click())
                    .on_hover_text(format!("{label} — click to choose the counter"));
                display.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        true,
                        "Choose second counter",
                    )
                });
                crate::primitives::menu::show(crate::primitives::menu::under(&display), |ui| {
                    use clef_model::project::{SMPTE_FPS, SecondCounter};
                    let choices = [SecondCounter::Time, SecondCounter::Samples]
                        .map(|counter| (counter, project.smpte_fps))
                        .into_iter()
                        .chain(SMPTE_FPS.map(|fps| (SecondCounter::Smpte, fps)));
                    for (counter, fps) in choices {
                        let ticked = project.counter == counter
                            && (counter != SecondCounter::Smpte || project.smpte_fps == fps);
                        if crate::primitives::menu::Item::new(&controls::second_counter_label(
                            counter, fps,
                        ))
                        .ticked(ticked)
                        .show(ui)
                        {
                            *format = Some(clef_model::project::ProjectFormat {
                                counter,
                                smpte_fps: fps,
                                ..project
                            });
                        }
                    }
                });
            }
            // T41 round 3 (S12): the project's sample rate over its bit depth, in segment
            // digits like the counters; a click opens Project Format.
            let rate = rate_label(self.project_rate()).unwrap_or_else(|| "— kHz".into());
            let (bits, bits_unit) = bit_digits(project.bits);
            let readout = crate::primitives::segment_display::two_row_readout(
                ui,
                "Project format",
                [
                    (&rate_digits(self.project_rate()), "kHz"),
                    (bits, bits_unit),
                ],
            );
            let tooltip = self.transport_tooltip(ui, Command::ProjectFormat);
            let readout = ui
                .interact(
                    readout.rect,
                    readout.id.with("format"),
                    egui::Sense::click(),
                )
                .on_hover_text(tooltip);
            readout.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    true,
                    format!("Project format, {rate}, {}", project.bits.label()),
                )
            });
            if readout.clicked() && self.enabled(Command::ProjectFormat) {
                chosen = Some(Command::ProjectFormat);
            }
            // Its own id: the piano roll has a Snap of its own.
            ui.push_id("transport-snap", |ui| timeline_ui::snap_control(ui, snap));
            if self.transport_button(ui, Command::TransportSettings, "Transport…", false) {
                chosen = Some(Command::TransportSettings);
            }
        });
        chosen
    }

    /// T41: the arrangement tool row under the transport. Over the track headers: the track
    /// search and the create-tracks glyph; at the right end: the fit toggles.
    pub fn tool_row(&mut self, ui: &mut egui::Ui, width: f32) -> Option<Command> {
        let mut chosen = None;
        let header = header_width(width, self.arrangement_ui.track_column);
        let mut search = std::mem::take(&mut self.arrangement_ui.track_search);
        let (row, _) = ui.allocate_exact_size(
            egui::vec2(width, crate::theme::metric::TOOL_TARGET),
            egui::Sense::hover(),
        );
        let layout = egui::Layout::left_to_right(egui::Align::Center);
        let create = crate::theme::metric::CONTROL_HEIGHT;
        let left = egui::Rect::from_min_size(row.min, egui::vec2(header, row.height()));
        let search_rect = egui::Rect::from_min_max(
            left.min,
            egui::pos2(left.right() - create - 6.0, left.bottom()),
        );
        let mut tools = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("arrangement-tools")
                .max_rect(row)
                .layout(layout),
        );
        tools.scope_builder(
            egui::UiBuilder::new().max_rect(search_rect).layout(layout),
            |ui| {
                SearchField::new(&mut search, "Find tracks")
                    .width(search_rect.width().max(48.0))
                    .show(ui);
            },
        );
        tools.scope_builder(
            egui::UiBuilder::new()
                .max_rect(egui::Rect::from_min_max(
                    egui::pos2(search_rect.right() + 6.0, left.top()),
                    left.max,
                ))
                .layout(layout),
            |ui| {
                let name = self.keymap.tooltip(ui.ctx(), Command::NewTracks);
                if crate::primitives::tool_button::create_button(
                    ui,
                    &name,
                    self.command_available(Command::NewTracks, false),
                )
                .clicked()
                    && self.enabled(Command::NewTracks)
                {
                    chosen = Some(Command::NewTracks);
                }
            },
        );
        tools.scope_builder(
            egui::UiBuilder::new()
                .max_rect(egui::Rect::from_min_max(
                    egui::pos2(left.right() + 6.0, row.top()),
                    row.max,
                ))
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                // Right to left: Fit Height ends the row, Fit Width before it.
                for (command, glyph, on) in [
                    (
                        Command::FitArrangementHeight,
                        Icon::FitHeight,
                        self.arrangement_ui.fit_height.active(),
                    ),
                    (
                        Command::FitArrangementWidth,
                        Icon::FitWidth,
                        self.arrangement_ui.fit_width.active(),
                    ),
                ] {
                    if self.transport_tool(ui, command, glyph, on) {
                        chosen = Some(command);
                    }
                }
            },
        );
        self.arrangement_ui.track_search = search;
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
    ) -> (Option<u64>, Option<TransportAction>) {
        let start_top = ui.cursor().top();
        let header_width = header_width(width, self.arrangement_ui.track_column);
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
        let fit = self.arrangement_ui.fit.take();
        let mut settings = self.arrangement_ui.settings;
        let mut v_offset = self.arrangement_ui.v_offset;
        let surface_top = ui.cursor().top();
        let surface_left = ui.cursor().left();
        let lane_left = surface_left + header_width;
        let row_height = f32::from(settings.track_height);
        let master_height = crate::theme::ARRANGEMENT_TRACK_HEIGHT;
        let bar = crate::theme::metric::SCROLL_BAR;
        // T41 round 3 (S11): the optional second ruler under the bars ruler.
        let format = self.view.project.format;
        let second_ruler = format.second_ruler;
        let rulers_height = RULER_HEIGHT
            + LOOP_HEIGHT
            + if second_ruler.is_some() {
                timeline_ui::UNIT_RULER_HEIGHT
            } else {
                0.0
            };
        let rate = self.project_rate();
        let tracks_height =
            (height - (surface_top - start_top) - rulers_height - master_height - bar)
                .max(row_height);
        let instruments: Vec<_> = self
            .view
            .project
            .tracks
            .iter()
            .filter(|track| track.is_instrument() && self.arrangement_ui.matches(&track.name))
            .collect();
        let order: Vec<u64> = instruments.iter().map(|track| track.id).collect();
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
            egui::vec2(width, rulers_height + tracks_height + master_height + bar),
        );
        let wheel = scroll_zoom_bars::wheel(ui, surface);
        let tracks_top = surface_top + rulers_height;
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
        // Fit buttons: a second press restores the view saved by the first (T41).
        let restored = match fit {
            Some(Fit::Width) => self
                .arrangement_ui
                .fit_width
                .press()
                .map(|(offset, zoom_milli)| {
                    settings.offset = offset;
                    settings.zoom_milli = zoom_milli;
                })
                .is_some(),
            Some(Fit::Height) => self
                .arrangement_ui
                .fit_height
                .press()
                .map(|(track_height, offset)| {
                    settings.track_height = track_height;
                    v_offset = offset;
                    height_steps = 0;
                })
                .is_some(),
            None => false,
        };
        match fit.filter(|_| !restored) {
            Some(Fit::Width) => {
                let before = (settings.offset, settings.zoom_milli);
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
                self.arrangement_ui
                    .fit_width
                    .fitted(before, settings.zoom_milli);
            }
            Some(Fit::Height) => {
                let before = (settings.track_height, v_offset);
                let takes: f32 = instruments
                    .iter()
                    .map(|track| take_rows(track).iter().sum::<f32>())
                    .sum();
                let count = instruments.len().max(1) as f32;
                let fitted = ((tracks_height - takes) / count).floor();
                settings.track_height = clamp_height(fitted);
                v_offset = 0.0;
                height_steps = 0;
                self.arrangement_ui
                    .fit_height
                    .fitted(before, settings.track_height);
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
        let mut clip_command = None;
        let mut lanes = Vec::new();
        let mut take_action = None;
        let mut take_toggle = None;
        let mut mix_command = None;
        let mut rename_end = None;
        let mut reorder_press = None;
        // T41 round 2: header drag-to-reorder. A press past 4 points of travel becomes a drag.
        let mut blocks: Vec<(u64, f32)> = instruments
            .iter()
            .map(|track| (track.id, row_height + take_rows(track).iter().sum::<f32>()))
            .collect();
        // A drop waiting for the session keeps its new place, so nothing jumps back.
        if let Some((track, before)) = self.arrangement_ui.dropped {
            let wanted = crate::arrangement::reordered(&blocks, track, before);
            // Cleared once the session's order matches, or once nothing is in flight (a
            // refused move falls back to the session's order).
            if wanted == blocks
                || self.actions_in_flight == 0
                || !blocks.iter().any(|(id, _)| *id == track)
            {
                self.arrangement_ui.dropped = None;
            } else {
                blocks = wanted;
            }
        }
        let (pointer_y, primary_down, escape) = ui.input(|input| {
            (
                input.pointer.interact_pos().map(|pos| pos.y),
                input.pointer.primary_down(),
                input.key_pressed(Key::Escape),
            )
        });
        let mut reorder = self.arrangement_ui.reorder.take();
        if let Some(drag) = reorder.as_mut()
            && let Some(y) = pointer_y
            && (y - drag.press).abs() > 4.0
        {
            drag.active = true;
        }
        let layout = crate::arrangement::RowLayout::new(
            &blocks,
            tracks_top - v_offset,
            reorder.filter(|drag| drag.active),
            pointer_y,
        );
        let mut reorder_drop = None;
        if let Some(drag) = reorder {
            if escape {
                ui.ctx().request_repaint();
            } else if primary_down {
                self.arrangement_ui.reorder = Some(drag);
                if drag.active {
                    ui.ctx().request_repaint();
                }
            } else if drag.active {
                reorder_drop = layout.drop_after(drag.track);
            }
        }
        let mut context = None;
        let mut headers = Vec::new();
        let mix_enabled = self.mixer_controls_enabled();
        let mut loop_gesture = self.transport_ui.loop_gesture.take();
        let mut shown_playhead = transport.audible_tick;
        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
        let mut ruler_change = None;
        ui.horizontal_top(|ui| {
            // T41 round 3 (S11): each ruler's unit label over the track headers; a click on
            // either opens the second-ruler menu.
            ui.vertical(|ui| {
                ruler_change =
                    timeline_ui::ruler_labels(ui, header_width, second_ruler, format.smpte_fps);
                ui.allocate_space(egui::vec2(header_width, LOOP_HEIGHT));
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
                if let Some(unit) = second_ruler {
                    viewport.unit_ruler(ui, lane_width, timeline, unit, format.smpte_fps, rate);
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
                // T41 round 2: each track's block sits at an animated top, so a header drag
                // slides the other tracks out of its way; the dragged block is drawn last.
                let mut order_drawn: Vec<(usize, &clef_model::project::Track)> =
                    instruments.iter().copied().enumerate().collect();
                if let Some(dragged) = layout.dragged
                    && let Some(at) = order_drawn.iter().position(|(_, t)| t.id == dragged)
                {
                    let item = order_drawn.remove(at);
                    order_drawn.push(item);
                }
                for (index, track) in order_drawn {
                    let block_top = layout.top(ui.ctx(), track.id);
                    let mut next_top = block_top;
                    let mut place = |height: f32| {
                        let rect = egui::Rect::from_min_size(
                            egui::pos2(rows_area.left(), next_top),
                            egui::vec2(rows_area.width(), height),
                        );
                        next_top += height;
                        rect
                    };
                    if layout.dragged == Some(track.id) {
                        let block = egui::Rect::from_min_size(
                            egui::pos2(rows_area.left(), block_top),
                            egui::vec2(header_width, layout.height(track.id)),
                        );
                        crate::paint::lift_shadow(ui.painter(), block);
                    }
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
                    let row = place(row_height);
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
                            // Visual (and focus) order: name, take chip, arm, mute, solo, meter at
                            // the edge (T41; Monitor waits for T14 audio tracks).
                            let meter_left = header.right() - crate::theme::HEADER_METER;
                            let lamps_left = meter_left
                                - 2.0
                                - 2.0 * (crate::primitives::mute_solo::LAMP_WIDTH + 2.0);
                            let arm_left = lamps_left - 28.0;
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
                            let name_width = (chip_left - 2.0 - name_left).max(24.0);
                            let header_response = ui
                                .scope_builder(slot(name_left, chip_left - 2.0), |ui| {
                                    if let Some(edit) = self.arrangement_ui.renaming.as_mut()
                                        && edit.track == track.id
                                    {
                                        // T41 round 2: rename in place.
                                        let id = ui.make_persistent_id(("rename", track.id));
                                        let focused = ui.memory(|memory| memory.has_focus(id));
                                        let key = |ui: &mut egui::Ui, modifiers, key| {
                                            focused
                                                && ui.input_mut(|i| i.consume_key(modifiers, key))
                                        };
                                        let end = if key(ui, Modifiers::SHIFT, Key::Tab) {
                                            Some(RenameEnd::Next(-1))
                                        } else if key(ui, Modifiers::NONE, Key::Tab) {
                                            Some(RenameEnd::Next(1))
                                        } else if key(ui, Modifiers::NONE, Key::Escape) {
                                            Some(RenameEnd::Cancel)
                                        } else {
                                            None
                                        };
                                        let field = TextField::new(&mut edit.text, "Track name")
                                            .id(id)
                                            .width(name_width - 4.0)
                                            .show(ui);
                                        if edit.focus {
                                            field.request_focus();
                                            edit.focus = false;
                                        }
                                        rename_end = end.or((!edit.focus && field.lost_focus())
                                            .then_some(RenameEnd::Commit));
                                        return field;
                                    }
                                    let mut row_widget = ListRow::new(&track.name)
                                        .accessible(&full)
                                        .height(row_height)
                                        .width(name_width)
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
                            ui.scope_builder(slot(arm_left, lamps_left - 2.0), |ui| {
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
                            ui.scope_builder(slot(lamps_left, meter_left - 2.0), |ui| {
                                use clef_model::mixer::{Control, Target};
                                for (label, command, control, lit) in [
                                    ("M", Command::MixerMute, Control::Mute, track.mix.mute),
                                    ("S", Command::MixerSolo, Control::Solo, track.mix.solo),
                                ] {
                                    let solo = control == Control::Solo;
                                    let verb = if solo { "Solo" } else { "Mute" };
                                    let mut hint = format!("{verb} {}", track.name);
                                    let keys = self.keymap.text(ui.ctx(), command);
                                    if !keys.is_empty() {
                                        hint.push_str(&format!(" ({keys})"));
                                    }
                                    let target = Target::Track(track.id);
                                    let shown = self.mixer_ui.painted(target, control, lit);
                                    let response = crate::primitives::mute_solo::lamp(
                                        ui,
                                        label,
                                        &hint,
                                        solo,
                                        shown,
                                        mix_enabled,
                                    );
                                    // Press and drag paints the state over other tracks (T41).
                                    if self
                                        .mixer_ui
                                        .paint_lamp(ui, &response, target, control, shown)
                                    {
                                        mix_command = Some((command, track.id));
                                    }
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
                    if self.view.selected == Some(track.id) {
                        crate::paint::selection_edge(ui.painter(), header);
                    }
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
                    if response.is_pointer_button_down_on()
                        && ui.input(|input| input.pointer.primary_pressed())
                        && let Some(pointer) = ui.input(|input| input.pointer.interact_pos())
                    {
                        reorder_press = Some(crate::arrangement::Reorder {
                            track: track.id,
                            press: pointer.y,
                            grab: pointer.y - block_top,
                            active: false,
                        });
                    }
                    if response.clicked() && layout.dragged.is_none() {
                        choose = Some(track.id);
                    }
                    if response.double_clicked() {
                        choose = Some(track.id);
                        self.arrangement_ui.renaming = Some(Renaming {
                            track: track.id,
                            text: track.name.clone(),
                            focus: true,
                        });
                    }
                    headers.push((track.id, response.clone()));
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
                        let row = place(height);
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
        // The vertical bar beside the rows: track scroll and track-height zoom. It runs down
        // past the Master row so its zoom buttons meet the horizontal bar's at the corner (T41).
        let bar_rect = egui::Rect::from_min_max(
            egui::pos2(rows_area.right(), tracks.top()),
            tracks.max + egui::vec2(0.0, master_height),
        );
        // A child, not a scope: the bar reaches into the Master row's height and must not
        // push the parent layout down.
        let moved = scroll_zoom_bars::vertical(
            &mut ui.new_child(egui::UiBuilder::new().max_rect(bar_rect)),
            "Arrangement tracks",
            scroll_zoom_bars::Axis {
                offset: v_offset,
                visible: tracks_height,
                total: content_height(row_height).max(tracks_height),
            },
            true,
        );
        // The rows' content and the bar are children; the layout continues under the rows.
        ui.advance_cursor_after_rect(tracks);
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
        // T41 round 3 (S9): no tracks yet, an empty state centred in the track rows.
        if self
            .view
            .project
            .tracks
            .iter()
            .all(|track| !track.is_instrument())
        {
            crate::primitives::empty_state::paint(
                ui,
                rows_area,
                crate::primitives::icons::Icon::Lanes,
                "No tracks yet.",
            );
        }
        // T41 round 3 (S10): the divider between headers and lanes resizes the track column.
        {
            let divider = egui::Rect::from_center_size(
                egui::pos2(lane_left, (surface_top + master.bottom()) / 2.0),
                egui::vec2(6.0, master.bottom() - surface_top),
            );
            let response = ui
                .interact(
                    divider,
                    ui.id().with("track-column-divider"),
                    egui::Sense::drag(),
                )
                .on_hover_cursor(egui::CursorIcon::ResizeColumn);
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Track column width")
            });
            if response.dragged() {
                let start = self.arrangement_ui.track_column.unwrap_or(header_width);
                self.arrangement_ui.track_column = Some((start + response.drag_delta().x).clamp(
                    crate::theme::metric::MINIMUM_HEADER_WIDTH,
                    crate::theme::metric::MAXIMUM_HEADER_WIDTH,
                ));
            }
            if response.hovered() || response.dragged() {
                ui.painter().vline(
                    lane_left,
                    divider.y_range(),
                    egui::Stroke::new(1.0_f32, crate::theme::ACCENT),
                );
            }
        }
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
        self.arrangement_ui.fit_width.track(zoom_milli);
        self.arrangement_ui.fit_height.track(settings.track_height);
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
            && choose.is_none()
            && self.confirm.is_none()
            && self.file.is_none()
            && !ui.input(|input| input.pointer.any_down())
        {
            self.send(Action::ArrangementView(next));
        }
        if let Some((second_ruler, smpte_fps)) = ruler_change {
            self.send(Action::SetFormat(clef_model::project::ProjectFormat {
                second_ruler,
                smpte_fps,
                ..self.view.project.format
            }));
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
        // Header context menus: the track actions that left the inspector (T41 round 2).
        for (id, response) in &headers {
            response.context_menu(|ui| {
                if self.view.selected != Some(*id) {
                    choose = Some(*id);
                }
                for command in [Command::Rename, Command::Instrument, Command::RemoveTrack] {
                    if self.menu_command(ui, command, None) {
                        context = Some((*id, command));
                        ui.close();
                    }
                }
            });
        }
        if let Some(end) = rename_end {
            self.finish_rename(end, &order);
        }
        if let Some(press) = reorder_press {
            self.arrangement_ui.reorder = Some(press);
        }
        if let Some((track, before)) = reorder_drop {
            self.arrangement_ui.dropped = Some((track, before));
            self.drop_track(track, before);
            ui.ctx().request_repaint();
        }
        if let Some((command, track)) = mix_command {
            self.run_mixer_command(command, clef_model::mixer::Target::Track(track));
        }
        if let Some((id, command)) = context {
            if self.view.selected != Some(id) {
                self.select_track(id);
            }
            self.dispatch(command, ui.ctx());
        }
        (choose, action)
    }

    /// T41 round 2: moves `track` so it sits before the track `before` (or last when `None`)
    /// in the project's track list, as one undo step.
    fn drop_track(&mut self, track: u64, before: Option<u64>) {
        let tracks = &self.view.project.tracks;
        let Some(from) = tracks.iter().position(|t| t.id == track) else {
            return;
        };
        let mut rest: Vec<u64> = tracks
            .iter()
            .map(|t| t.id)
            .filter(|&id| id != track)
            .collect();
        let to = before
            .and_then(|id| rest.iter().position(|&other| other == id))
            .unwrap_or(rest.len());
        rest.insert(to, track);
        let delta = to as isize - from as isize;
        if delta != 0 {
            self.send(Action::MoveTrack(track, delta));
        }
    }

    /// T41 round 2: ends an inline rename; Tab moves the edit to the neighbouring track in
    /// `order` (the visible header order).
    pub(crate) fn finish_rename(&mut self, end: RenameEnd, order: &[u64]) {
        let Some(edit) = self.arrangement_ui.renaming.take() else {
            return;
        };
        if end != RenameEnd::Cancel {
            let name = edit.text.trim();
            let current = self
                .view
                .project
                .tracks
                .iter()
                .find(|track| track.id == edit.track)
                .map(|track| track.name.as_str());
            if !name.is_empty() && current.is_some_and(|current| current != name) {
                self.send(Action::Rename(edit.track, name.to_owned()));
            }
        }
        if let RenameEnd::Next(step) = end
            && let Some(index) = order.iter().position(|&id| id == edit.track)
            && let Some(&next) = order.get((index as i64 + i64::from(step)) as usize)
            && let Some(track) = self.view.project.tracks.iter().find(|t| t.id == next)
        {
            self.arrangement_ui.renaming = Some(Renaming {
                track: next,
                text: track.name.clone(),
                focus: true,
            });
            self.select_track(next);
        }
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

/// The track-header column width for an arrangement `width` (T40), shared with the tool row.
/// T41 round 3 (S10): `column` is the width the user chose, kept within
/// `minimumHeaderWidth`–`maximumHeaderWidth` and leaving the lanes at least 240 points.
pub fn header_width(width: f32, column: Option<f32>) -> f32 {
    use crate::theme::metric::{MAXIMUM_HEADER_WIDTH, MINIMUM_HEADER_WIDTH};
    let wanted = column
        .unwrap_or(if width >= 550.0 { 220.0 } else { 176.0 })
        .clamp(MINIMUM_HEADER_WIDTH, MAXIMUM_HEADER_WIDTH);
    wanted.min((width - 240.0).max(MINIMUM_HEADER_WIDTH))
}

/// T41 round 3 (S12): the sample rate in kilohertz with one decimal for the segment
/// readout (`48.0`, `44.1`), dashes while audio is off.
fn rate_digits(rate: u32) -> String {
    if rate == 0 {
        return "--.-".into();
    }
    format!("{:.1}", f64::from(rate) / 1_000.0)
}

/// T41 round 3 (S12): the bit depth's digits and unit caption for the segment readout.
fn bit_digits(bits: clef_model::project::BitDepth) -> (&'static str, &'static str) {
    use clef_model::project::BitDepth;
    match bits {
        BitDepth::Int16 => ("16", "bit"),
        BitDepth::Int24 => ("24", "bit"),
        BitDepth::Float32 => ("32", "float"),
    }
}

/// T41: the transport's sample-rate readout, `None` while audio is off.
fn rate_label(rate: u32) -> Option<String> {
    (rate > 0).then(|| {
        let khz = f64::from(rate) / 1_000.0;
        if rate.is_multiple_of(1_000) {
            format!("{khz:.0} kHz")
        } else {
            format!("{khz:.1} kHz")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, time::Instant};

    #[test]
    fn the_rate_readout_shows_kilohertz_and_hides_while_audio_is_off() {
        assert_eq!(rate_label(0), None);
        assert_eq!(rate_label(48_000).as_deref(), Some("48 kHz"));
        assert_eq!(rate_label(44_100).as_deref(), Some("44.1 kHz"));
        assert_eq!(rate_label(96_000).as_deref(), Some("96 kHz"));
    }

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

    #[test]
    fn the_track_column_keeps_its_range_and_leaves_room_for_the_lanes() {
        assert_eq!(header_width(1024.0, None), 220.0);
        assert_eq!(header_width(500.0, None), 176.0);
        assert_eq!(header_width(1400.0, Some(320.0)), 320.0);
        assert_eq!(header_width(1400.0, Some(900.0)), 400.0);
        assert_eq!(header_width(1400.0, Some(20.0)), 160.0);
        // A narrow window keeps 240 points of lanes, never below the minimum column.
        assert_eq!(header_width(600.0, Some(400.0)), 360.0);
        assert_eq!(header_width(300.0, Some(400.0)), 160.0);
    }

    #[test]
    fn the_format_readout_reads_kilohertz_and_bits_as_digits() {
        use clef_model::project::BitDepth;
        assert_eq!(rate_digits(48_000), "48.0");
        assert_eq!(rate_digits(44_100), "44.1");
        assert_eq!(rate_digits(0), "--.-");
        assert_eq!(bit_digits(BitDepth::Int24), ("24", "bit"));
        assert_eq!(bit_digits(BitDepth::Float32), ("32", "float"));
    }
}
