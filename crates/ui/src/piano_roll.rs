use crate::{
    Action, Snapshot, colors,
    primitives::{piano_roll as draw, timeline, transport},
    theme,
};
use clef_model::{
    midi::{
        Clip, ClipEdit, ClipId, Event, EventChange, EventId, Message, RollEvent, Snap,
        continuous_cc,
    },
    project::Track,
    timeline::{MAX_TICK, QUARTER, Timeline},
};
use eframe::egui::{self, Id, Key, Modifiers, Pos2, Rect, Ui};
use std::collections::{BTreeSet, HashMap};

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
    pitch: HashMap<ClipId, u8>,
    key_cursor: HashMap<ClipId, u8>,
    rows: u8,
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
}

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
            pitch: HashMap::new(),
            key_cursor: HashMap::new(),
            rows: 16,
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
}

impl PianoRoll {
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
        self.offset = clip.start;
        self.focus_lane = true;
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
        open: &mut bool,
    ) -> Option<Action> {
        self.cancel(ui);
        let timeline = &view.project.timeline;
        let arrangement = &view.project.midi;
        let playhead = view.transport.audible_tick;
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
        if let Some((_, first)) = clips.first()
            && self.active.is_none_or(|id| !selected.contains(&id))
        {
            self.activate(first.id());
            self.offset = first.start;
        }
        let mut action = None;
        let right = clips
            .iter()
            .map(|(_, clip)| clip.end)
            .max()
            .unwrap_or(playhead)
            .max(playhead);
        let max_offset = right.max(self.offset);
        ui.horizontal_wrapped(|ui| {
            draw::heading(ui, "Piano Roll");
            timeline::snap_control(ui, &mut self.snap);
            draw::toggle(ui, "Follow", &mut self.follow);
            draw::toggle(ui, "MIDI Functions", &mut self.functions);
            if draw::close(ui) {
                *open = false;
                self.gesture = None;
            }
            transport::navigation(ui, &mut self.offset, &mut self.zoom, max_offset);
        });
        let Some((_, active)) = clips
            .iter()
            .find(|(_, clip)| Some(clip.id()) == self.active)
        else {
            draw::heading(ui, "Select a MIDI clip in the arrangement.");
            return None;
        };
        let width = (ui.available_width()
            - theme::ROLL_KEYBED
            - ui.spacing().item_spacing.x
            - ui.spacing().scroll.bar_width)
            .max(theme::ROLL_KEYBED);
        let mut time =
            timeline::Viewport::with_snap(timeline, self.offset, self.zoom, width, self.snap);
        if self.follow
            && view.transport.phase != clef_session::TransportPhase::Stopped
            && !(0.0..width).contains(&time.x(playhead))
        {
            self.offset = playhead;
            time =
                timeline::Viewport::with_snap(timeline, self.offset, self.zoom, width, self.snap);
        }
        draw::ruler(ui, &time, width, playhead, timeline);
        let mut active_context = LaneContext {
            time: &time,
            timeline,
            clip: active,
            width,
            playhead,
            enabled,
        };
        if self.functions {
            action = self
                .midi_functions(ui, &active_context, view)
                .or(action.take());
        }
        let height = height
            .min(ui.available_height())
            .max(ui.spacing().interact_size.y);
        let pitch_height = if clips.len() > 1 {
            theme::ROLL_MULTI_LANE_HEIGHT
        } else {
            theme::ROLL_LANE_HEIGHT
        };
        let names = lane_names(&view.project.tracks, timeline, &clips);
        egui::ScrollArea::vertical()
            .id_salt("piano-roll-lanes")
            .max_height(height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for (index, &(_track, clip)) in clips.iter().enumerate() {
                    ui.push_id(("piano-roll-lane", index), |ui| {
                        let name = names[index].as_str();
                        ui.horizontal_wrapped(|ui| {
                            let focused = self.active == Some(clip.id());
                            if draw::summary(ui, name, focused, clip.cycle().is_some()) {
                                self.activate(clip.id());
                            }
                            if self.active == Some(clip.id()) {
                                let pitch = self
                                    .pitch
                                    .entry(clip.id())
                                    .or_insert_with(|| initial_pitch(clip, self.rows));
                                draw::number(ui, "Lowest pitch", pitch, 0..=128 - self.rows);
                                draw::number(ui, "Rows", &mut self.rows, 8..=24);
                            }
                        });
                        let lane = LaneContext {
                            clip,
                            ..active_context
                        };
                        action = self
                            .notes(ui, &lane, view, pitch_height, name)
                            .or(action.take());
                    });
                }
                if let Some((_, clip)) = clips
                    .iter()
                    .find(|(_, clip)| Some(clip.id()) == self.active)
                {
                    active_context.clip = clip;
                }
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
                ui.horizontal_wrapped(|ui| {
                    draw::cc_picker(ui, &mut self.selected_cc);
                    draw::number(ui, "Channel", &mut self.channel, 1..=16);
                    if draw::button(ui, "Add Lane", true) {
                        self.cc_lanes.insert((self.channel, self.selected_cc));
                    }
                });
            });
        action
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
        height: f32,
        name: &str,
    ) -> Option<Action> {
        let clip = lane.clip;
        let id = clip.id();
        let mut lowest = self
            .pitch
            .get(&id)
            .copied()
            .unwrap_or_else(|| initial_pitch(clip, self.rows))
            .min(128 - self.rows);
        let (grid, keyboard, pointer) = draw::pitch_lane(
            ui,
            draw::Lane {
                time: lane.time,
                width: lane.width,
                height,
                lowest,
                rows: self.rows,
                enabled: lane.enabled,
                name,
            },
        );
        if self.focus_lane && self.active == Some(id) {
            pointer.request_focus();
            self.focus_lane = false;
        }
        if (pointer.has_focus() || keyboard.has_focus()) && self.active != Some(id) {
            self.activate(id);
        }
        let scroll = ui.input(|input| {
            if input.modifiers.shift && pointer.hovered() {
                input.smooth_scroll_delta.x + input.smooth_scroll_delta.y
            } else {
                0.0
            }
        });
        if scroll != 0.0 {
            lowest = (i16::from(lowest) + scroll.signum() as i16)
                .clamp(0, i16::from(128 - self.rows)) as u8;
        }
        self.pitch.insert(id, lowest);
        grid.paint(ui, [clip.start, clip.end], lane.playhead);
        let window = [
            lane.time.tick_at(0.0),
            lane.time.tick_at(lane.width).saturating_add(1),
        ];
        draw::repeat_markers(ui, grid.grid, lane.time, clip, window);
        let events = clip.roll_events(window);
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
                selected,
                selected && primary == Some(event.id),
                false,
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
        let mut action = if lane.enabled && keyboard.clicked() {
            keyboard.request_focus();
            keyboard.interact_pointer_pos().map(|pos| Action::Audition {
                clip: id,
                channel: clip_channel,
                key: grid.key(pos.y),
                velocity: 96,
            })
        } else {
            None
        };
        if keyboard.has_focus() {
            if self
                .key_cursor
                .get(&id)
                .is_none_or(|key| *key < lowest || *key >= lowest.saturating_add(self.rows))
            {
                self.key_cursor.insert(id, lowest + self.rows / 2);
            }
            let mut cursor = self.key_cursor.get(&id).copied().unwrap_or(lowest);
            if ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::ArrowUp)) {
                cursor = (cursor + 1).min(127);
            }
            if ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::ArrowDown)) {
                cursor = cursor.saturating_sub(1);
            }
            self.key_cursor.insert(id, cursor);
            draw::paint_key_cursor(ui, grid.keys, lowest, self.rows, cursor);
            if lane.enabled
                && ui.input_mut(|input| {
                    input.consume_key(Modifiers::NONE, Key::Enter)
                        || input.consume_key(Modifiers::NONE, Key::Space)
                })
            {
                action = Some(Action::Audition {
                    clip: id,
                    channel: clip_channel,
                    key: cursor,
                    velocity: 96,
                });
            }
        }
        if !lane.enabled {
            self.gesture = None;
            return action;
        }
        if let Some((focus, event)) = focused_note {
            if self.focus != Some(focus) {
                self.hit_selection(id, Some(event.id), false);
                self.focus = Some(focus);
            }
            if ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Enter))
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
                    draw::paint_marker(ui, grid.grid, lane.time, target_tick, colors::ACCENT);
                    self.preview_notes(ui, &grid, &notes, &changes, clip.end);
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
            self.preview_notes(ui, &grid, &notes, changes, lane.clip.end);
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
                    draw::paint_note(ui, grid.grid, grid.note(tick, end, key), true, false, true);
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
        let requested = ui.input_mut(|input| {
            input.consume_key(Modifiers::NONE, Key::Enter)
                || input.consume_key(Modifiers::NONE, Key::Insert)
        });
        if !requested {
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
        clip.roll_events([clip.start, clip.end])
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
        let consume = |modifiers, key| ui.input_mut(|input| input.consume_key(modifiers, key));
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
        if consume(Modifiers::COMMAND, Key::A) {
            self.selected = clip
                .roll_events([clip.start, clip.end])
                .into_iter()
                .filter(on_lane)
                .map(|event| event.id)
                .collect();
        }
        if consume(Modifiers::NONE, Key::Backspace) || consume(Modifiers::NONE, Key::Delete) {
            let changes = self
                .picked(clip)
                .into_iter()
                .filter(on_lane)
                .map(|event| EventChange::Remove(event.id))
                .collect();
            return self.edit(view, lane, "Delete MIDI Events", changes);
        }
        if consume(Modifiers::COMMAND, Key::C) || take_clipboard(ui, false) {
            self.clipboard = self
                .picked(clip)
                .into_iter()
                .filter(on_lane)
                .map(|event| event.source)
                .collect();
        }
        let paste = consume(Modifiers::COMMAND, Key::V) || take_clipboard(ui, true);
        if paste || consume(Modifiers::COMMAND, Key::D) {
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
        if !is_value
            && (consume(Modifiers::NONE, Key::Insert) || consume(Modifiers::NONE, Key::Enter))
        {
            let pitch = self.pitch.get(&clip.id()).copied().unwrap_or(54) + self.rows / 2;
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
        for (key, direction) in [
            (Key::ArrowLeft, -1),
            (Key::ArrowRight, 1),
            (Key::ArrowDown, -1),
            (Key::ArrowUp, 1),
        ] {
            let horizontal = matches!(key, Key::ArrowLeft | Key::ArrowRight);
            let resize = !is_value && horizontal && consume(Modifiers::SHIFT, key);
            if !resize && !consume(Modifiers::NONE, key) {
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

    fn midi_functions(
        &mut self,
        ui: &mut Ui,
        lane: &LaneContext<'_>,
        view: &Snapshot,
    ) -> Option<Action> {
        let mut action = None;
        let notes = self
            .picked(lane.clip)
            .into_iter()
            .filter(|event| matches!(event.source.message, Message::Note { .. }))
            .count();
        let enabled = lane.enabled && notes > 0;
        let bounds = lane.clip.roll_source_bounds();
        ui.horizontal_wrapped(|ui| {
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
            lane.clip
                .roll_events([lane.clip.start, lane.clip.end])
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
            draw::paint_values(ui, rect, lane.time, &events, &self.selected, cc, lane.clip);
            lane.time.paint_playhead(ui, rect, lane.playhead);
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
            draw::paint_values(ui, rect, lane.time, &events, &self.selected, cc, lane.clip);
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
            draw::paint_values(ui, rect, lane.time, &updated, &self.selected, cc, lane.clip);
        }
        lane.time.paint_playhead(ui, rect, lane.playhead);
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

fn initial_pitch(clip: &Clip, rows: u8) -> u8 {
    let first = clip
        .active_events()
        .find_map(|event| match event.message {
            Message::Note { key, .. } if event.tick < clip.end => Some(key),
            _ => None,
        })
        .unwrap_or(60);
    first.saturating_sub(rows / 3).min(128 - rows)
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
        roll.pitch.insert(clip_id, 48);
        let context = egui::Context::default();
        let timeline = &view.project.timeline;
        let time = timeline::Viewport::with_snap(timeline, 0, 96.0, 480.0, Snap::Sixteenth);
        let lane = LaneContext {
            time: &time,
            timeline,
            clip,
            width: 480.0,
            playhead: 160,
            enabled: true,
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
                        origin.x + theme::ROLL_KEYBED + ui.spacing().item_spacing.x + time.x(240),
                        origin.y + theme::ROLL_LANE_HEIGHT * (1.0 - 12.5 / 16.0),
                    );
                    action = roll.notes(ui, &lane, &view, theme::ROLL_LANE_HEIGHT, "Strings · 1.1");
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
        roll.pitch.insert(clip_id, 48);
        let context = egui::Context::default();
        let timeline = &view.project.timeline;
        let time = timeline::Viewport::with_snap(timeline, 0, 96.0, 480.0, Snap::Sixteenth);
        let lane = LaneContext {
            time: &time,
            timeline,
            clip,
            width: 480.0,
            playhead: 160,
            enabled: true,
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
                    action = roll.notes(ui, &lane, &view, theme::ROLL_LANE_HEIGHT, "Strings · 1.1");
                },
            );
            (action, origin)
        };
        let (_, origin) = run(&mut roll, Vec::new(), Modifiers::NONE);
        let keys = egui::pos2(
            origin.x + theme::ROLL_KEYBED * 0.5,
            origin.y + theme::ROLL_LANE_HEIGHT * 0.5,
        );
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
        };
        let events = clip
            .roll_events([clip.start, clip.end])
            .into_iter()
            .filter(|event| matches!(event.source.message, Message::CurvedControl { .. }))
            .collect::<Vec<_>>();
        let roll = PianoRoll::default();
        let context = egui::Context::default();
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
            id: 1,
            name: "Strings".into(),
            plugin: None,
            state: Vec::new(),
        });
        view.project.tracks.push(Track {
            id: 2,
            name: "Piano".into(),
            plugin: None,
            state: Vec::new(),
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
                id: 1,
                name: "One".into(),
                plugin: None,
                state: Vec::new(),
            },
            clef_model::project::Track {
                id: 2,
                name: "Two".into(),
                plugin: None,
                state: Vec::new(),
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
        let mut open = true;
        let mut show = |roll: &mut PianoRoll, selected: &BTreeSet<ClipId>| {
            let _ = context.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1024.0, 768.0))),
                    ..Default::default()
                },
                |ui| {
                    let _ = roll.show(ui, 600.0, &view, selected, true, &mut open);
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
        let timeline = &view.project.timeline;
        let time = timeline::Viewport::with_snap(timeline, 0, 96.0, 480.0, Snap::Sixteenth);
        let lane = LaneContext {
            time: &time,
            timeline,
            clip,
            width: 480.0,
            playhead: 0,
            enabled: true,
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
}
