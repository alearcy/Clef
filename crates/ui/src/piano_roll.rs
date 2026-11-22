use crate::commands::Command;
use crate::{
    Action, Snapshot,
    arrangement::Fit,
    primitives::{icons::Icon, piano_roll as draw, scroll_zoom_bars, timeline, transport},
    theme,
};
use clef_model::{
    midi::{
        Clip, ClipEdit, ClipId, Event, EventChange, EventId, Message, RollEvent, Snap,
        continuous_cc,
    },
    project::{ROW_HEIGHT, Track},
    timeline::{MAX_TICK, QUARTER, Timeline},
};
use eframe::egui::{self, Id, Key, Modifiers, Pos2, Rect, Ui};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Clone, Copy)]
enum NoteDrag {
    Move,
    Start,
    End,
}

#[derive(Clone, Copy)]
enum Gesture {
    Note(ClipId, RollEvent, NoteDrag, Pos2),
    Marquee(ClipId, Pos2, bool),
    Value(ClipId, RollEvent, bool, f32),
    Bend(ClipId, RollEvent, f32),
}

pub struct PianoRoll {
    active: Option<ClipId>,
    focus: Option<Id>,
    selected: BTreeSet<EventId>,
    gesture: Option<Gesture>,
    pending: Option<(ClipId, Vec<EventChange>)>,
    clipboard: Vec<Event>,
    snap: Snap,
    offset: u64,
    zoom: f32,
    /// Pitch-row height in points (T40, saved with the project).
    row: f32,
    /// Points scrolled down from the top of pitch 127.
    v_scroll: f32,
    /// A pitch range to centre vertically on the next frame.
    center: Option<(u8, u8)>,
    /// A clip whose notes are centred vertically on the next frame.
    reveal_pitch: Option<ClipId>,
    /// Zoom and row height have been read from the project.
    view_synced: bool,
    /// Heights measured on the previous frame, so the grid fills what is left.
    lanes_height: f32,
    functions_height: f32,
    key_cursor: HashMap<ClipId, u8>,
    follow: bool,
    strength: u8,
    transpose: i8,
    timing: u16,
    dynamics: u8,
    random: u64,
    selected_cc: u8,
    channel: u8,
    cc_lanes: BTreeSet<(u8, u8)>,
    collapsed: BTreeSet<(u8, u8)>,
    functions: bool,
    bend: i8,
    bend_source: Option<EventId>,
    split: f32,
    focus_lane: bool,
    takes: BTreeMap<ClipId, u64>,
    take_lane: Option<usize>,
    pub fit: Option<crate::arrangement::Fit>,
    /// T41: the time view (offset, zoom) before Fit Width, while the fitted zoom holds.
    fit_width: crate::arrangement::FitToggle<(u64, f32), f32>,
    /// T41: the row height and scroll before Fit Height, while the fitted height holds.
    fit_height: crate::arrangement::FitToggle<(f32, f32), f32>,
    /// The live roll shortcuts, copied from the app's key map when it changes (T18).
    pub(crate) keys: crate::commands::Keymap,
    /// A roll command chosen in the palette, run by the focused lane on the next frame.
    pub(crate) requested: std::cell::Cell<Option<Command>>,
    /// A roll lane had keyboard focus on the last frame.
    pub(crate) had_focus: bool,
    /// T41 round 3 (S2): the lower panel state the controls show, set by the app each frame.
    pub(crate) panel: crate::primitives::panel_controls::PanelState,
    /// The app is in keyboard mode (focus rings on the panel controls).
    pub(crate) keyboard: bool,
    /// A panel control pressed this frame, for the app to apply.
    pub(crate) panel_request: Option<crate::primitives::panel_controls::PanelControl>,
    /// T41 round 3 (S6): which MIDI Functions drawer sections are open (Quantize, Transpose,
    /// Humanize; window session only).
    sections: [bool; 3],
}

/// Width of the MIDI Functions drawer (S6), the inspector's width.
const DRAWER_WIDTH: f32 = theme::metric::INSPECTOR_WIDTH;
/// Gap between the drawer and the keyboard.
const DRAWER_GAP: f32 = 8.0;

impl Default for PianoRoll {
    fn default() -> Self {
        Self {
            active: None,
            focus: None,
            selected: BTreeSet::new(),
            gesture: None,
            pending: None,
            clipboard: Vec::new(),
            snap: Snap::Sixteenth,
            offset: 0,
            zoom: 96.0,
            row: f32::from(clef_model::project::DEFAULT_ROW_HEIGHT),
            v_scroll: 0.0,
            center: None,
            reveal_pitch: None,
            view_synced: false,
            lanes_height: 0.0,
            functions_height: 0.0,
            key_cursor: HashMap::new(),
            follow: false,
            strength: 100,
            transpose: 1,
            timing: 12,
            dynamics: 8,
            random: 0x7b31_9919,
            selected_cc: 1,
            channel: 1,
            cc_lanes: BTreeSet::new(),
            collapsed: BTreeSet::new(),
            functions: false,
            bend: 0,
            bend_source: None,
            split: 0.34,
            focus_lane: false,
            takes: BTreeMap::new(),
            take_lane: None,
            fit: None,
            fit_width: Default::default(),
            fit_height: Default::default(),
            keys: crate::commands::Keymap::default(),
            requested: std::cell::Cell::new(None),
            had_focus: false,
            panel: Default::default(),
            keyboard: false,
            panel_request: None,
            sections: [true, false, false],
        }
    }
}

struct LaneContext<'a> {
    time: &'a timeline::Viewport,
    timeline: &'a Timeline,
    clip: &'a Clip,
    width: f32,
    playhead: u64,
    enabled: bool,
    colour: egui::Color32,
}

impl PianoRoll {
    /// Gallery: open the MIDI Functions drawer with the given sections expanded, or close it.
    pub(crate) fn gallery_functions(&mut self, sections: Option<[bool; 3]>) {
        self.functions = sections.is_some();
        self.sections = sections.unwrap_or([true, false, false]);
    }

    pub(crate) fn gallery_state(&mut self, case: &str, clip: &Clip) {
        self.open(clip.id(), clip);
        if matches!(case, "selection" | "snap-preview" | "unavailable") {
            self.functions = case == "unavailable";
            if let Some(event) = clip
                .roll_events([clip.start, clip.end])
                .into_iter()
                .find(|event| matches!(event.source.message, Message::Note { .. }))
            {
                self.selected.insert(event.id);
                if case == "snap-preview" {
                    self.pending = Some((
                        clip.id(),
                        vec![EventChange::Replace(
                            event.id,
                            note_shift(
                                event.source,
                                clip.roll_source_bounds(),
                                QUARTER as i64 / 2,
                                2,
                                NoteDrag::Move,
                            )
                            .unwrap(),
                        )],
                    ));
                }
            }
        } else if matches!(case, "linear-cc" | "curved-cc" | "switch-cc") {
            self.cc_lanes
                .insert((1, if case == "switch-cc" { 64 } else { 11 }));
        }
    }

    pub fn complete(
        &mut self,
        view: &Snapshot,
        completed: bool,
        changed: bool,
        midi_changed: bool,
    ) {
        if changed {
            *self = Self::default();
        } else if completed && let Some((clip, changes)) = self.pending.take() {
            if midi_changed
                && self.active == Some(clip)
                && let Some((_, clip)) = view.project.midi.find_clip(clip)
            {
                let expected = changes
                    .iter()
                    .filter_map(|change| match change {
                        EventChange::Replace(_, event) => Some(event),
                        _ => None,
                    })
                    .copied()
                    .collect::<std::collections::HashSet<_>>();
                self.selected = clip
                    .roll_events([clip.start, clip.end])
                    .into_iter()
                    .filter(|event| expected.contains(&event.source))
                    .map(|event| event.id)
                    .collect();
            }
        } else if midi_changed && self.pending.is_none() {
            self.selected.clear();
            self.gesture = None;
        }
    }

    pub fn open(&mut self, id: ClipId, clip: &Clip) {
        self.activate(id);
        self.reveal_pitch = Some(id);
        self.offset = clip.start;
        self.focus_lane = true;
        self.takes.clear();
        self.take_lane = None;
    }

    pub fn open_take_lane(&mut self, clips: &[(ClipId, u64)], lane: usize) {
        self.takes = clips.iter().copied().collect();
        self.take_lane = Some(lane);
        if let Some(&(clip, _)) = clips.first() {
            self.activate(clip);
        }
        self.focus_lane = true;
    }

    pub fn close_take(&mut self) {
        self.takes.clear();
        self.take_lane = None;
    }

    pub fn reveal(&mut self, tick: u64) {
        self.offset = tick;
    }

    fn clip_events(&self, clip: &Clip, visible: [u64; 2]) -> Vec<RollEvent> {
        self.takes.get(&clip.id()).map_or_else(
            || clip.roll_events(visible),
            |take| clip.roll_take_events(*take, visible),
        )
    }

    pub fn split_height(&self, height: f32) -> f32 {
        height * self.split
    }

    pub fn gutter(&mut self, ui: &mut Ui, height: f32) {
        draw::resize_gutter(ui, height, &mut self.split);
    }

    pub fn cancel(&mut self, ui: &Ui) {
        let focused = self
            .focus
            .is_some_and(|id| ui.memory(|memory| memory.has_focus(id)));
        if (self.gesture.is_some() || focused)
            && ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape))
            && self.gesture.take().is_none()
        {
            self.selected.clear();
        }
    }

    fn activate(&mut self, clip: ClipId) {
        if self.active != Some(clip) {
            self.active = Some(clip);
            self.selected.clear();
            self.gesture = None;
        }
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        height: f32,
        view: &Snapshot,
        selected: &BTreeSet<ClipId>,
        enabled: bool,
    ) -> Option<Action> {
        let action = self.show_lanes(ui, height, view, selected, enabled);
        // A palette request no focused lane took this frame is dropped.
        self.requested.set(None);
        action
    }

    /// True when `command`'s shortcut was pressed, or the palette asked for it.
    fn fired(&self, ui: &Ui, command: Command) -> bool {
        if self.requested.get() == Some(command) {
            self.requested.set(None);
            return true;
        }
        ui.input_mut(|input| self.keys.consume(input, command))
    }

    fn show_lanes(
        &mut self,
        ui: &mut Ui,
        height: f32,
        view: &Snapshot,
        selected: &BTreeSet<ClipId>,
        enabled: bool,
    ) -> Option<Action> {
        self.had_focus = false;
        self.cancel(ui);
        let start_top = ui.cursor().top();
        let timeline = &view.project.timeline;
        let arrangement = &view.project.midi;
        let playhead = view.transport.audible_tick;
        if !self.view_synced {
            let saved = view.project.piano_roll_view;
            self.zoom = saved.zoom_milli as f32 / 1_000.0;
            self.row = f32::from(saved.row_height);
            self.view_synced = true;
        }
        let mut clips = selected
            .iter()
            .filter_map(|&id| arrangement.find_clip(id))
            .collect::<Vec<_>>();
        clips.sort_by_key(|(track, clip)| {
            (
                view.project
                    .tracks
                    .iter()
                    .position(|item| item.id == *track),
                clip.start,
                clip.layer(),
            )
        });
        if !self.takes.is_empty() {
            let current: BTreeSet<ClipId> = clips.iter().map(|(_, clip)| clip.id()).collect();
            let missing = self.takes.iter().any(|(id, take)| {
                clips
                    .iter()
                    .find(|(_, clip)| clip.id() == *id)
                    .is_none_or(|(_, clip)| !clip.takes.iter().any(|item| item.id == *take))
            });
            if current != self.takes.keys().copied().collect() || missing {
                self.close_take();
            }
        }
        if let Some((_, first)) = clips.first()
            && self.active.is_none_or(|id| !selected.contains(&id))
        {
            self.activate(first.id());
            self.reveal_pitch = Some(first.id());
            self.offset = first.start;
        }
        let mut action = None;
        let mut fit = self.fit.take();
        // T41 round 3: no title (S3); Snap, Follow and MIDI Functions at the left, the fit
        // toggles right-aligned (S5) just left of the panel controls (S2).
        let (row, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), theme::TOOL_TARGET),
            egui::Sense::hover(),
        );
        let controls_left = row.right() - crate::primitives::panel_controls::WIDTH - 12.0;
        let fits_width = 2.0 * theme::TOOL_TARGET + 4.0;
        let fits = Rect::from_min_max(
            egui::pos2(controls_left - fits_width, row.top()),
            egui::pos2(controls_left, row.bottom()),
        );
        let tools = Rect::from_min_max(row.min, egui::pos2(fits.left() - 8.0, row.bottom()));
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(tools)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                if let Some(lane) = self.take_lane {
                    let name = clips
                        .first()
                        .and_then(|(track, _)| {
                            view.project.tracks.iter().find(|item| item.id == *track)
                        })
                        .map(|track| track.take_lanes.name(lane))
                        .unwrap_or_else(|| format!("Take {}", lane + 1));
                    ui.colored_label(
                        theme::ACCENT,
                        format!(
                            "Editing Take {} · {name} · {} clips",
                            lane + 1,
                            self.takes.len()
                        ),
                    );
                    if draw::button(ui, "Back to Comp", true) {
                        self.close_take();
                    }
                }
                timeline::snap_control(ui, &mut self.snap);
                draw::toggle(ui, "Follow", &mut self.follow);
                draw::toggle(ui, "MIDI Functions", &mut self.functions);
            },
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(fits)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for (glyph, name, kind, on) in [
                    (
                        Icon::FitWidth,
                        "Fit Piano Roll to Width",
                        Fit::Width,
                        self.fit_width.active(),
                    ),
                    (
                        Icon::FitHeight,
                        "Fit Piano Roll to Height",
                        Fit::Height,
                        self.fit_height.active(),
                    ),
                ] {
                    if transport::tool_button(ui, glyph, name, true, on, false).clicked() {
                        fit = Some(kind);
                    }
                }
            },
        );
        if let Some(control) =
            crate::primitives::panel_controls::at_right(ui, row, self.panel, self.keyboard)
        {
            if control == crate::primitives::panel_controls::PanelControl::Close {
                self.gesture = None;
            }
            self.panel_request = Some(control);
        }
        let names = lane_names(&view.project.tracks, timeline, &clips);
        if clips.len() > 1 {
            ui.horizontal_wrapped(|ui| {
                for (index, &(_, clip)) in clips.iter().enumerate() {
                    let focused = self.active == Some(clip.id());
                    if draw::summary(ui, &names[index], focused, clip.cycle().is_some()) {
                        self.activate(clip.id());
                        self.reveal_pitch = Some(clip.id());
                        self.offset = clip.start;
                    }
                }
            });
        }
        let Some(&(active_track, active)) = clips
            .iter()
            .find(|(_, clip)| Some(clip.id()) == self.active)
        else {
            crate::primitives::empty_state::fill(
                ui,
                120.0,
                Icon::Keys,
                "Select a MIDI clip in the arrangement.",
            );
            return None;
        };
        // T41 round 3 (S6): the MIDI Functions drawer, left of the keyboard over the whole
        // editor height; the ruler, grid and lanes take the rest.
        let mut body = None;
        if self.functions {
            let top = ui.cursor().top();
            let area = Rect::from_min_size(
                egui::pos2(ui.cursor().left(), top),
                egui::vec2(
                    ui.available_width(),
                    (height - (top - start_top)).max(theme::roll::LANE_MINIMUM_HEIGHT),
                ),
            );
            ui.allocate_rect(area, egui::Sense::hover());
            let drawer = Rect::from_min_size(area.min, egui::vec2(DRAWER_WIDTH, area.height()));
            let lane = LaneContext {
                time: &timeline::Viewport::with_snap(
                    timeline,
                    self.offset,
                    self.zoom,
                    1.0,
                    self.snap,
                ),
                timeline,
                clip: active,
                width: 1.0,
                playhead,
                enabled,
                colour: theme::ACCENT,
            };
            action = self
                .functions_drawer(ui, drawer, &lane, view)
                .or(action.take());
            body = Some(
                ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(Rect::from_min_max(
                            egui::pos2(drawer.right() + DRAWER_GAP, area.top()),
                            area.max,
                        ))
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                ),
            );
        }
        let ui = match body.as_mut() {
            Some(body) => body,
            None => ui,
        };
        let bar = theme::metric::SCROLL_BAR;
        let width = (ui.available_width() - theme::ROLL_KEYBED - bar).max(theme::ROLL_KEYBED);
        let colour_of = |track: u64| {
            theme::track_colour(
                view.project
                    .tracks
                    .iter()
                    .find(|item| item.id == track)
                    .map_or(clef_model::project::TRACK_COLOURS[0], |item| item.colour()),
            )
        };
        if self.follow && view.transport.phase != clef_session::TransportPhase::Stopped {
            let time =
                timeline::Viewport::with_snap(timeline, self.offset, self.zoom, width, self.snap);
            if !(0.0..width).contains(&time.x(playhead)) {
                self.offset = playhead;
            }
        }
        let column_top = ui.cursor().top();
        let grid_left = ui.cursor().left() + theme::ROLL_KEYBED;
        // Horizontal zoom, scroll and fit happen before the time transform is built.
        let remaining = height - (column_top - start_top);
        let fixed = theme::roll::RULER_HEIGHT
            + if self.take_lane.is_some() { 3.0 } else { 0.0 }
            + self.functions_height
            + self.lanes_height
            + bar;
        let grid_height = (remaining - fixed)
            .max(self.row * 6.0)
            .max(theme::roll::LANE_MINIMUM_HEIGHT);
        let grid_top = column_top
            + theme::roll::RULER_HEIGHT
            + if self.take_lane.is_some() { 3.0 } else { 0.0 }
            + self.functions_height;
        let pitch_rect = Rect::from_min_size(
            egui::pos2(grid_left - theme::ROLL_KEYBED, grid_top),
            egui::vec2(theme::ROLL_KEYBED + width, grid_height),
        );
        let wheel = scroll_zoom_bars::wheel(ui, pitch_rect);
        let before =
            timeline::Viewport::with_snap(timeline, self.offset, self.zoom, width, self.snap);
        if let Some(factor) = wheel.zoom_horizontal {
            let next = (self.zoom * factor)
                .clamp(*transport::ZOOM_RANGE.start(), *transport::ZOOM_RANGE.end());
            let anchor = wheel.pointer.map_or(width / 2.0, |pointer| {
                (pointer.x - grid_left).clamp(0.0, width)
            });
            self.offset = before.offset_for_zoom(next, anchor);
            self.zoom = next;
        }
        if wheel.scroll.x != 0.0 {
            self.offset = before.offset_scrolled(-wheel.scroll.x);
        }
        self.v_scroll -= wheel.scroll.y;
        let anchor = wheel.pointer.map_or(grid_height / 2.0, |pointer| {
            (pointer.y - grid_top).clamp(0.0, grid_height)
        });
        self.zoom_rows(wheel.zoom_vertical, anchor);
        let range = |clip: &Clip| {
            clip.roll_events([clip.start, clip.end])
                .into_iter()
                .filter_map(|event| match event.source.message {
                    Message::Note { key, .. } => Some(key),
                    _ => None,
                })
                .fold(None, |range: Option<(u8, u8)>, key| {
                    Some(range.map_or((key, key), |(low, high)| (low.min(key), high.max(key))))
                })
        };
        // A second press restores the view saved by the first (T41).
        let restored = match fit {
            Some(Fit::Width) => self
                .fit_width
                .press()
                .map(|(offset, zoom)| {
                    self.offset = offset;
                    self.zoom = zoom;
                })
                .is_some(),
            Some(Fit::Height) => self
                .fit_height
                .press()
                .map(|(row, scroll)| {
                    self.row = row;
                    self.v_scroll = scroll;
                })
                .is_some(),
            None => false,
        };
        match fit.filter(|_| !restored) {
            Some(Fit::Width) => {
                let before = (self.offset, self.zoom);
                let start = clips.iter().map(|(_, clip)| clip.start).min().unwrap_or(0);
                let end = clips
                    .iter()
                    .map(|(_, clip)| clip.end)
                    .max()
                    .unwrap_or(start + 1);
                self.zoom = (width * QUARTER as f32 / (end - start).max(1) as f32)
                    .clamp(*transport::ZOOM_RANGE.start(), *transport::ZOOM_RANGE.end());
                self.offset = start;
                self.fit_width.fitted(before, self.zoom);
            }
            Some(Fit::Height) => {
                let before = (self.row, self.v_scroll);
                let (low, high) = clips
                    .iter()
                    .filter_map(|(_, clip)| range(clip))
                    .fold((60, 60), |(low, high), (a, b)| (low.min(a), high.max(b)));
                let rows = f32::from(high - low) + 3.0;
                self.row = (grid_height / rows)
                    .floor()
                    .clamp(f32::from(*ROW_HEIGHT.start()), f32::from(*ROW_HEIGHT.end()));
                self.center = Some((low, high));
                self.fit_height.fitted(before, self.row);
            }
            None => {}
        }
        if let Some(id) = self.reveal_pitch.take()
            && let Some((_, clip)) = clips.iter().find(|(_, clip)| clip.id() == id)
        {
            self.center = Some(range(clip).unwrap_or((60, 60)));
        }
        let centred = self.center.take();
        if let Some((low, high)) = centred {
            let middle = (f32::from(127 - high) + f32::from(127 - low) + 1.0) / 2.0;
            self.v_scroll = middle * self.row - grid_height / 2.0;
        }
        let content = 128.0 * self.row;
        self.v_scroll = self.v_scroll.clamp(0.0, (content - grid_height).max(0.0));
        let right = clips
            .iter()
            .map(|(_, clip)| clip.end)
            .max()
            .unwrap_or(playhead)
            .max(playhead);
        let max_offset = right.max(self.offset);
        self.offset = self.offset.min(max_offset);
        let time =
            timeline::Viewport::with_snap(timeline, self.offset, self.zoom, width, self.snap);
        ui.spacing_mut().item_spacing.y = 0.0;
        let shown = draw::ruler(ui, &time, width, playhead, timeline);
        if let (Some(_), Some(take)) = (self.take_lane, self.takes.get(&active.id()).copied()) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(theme::ROLL_KEYBED);
                draw::comp_strip(ui, &time, width, &active.take_active_ranges(take));
            });
        }
        let active_context = LaneContext {
            time: &time,
            timeline,
            clip: active,
            width,
            playhead,
            enabled,
            colour: colour_of(active_track),
        };
        let functions_height = 0.0;
        let others = clips
            .iter()
            .filter(|(_, clip)| clip.id() != active.id())
            .map(|&(track, clip)| (clip, colour_of(track)))
            .collect::<Vec<_>>();
        let (pitch_action, grid) = ui
            .horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let result = self.pitch_editor(ui, &active_context, view, grid_height, &others);
                let moved = ui
                    .allocate_ui(egui::vec2(bar, grid_height), |ui| {
                        scroll_zoom_bars::vertical(
                            ui,
                            "Piano roll pitch",
                            scroll_zoom_bars::Axis {
                                offset: self.v_scroll,
                                visible: grid_height,
                                total: content,
                            },
                            true,
                        )
                    })
                    .inner;
                if let Some(offset) = moved.offset {
                    self.v_scroll = offset;
                }
                match moved.zoom {
                    Some(scroll_zoom_bars::Zoom::In) => self.zoom_rows(1, grid_height / 2.0),
                    Some(scroll_zoom_bars::Zoom::Out) => self.zoom_rows(-1, grid_height / 2.0),
                    None => {}
                }
                result
            })
            .inner;
        action = pitch_action.or(action.take());
        let lanes_top = ui.cursor().top();
        let lane = &active_context;
        action = self.values(ui, lane, None, view).or(action.take());
        for (channel, cc) in self.cc_lanes.clone() {
            ui.push_id((channel, cc), |ui| {
                let name = format!("{} · Ch {channel}", draw::cc_name(cc));
                ui.horizontal_wrapped(|ui| {
                    let mut collapsed = self.collapsed.contains(&(channel, cc));
                    draw::toggle(ui, &name, &mut collapsed);
                    if collapsed {
                        self.collapsed.insert((channel, cc));
                    } else {
                        self.collapsed.remove(&(channel, cc));
                    }
                    if draw::button(ui, "Remove Lane", true) {
                        self.cc_lanes.remove(&(channel, cc));
                        self.collapsed.remove(&(channel, cc));
                    }
                });
                if !self.collapsed.contains(&(channel, cc)) {
                    action = self
                        .values(ui, lane, Some((channel, cc)), view)
                        .or(action.take());
                }
            });
        }
        let lanes_bottom = ui.cursor().top();
        ui.horizontal_wrapped(|ui| {
            draw::cc_picker(ui, &mut self.selected_cc);
            draw::number(ui, "Channel", &mut self.channel, 1..=16);
            if draw::button(ui, "Add Lane", true) {
                self.cc_lanes.insert((self.channel, self.selected_cc));
            }
        });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(theme::ROLL_KEYBED);
            transport::navigation(
                ui,
                "Piano roll time",
                &mut self.offset,
                &mut self.zoom,
                max_offset,
                width,
                width,
            );
        });
        let measured = ui.cursor().top() - lanes_top - bar;
        if (measured - self.lanes_height).abs() > 0.5
            || (functions_height - self.functions_height).abs() > 0.5
        {
            self.lanes_height = measured;
            self.functions_height = functions_height;
            // The grid height changes with the measurement: centre again on the next frame.
            self.center = self.center.or(centred);
            ui.ctx().request_repaint();
        }
        // The one playhead: from the ruler through the grid and the controller lanes.
        time.playhead(
            ui,
            Rect::from_x_y_ranges(grid.x_range(), column_top..=lanes_bottom),
            shown,
        );
        self.fit_width.track(self.zoom);
        self.fit_height.track(self.row);
        let saved = clef_model::project::PianoRollView {
            zoom_milli: (self.zoom * 1_000.0).round() as u32,
            row_height: self.row.round() as u8,
        };
        if action.is_none()
            && saved != view.project.piano_roll_view
            && saved.validate().is_ok()
            && !ui.input(|input| input.pointer.any_down())
        {
            action = Some(Action::PianoRollView(saved));
        }
        action
    }

    /// Steps the pitch-row height ×1.25 per step, keeping the row under `anchor` in place.
    fn zoom_rows(&mut self, steps: i32, anchor: f32) {
        if steps == 0 {
            return;
        }
        let old = self.row;
        self.row = (old * 1.25_f32.powi(steps))
            .round()
            .clamp(f32::from(*ROW_HEIGHT.start()), f32::from(*ROW_HEIGHT.end()));
        self.v_scroll = (self.v_scroll + anchor) * self.row / old - anchor;
    }

    /// Allocates the keybed and the one grid, paints every selected clip (the active one in
    /// front) and edits the active clip. Returns the action and the grid rectangle.
    fn pitch_editor(
        &mut self,
        ui: &mut Ui,
        lane: &LaneContext<'_>,
        view: &Snapshot,
        height: f32,
        others: &[(&Clip, egui::Color32)],
    ) -> (Option<Action>, Rect) {
        let (grid, keyboard, pointer) = draw::pitch_area(
            ui,
            lane.time,
            [lane.width, height],
            [self.row, self.v_scroll],
            lane.enabled,
        );
        grid.paint_grid(ui);
        let window = [
            lane.time.tick_at(0.0),
            lane.time.tick_at(lane.width).saturating_add(1),
        ];
        let mut inactive = Vec::new();
        for &(clip, colour) in others {
            for event in self.clip_events(clip, window) {
                let Message::Note { key, .. } = event.source.message else {
                    continue;
                };
                let area = grid.note(event.tick, event.end, key);
                if area.intersects(grid.grid) {
                    draw::paint_note(
                        ui,
                        grid.grid,
                        area,
                        draw::NoteStyle {
                            colour: colour.gamma_multiply(0.45),
                            selected: false,
                            active: event.active,
                            primary: false,
                            preview: false,
                        },
                    );
                    inactive.push((clip.id(), area));
                }
            }
        }
        // A click on another clip's note makes that clip the edit target.
        if lane.enabled
            && pointer.clicked()
            && let Some(pos) = pointer.interact_pointer_pos()
            && let Some(&(clip, _)) = inactive.iter().rev().find(|(_, area)| area.contains(pos))
        {
            let on_active = self.clip_events(lane.clip, window).iter().any(|event| {
                matches!(event.source.message, Message::Note { key, .. }
                    if grid.note(event.tick, event.end, key).contains(pos))
            });
            if !on_active {
                self.activate(clip);
                self.focus_lane = true;
                return (None, grid.grid);
            }
        }
        let action = self.notes(ui, lane, view, &grid, keyboard, pointer);
        (action, grid.grid)
    }

    fn hit_selection(&mut self, clip: ClipId, event: Option<EventId>, additive: bool) {
        self.activate(clip);
        if !additive {
            self.selected.clear();
        }
        if let Some(id) = event
            && !self.selected.insert(id)
            && additive
        {
            self.selected.remove(&id);
        }
    }

    fn edit(
        &mut self,
        view: &Snapshot,
        lane: &LaneContext<'_>,
        label: &'static str,
        changes: Vec<EventChange>,
    ) -> Option<Action> {
        if !lane.enabled || changes.is_empty() || self.pending.is_some() {
            return None;
        }
        let clip = lane.clip.id();
        self.pending = Some((clip, changes.clone()));
        Some(Action::EditClips {
            expected: view.project.midi.clone(),
            edit: ClipEdit::EditEvents {
                take: self.takes.get(&clip).copied(),
                clip,
                label,
                changes,
            },
        })
    }

    fn notes(
        &mut self,
        ui: &mut Ui,
        lane: &LaneContext<'_>,
        view: &Snapshot,
        grid: &draw::PitchGrid<'_>,
        keyboard: egui::Response,
        pointer: egui::Response,
    ) -> Option<Action> {
        let clip = lane.clip;
        let id = clip.id();
        if self.focus_lane && self.active == Some(id) {
            pointer.request_focus();
            self.focus_lane = false;
        }
        if (pointer.has_focus() || keyboard.has_focus()) && self.active != Some(id) {
            self.activate(id);
        }
        grid.paint_bounds(ui, [clip.start, clip.end]);
        let window = [
            lane.time.tick_at(0.0),
            lane.time.tick_at(lane.width).saturating_add(1),
        ];
        draw::repeat_markers(ui, grid.grid, lane.time, clip, window);
        let events = self.clip_events(clip, window);
        let notes = events
            .iter()
            .filter_map(|event| {
                let Message::Note { key, .. } = event.source.message else {
                    return None;
                };
                let area = grid.note(event.tick, event.end, key);
                area.intersects(grid.grid).then_some((*event, area))
            })
            .collect::<Vec<_>>();
        let primary = self.selected.first().copied();
        let mut focused_note = None;
        for &(event, area) in &notes {
            let selected = self.active == Some(id) && self.selected.contains(&event.id);
            draw::paint_note(
                ui,
                grid.grid,
                area,
                draw::NoteStyle {
                    colour: lane.colour,
                    selected,
                    active: event.active,
                    primary: selected && primary == Some(event.id),
                    preview: false,
                },
            );
            if let Some(focus) = draw::note_focus(
                ui,
                area.intersect(grid.grid),
                lane.clip.id(),
                &event,
                selected,
                lane.enabled,
            ) {
                focused_note = Some((focus, event));
            }
        }
        let clip_channel = events
            .iter()
            .find_map(|event| match event.source.message {
                Message::Note { channel, .. } => Some(channel),
                _ => None,
            })
            .unwrap_or(self.channel - 1);
        let mut action = None;
        if lane.enabled
            && keyboard.clicked()
            && let Some(pos) = keyboard.interact_pointer_pos()
        {
            keyboard.request_focus();
            let key = grid.keybed_key(pos);
            self.key_cursor.insert(id, key);
            action = Some(Action::Audition {
                clip: id,
                channel: clip_channel,
                key,
                velocity: 96,
            });
        }
        let mut cursor = None;
        if keyboard.has_focus() {
            let visible = grid.visible();
            if self
                .key_cursor
                .get(&id)
                .is_none_or(|key| !visible.contains(key))
            {
                self.key_cursor.insert(id, grid.key(grid.grid.center().y));
            }
            let mut key = self.key_cursor.get(&id).copied().unwrap_or(60);
            self.had_focus = true;
            if self.fired(ui, Command::RollUp) {
                key = (key + 1).min(127);
            }
            if self.fired(ui, Command::RollDown) {
                key = key.saturating_sub(1);
            }
            self.key_cursor.insert(id, key);
            cursor = Some(key);
            if lane.enabled
                && (self.fired(ui, Command::RollEnter)
                    || ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Space)))
            {
                action = Some(Action::Audition {
                    clip: id,
                    channel: clip_channel,
                    key,
                    velocity: 96,
                });
            }
        }
        grid.paint_keybed(ui, cursor);
        if !lane.enabled {
            self.gesture = None;
            return action;
        }
        if let Some((focus, event)) = focused_note {
            if self.focus != Some(focus) {
                self.hit_selection(id, Some(event.id), false);
                self.focus = Some(focus);
            }
            self.had_focus = true;
            if self.fired(ui, Command::RollEnter)
                && let Message::Note {
                    channel,
                    key,
                    velocity,
                    ..
                } = event.source.message
            {
                return Some(Action::Audition {
                    clip: id,
                    channel,
                    key,
                    velocity,
                });
            }
            return self.shortcuts(ui, lane, view, false, None);
        }
        let hit = |pos: Pos2| {
            notes
                .iter()
                .rev()
                .find(|(_, rect)| rect.contains(pos))
                .copied()
        };
        let duration_tick = |pos: Pos2| lane.time.tick_at(pos.x - grid.grid.left());
        let additive = ui.input(|input| input.modifiers.command || input.modifiers.shift);
        let free = ui.input(|input| input.modifiers.alt);
        let snap = if free { Snap::Off } else { self.snap };
        if pointer.clicked() || pointer.secondary_clicked() {
            self.activate(id);
            self.focus = Some(pointer.id);
            pointer.request_focus();
            if let Some(pos) = pointer.interact_pointer_pos() {
                let event = hit(pos).map(|(event, _)| event);
                if pointer.secondary_clicked()
                    && let Some(event) = event
                {
                    return self.edit(
                        view,
                        lane,
                        "Delete Note",
                        vec![EventChange::Remove(event.id)],
                    );
                }
                if pointer.double_clicked() && event.is_none() {
                    let raw = duration_tick(pos);
                    clip.roll_source_tick(raw)?;
                    let tick = snap
                        .nearest(lane.timeline, raw)
                        .clamp(clip.start, clip.end - 1);
                    return self.add_note(view, lane, tick, grid.key(pos.y), snap);
                }
                self.hit_selection(id, event.map(|event| event.id), additive);
                if let Some(RollEvent {
                    source:
                        Event {
                            message:
                                Message::Note {
                                    channel,
                                    key,
                                    velocity,
                                    ..
                                },
                            ..
                        },
                    ..
                }) = event
                {
                    return Some(Action::Audition {
                        clip: id,
                        channel,
                        key,
                        velocity,
                    });
                }
            }
        }
        if pointer.drag_started()
            && let Some(pos) = ui.input(|input| input.pointer.press_origin())
        {
            self.activate(id);
            pointer.request_focus();
            self.focus = Some(pointer.id);
            self.gesture = Some(if let Some((event, area)) = hit(pos) {
                if !self.selected.contains(&event.id) {
                    self.hit_selection(id, Some(event.id), additive);
                }
                let mode = if area.width() >= theme::ROLL_HANDLE * 3.0
                    && pos.x - area.left() <= theme::ROLL_HANDLE
                {
                    NoteDrag::Start
                } else if area.width() >= theme::ROLL_HANDLE * 3.0
                    && area.right() - pos.x <= theme::ROLL_HANDLE
                {
                    NoteDrag::End
                } else {
                    NoteDrag::Move
                };
                Gesture::Note(id, event, mode, pos)
            } else {
                Gesture::Marquee(id, pos, additive)
            });
        }
        if let Some(pos) = ui.input(|input| input.pointer.interact_pos()) {
            match self.gesture {
                Some(Gesture::Marquee(target, origin, additive)) if target == id => {
                    let area = Rect::from_two_pos(origin, pos).intersect(grid.grid);
                    timeline::marquee(ui, grid.grid, area);
                    if pointer.drag_stopped() {
                        if !additive {
                            self.selected.clear();
                        }
                        self.selected.extend(
                            notes
                                .iter()
                                .filter(|(_, note)| area.intersects(*note))
                                .map(|(event, _)| event.id),
                        );
                        self.gesture = None;
                    }
                }
                Some(Gesture::Note(target, anchor, mode, origin)) if target == id => {
                    let relative = duration_tick(pos) as i64 - duration_tick(origin) as i64;
                    let project_tick = match mode {
                        NoteDrag::End => anchor.end,
                        _ => anchor.tick,
                    };
                    let target_tick = if relative == 0 {
                        project_tick
                    } else {
                        snap.nearest(
                            lane.timeline,
                            project_tick.saturating_add_signed(relative).min(MAX_TICK),
                        )
                    };
                    let delta = target_tick as i64 - project_tick as i64;
                    let pitch = i16::from(grid.key(pos.y)) - i16::from(grid.key(origin.y));
                    let changes = self.transform(clip, |event| {
                        note_shift(event.source, clip.roll_source_bounds(), delta, pitch, mode)
                    });
                    draw::paint_marker(ui, grid.grid, lane.time, target_tick, theme::ACCENT);
                    self.preview_notes(ui, grid, &notes, &changes, clip.end);
                    if pointer.drag_stopped() {
                        self.gesture = None;
                        return self.edit(
                            view,
                            lane,
                            if matches!(mode, NoteDrag::Move) {
                                "Move Notes"
                            } else {
                                "Resize Notes"
                            },
                            changes,
                        );
                    }
                }
                _ => {}
            }
        }
        if let Some((target, changes)) = &self.pending
            && *target == id
        {
            self.preview_notes(ui, grid, &notes, changes, lane.clip.end);
        }
        if pointer.has_focus() {
            return self.shortcuts(ui, lane, view, false, None).or(action);
        }
        action
    }

    fn preview_notes(
        &self,
        ui: &Ui,
        grid: &draw::PitchGrid<'_>,
        notes: &[(RollEvent, Rect)],
        changes: &[EventChange],
        clip_end: u64,
    ) {
        for change in changes {
            if let EventChange::Replace(id, event) = *change
                && let Message::Note { key, end, .. } = event.message
            {
                for &(copy, _) in notes.iter().filter(|(copy, _)| copy.id == id) {
                    let shift = event.tick as i64 - copy.source.tick as i64;
                    let tick = copy.tick.saturating_add_signed(shift);
                    let end = (tick + end - event.tick).min(clip_end);
                    draw::paint_note(
                        ui,
                        grid.grid,
                        grid.note(tick, end, key),
                        draw::NoteStyle {
                            colour: theme::ACCENT,
                            selected: true,
                            active: true,
                            primary: false,
                            preview: true,
                        },
                    );
                }
            }
        }
    }

    fn add_note(
        &mut self,
        view: &Snapshot,
        lane: &LaneContext<'_>,
        project_tick: u64,
        key: u8,
        snap: Snap,
    ) -> Option<Action> {
        let tick = lane.clip.roll_source_tick(project_tick)?;
        let length = snap.step(lane.timeline, project_tick).max(QUARTER / 8);
        self.edit(
            view,
            lane,
            "Add Note",
            vec![EventChange::Insert(Event {
                tick,
                order: 0,
                message: Message::Note {
                    channel: self.channel - 1,
                    key,
                    velocity: 96,
                    end: tick + length,
                    release: 0,
                    release_order: 0,
                },
            })],
        )
    }

    fn insert_point(
        &self,
        ui: &Ui,
        lane: &LaneContext<'_>,
        events: &[RollEvent],
    ) -> Option<(u64, u8)> {
        if !self.fired(ui, Command::RollEnter) {
            return None;
        }
        let tick = lane
            .clip
            .roll_source_tick(lane.playhead.clamp(lane.clip.start, lane.clip.end - 1))?;
        let value = events
            .iter()
            .rev()
            .find(|event| event.tick <= tick)
            .map_or(64, |event| value(event.source));
        Some((tick, value))
    }

    fn picked(&self, clip: &Clip) -> Vec<RollEvent> {
        let mut seen = BTreeSet::new();
        self.clip_events(clip, [clip.start, clip.end])
            .into_iter()
            .filter(|event| self.selected.contains(&event.id) && seen.insert(event.id))
            .collect()
    }

    fn transform(
        &self,
        clip: &Clip,
        mut map: impl FnMut(RollEvent) -> Option<Event>,
    ) -> Vec<EventChange> {
        self.picked(clip)
            .into_iter()
            .filter_map(|event| map(event).map(|updated| EventChange::Replace(event.id, updated)))
            .collect()
    }

    fn shortcuts(
        &mut self,
        ui: &Ui,
        lane: &LaneContext<'_>,
        view: &Snapshot,
        is_value: bool,
        controller: Option<(u8, u8)>,
    ) -> Option<Action> {
        self.had_focus = true;
        let clip = lane.clip;
        let on_lane = |event: &RollEvent| {
            if let Some((channel, cc)) = controller {
                event
                    .source
                    .message
                    .control()
                    .is_some_and(|bytes| bytes[0] == (0xb0 | (channel - 1)) && bytes[1] == cc)
            } else {
                matches!(event.source.message, Message::Note { .. })
            }
        };
        if self.fired(ui, Command::RollSelectAll) {
            self.selected = self
                .clip_events(clip, [clip.start, clip.end])
                .into_iter()
                .filter(on_lane)
                .map(|event| event.id)
                .collect();
        }
        if self.fired(ui, Command::RollDelete) {
            let changes = self
                .picked(clip)
                .into_iter()
                .filter(on_lane)
                .map(|event| EventChange::Remove(event.id))
                .collect();
            return self.edit(view, lane, "Delete MIDI Events", changes);
        }
        // ⌘C and ⌘V arrive as Copy and Paste events while they keep their default keys.
        let default = |command: Command| self.keys.shortcut(command) == command.spec().default;
        if self.fired(ui, Command::RollCopy)
            || default(Command::RollCopy) && take_clipboard(ui, false)
        {
            self.clipboard = self
                .picked(clip)
                .into_iter()
                .filter(on_lane)
                .map(|event| event.source)
                .collect();
        }
        let paste = self.fired(ui, Command::RollPaste)
            || default(Command::RollPaste) && take_clipboard(ui, true);
        if paste || self.fired(ui, Command::RollDuplicate) {
            let events = if paste {
                self.clipboard.clone()
            } else {
                self.picked(clip)
                    .into_iter()
                    .filter(on_lane)
                    .map(|event| event.source)
                    .collect()
            };
            let origin = events.iter().map(|event| event.tick).min()?;
            let target = if paste {
                clip.roll_source_tick(lane.playhead.clamp(clip.start, clip.end - 1))?
            } else {
                events
                    .iter()
                    .map(|event| match event.message {
                        Message::Note { end, .. } => end,
                        _ => event.tick + 1,
                    })
                    .max()?
            };
            let bounds = clip.roll_source_bounds();
            let changes = events
                .into_iter()
                .filter_map(|event| {
                    let tick = target + event.tick - origin;
                    let shift = target as i64 - origin as i64;
                    (bounds[0]..bounds[1]).contains(&tick).then(|| {
                        let updated = note_shift(event, bounds, shift, 0, NoteDrag::Move)
                            .unwrap_or(Event { tick, ..event });
                        EventChange::Insert(updated)
                    })
                })
                .collect();
            return self.edit(
                view,
                lane,
                if paste {
                    "Paste MIDI Events"
                } else {
                    "Duplicate MIDI Events"
                },
                changes,
            );
        }
        if !is_value && self.fired(ui, Command::RollEnter) {
            let pitch = self.key_cursor.get(&clip.id()).copied().unwrap_or(60);
            let snap = if ui.input(|input| input.modifiers.alt) {
                Snap::Off
            } else {
                self.snap
            };
            return self.add_note(
                view,
                lane,
                lane.playhead.clamp(clip.start, clip.end - 1),
                pitch,
                snap,
            );
        }
        for (step, resize, direction) in [
            (Command::RollEarlier, Some(Command::RollShorten), -1),
            (Command::RollLater, Some(Command::RollLengthen), 1),
            (Command::RollDown, None, -1),
            (Command::RollUp, None, 1),
        ] {
            let horizontal = resize.is_some();
            let resize = !is_value && resize.is_some_and(|command| self.fired(ui, command));
            if !resize && !self.fired(ui, step) {
                continue;
            }
            let snap = if ui.input(|input| input.modifiers.alt) {
                Snap::Off
            } else {
                self.snap
            };
            let step = snap.step(lane.timeline, lane.playhead) as i64;
            let delta = if horizontal { step * direction } else { 0 };
            let bounds = clip.roll_source_bounds();
            let changes = self.transform(clip, |event| {
                if !on_lane(&event) {
                    return None;
                }
                if is_value && !horizontal {
                    return Some(change_value(
                        event.source,
                        value(event.source).saturating_add_signed(direction as i8),
                    ));
                }
                if controller.is_some() {
                    return Some(Event {
                        tick: event
                            .source
                            .tick
                            .saturating_add_signed(delta)
                            .clamp(bounds[0], bounds[1] - 1),
                        ..event.source
                    });
                }
                if is_value && horizontal {
                    return None;
                }
                let pitch = if horizontal { 0 } else { direction as i16 };
                note_shift(
                    event.source,
                    bounds,
                    delta,
                    pitch,
                    if resize {
                        NoteDrag::End
                    } else {
                        NoteDrag::Move
                    },
                )
            });
            return self.edit(view, lane, "Edit MIDI Events", changes);
        }
        None
    }

    /// T41 round 3 (S6): the MIDI Functions drawer, its sections stacked as accordions.
    fn functions_drawer(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        lane: &LaneContext<'_>,
        view: &Snapshot,
    ) -> Option<Action> {
        ui.painter().rect_filled(rect, 0, theme::PANEL);
        let mut drawer = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect.shrink2(egui::vec2(8.0, 8.0)))
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        egui::ScrollArea::vertical()
            .id_salt("midi-functions")
            .auto_shrink([false, false])
            .show(&mut drawer, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                self.midi_functions(ui, lane, view)
            })
            .inner
    }

    fn midi_functions(
        &mut self,
        ui: &mut Ui,
        lane: &LaneContext<'_>,
        view: &Snapshot,
    ) -> Option<Action> {
        use crate::primitives::accordion::section;
        let mut action = None;
        let notes = self
            .picked(lane.clip)
            .into_iter()
            .filter(|event| matches!(event.source.message, Message::Note { .. }))
            .count();
        let enabled = lane.enabled && notes > 0;
        let bounds = lane.clip.roll_source_bounds();
        let mut sections = self.sections;
        section(ui, "Quantize", &mut sections[0], |ui| {
            draw::number(ui, "Strength %", &mut self.strength, 0..=100);
            if draw::button(
                ui,
                "Quantize",
                enabled && self.snap != Snap::Off && self.strength > 0,
            ) {
                let strength = self.strength;
                let changes = self.transform(lane.clip, |event| {
                    let delta = quantize_shift(lane.timeline, self.snap, event.tick, strength);
                    note_shift(event.source, bounds, delta, 0, NoteDrag::Move)
                });
                action = self.edit(view, lane, "Quantize Notes", changes);
            }
        });
        section(ui, "Transpose", &mut sections[1], |ui| {
            draw::number(ui, "Semitones", &mut self.transpose, -127..=127);
            if draw::button(ui, "Transpose", enabled && self.transpose != 0) {
                let changes = self.transform(lane.clip, |event| {
                    note_shift(
                        event.source,
                        bounds,
                        0,
                        i16::from(self.transpose),
                        NoteDrag::Move,
                    )
                });
                action = self.edit(view, lane, "Transpose Notes", changes);
            }
        });
        section(ui, "Humanize", &mut sections[2], |ui| {
            draw::number(ui, "Timing (ticks)", &mut self.timing, 0..=960);
            draw::number(ui, "Velocity deviation", &mut self.dynamics, 0..=126);
            if draw::button(
                ui,
                "Humanize",
                enabled && (self.timing > 0 || self.dynamics > 0),
            ) {
                let mut random = self.random;
                let changes = self.transform(lane.clip, |event| {
                    let delta = jitter(&mut random, i64::from(self.timing));
                    let event = note_shift(event.source, bounds, delta, 0, NoteDrag::Move)?;
                    let velocity =
                        i64::from(value(event)) + jitter(&mut random, i64::from(self.dynamics));
                    Some(change_value(event, velocity.clamp(1, 127) as u8))
                });
                self.random = random;
                action = self.edit(view, lane, "Humanize Notes", changes);
            }
        });
        self.sections = sections;
        action
    }

    fn values(
        &mut self,
        ui: &mut Ui,
        lane: &LaneContext<'_>,
        controller: Option<(u8, u8)>,
        view: &Snapshot,
    ) -> Option<Action> {
        let active = lane.clip.id();
        let name = controller.map_or_else(|| "Velocity".into(), |(_, cc)| draw::cc_name(cc));
        let (rect, response) = draw::value_lane(ui, &name, lane.width, lane.enabled);
        let events =
            self.clip_events(lane.clip, [lane.clip.start, lane.clip.end])
                .into_iter()
                .filter(|event| {
                    if let Some((channel, cc)) = controller {
                        event.source.message.control().is_some_and(|bytes| {
                            bytes[0] == (0xb0 | (channel - 1)) && bytes[1] == cc
                        })
                    } else {
                        matches!(event.source.message, Message::Note { .. })
                    }
                })
                .collect::<Vec<_>>();
        let points = |position: Pos2| {
            events
                .iter()
                .rev()
                .find(|event| {
                    let point = draw::point(rect, lane.time, event.tick, value(event.source));
                    (point.x - position.x).abs() <= theme::ROLL_HIT
                        && (controller.is_none() || (point.y - position.y).abs() <= theme::ROLL_HIT)
                })
                .copied()
        };
        let cc = controller.map(|(_, cc)| cc);
        let continuous = cc.is_some_and(continuous_cc);
        if !lane.enabled {
            draw::paint_values(
                ui,
                rect,
                lane.time,
                &events,
                &self.selected,
                cc,
                (lane.clip, lane.colour),
            );
            return None;
        }
        let mut value_action = None;
        if continuous {
            let primary = events
                .iter()
                .find(|event| self.selected.contains(&event.id))
                .copied();
            if let Some(event) = primary
                && self.bend_source != Some(event.id)
            {
                self.bend = match event.source.message {
                    Message::CurvedControl { bend, .. } => bend,
                    _ => 0,
                };
                self.bend_source = Some(event.id);
            }
            let previous = self.bend;
            ui.horizontal_wrapped(|ui| {
                draw::number(ui, "Curvature", &mut self.bend, -100..=100);
                let reset = draw::button(
                    ui,
                    "Linear",
                    events.iter().any(|event| self.selected.contains(&event.id)),
                );
                let apply = draw::button(
                    ui,
                    "Apply Curvature",
                    events.iter().any(|event| self.selected.contains(&event.id)),
                );
                if reset || apply {
                    let bend = if reset { 0 } else { self.bend };
                    self.bend = bend;
                    self.bend_source = None;
                    let changes = events
                        .iter()
                        .filter(|event| self.selected.contains(&event.id))
                        .map(|event| EventChange::Replace(event.id, bend_event(event.source, bend)))
                        .collect::<Vec<_>>();
                    let changes = unique_replacements(changes);
                    value_action = self.edit(view, lane, "Change CC Curvature", changes);
                }
            });
            if self.bend != previous {
                ui.ctx().request_repaint();
            }
        }
        let snap = if ui.input(|input| input.modifiers.alt) {
            Snap::Off
        } else {
            self.snap
        };
        let time_at = |x| snap.nearest(lane.timeline, lane.time.tick_at(x - rect.left()));
        if response.drag_started()
            && let Some(position) = ui.input(|input| input.pointer.press_origin())
        {
            response.request_focus();
            self.focus = Some(response.id);
            self.gesture = if let Some(event) = points(position) {
                self.hit_selection(active, Some(event.id), false);
                Some(Gesture::Value(
                    active,
                    event,
                    controller.is_some(),
                    position.y,
                ))
            } else if continuous {
                events.windows(2).find_map(|pair| {
                    let [from, to] = [pair[0], pair[1]];
                    let value = from.source.message.interpolated(to.source.message, 0.5)?;
                    let center = draw::point(rect, lane.time, from.tick.midpoint(to.tick), value);
                    (center.distance(position) < theme::ROLL_HIT)
                        .then_some(Gesture::Bend(active, from, position.y))
                })
            } else {
                None
            };
        }
        let mut preview = self
            .pending
            .as_ref()
            .filter(|(id, _)| *id == active)
            .map(|(_, changes)| changes.clone())
            .unwrap_or_default();
        if let Some(position) = ui.input(|input| input.pointer.interact_pos()) {
            let updated = match self.gesture {
                Some(Gesture::Value(id, event, is_cc, origin))
                    if id == active && is_cc == controller.is_some() =>
                {
                    let mut updated = change_value(
                        event.source,
                        draw::velocity_dragged(rect, value(event.source), origin, position.y),
                    );
                    if is_cc {
                        let [from, until] = lane.clip.roll_source_bounds();
                        let delta = time_at(position.x) as i64 - event.tick as i64;
                        updated.tick = updated
                            .tick
                            .saturating_add_signed(delta)
                            .clamp(from, until - 1);
                    }
                    Some((
                        if is_cc {
                            "Move CC Point"
                        } else {
                            "Change Velocity"
                        },
                        EventChange::Replace(event.id, updated),
                    ))
                }
                Some(Gesture::Bend(id, event, origin)) if id == active && continuous => {
                    let original = match event.source.message {
                        Message::CurvedControl { bend, .. } => i16::from(bend),
                        _ => 0,
                    };
                    let bend =
                        (original + ((position.y - origin) * 3.0) as i16).clamp(-100, 100) as i8;
                    Some((
                        "Change CC Curvature",
                        EventChange::Replace(event.id, bend_event(event.source, bend)),
                    ))
                }
                _ => None,
            };
            if let Some((label, change)) = updated
                && (response.dragged() || response.drag_stopped())
            {
                preview = vec![change];
                if let EventChange::Replace(id, event) = change
                    && let Message::CurvedControl { bend, .. } = event.message
                {
                    self.bend = bend;
                    self.bend_source = Some(id);
                }
                if response.drag_stopped() {
                    self.gesture = None;
                    value_action = self.edit(view, lane, label, preview.clone());
                }
            }
        }
        if preview.is_empty() {
            draw::paint_values(
                ui,
                rect,
                lane.time,
                &events,
                &self.selected,
                cc,
                (lane.clip, lane.colour),
            );
        } else {
            let mut updated = events.clone();
            for change in preview {
                if let EventChange::Replace(id, event) = change {
                    for item in updated.iter_mut().filter(|item| item.id == id) {
                        item.tick = item
                            .tick
                            .saturating_add_signed(event.tick as i64 - item.source.tick as i64);
                        item.source = event;
                    }
                }
            }
            updated.sort_by_key(|event| (event.tick, event.source.order));
            draw::paint_values(
                ui,
                rect,
                lane.time,
                &updated,
                &self.selected,
                cc,
                (lane.clip, lane.colour),
            );
        }
        let focused_point = draw::point_focus(
            ui,
            draw::ValueFocus {
                rect,
                clip: lane.clip.id(),
                time: lane.time,
                timeline: lane.timeline,
                events: &events,
                selection: &self.selected,
                cc,
                enabled: lane.enabled,
                name: &name,
            },
        );
        if let Some((event, focus)) = focused_point
            && self.focus != Some(focus)
        {
            self.hit_selection(active, Some(event), false);
            self.focus = Some(focus);
        }
        if self.gesture.is_some() {
            ui.ctx().request_repaint();
        }
        if (response.clicked() || response.secondary_clicked())
            && let Some(position) = response.interact_pointer_pos()
        {
            response.request_focus();
            self.focus = Some(response.id);
            let event = points(position);
            self.hit_selection(
                active,
                event.map(|event| event.id),
                ui.input(|input| input.modifiers.command),
            );
            if response.secondary_clicked() {
                return event.and_then(|event| {
                    controller.and_then(|_| {
                        self.edit(
                            view,
                            lane,
                            "Delete CC Point",
                            vec![EventChange::Remove(event.id)],
                        )
                    })
                });
            }
            if let Some((channel, cc)) = controller
                && event.is_none()
            {
                let tick = lane.clip.roll_source_tick(
                    time_at(position.x).clamp(lane.clip.start, lane.clip.end - 1),
                )?;
                let bytes = [
                    0xb0 | (channel - 1),
                    cc,
                    draw::velocity_at(rect, position.y),
                ];
                let message = if continuous {
                    Message::CurvedControl { bytes, bend: 0 }
                } else {
                    Message::Control(bytes)
                };
                return self.edit(
                    view,
                    lane,
                    "Add CC Point",
                    vec![EventChange::Insert(Event {
                        tick,
                        order: 0,
                        message,
                    })],
                );
            }
        }
        if let Some((_, focus)) = focused_point
            && self.focus == Some(focus)
        {
            value_action = self
                .shortcuts(ui, lane, view, true, controller)
                .or(value_action);
        }
        if response.has_focus()
            && let Some((channel, cc)) = controller
            && let Some((tick, insert)) = self.insert_point(ui, lane, &events)
        {
            let bytes = [0xb0 | (channel - 1), cc, insert];
            let message = if continuous {
                Message::CurvedControl { bytes, bend: 0 }
            } else {
                Message::Control(bytes)
            };
            value_action = self.edit(
                view,
                lane,
                "Add CC Point",
                vec![EventChange::Insert(Event {
                    tick,
                    order: 0,
                    message,
                })],
            );
        }
        value_action
    }
}

fn take_clipboard(ui: &Ui, paste: bool) -> bool {
    ui.input_mut(|input| {
        let mut found = false;
        input.events.retain(|event| {
            let matching = if paste {
                matches!(event, egui::Event::Paste(_))
            } else {
                matches!(event, egui::Event::Copy)
            };
            found |= matching;
            !matching
        });
        found
    })
}

fn note_shift(
    mut event: Event,
    bounds: [u64; 2],
    ticks: i64,
    pitch: i16,
    mode: NoteDrag,
) -> Option<Event> {
    let Message::Note { key, end, .. } = &mut event.message else {
        return None;
    };
    match mode {
        NoteDrag::Move => {
            let length = (*end - event.tick).min(bounds[1] - bounds[0]);
            if ticks != 0 {
                event.tick = event
                    .tick
                    .saturating_add_signed(ticks)
                    .clamp(bounds[0], bounds[1] - length);
                *end = event.tick + length;
            }
            *key = (i16::from(*key) + pitch).clamp(0, 127) as u8;
        }
        NoteDrag::Start => {
            event.tick = event
                .tick
                .saturating_add_signed(ticks)
                .clamp(bounds[0], (*end).min(bounds[1]) - 1)
        }
        NoteDrag::End => {
            if *end > bounds[1] {
                *end = bounds[1];
            }
            *end = end
                .saturating_add_signed(ticks)
                .clamp(event.tick + 1, bounds[1])
        }
    }
    Some(event)
}

fn value(event: Event) -> u8 {
    match event.message {
        Message::Note { velocity, .. } => velocity,
        message => message.control().unwrap()[2],
    }
}

fn change_value(mut event: Event, value: u8) -> Event {
    match &mut event.message {
        Message::Note { velocity, .. } => *velocity = value.clamp(1, 127),
        Message::Control(bytes) | Message::CurvedControl { bytes, .. } => bytes[2] = value.min(127),
    }
    event
}

fn bend_event(event: Event, bend: i8) -> Event {
    Event {
        message: Message::CurvedControl {
            bytes: event.message.control().unwrap(),
            bend,
        },
        ..event
    }
}

fn unique_replacements(mut changes: Vec<EventChange>) -> Vec<EventChange> {
    let mut seen = BTreeSet::new();
    changes.retain(|change| matches!(change, EventChange::Replace(id, _) if seen.insert(*id)));
    changes
}

fn lane_names(tracks: &[Track], timeline: &Timeline, clips: &[(u64, &Clip)]) -> Vec<String> {
    let base = clips
        .iter()
        .map(|(track, clip)| {
            let track_name = tracks
                .iter()
                .find(|item| item.id == *track)
                .map_or("MIDI", |item| item.name.as_str());
            let position = timeline.position(clip.start);
            format!("{track_name} · {}.{}", position.bar, position.beat)
        })
        .collect::<Vec<_>>();
    base.iter()
        .enumerate()
        .map(|(index, name)| {
            if base.iter().filter(|other| *other == name).count() > 1 {
                let ordinal = base[..index].iter().filter(|other| *other == name).count();
                format!("{name} · {}", ordinal + 1)
            } else {
                name.clone()
            }
        })
        .collect()
}

fn quantize_shift(timeline: &Timeline, snap: Snap, tick: u64, strength: u8) -> i64 {
    let target = snap.nearest(timeline, tick);
    let distance = target as i64 - tick as i64;
    (distance * i64::from(strength) + distance.signum() * 50) / 100
}

fn jitter(state: &mut u64, amount: i64) -> i64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state % (amount as u64 * 2 + 1)) as i64 - amount
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event as InputEvent, PointerButton, RawInput};

    #[test]
    fn pointer_selection_targets_the_owning_clip_for_keyboard_edit_and_audition() {
        let mut view = Snapshot::default();
        let model = std::sync::Arc::make_mut(&mut view.project.midi);
        let source = Event {
            tick: 120,
            order: 1,
            message: Message::Note {
                channel: 2,
                key: 60,
                velocity: 83,
                end: 480,
                release: 0,
                release_order: 2,
            },
        };
        model
            .record(
                1,
                clef_model::midi::Mode::Merge,
                [0, 960],
                vec![vec![source]],
                false,
            )
            .unwrap();
        let clip = view.project.midi.clips(1).first().unwrap();
        let clip_id = clip.id();
        let mut roll = PianoRoll::default();
        roll.open(clip_id, clip);
        roll.row = 9.0;
        roll.v_scroll = 576.0;
        let context = egui::Context::default();
        crate::theme::install_fonts(&context);
        let timeline = &view.project.timeline;
        let time = timeline::Viewport::with_snap(timeline, 0, 96.0, 480.0, Snap::Sixteenth);
        let lane = LaneContext {
            time: &time,
            timeline,
            clip,
            width: 480.0,
            playhead: 160,
            enabled: true,
            colour: theme::PEACOCK,
        };
        let mut point = Pos2::ZERO;
        let mut run = |roll: &mut PianoRoll, events, modifiers| {
            let mut action = None;
            let _ = context.run_ui(
                RawInput {
                    events,
                    modifiers,
                    ..Default::default()
                },
                |ui| {
                    let origin = ui.next_widget_position();
                    point = egui::pos2(
                        origin.x + theme::ROLL_KEYBED + time.x(240),
                        origin.y + theme::ROLL_LANE_HEIGHT * (1.0 - 12.5 / 16.0),
                    );
                    action = roll
                        .pitch_editor(ui, &lane, &view, theme::ROLL_LANE_HEIGHT, &[])
                        .0;
                },
            );
            (action, point)
        };
        let (_, point) = run(&mut roll, Vec::new(), Modifiers::NONE);
        run(
            &mut roll,
            vec![InputEvent::PointerMoved(point)],
            Modifiers::NONE,
        );
        for pressed in [true, false] {
            let (action, _) = run(
                &mut roll,
                vec![InputEvent::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                }],
                Modifiers::NONE,
            );
            if !pressed {
                assert!(
                    matches!(action, Some(Action::Audition { clip, channel: 2, key: 60, velocity: 83 }) if clip == clip_id)
                );
            }
        }
        let (action, _) = run(
            &mut roll,
            vec![InputEvent::Key {
                key: Key::ArrowRight,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
            Modifiers::NONE,
        );
        let Some(Action::EditClips {
            edit: ClipEdit::EditEvents { clip, changes, .. },
            ..
        }) = action
        else {
            panic!("keyboard move did not dispatch");
        };
        assert_eq!(clip, clip_id);
        assert!(matches!(
            changes.as_slice(),
            [EventChange::Replace(
                _,
                Event {
                    tick: 360,
                    message: Message::Note { end: 720, .. },
                    ..
                }
            )]
        ));
        roll.pending = None;
        let (action, _) = run(
            &mut roll,
            vec![InputEvent::Key {
                key: Key::ArrowRight,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::ALT,
            }],
            Modifiers::ALT,
        );
        let Some(Action::EditClips {
            edit: ClipEdit::EditEvents { changes, .. },
            ..
        }) = action
        else {
            panic!("Option bypass move did not dispatch");
        };
        assert!(matches!(
            changes.as_slice(),
            [EventChange::Replace(_, Event { tick: 121, .. })]
        ));
    }

    #[test]
    fn the_keybed_auditions_the_focused_pitch_from_the_keyboard() {
        let mut view = Snapshot::default();
        let model = std::sync::Arc::make_mut(&mut view.project.midi);
        model
            .record(
                1,
                clef_model::midi::Mode::Merge,
                [0, 960],
                vec![vec![Event {
                    tick: 120,
                    order: 1,
                    message: Message::Note {
                        channel: 2,
                        key: 60,
                        velocity: 83,
                        end: 480,
                        release: 0,
                        release_order: 2,
                    },
                }]],
                false,
            )
            .unwrap();
        let clip = view.project.midi.clips(1).first().unwrap();
        let clip_id = clip.id();
        let mut roll = PianoRoll::default();
        roll.open(clip_id, clip);
        roll.row = 9.0;
        roll.v_scroll = 576.0;
        let context = egui::Context::default();
        crate::theme::install_fonts(&context);
        let timeline = &view.project.timeline;
        let time = timeline::Viewport::with_snap(timeline, 0, 96.0, 480.0, Snap::Sixteenth);
        let lane = LaneContext {
            time: &time,
            timeline,
            clip,
            width: 480.0,
            playhead: 160,
            enabled: true,
            colour: theme::PEACOCK,
        };
        let run = |roll: &mut PianoRoll, events, modifiers| {
            let mut action = None;
            let mut origin = Pos2::ZERO;
            let _ = context.run_ui(
                RawInput {
                    events,
                    modifiers,
                    ..Default::default()
                },
                |ui| {
                    origin = ui.next_widget_position();
                    action = roll
                        .pitch_editor(ui, &lane, &view, theme::ROLL_LANE_HEIGHT, &[])
                        .0;
                },
            );
            (action, origin)
        };
        let (_, origin) = run(&mut roll, Vec::new(), Modifiers::NONE);
        let keys = egui::pos2(origin.x + theme::ROLL_KEYBED * 0.5, origin.y + 9.0 * 7.5);
        run(
            &mut roll,
            vec![InputEvent::PointerMoved(keys)],
            Modifiers::NONE,
        );
        for pressed in [true, false] {
            run(
                &mut roll,
                vec![InputEvent::PointerButton {
                    pos: keys,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                }],
                Modifiers::NONE,
            );
        }
        let enter = InputEvent::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let (action, _) = run(&mut roll, vec![enter.clone()], Modifiers::NONE);
        assert!(
            matches!(action, Some(Action::Audition { clip, channel: 2, key: 56, velocity: 96 }) if clip == clip_id),
            "Enter auditions the keybed cursor on the clip channel"
        );
        let up = InputEvent::Key {
            key: Key::ArrowUp,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let (action, _) = run(&mut roll, vec![up, enter], Modifiers::NONE);
        assert!(
            matches!(action, Some(Action::Audition { key: 57, .. })),
            "arrows move the keybed cursor"
        );
    }

    #[test]
    fn the_value_lane_inserts_a_point_at_the_playhead_from_the_keyboard() {
        let mut view = Snapshot::default();
        let model = std::sync::Arc::make_mut(&mut view.project.midi);
        model
            .record(
                1,
                clef_model::midi::Mode::Merge,
                [0, 960],
                vec![vec![Event {
                    tick: 0,
                    order: 0,
                    message: Message::CurvedControl {
                        bytes: [0xb0, 11, 40],
                        bend: 0,
                    },
                }]],
                false,
            )
            .unwrap();
        let clip = view.project.midi.clips(1).first().unwrap();
        let timeline = &view.project.timeline;
        let time = timeline::Viewport::with_snap(timeline, 0, 96.0, 480.0, Snap::Sixteenth);
        let lane = LaneContext {
            time: &time,
            timeline,
            clip,
            width: 480.0,
            playhead: 240,
            enabled: true,
            colour: theme::PEACOCK,
        };
        let events = clip
            .roll_events([clip.start, clip.end])
            .into_iter()
            .filter(|event| matches!(event.source.message, Message::CurvedControl { .. }))
            .collect::<Vec<_>>();
        let roll = PianoRoll::default();
        let context = egui::Context::default();
        crate::theme::install_fonts(&context);
        let mut inserted = None;
        let _ = context.run_ui(
            RawInput {
                events: vec![InputEvent::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| {
                inserted = roll.insert_point(ui, &lane, &events);
            },
        );
        assert_eq!(
            inserted,
            Some((240, 40)),
            "Enter adds a point at the playhead holding the value in force"
        );
    }

    #[test]
    fn co_located_lanes_get_distinct_names_and_labels() {
        let mut view = Snapshot::default();
        view.project.tracks.push(Track {
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
        view.project.tracks.push(Track {
            colour: None,
            id: 2,
            name: "Piano".into(),
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
        let model = std::sync::Arc::make_mut(&mut view.project.midi);
        let note = |tick, key| Event {
            tick,
            order: 0,
            message: Message::Note {
                channel: 0,
                key,
                velocity: 96,
                end: tick + 120,
                release: 0,
                release_order: 1,
            },
        };
        model
            .record(
                1,
                clef_model::midi::Mode::Merge,
                [0, 480],
                vec![vec![note(0, 60)]],
                false,
            )
            .unwrap();
        model
            .record(
                2,
                clef_model::midi::Mode::Merge,
                [960, 1_440],
                vec![vec![note(960, 64)]],
                false,
            )
            .unwrap();
        let upper = model.clips(2)[0].id();
        model
            .edit(
                &ClipEdit::Move {
                    destinations: vec![(upper, 1)],
                    delta: -960,
                },
                [1, 2].into_iter(),
            )
            .unwrap();
        let clips = model
            .clips(1)
            .iter()
            .map(|clip| (1, clip))
            .collect::<Vec<_>>();
        assert_eq!(clips.len(), 2);
        let names = lane_names(&view.project.tracks, &Timeline::default(), &clips);
        assert_eq!(names.len(), 2);
        assert_ne!(names[0], names[1], "co-located lanes need distinct names");
        assert!(names[0].starts_with("Strings · 1.1"));
    }

    #[test]
    fn a_trimmed_note_resizes_from_its_visible_end() {
        let trimmed = Event {
            tick: 0,
            order: 0,
            message: Message::Note {
                channel: 0,
                key: 60,
                velocity: 96,
                end: 144,
                release: 0,
                release_order: 1,
            },
        };
        let bounds = [0, 96];
        let shortened = note_shift(trimmed, bounds, -24, 0, NoteDrag::End).unwrap();
        assert!(matches!(shortened.message, Message::Note { end: 72, .. }));
        let held = note_shift(trimmed, bounds, 24, 0, NoteDrag::End).unwrap();
        assert!(matches!(held.message, Message::Note { end: 96, .. }));
    }

    #[test]
    fn quantize_strength_moves_note_starts_a_matching_fraction_toward_the_grid() {
        let timeline = Timeline::default();
        for (tick, expected) in [(130, [0, 55, 110]), (350, [0, -55, -110])] {
            let shifts = [0, 50, 100]
                .map(|strength| quantize_shift(&timeline, Snap::Sixteenth, tick, strength));
            assert_eq!(shifts, expected, "sixteenth quantize at tick {tick}");
        }
        assert_eq!(
            quantize_shift(&timeline, Snap::Off, 130, 100),
            0,
            "the free grid keeps the pointer tick"
        );
    }

    #[test]
    fn transpose_and_moves_stay_inside_midi_and_source_bounds() {
        let note = Event {
            tick: 0,
            order: 0,
            message: Message::Note {
                channel: 0,
                key: 125,
                velocity: 100,
                end: 480,
                release: 0,
                release_order: 1,
            },
        };
        let bounds = [0, 960];
        let up = note_shift(note, bounds, 0, 10, NoteDrag::Move).unwrap();
        assert!(matches!(
            up.message,
            Message::Note {
                key: 127,
                end: 480,
                ..
            }
        ));
        let back = note_shift(note, bounds, -600, 0, NoteDrag::Move).unwrap();
        assert_eq!((back.tick, value(back)), (0, 100));
        assert!(matches!(back.message, Message::Note { end: 480, .. }));
        let forward = note_shift(note, bounds, 2_000, 0, NoteDrag::Move).unwrap();
        assert!(matches!(forward.message, Message::Note { end: 960, .. }));
        assert_eq!(forward.tick, 480);
        let shrunk = note_shift(note, bounds, 2_000, 0, NoteDrag::End).unwrap();
        assert!(matches!(shrunk.message, Message::Note { end: 960, .. }));
        let raised = note_shift(note, bounds, -600, 0, NoteDrag::Start).unwrap();
        assert_eq!(raised.tick, 0);
    }

    #[test]
    fn switching_the_edited_clip_reveals_its_start() {
        let mut view = Snapshot::default();
        view.project.tracks = vec![
            clef_model::project::Track {
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
            clef_model::project::Track {
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
        let model = std::sync::Arc::make_mut(&mut view.project.midi);
        for (track, start) in [(1u64, 0u64), (2, 1920)] {
            model
                .record(
                    track,
                    clef_model::midi::Mode::Merge,
                    [start, start + 480],
                    vec![vec![Event {
                        tick: start,
                        order: 1,
                        message: Message::Note {
                            channel: 2,
                            key: 60,
                            velocity: 90,
                            end: start + 240,
                            release: 0,
                            release_order: 2,
                        },
                    }]],
                    false,
                )
                .unwrap();
        }
        let a = view.project.midi.clips(1)[0].id();
        let b = view.project.midi.clips(2)[0].id();
        let mut roll = PianoRoll::default();
        roll.open(b, view.project.midi.find_clip(b).unwrap().1);
        let context = egui::Context::default();
        crate::theme::install_fonts(&context);
        let show = |roll: &mut PianoRoll, selected: &BTreeSet<ClipId>| {
            let _ = context.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1024.0, 768.0))),
                    ..Default::default()
                },
                |ui| {
                    let _ = roll.show(ui, 600.0, &view, selected, true);
                },
            );
        };
        let b_only = BTreeSet::from([b]);
        show(&mut roll, &b_only);
        assert_eq!(roll.offset, 1920, "opening the later clip scrolls to it");
        let a_only = BTreeSet::from([a]);
        show(&mut roll, &a_only);
        assert_eq!(roll.active, Some(a));
        assert_eq!(
            roll.offset, 0,
            "selecting another clip must bring its start into view"
        );
    }

    #[test]
    fn velocity_drag_keeps_the_grabbed_value_until_the_pointer_moves() {
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(480.0, 100.0));
        let grabbed_at_low_point = rect.bottom() - rect.height() * 40.0 / 127.0;
        assert_eq!(
            draw::velocity_dragged(rect, 90, grabbed_at_low_point, grabbed_at_low_point),
            90,
            "grabbing a bar below its tip must not jump to the pointer value"
        );
        let up =
            draw::velocity_dragged(rect, 90, grabbed_at_low_point, grabbed_at_low_point - 10.0);
        assert_eq!(up, 103, "10 points up adds about 13 velocity steps");
        let down =
            draw::velocity_dragged(rect, 90, grabbed_at_low_point, grabbed_at_low_point + 10.0);
        assert_eq!(down, 77, "10 points down subtracts about 13 velocity steps");
        let bottom = draw::velocity_dragged(rect, 90, 0.0, rect.height() + 10.0);
        assert_eq!(bottom, 0, "the drag clamps to the MIDI range");
    }

    #[test]
    fn velocity_drag_previews_relative_to_the_grab_and_edits_once_on_release() {
        let mut view = Snapshot::default();
        let model = std::sync::Arc::make_mut(&mut view.project.midi);
        model
            .record(
                1,
                clef_model::midi::Mode::Merge,
                [0, 960],
                vec![vec![Event {
                    tick: 0,
                    order: 1,
                    message: Message::Note {
                        channel: 2,
                        key: 60,
                        velocity: 83,
                        end: 480,
                        release: 0,
                        release_order: 2,
                    },
                }]],
                false,
            )
            .unwrap();
        let clip = view.project.midi.clips(1).first().unwrap();
        let clip_id = clip.id();
        let mut roll = PianoRoll::default();
        roll.open(clip_id, clip);
        let context = egui::Context::default();
        crate::theme::install_fonts(&context);
        let timeline = &view.project.timeline;
        let time = timeline::Viewport::with_snap(timeline, 0, 96.0, 480.0, Snap::Sixteenth);
        let lane = LaneContext {
            time: &time,
            timeline,
            clip,
            width: 480.0,
            playhead: 0,
            enabled: true,
            colour: theme::PEACOCK,
        };
        let run = |roll: &mut PianoRoll, events| {
            let mut action = None;
            let mut rect = Rect::ZERO;
            let _ = context.run_ui(
                RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    let origin = ui.next_widget_position();
                    rect = Rect::from_min_size(
                        egui::pos2(
                            origin.x + theme::ROLL_KEYBED + ui.spacing().item_spacing.x,
                            origin.y,
                        ),
                        egui::vec2(480.0, theme::ROLL_CONTROLLER_HEIGHT),
                    );
                    action = roll.values(ui, &lane, None, &view);
                },
            );
            (action, rect)
        };
        let (_, rect) = run(&mut roll, Vec::new());
        let grabbed = draw::point(rect, &time, 0, 40);
        run(&mut roll, vec![InputEvent::PointerMoved(grabbed)]);
        run(
            &mut roll,
            vec![InputEvent::PointerButton {
                pos: grabbed,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            }],
        );
        let moved = grabbed - egui::vec2(0.0, 10.0);
        let (action, _) = run(&mut roll, vec![InputEvent::PointerMoved(moved)]);
        assert!(action.is_none(), "the drag previews without committing");
        let (action, _) = run(
            &mut roll,
            vec![InputEvent::PointerButton {
                pos: moved,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        let Some(Action::EditClips {
            edit: ClipEdit::EditEvents { changes, .. },
            ..
        }) = action
        else {
            panic!("velocity drag did not dispatch on release");
        };
        let Some(EventChange::Replace(_, event)) = changes.first() else {
            panic!("velocity drag did not replace the note");
        };
        assert!(
            matches!(event.message, Message::Note { velocity: 103, .. }),
            "10 points up from the grabbed value 83 must land near 103, not at the pointer value"
        );
    }

    #[test]
    fn repeated_clip_projections_get_distinct_point_focus_ids() {
        use clef_model::midi::{Arrangement, ClipEditResult, Mode};
        use std::collections::HashSet;

        let mut arrangement = Arrangement::default();
        arrangement
            .record(
                1,
                Mode::Merge,
                [0, 480],
                vec![vec![Event {
                    tick: 96,
                    order: 1,
                    message: Message::Note {
                        channel: 2,
                        key: 60,
                        velocity: 90,
                        end: 288,
                        release: 5,
                        release_order: 2,
                    },
                }]],
                false,
            )
            .unwrap();
        let clip_id = arrangement.clips(1)[0].id();
        let result = arrangement
            .edit(
                &ClipEdit::Resize {
                    clip: clip_id,
                    end: 1920,
                    repeat: true,
                },
                [1].into_iter(),
            )
            .unwrap();
        assert_eq!(result, ClipEditResult::Applied(vec![clip_id]));
        let (_, clip) = arrangement.find_clip(clip_id).unwrap();
        let events = clip.roll_events([clip.start, clip.end]);
        assert!(events.len() > 1, "the repeat projects several points");
        assert_eq!(
            events
                .iter()
                .map(|event| event.id)
                .collect::<BTreeSet<_>>()
                .len(),
            1,
            "all projections share the source event"
        );
        let base = Id::new("piano-roll-point");
        let ids = events
            .iter()
            .map(|event| draw::point_id(base, clip_id, event.id, event.tick, None))
            .collect::<HashSet<_>>();
        assert_eq!(
            ids.len(),
            events.len(),
            "every projected point needs its own focus id"
        );
    }
    #[test]
    fn take_view_lists_inactive_events_and_returns_to_the_comp() {
        use clef_model::midi::{Event, Message, Mode};
        let note = |tick, end, key| Event {
            tick,
            order: 0,
            message: Message::Note {
                channel: 0,
                key,
                velocity: 90,
                end,
                release: 10,
                release_order: key.into(),
            },
        };
        let mut model = clef_model::midi::Arrangement::default();
        model
            .record(
                1,
                Mode::NewTake,
                [0, 960],
                vec![vec![note(100, 200, 60)], vec![note(500, 700, 62)]],
                false,
            )
            .unwrap();
        let clip = &model.clips(1)[0];
        let first = clip.takes[0].id;
        let mut roll = PianoRoll::default();
        roll.open_take_lane(&[(clip.id(), first)], 0);
        let events = roll.clip_events(clip, [0, 960]);
        assert_eq!(events.len(), 1);
        assert!(!events[0].active);
        roll.close_take();
        let events = roll.clip_events(clip, [0, 960]);
        assert_eq!(events.len(), 1);
        assert!(events[0].active);
    }
}
