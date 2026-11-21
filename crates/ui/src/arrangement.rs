use crate::{
    Action, App, Command, Confirm,
    primitives::{
        menu,
        timeline::{self, Viewport},
    },
    theme,
};
use clef_model::{
    midi::{Arrangement, Clip, ClipEdit, ClipId, CompEdit, Message, Snap},
    project::{
        ArrangementView, TAKE_LANE_MAX_HEIGHT as MODEL_MAX_HEIGHT,
        TAKE_LANE_MIN_HEIGHT as MODEL_MIN_HEIGHT, TakeLanes, Track,
    },
    timeline::{MAX_TICK, Timeline},
};
use eframe::egui::{self, Color32, Id, Key, Modifiers, Pos2, Rect, Sense, Ui};
use std::{collections::BTreeSet, sync::Arc};

pub const CLIP_MENU: [Command; 3] = [
    Command::SplitClips,
    Command::DuplicateClips,
    Command::DeleteClips,
];

#[derive(Clone, Copy)]
pub struct Lane {
    pub track: u64,
    pub rect: Rect,
    pub visible: Rect,
    pub colour: egui::Color32,
}

enum Drag {
    Marquee {
        start: Pos2,
        additive: bool,
    },
    Move {
        track: u64,
        tick: u64,
        grab: u64,
    },
    Edge {
        clip: ClipId,
        repeat: bool,
        grab_offset: u64,
    },
}

#[derive(Default)]
pub struct ArrangementUi {
    /// The live shortcuts for menu rows, copied from the app's key map when it changes.
    pub(crate) keys: crate::commands::Keymap,
    pub settings: ArrangementView,
    pub selected: BTreeSet<ClipId>,
    pub open_roll: Option<ClipId>,
    before_click: Option<(ClipId, BTreeSet<ClipId>)>,
    focus: Option<Id>,
    focused_track: Option<u64>,
    menu_target: Option<(u64, u64, bool)>,
    menu_anchor: Option<(u64, Pos2)>,
    pub gallery_menu: Option<u64>,
    create_at: Option<(u64, u64)>,
    drag: Option<Drag>,
    pub pending: Option<ClipEdit>,
    pub focused_lane: Option<(u64, usize)>,
    pub focused_take_clip: Option<ClipId>,
    lane_rename: Option<(u64, usize)>,
    lane_name: String,
    take_drag: Option<TakeDrag>,
    pub lane_height_preview: Option<(u64, usize, u16)>,
    /// T40: first visible point of the scrolled track rows (view state, not saved).
    pub v_offset: f32,
    /// T40: a fit requested by a command, applied on the next frame.
    pub fit: Option<Fit>,
}

/// T40: fit the content to the visible width or height.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fit {
    Width,
    Height,
}

/// A track height in points, clamped to the saved range.
pub fn clamp_height(height: f32) -> u16 {
    let range = clef_model::project::TRACK_HEIGHT;
    height
        .round()
        .clamp(f32::from(*range.start()), f32::from(*range.end())) as u16
}

pub struct LaneInput<'a> {
    pub viewport: &'a Viewport,
    pub timeline: &'a Timeline,
    pub arrangement: &'a Arrangement,
    pub track: &'a Track,
    pub enabled: bool,
    pub playhead: u64,
}

impl ArrangementUi {
    pub fn focused(&self, ctx: &egui::Context) -> bool {
        self.focus
            .is_some_and(|id| ctx.memory(|memory| memory.has_focus(id)))
    }

    pub fn acknowledge(
        &mut self,
        project_changed: bool,
        completed: bool,
        selection: Option<&[ClipId]>,
        arrangement: &Arrangement,
        settings: ArrangementView,
    ) {
        if project_changed {
            *self = Self {
                settings,
                ..Self::default()
            };
        }
        if completed {
            if self.pending.take().is_some()
                && let Some(selection) = selection
            {
                self.selected = selection.iter().copied().collect();
            }
            self.selected
                .retain(|id| arrangement.find_clip(*id).is_some());
        }
    }

    pub fn cancel(&mut self, ctx: &egui::Context) {
        if (self.drag.is_some() || self.focused(ctx))
            && ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape))
            && self.drag.take().is_none()
        {
            self.selected.clear();
        }
    }

    pub fn lane(
        &mut self,
        ui: &Ui,
        lane: Lane,
        input: LaneInput<'_>,
        labels: impl Fn(Command) -> (&'static str, String),
    ) -> Option<Command> {
        let LaneInput {
            viewport,
            timeline,
            arrangement,
            track,
            enabled,
            playhead,
        } = input;
        let clips = arrangement.clips(track.id);
        let response = ui.interact(
            lane.visible,
            ui.id().with(("arrangement lane", track.id)),
            Sense::click_and_drag(),
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Other,
                enabled,
                format!("{} arrangement lane", track.name),
            )
        });
        let clip_focus = viewport.clips(
            ui,
            lane.rect,
            timeline,
            (&track.name, lane.colour),
            clips,
            (&self.selected, enabled),
        );
        if response.has_focus() {
            self.focus = Some(response.id);
            self.focused_track = Some(lane.track);
        }
        if let Some((id, clip)) = clip_focus {
            self.focus = Some(id);
            self.focused_track = Some(lane.track);
            if ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Enter)) {
                if !self.selected.contains(&clip) {
                    self.selected = BTreeSet::from([clip]);
                }
                self.open_roll = Some(clip);
            }
        }
        if response.has_focus()
            && let Some(id) = self.selected.first().copied()
            && ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Enter))
        {
            self.open_roll = Some(id);
        }
        if let Some(pos) = response.hover_pos()
            && let Some(clip) = viewport.clip_at(lane.rect, clips, pos)
        {
            let edge = viewport.edge_target(lane.rect, clip, pos);
            if enabled {
                viewport.edge_handles(ui, lane.rect, clip, edge);
                if let Some(repeat) = edge {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    let (label, shortcut) = labels(if repeat {
                        Command::LoopClipLonger
                    } else {
                        Command::ResizeClipLonger
                    });
                    response
                        .clone()
                        .on_hover_text(format!("{label}  {shortcut}"));
                }
            }
        }
        let bypass = ui.input(|input| input.modifiers.alt);
        let tick_at = |x| viewport.tick_at(x - lane.rect.left());
        let snapped = |tick| {
            if bypass {
                tick
            } else {
                self.settings.snap.nearest(timeline, tick)
            }
        };
        if enabled
            && response.drag_started_by(egui::PointerButton::Primary)
            && let Some(pos) = ui.input(|input| input.pointer.press_origin())
        {
            let additive = ui.input(|input| input.modifiers.command);
            self.drag = Some(
                if let Some(clip) = viewport.clip_at(lane.rect, clips, pos) {
                    if !self.selected.contains(&clip.id()) {
                        if !additive {
                            self.selected.clear();
                        }
                        self.selected.insert(clip.id());
                    }
                    if let Some(repeat) = viewport.edge_target(lane.rect, clip, pos) {
                        Drag::Edge {
                            clip: clip.id(),
                            repeat,
                            grab_offset: clip.end.saturating_sub(tick_at(pos.x)),
                        }
                    } else {
                        Drag::Move {
                            track: lane.track,
                            tick: clip.start,
                            grab: tick_at(pos.x),
                        }
                    }
                } else {
                    Drag::Marquee {
                        start: pos,
                        additive,
                    }
                },
            );
            response.request_focus();
            self.focus = Some(response.id);
            self.focused_track = Some(lane.track);
        }
        if (response.clicked() || response.secondary_clicked())
            && let Some(pos) = response.interact_pointer_pos()
        {
            response.request_focus();
            self.focus = Some(response.id);
            self.focused_track = Some(lane.track);
            let target = viewport
                .clip_at(lane.rect, clips, pos)
                .map(|clip| clip.id());
            if response.secondary_clicked() {
                self.menu_anchor = None;
                self.menu_target = Some((lane.track, snapped(tick_at(pos.x)), target.is_none()));
                if let Some(id) = target.filter(|id| !self.selected.contains(id)) {
                    self.selected = BTreeSet::from([id]);
                }
            } else {
                self.menu_target = None;
                let additive = ui.input(|input| input.modifiers.command);
                if response.double_clicked()
                    && let Some(id) = target
                {
                    if let Some((_, previous)) =
                        self.before_click.take().filter(|(last, _)| *last == id)
                    {
                        self.selected = previous;
                    }
                    self.selected.insert(id);
                    self.open_roll = Some(id);
                    return None;
                }
                self.before_click = target
                    .filter(|id| self.selected.contains(id))
                    .map(|id| (id, self.selected.clone()));
                if !additive {
                    self.selected.clear();
                }
                if let Some(id) = target {
                    if !self.selected.insert(id) && additive {
                        self.selected.remove(&id);
                    }
                } else if enabled && response.double_clicked() {
                    self.create_at = Some((lane.track, snapped(tick_at(pos.x))));
                    return Some(Command::CreateClip);
                }
            }
        }
        let open = self.gallery_menu == Some(lane.track)
            || (clip_focus.is_some() || response.has_focus())
                && ui.input_mut(|input| input.consume_key(Modifiers::SHIFT, Key::F10));
        if open {
            self.gallery_menu = None;
            let clip = clip_focus
                .map(|(_, clip)| clip)
                .or_else(|| self.selected.first().copied())
                .and_then(|clip| arrangement.find_clip(clip))
                .filter(|(track, _)| *track == lane.track)
                .map(|(_, clip)| clip);
            if let Some(clip) = clip.filter(|clip| !self.selected.contains(&clip.id())) {
                self.selected = BTreeSet::from([clip.id()]);
            }
            let anchor = clip.map_or(lane.visible.center(), |clip| {
                viewport
                    .clip_rect(lane.rect, [clip.start, clip.end])
                    .intersect(lane.visible)
                    .center()
            });
            self.menu_anchor = Some((lane.track, anchor));
            self.menu_target = Some((lane.track, snapped(playhead), clip.is_none()));
        }
        if open || response.context_menu_opened() || response.secondary_clicked() {
            let commands = if self
                .menu_target
                .is_some_and(|(track, _, empty)| track == lane.track && empty)
            {
                &[Command::CreateClip][..]
            } else {
                &CLIP_MENU[..]
            };
            let rows = commands.iter().map(|&command| {
                let (label, shortcut) = labels(command);
                let available = match command {
                    Command::SplitClips => self.selected.iter().any(|id| {
                        arrangement
                            .find_clip(*id)
                            .is_some_and(|(_, clip)| clip.start < playhead && playhead < clip.end)
                    }),
                    Command::CreateClip => true,
                    _ => !self.selected.is_empty(),
                };
                (label, shortcut, enabled && available)
            });
            let anchor = self
                .menu_anchor
                .filter(|(track, _)| *track == lane.track)
                .map(|(_, anchor)| anchor);
            if let Some(index) = timeline::clip_menu(&response, rows, anchor, open) {
                let command = commands[index];
                if command == Command::CreateClip {
                    self.create_at = self.menu_target.map(|(track, tick, _)| (track, tick));
                }
                return Some(command);
            }
        }
        None
    }

    pub fn finish_frame(
        &mut self,
        ui: &Ui,
        lanes: &[Lane],
        viewport: &Viewport,
        timeline: &Timeline,
        arrangement: &Arrangement,
        enabled: bool,
    ) -> Result<Option<ClipEdit>, &'static str> {
        if !enabled {
            self.drag = None;
        }
        let Some(first) = lanes.first() else {
            return Ok(None);
        };
        let (pointer, down, released, bypass) = ui.input(|input| {
            (
                input.pointer.interact_pos(),
                input.pointer.primary_down(),
                input.pointer.primary_released(),
                input.modifiers.alt,
            )
        });
        let snap = if bypass {
            Snap::Off
        } else {
            self.settings.snap
        };
        let tick_at = |x| viewport.tick_at(x - first.rect.left());
        let mut edit = None;
        if let (Some(drag), Some(pointer)) = (&self.drag, pointer) {
            match *drag {
                Drag::Marquee { start, additive } => {
                    let rect = Rect::from_two_pos(start, pointer);
                    if released {
                        if !additive {
                            self.selected.clear();
                        }
                        for lane in lanes.iter().filter(|lane| lane.visible.intersects(rect)) {
                            for clip in arrangement.clips(lane.track) {
                                if viewport
                                    .clip_rect(lane.rect, [clip.start, clip.end])
                                    .intersect(lane.visible)
                                    .intersects(rect)
                                {
                                    self.selected.insert(clip.id());
                                }
                            }
                        }
                    } else {
                        let bounds = lanes
                            .iter()
                            .fold(Rect::NOTHING, |bounds, lane| bounds.union(lane.visible));
                        timeline::marquee(ui, bounds, rect);
                    }
                }
                Drag::Move { track, tick, grab } => {
                    if let Some(index) = lanes.iter().position(|lane| lane.track == track)
                        && let Some(target) = lanes
                            .iter()
                            .position(|lane| lane.visible.y_range().contains(pointer.y))
                    {
                        let delta = target as isize - index as isize;
                        let destinations = self
                            .selected
                            .iter()
                            .map(|&id| {
                                let (track, _) = arrangement.find_clip(id)?;
                                let index = lanes.iter().position(|lane| lane.track == track)?;
                                Some((id, lanes.get(index.checked_add_signed(delta)?)?.track))
                            })
                            .collect::<Option<Vec<_>>>();
                        if let Some(destinations) = destinations {
                            let requested = (tick as i64 + tick_at(pointer.x) as i64 - grab as i64)
                                .clamp(0, MAX_TICK as i64)
                                as u64;
                            let offset = snap.nearest(timeline, requested) as i64 - tick as i64;
                            edit = Some(ClipEdit::Move {
                                destinations,
                                delta: offset,
                            });
                        }
                    }
                }
                Drag::Edge {
                    clip,
                    repeat,
                    grab_offset,
                } => {
                    let pointer_tick = tick_at(pointer.x);
                    if let Some((_, item)) = arrangement.find_clip(clip) {
                        let end = if repeat {
                            item.loop_end(pointer_tick + grab_offset)
                        } else {
                            snap.nearest(timeline, pointer_tick)
                        };
                        edit = Some(ClipEdit::Resize { clip, end, repeat });
                    }
                }
            }
        }
        if let Some(preview) = edit.as_ref().or(self.pending.as_ref()) {
            for lane in lanes.iter().filter(|lane| lane.visible.is_positive()) {
                let paint = |range: [u64; 2], tick: u64, verb, free| {
                    let position = timeline.position(tick.min(MAX_TICK));
                    let label = format!(
                        "{verb}  {}.{}.{}{}",
                        position.bar,
                        position.beat,
                        position.tick,
                        if free { "  ·  Free" } else { "" }
                    );
                    viewport.edit_preview(
                        ui,
                        [lane.rect, lane.visible],
                        lane.colour,
                        crate::primitives::timeline::EditPreview {
                            range,
                            tick,
                            snap: !free,
                            label: &label,
                        },
                    );
                };
                match preview {
                    ClipEdit::Move {
                        destinations,
                        delta,
                    } => {
                        for &(id, _) in destinations
                            .iter()
                            .filter(|(_, track)| *track == lane.track)
                        {
                            if let Some((_, clip)) = arrangement.find_clip(id)
                                && let (Some(start), Some(end)) = (
                                    clip.start.checked_add_signed(*delta),
                                    clip.end.checked_add_signed(*delta),
                                )
                            {
                                paint([start, end], start, "Move", snap == Snap::Off);
                            }
                        }
                    }
                    ClipEdit::Resize { clip, end, repeat } => {
                        if let Some((track, clip)) = arrangement.find_clip(*clip)
                            && track == lane.track
                            && *end > clip.start
                        {
                            paint(
                                [clip.start, *end],
                                *end,
                                if *repeat { "Loop" } else { "Resize" },
                                !repeat && snap == Snap::Off,
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
        let outside_tracks =
            released && edit.is_none() && matches!(self.drag, Some(Drag::Move { .. }));
        if !down {
            self.drag = None;
        }
        if outside_tracks {
            return Err("The selection cannot be moved to those tracks.");
        }
        Ok(edit.filter(|_| released))
    }
}

impl App {
    pub fn clip_command_enabled(&self, command: Command) -> bool {
        use Command::*;
        let selected = &self.arrangement_ui.selected;
        let arrangement = &self.view.project.midi;
        let tick = self.view.transport.audible_tick;
        match command {
            SelectAllClips => !arrangement.tracks.is_empty(),
            CreateClip => self.view.edit_history.available && !self.view.project.tracks.is_empty(),
            SplitClips => {
                self.view.edit_history.available
                    && selected.iter().any(|&id| {
                        arrangement
                            .find_clip(id)
                            .is_some_and(|(_, clip)| clip.start < tick && tick < clip.end)
                    })
            }
            ResizeClipShorter | ResizeClipLonger | LoopClipShorter | LoopClipLonger => {
                self.view.edit_history.available && selected.len() == 1
            }
            _ => self.view.edit_history.available && !selected.is_empty(),
        }
    }

    pub fn dispatch_clip_command(&mut self, command: Command) {
        use Command::*;
        let arrangement = &self.view.project.midi;
        let timeline = &self.view.project.timeline;
        let snap = self.arrangement_ui.settings.snap;
        let selected: Vec<_> = self.arrangement_ui.selected.iter().copied().collect();
        let tick = self.view.transport.audible_tick;
        let edit = match command {
            SelectAllClips => {
                self.arrangement_ui.selected = arrangement
                    .tracks
                    .values()
                    .flatten()
                    .map(|clip| clip.id())
                    .collect();
                return;
            }
            CreateClip => {
                let target = self.arrangement_ui.create_at.take().or_else(|| {
                    self.arrangement_ui
                        .focused_track
                        .or(self.view.selected)
                        .map(|id| (id, snap.nearest(timeline, tick)))
                });
                let Some((track, start)) = target else { return };
                let end = (start + snap_step_for_bar(timeline, start)).min(MAX_TICK);
                ClipEdit::Create { track, start, end }
            }
            SplitClips => ClipEdit::Split {
                clips: selected,
                tick,
                confirmed: false,
            },
            DuplicateClips => ClipEdit::Duplicate(selected),
            DeleteClips => ClipEdit::Delete(selected),
            MoveClipsLeft | MoveClipsRight => {
                let anchor = selected
                    .iter()
                    .filter_map(|&id| arrangement.find_clip(id))
                    .map(|(_, clip)| clip.start)
                    .min()
                    .unwrap_or(tick);
                let step = snap.step(timeline, anchor) as i64;
                let destinations = selected
                    .into_iter()
                    .filter_map(|id| arrangement.find_clip(id).map(|(track, _)| (id, track)))
                    .collect();
                let delta = if command == MoveClipsLeft {
                    -step
                } else {
                    step
                };
                ClipEdit::Move {
                    destinations,
                    delta,
                }
            }
            MoveClipsUp | MoveClipsDown => {
                let tracks: Vec<_> = self
                    .view
                    .project
                    .tracks
                    .iter()
                    .filter(|track| track.is_instrument())
                    .collect();
                let step = if command == MoveClipsUp { -1 } else { 1 };
                let destinations = selected
                    .into_iter()
                    .map(|id| {
                        let (track, _) = arrangement.find_clip(id)?;
                        let index = tracks.iter().position(|candidate| candidate.id == track)?;
                        Some((id, tracks.get(index.checked_add_signed(step)?)?.id))
                    })
                    .collect::<Option<Vec<_>>>();
                let Some(destinations) = destinations else {
                    self.status =
                        "The selection cannot be moved past the first or last track.".into();
                    return;
                };
                ClipEdit::Move {
                    destinations,
                    delta: 0,
                }
            }
            ResizeClipShorter | ResizeClipLonger | LoopClipShorter | LoopClipLonger => {
                let Some((_, clip)) = selected.first().and_then(|&id| arrangement.find_clip(id))
                else {
                    return;
                };
                let repeat = matches!(command, LoopClipShorter | LoopClipLonger);
                let step = if repeat {
                    1
                } else {
                    snap.step(timeline, clip.end)
                };
                let end = if matches!(command, ResizeClipShorter | LoopClipShorter) {
                    clip.end.saturating_sub(step)
                } else {
                    (clip.end + step).min(MAX_TICK)
                };
                ClipEdit::Resize {
                    clip: clip.id(),
                    end,
                    repeat,
                }
            }
            _ => return,
        };
        self.commit_clip_edit(edit);
    }

    pub fn commit_clip_edit(&mut self, edit: ClipEdit) {
        if !self.view.edit_history.available || self.busy_since.is_some() {
            self.status =
                "Stop transport and wait for instrument loads before editing clips.".into();
            return;
        }
        let expected = self.view.project.midi.clone();
        if let ClipEdit::Split {
            clips,
            tick,
            confirmed: false,
        } = &edit
            && expected.split_crosses_notes(clips, *tick)
        {
            self.confirm = Some(Confirm::SplitClips(
                expected,
                ClipEdit::Split {
                    clips: clips.clone(),
                    tick: *tick,
                    confirmed: true,
                },
            ));
            return;
        }
        self.send_clip_edit(expected, edit);
    }

    pub fn send_clip_edit(&mut self, expected: Arc<Arrangement>, edit: ClipEdit) {
        self.send(Action::EditClips {
            expected,
            edit: edit.clone(),
        });
        if self.busy_since.is_some() {
            self.arrangement_ui.pending = Some(edit);
        }
    }
}

fn snap_step_for_bar(timeline: &Timeline, tick: u64) -> u64 {
    timeline.section_at(tick).1.bar_ticks()
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, PointerButton, RawInput, pos2, vec2};

    struct Harness {
        context: egui::Context,
        ui: ArrangementUi,
        model: Arrangement,
        tracks: Vec<Track>,
    }

    impl Harness {
        fn new() -> Self {
            let mut model = Arrangement::default();
            model
                .edit(
                    &ClipEdit::Create {
                        track: 1,
                        start: 0,
                        end: 1_920,
                    },
                    [1, 2].into_iter(),
                )
                .unwrap();
            let tracks = (1..=2)
                .map(|id| Track {
                    colour: None,
                    id,
                    name: format!("Track {id}"),
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
                })
                .collect();
            Self {
                context: egui::Context::default(),
                ui: ArrangementUi::default(),
                model,
                tracks,
            }
        }

        fn frame(&mut self, events: Vec<Event>, modifiers: Modifiers) -> Option<ClipEdit> {
            let timeline = Timeline::default();
            let viewport = Viewport::with_snap(&timeline, 0, 96.0, 480.0, self.ui.settings.snap);
            let lanes: Vec<_> = self
                .tracks
                .iter()
                .enumerate()
                .map(|(index, track)| {
                    let rect = Rect::from_min_size(
                        pos2(30.0, 100.0 + 44.0 * index as f32),
                        vec2(480.0, 44.0),
                    );
                    Lane {
                        track: track.id,
                        rect,
                        visible: rect,
                        colour: crate::theme::PEACOCK,
                    }
                })
                .collect();
            let mut result = None;
            let _ = self.context.run_ui(
                RawInput {
                    events,
                    modifiers,
                    ..Default::default()
                },
                |ui| {
                    for (&lane, track) in lanes.iter().zip(&self.tracks) {
                        self.ui.lane(
                            ui,
                            lane,
                            LaneInput {
                                viewport: &viewport,
                                timeline: &timeline,
                                arrangement: &self.model,
                                track,
                                enabled: true,
                                playhead: 960,
                            },
                            |_| ("Clip command", String::new()),
                        );
                    }
                    result = self
                        .ui
                        .finish_frame(ui, &lanes, &viewport, &timeline, &self.model, true)
                        .unwrap();
                },
            );
            result
        }

        fn drag(&mut self, from: Pos2, to: Pos2, modifiers: Modifiers) -> ClipEdit {
            self.frame(vec![Event::PointerMoved(from)], modifiers);
            self.frame(
                vec![Event::PointerButton {
                    pos: from,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers,
                }],
                modifiers,
            );
            assert!(
                self.frame(vec![Event::PointerMoved(to)], modifiers)
                    .is_none()
            );
            self.frame(
                vec![Event::PointerButton {
                    pos: to,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers,
                }],
                modifiers,
            )
            .unwrap()
        }
    }

    #[test]
    fn double_clicking_a_selected_clip_opens_the_entire_selection() {
        let mut harness = Harness::new();
        harness
            .model
            .edit(
                &ClipEdit::Create {
                    track: 2,
                    start: 0,
                    end: 960,
                },
                [1, 2].into_iter(),
            )
            .unwrap();
        let first = harness.model.clips(1)[0].id();
        let second = harness.model.clips(2)[0].id();
        harness.ui.selected = BTreeSet::from([first, second]);
        let point = pos2(70.0, 120.0);
        harness.frame(vec![Event::PointerMoved(point)], Modifiers::NONE);
        for _ in 0..2 {
            for pressed in [true, false] {
                harness.frame(
                    vec![Event::PointerButton {
                        pos: point,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    }],
                    Modifiers::NONE,
                );
            }
        }
        assert_eq!(harness.ui.open_roll, Some(first));
        assert_eq!(harness.ui.selected, BTreeSet::from([first, second]));
    }

    #[test]
    fn pointer_uses_distinct_edge_targets_and_option_bypasses_snap() {
        let mut harness = Harness::new();
        for modifiers in [Modifiers::ALT, Modifiers::NONE] {
            let upper = harness.drag(pos2(219.0, 108.0), pos2(278.0, 108.0), modifiers);
            assert!(matches!(
                upper,
                ClipEdit::Resize {
                    end: 3_840,
                    repeat: true,
                    ..
                }
            ));
        }
        let lower = harness.drag(pos2(219.0, 135.0), pos2(278.0, 135.0), Modifiers::NONE);
        assert!(matches!(
            lower,
            ClipEdit::Resize {
                end: 2_880,
                repeat: false,
                ..
            }
        ));
        let free = harness.drag(pos2(219.0, 135.0), pos2(278.0, 135.0), Modifiers::ALT);
        assert!(matches!(
            free,
            ClipEdit::Resize {
                end: 2_480,
                repeat: false,
                ..
            }
        ));
    }

    #[test]
    fn pointer_move_uses_a_shared_time_and_track_delta() {
        let mut harness = Harness::new();
        let id = harness.model.clips(1)[0].id();
        let edit = harness.drag(pos2(80.0, 120.0), pos2(176.0, 163.0), Modifiers::NONE);
        assert!(
            matches!(edit, ClipEdit::Move { destinations, delta: 960 } if destinations == vec![(id, 2)])
        );
    }
    #[test]
    fn take_lane_menu_opens_with_the_shared_popup() {
        use clef_model::midi::{Event, Message, Mode};
        use egui::{RawInput, pos2, vec2};
        let mut model = Arrangement::default();
        let note = |tick, key| Event {
            tick,
            order: 0,
            message: Message::Note {
                channel: 0,
                key,
                velocity: 90,
                end: tick + 100,
                release: 12,
                release_order: 1,
            },
        };
        model
            .record(
                1,
                Mode::NewTake,
                [0, 960],
                vec![vec![note(100, 60)], vec![note(500, 62)]],
                false,
            )
            .unwrap();
        let track = Track {
            colour: None,
            id: 1,
            name: "Strings".into(),
            plugin: None,
            instrument_bypassed: false,
            instrument_offline: false,
            record_mode: Default::default(),
            mix: Default::default(),
            inserts: Vec::new(),
            take_lanes: TakeLanes {
                visible: true,
                lanes: Vec::new(),
            },
            kind: Default::default(),
            output: None,
            sends: Vec::new(),
            state: Default::default(),
        };
        let context = egui::Context::default();
        crate::theme::install_fonts(&context);
        context.enable_accesskit();
        let mut editor = ArrangementUi::default();
        let timeline = Timeline::default();
        let viewport = Viewport::with_snap(&timeline, 0, 96.0, 480.0, Snap::Off);
        let header = Rect::from_min_size(pos2(0.0, 100.0), vec2(160.0, 40.0));
        let band = Rect::from_min_size(pos2(170.0, 100.0), vec2(480.0, 40.0));
        let point = pos2(header.right() - 15.0, header.center().y);
        let mut labels = Vec::new();
        for (events, capture) in [
            (vec![egui::Event::PointerMoved(point)], false),
            (
                vec![egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                }],
                false,
            ),
            (
                vec![egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                }],
                true,
            ),
            (Vec::new(), true),
        ] {
            let output = context.run_ui(
                RawInput {
                    events,
                    modifiers: Modifiers::NONE,
                    ..Default::default()
                },
                |ui| {
                    let _ = editor.take_lane(
                        ui,
                        TakeLaneInput {
                            viewport: &viewport,
                            timeline: &timeline,
                            arrangement: &model,
                            track: &track,
                            row: TakeLaneRow {
                                track: 1,
                                lane: 0,
                                header,
                                band,
                            },
                            enabled: true,
                        },
                    );
                },
            );
            if capture && let Some(tree) = output.platform_output.accesskit_update {
                labels.extend(tree.nodes.iter().filter_map(|(_, node)| {
                    node.label()
                        .or_else(|| node.value())
                        .map(|label| label.to_owned())
                }));
            }
        }
        assert!(
            labels.iter().any(|label| label == "Use Whole Take"),
            "take menu did not open: {labels:?}"
        );
    }

    #[test]
    fn take_lane_draws_note_marks_for_the_take() {
        use clef_model::midi::{Event, Message, Mode};
        use egui::{pos2, vec2};
        let mut model = Arrangement::default();
        let note = |tick, key| Event {
            tick,
            order: 0,
            message: Message::Note {
                channel: 0,
                key,
                velocity: 90,
                end: tick + 100,
                release: 12,
                release_order: 1,
            },
        };
        model
            .record(
                1,
                Mode::NewTake,
                [0, 960],
                vec![vec![note(100, 60), note(300, 72)], vec![note(500, 62)]],
                false,
            )
            .unwrap();
        let clip = &model.clips(1)[0];
        let timeline = Timeline::default();
        let viewport = Viewport::with_snap(&timeline, 0, 96.0, 480.0, Snap::Off);
        let band = Rect::from_min_size(pos2(170.0, 100.0), vec2(480.0, 40.0));
        let first = take_note_rects(clip, 0, band, &viewport);
        assert_eq!(first.len(), 2);
        assert!(!first[0].1, "the inactive take stays dimmed");
        assert!(
            (first[0].0.center().y - first[1].0.center().y).abs() > 15.0,
            "pitch spread should follow the take's notes"
        );
        let second = take_note_rects(clip, 1, band, &viewport);
        assert_eq!(second.len(), 1);
        assert!(second[0].1, "the comp take is drawn active");
        assert!(band.contains_rect(first[0].0));
        assert!(band.contains_rect(second[0].0));
    }

    #[test]
    fn double_clicking_a_lane_clip_opens_that_take() {
        use clef_model::midi::{Event, Message, Mode};
        use egui::{RawInput, pos2, vec2};
        let mut model = Arrangement::default();
        let note = |tick, key| Event {
            tick,
            order: 0,
            message: Message::Note {
                channel: 0,
                key,
                velocity: 90,
                end: tick + 100,
                release: 12,
                release_order: 1,
            },
        };
        model
            .record(
                1,
                Mode::NewTake,
                [0, 960],
                vec![vec![note(100, 60)], vec![note(500, 62)]],
                false,
            )
            .unwrap();
        let id = model.clips(1)[0].id();
        let track = Track {
            colour: None,
            id: 1,
            name: "Strings".into(),
            plugin: None,
            instrument_bypassed: false,
            instrument_offline: false,
            record_mode: Default::default(),
            mix: Default::default(),
            inserts: Vec::new(),
            take_lanes: TakeLanes {
                visible: true,
                lanes: Vec::new(),
            },
            kind: Default::default(),
            output: None,
            sends: Vec::new(),
            state: Default::default(),
        };
        let context = egui::Context::default();
        crate::theme::install_fonts(&context);
        let mut editor = ArrangementUi::default();
        let timeline = Timeline::default();
        let viewport = Viewport::with_snap(&timeline, 0, 96.0, 480.0, Snap::Off);
        let header = Rect::from_min_size(pos2(0.0, 100.0), vec2(160.0, 40.0));
        let band = Rect::from_min_size(pos2(170.0, 100.0), vec2(480.0, 40.0));
        let point = pos2(band.left() + viewport.x(600), 120.0);
        let mut action = None;
        let mut frame = |events: Vec<egui::Event>, action: &mut Option<TakeAction>| {
            let _ = context.run_ui(
                RawInput {
                    events,
                    modifiers: Modifiers::NONE,
                    ..Default::default()
                },
                |ui| {
                    *action = editor.take_lane(
                        ui,
                        TakeLaneInput {
                            viewport: &viewport,
                            timeline: &timeline,
                            arrangement: &model,
                            track: &track,
                            row: TakeLaneRow {
                                track: 1,
                                lane: 0,
                                header,
                                band,
                            },
                            enabled: true,
                        },
                    );
                },
            );
        };
        frame(vec![egui::Event::PointerMoved(point)], &mut action);
        for _ in 0..2 {
            for pressed in [true, false] {
                frame(
                    vec![egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    }],
                    &mut action,
                );
            }
        }
        assert!(matches!(
            action,
            Some(TakeAction::EditTake {
                track: 1,
                lane: 0,
                clip: Some(clip),
            }) if clip == id
        ));
    }

    #[test]
    fn take_lane_cut_drag_moves_the_cut() {
        use clef_model::midi::{Event, Message, Mode};
        use egui::{RawInput, pos2, vec2};
        let mut model = Arrangement::default();
        let note = |tick, key| Event {
            tick,
            order: 0,
            message: Message::Note {
                channel: 0,
                key,
                velocity: 90,
                end: tick + 100,
                release: 12,
                release_order: 1,
            },
        };
        model
            .record(
                1,
                Mode::NewTake,
                [0, 960],
                vec![vec![note(100, 60)], vec![note(500, 62)]],
                false,
            )
            .unwrap();
        model
            .comp(
                1,
                0,
                CompEdit::SelectRange { start: 0, end: 300 },
                [1].into_iter(),
            )
            .unwrap();
        let track = Track {
            colour: None,
            id: 1,
            name: "Strings".into(),
            plugin: None,
            instrument_bypassed: false,
            instrument_offline: false,
            record_mode: Default::default(),
            mix: Default::default(),
            inserts: Vec::new(),
            take_lanes: TakeLanes {
                visible: true,
                lanes: Vec::new(),
            },
            kind: Default::default(),
            output: None,
            sends: Vec::new(),
            state: Default::default(),
        };
        let context = egui::Context::default();
        crate::theme::install_fonts(&context);
        let mut editor = ArrangementUi::default();
        editor.settings.snap = Snap::Off;
        let timeline = Timeline::default();
        let viewport = Viewport::with_snap(&timeline, 0, 96.0, 480.0, Snap::Off);
        let header = Rect::from_min_size(pos2(0.0, 100.0), vec2(160.0, 40.0));
        let band = Rect::from_min_size(pos2(170.0, 100.0), vec2(480.0, 40.0));
        let from = pos2(band.left() + viewport.x(300), 120.0);
        let to = pos2(band.left() + viewport.x(600), 120.0);
        let mut action = None;
        let mut frame =
            |events: Vec<egui::Event>, modifiers: Modifiers, action: &mut Option<TakeAction>| {
                let _ = context.run_ui(
                    RawInput {
                        events,
                        modifiers,
                        ..Default::default()
                    },
                    |ui| {
                        *action = editor.take_lane(
                            ui,
                            TakeLaneInput {
                                viewport: &viewport,
                                timeline: &timeline,
                                arrangement: &model,
                                track: &track,
                                row: TakeLaneRow {
                                    track: 1,
                                    lane: 0,
                                    header,
                                    band,
                                },
                                enabled: true,
                            },
                        );
                    },
                );
            };
        frame(
            vec![egui::Event::PointerMoved(from)],
            Modifiers::NONE,
            &mut action,
        );
        frame(
            vec![egui::Event::PointerButton {
                pos: from,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            }],
            Modifiers::NONE,
            &mut action,
        );
        frame(
            vec![egui::Event::PointerMoved(to)],
            Modifiers::NONE,
            &mut action,
        );
        frame(
            vec![egui::Event::PointerButton {
                pos: to,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
            Modifiers::NONE,
            &mut action,
        );
        assert!(matches!(
            action,
            Some(TakeAction::Comp {
                track: 1,
                lane: 0,
                edit: CompEdit::MoveCut {
                    from: 300,
                    to: 600,
                    ..
                }
            })
        ));
    }

    #[test]
    fn take_lane_drag_makes_one_comp_range_action() {
        use clef_model::midi::{Event, Message, Mode};
        use egui::{RawInput, pos2, vec2};
        let mut model = Arrangement::default();
        let note = |tick, key| Event {
            tick,
            order: 0,
            message: Message::Note {
                channel: 0,
                key,
                velocity: 90,
                end: tick + 100,
                release: 12,
                release_order: 1,
            },
        };
        model
            .record(
                1,
                Mode::NewTake,
                [0, 960],
                vec![vec![note(100, 60)], vec![note(500, 62)]],
                false,
            )
            .unwrap();
        let track = Track {
            colour: None,
            id: 1,
            name: "Strings".into(),
            plugin: None,
            instrument_bypassed: false,
            instrument_offline: false,
            record_mode: Default::default(),
            mix: Default::default(),
            inserts: Vec::new(),
            take_lanes: TakeLanes {
                visible: true,
                lanes: Vec::new(),
            },
            kind: Default::default(),
            output: None,
            sends: Vec::new(),
            state: Default::default(),
        };
        let context = egui::Context::default();
        crate::theme::install_fonts(&context);
        let mut editor = ArrangementUi::default();
        editor.settings.snap = Snap::Off;
        let timeline = Timeline::default();
        let viewport = Viewport::with_snap(&timeline, 0, 96.0, 480.0, Snap::Off);
        let header = Rect::from_min_size(pos2(0.0, 100.0), vec2(160.0, 40.0));
        let band = Rect::from_min_size(pos2(170.0, 100.0), vec2(480.0, 40.0));
        let from = pos2(band.left() + viewport.x(100), 120.0);
        let to = pos2(band.left() + viewport.x(480), 120.0);
        let mut action = None;
        let mut frame =
            |events: Vec<egui::Event>, modifiers: Modifiers, action: &mut Option<TakeAction>| {
                let _ = context.run_ui(
                    RawInput {
                        events,
                        modifiers,
                        ..Default::default()
                    },
                    |ui| {
                        *action = editor.take_lane(
                            ui,
                            TakeLaneInput {
                                viewport: &viewport,
                                timeline: &timeline,
                                arrangement: &model,
                                track: &track,
                                row: TakeLaneRow {
                                    track: 1,
                                    lane: 0,
                                    header,
                                    band,
                                },
                                enabled: true,
                            },
                        );
                    },
                );
            };
        frame(
            vec![egui::Event::PointerMoved(from)],
            Modifiers::NONE,
            &mut action,
        );
        frame(
            vec![egui::Event::PointerButton {
                pos: from,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            }],
            Modifiers::NONE,
            &mut action,
        );
        frame(
            vec![egui::Event::PointerMoved(to)],
            Modifiers::NONE,
            &mut action,
        );
        frame(
            vec![egui::Event::PointerButton {
                pos: to,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
            Modifiers::NONE,
            &mut action,
        );
        assert!(matches!(
            action,
            Some(TakeAction::Comp {
                track: 1,
                lane: 0,
                edit: CompEdit::SelectRange {
                    start: 100,
                    end: 480
                }
            })
        ));
    }
}

const TAKE_LANE_HEIGHT_STEP: f32 = 4.0;
const TAKE_LANE_SEAM: f32 = 4.0;
const CUT_TARGET: f32 = 4.0;

#[derive(Clone, Debug)]
pub enum TakeAction {
    Comp {
        track: u64,
        lane: usize,
        edit: CompEdit,
    },
    TakeToNewTrack {
        track: u64,
        lane: usize,
    },
    View {
        track: u64,
        lanes: TakeLanes,
    },
    EditTake {
        track: u64,
        lane: usize,
        clip: Option<ClipId>,
    },
}

#[derive(Clone, Copy)]
pub struct TakeLaneRow {
    pub track: u64,
    pub lane: usize,
    pub header: Rect,
    pub band: Rect,
}

pub struct TakeLaneInput<'a> {
    pub viewport: &'a Viewport,
    pub timeline: &'a Timeline,
    pub arrangement: &'a Arrangement,
    pub track: &'a Track,
    pub row: TakeLaneRow,
    pub enabled: bool,
}

#[derive(Clone, Copy)]
enum TakeDrag {
    Comp {
        track: u64,
        lane: usize,
        anchor: u64,
        current: u64,
    },
    Cut {
        track: u64,
        lane: usize,
        clip: ClipId,
        from: u64,
        current: u64,
        lower: u64,
        upper: u64,
    },
    Height {
        track: u64,
        lane: usize,
        start: f32,
        height: u16,
        pointer: f32,
    },
}

impl ArrangementUi {
    pub fn take_count(arrangement: &Arrangement, track: u64) -> usize {
        arrangement
            .clips(track)
            .iter()
            .map(|clip| clip.takes.len())
            .max()
            .unwrap_or(0)
    }

    pub fn focused_take_lane(&self) -> Option<(u64, usize)> {
        self.focused_lane
    }

    pub fn take_lane_target(&self) -> Option<(u64, usize, Option<ClipId>)> {
        self.focused_lane
            .map(|(track, lane)| (track, lane, self.focused_take_clip))
    }

    pub fn begin_take_rename(&mut self, track: u64, lane: usize, name: String) {
        self.focused_lane = Some((track, lane));
        self.lane_rename = Some((track, lane));
        self.lane_name = name;
    }

    pub fn take_lane(&mut self, ui: &mut Ui, input: TakeLaneInput<'_>) -> Option<TakeAction> {
        let TakeLaneInput {
            viewport,
            timeline,
            arrangement,
            track,
            row,
            enabled,
        } = input;
        if self.take_drag.is_some() && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            self.take_drag = None;
        }
        let mut action = None;
        let clips = arrangement.clips(row.track);
        let name = track.take_lanes.name(row.lane);
        let active_ranges: Vec<[u64; 2]> = clips
            .iter()
            .filter(|clip| row.lane < clip.takes.len())
            .flat_map(|clip| clip.take_active_ranges(clip.takes[row.lane].id))
            .collect();
        let painter = ui.painter();
        painter.rect_filled(row.header, 0.0, theme::PANEL);
        for clip in clips {
            if row.lane >= clip.takes.len() {
                continue;
            }
            let left = row.band.left() + viewport.x(clip.start);
            let right = row.band.left() + viewport.x(clip.end);
            let clip_rect = Rect::from_min_max(
                egui::pos2(left.max(row.band.left()), row.band.top() + 1.0),
                egui::pos2(right.min(row.band.right()), row.band.bottom() - 1.0),
            );
            if !clip_rect.is_positive() {
                continue;
            }
            painter.rect_filled(clip_rect, 2.0, theme::ACCENT.gamma_multiply(0.08));
            painter.rect_stroke(
                clip_rect,
                2.0,
                egui::Stroke::new(1.0_f32, theme::DIVIDER),
                egui::StrokeKind::Inside,
            );
            let id = clip.takes[row.lane].id;
            for section in &clip.sections {
                if section.take != id {
                    continue;
                }
                let start = section.start.max(clip.start);
                let end = section.end.min(clip.end);
                if start >= end {
                    continue;
                }
                let rect = Rect::from_min_max(
                    egui::pos2(
                        (row.band.left() + viewport.x(start)).max(clip_rect.left()),
                        clip_rect.top(),
                    ),
                    egui::pos2(
                        (row.band.left() + viewport.x(end)).min(clip_rect.right()),
                        clip_rect.bottom(),
                    ),
                );
                if rect.is_positive() {
                    painter.rect_filled(rect, 1.0, theme::ACCENT.gamma_multiply(0.24));
                    painter.rect_stroke(
                        rect,
                        1.0,
                        egui::Stroke::new(1.0_f32, theme::ACCENT),
                        egui::StrokeKind::Inside,
                    );
                }
            }
            for pair in clip.sections.windows(2) {
                if pair[0].take != id && pair[1].take != id {
                    continue;
                }
                let x = row.band.left() + viewport.x(pair[1].start);
                if x > row.band.left() && x < row.band.right() {
                    dashed_line(
                        painter,
                        egui::pos2(x, row.band.top() + 1.0),
                        egui::pos2(x, row.band.bottom() - 1.0),
                        theme::ACCENT.gamma_multiply(0.6),
                    );
                }
            }
            for (rect, active) in take_note_rects(clip, row.lane, row.band, viewport) {
                let color = if active {
                    theme::TEXT
                } else {
                    theme::TEXT.gamma_multiply(0.28)
                };
                painter.rect_filled(rect, 1.0, color);
            }
        }
        let band_response = ui.interact(
            row.band,
            ui.id().with(("take lane band", row.track, row.lane)),
            if enabled {
                Sense::click_and_drag()
            } else {
                Sense::hover()
            },
        );
        band_response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Other,
                enabled,
                format!("{} take {} lane", track.name, row.lane + 1),
            )
        });
        let clip_at = |x: f32| {
            clips.iter().find(|clip| {
                row.lane < clip.takes.len()
                    && x >= row.band.left() + viewport.x(clip.start)
                    && x < row.band.left() + viewport.x(clip.end)
            })
        };
        let pointer = ui.input(|i| i.pointer.interact_pos());
        if band_response.hovered()
            && let Some(pos) = pointer
        {
            if let Some(target) = cut_at(
                timeline, clips, row.track, row.lane, row.band, viewport, pos,
            ) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                painter.rect_filled(
                    Rect::from_min_max(
                        egui::pos2(target.x - 1.0, row.band.top()),
                        egui::pos2(target.x + 1.0, row.band.bottom()),
                    ),
                    0.0,
                    theme::ACCENT,
                );
            } else if let Some(clip) = clip_at(pos.x) {
                self.focused_take_clip = Some(clip.id());
            }
        }
        if band_response.drag_started_by(egui::PointerButton::Primary)
            && enabled
            && let Some(pos) = pointer
        {
            let origin = ui
                .input(|input| input.pointer.press_origin())
                .unwrap_or(pos);
            self.focused_lane = Some((row.track, row.lane));
            if let Some(clip) = clip_at(origin.x) {
                self.focused_take_clip = Some(clip.id());
            }
            let raw = viewport.tick_at((origin.x - row.band.left()).max(0.0));
            if let Some(cut) = cut_at(
                timeline, clips, row.track, row.lane, row.band, viewport, origin,
            ) {
                let index = cut.section;
                let clip = clips
                    .iter()
                    .find(|clip| clip.id() == cut.clip)
                    .cloned()
                    .unwrap();
                let sections = &clip.sections;
                let lower = if index >= 2 {
                    sections[index - 2].end
                } else {
                    clip.start
                };
                let upper = sections
                    .get(index + 1)
                    .map_or(clip.source_end(), |section| section.start);
                self.take_drag = Some(TakeDrag::Cut {
                    track: row.track,
                    lane: row.lane,
                    clip: cut.clip,
                    from: cut.tick,
                    current: cut.tick,
                    lower,
                    upper,
                });
            } else {
                let anchor = snap(self.settings.snap, timeline, raw);
                self.take_drag = Some(TakeDrag::Comp {
                    track: row.track,
                    lane: row.lane,
                    anchor,
                    current: anchor,
                });
            }
        }
        if let Some(drag) = self.take_drag
            && band_response.dragged_by(egui::PointerButton::Primary)
            && let Some(pos) = pointer
        {
            let raw = viewport.tick_at((pos.x - row.band.left()).max(0.0));
            match drag {
                TakeDrag::Comp {
                    track,
                    lane,
                    anchor,
                    ..
                } => {
                    let current = snap(self.settings.snap, timeline, raw);
                    self.take_drag = Some(TakeDrag::Comp {
                        track,
                        lane,
                        anchor,
                        current,
                    });
                    let start = anchor.min(current);
                    let end = anchor.max(current);
                    if start < end {
                        let rect = Rect::from_min_max(
                            egui::pos2(row.band.left() + viewport.x(start), row.band.top() + 1.0),
                            egui::pos2(row.band.left() + viewport.x(end), row.band.bottom() - 1.0),
                        );
                        painter.rect_filled(rect, 1.0, theme::ACCENT.gamma_multiply(0.16));
                        painter.rect_stroke(
                            rect,
                            1.0,
                            egui::Stroke::new(1.0_f32, theme::ACCENT),
                            egui::StrokeKind::Inside,
                        );
                        let at = timeline.position(end);
                        readout(
                            painter,
                            pos,
                            &format!("Comp take · {}.{}.{:03}", at.bar, at.beat, at.tick),
                        );
                    }
                }
                TakeDrag::Cut {
                    track,
                    lane,
                    clip,
                    from,
                    lower,
                    upper,
                    ..
                } => {
                    let step = if ui.input(|input| input.modifiers.alt) {
                        1
                    } else {
                        self.settings.snap.step(timeline, from).max(1)
                    };
                    let mut current = if step == 1 {
                        raw
                    } else {
                        self.settings.snap.nearest(timeline, raw)
                    };
                    let low = lower.saturating_add(step);
                    let high = upper.saturating_sub(step);
                    if low <= high {
                        current = current.clamp(low, high);
                    }
                    self.take_drag = Some(TakeDrag::Cut {
                        track,
                        lane,
                        clip,
                        from,
                        current,
                        lower,
                        upper,
                    });
                    let x = row.band.left() + viewport.x(current);
                    painter.rect_filled(
                        Rect::from_min_max(
                            egui::pos2(x - 1.0, row.band.top()),
                            egui::pos2(x + 1.0, row.band.bottom()),
                        ),
                        0.0,
                        theme::ACCENT,
                    );
                    let at = timeline.position(current);
                    readout(
                        painter,
                        pos,
                        &format!("Cut {}.{}.{:03}", at.bar, at.beat, at.tick),
                    );
                }
                TakeDrag::Height { .. } => {}
            }
        }
        if band_response.drag_stopped() {
            match self.take_drag.take() {
                Some(TakeDrag::Comp {
                    track,
                    lane,
                    anchor,
                    current,
                }) => {
                    let start = anchor.min(current);
                    let end = anchor.max(current);
                    if start < end {
                        action = Some(TakeAction::Comp {
                            track,
                            lane,
                            edit: CompEdit::SelectRange { start, end },
                        });
                    }
                }
                Some(TakeDrag::Cut {
                    track,
                    lane,
                    clip,
                    from,
                    current,
                    ..
                }) if current != from => {
                    action = Some(TakeAction::Comp {
                        track,
                        lane,
                        edit: CompEdit::MoveCut {
                            clip,
                            from,
                            to: current,
                        },
                    });
                }
                _ => {}
            }
        }
        if band_response.double_clicked() {
            let clip = pointer
                .and_then(|pos| clip_at(pos.x).map(Clip::id))
                .or(self.focused_take_clip);
            self.focused_lane = Some((row.track, row.lane));
            self.focused_take_clip = clip;
            action = Some(TakeAction::EditTake {
                track: row.track,
                lane: row.lane,
                clip,
            });
        } else if band_response.clicked()
            && let Some(pos) = pointer
        {
            self.focused_lane = Some((row.track, row.lane));
            if let Some(clip) = clip_at(pos.x) {
                self.focused_take_clip = Some(clip.id());
            }
        }
        let header_response = ui.interact(
            row.header,
            ui.id().with(("take lane header", row.track, row.lane)),
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        header_response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::SelectableLabel,
                enabled,
                format!(
                    "Take {}, {}",
                    row.lane + 1,
                    plays_summary(timeline, &active_ranges)
                ),
            )
        });
        let stripe =
            Rect::from_min_size(row.header.left_top(), egui::vec2(2.0, row.header.height()));
        painter.rect_filled(stripe, 0.0, theme::ACCENT);
        let name_rect = Rect::from_min_size(
            egui::pos2(row.header.left() + 8.0, row.header.top() + 5.0),
            egui::vec2((row.header.width() - 44.0).max(20.0), 16.0),
        );
        if self.lane_rename == Some((row.track, row.lane)) {
            let edit = ui.put(
                name_rect,
                egui::TextEdit::singleline(&mut self.lane_name)
                    .id_salt(("take lane name", row.track, row.lane))
                    .desired_width(name_rect.width()),
            );
            if !edit.has_focus() {
                edit.request_focus();
            }
            let commit = ui.input(|i| i.key_pressed(Key::Enter));
            let cancel = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
            if commit || (edit.lost_focus() && !cancel) {
                let mut lanes = track.take_lanes.clone();
                let value: String = self.lane_name.trim().chars().take(256).collect();
                lanes.set_name(row.lane, (!value.is_empty()).then_some(value));
                self.lane_rename = None;
                action = Some(TakeAction::View {
                    track: row.track,
                    lanes,
                });
            } else if cancel {
                self.lane_rename = None;
            }
        } else {
            painter.text(
                name_rect.left_center(),
                egui::Align2::LEFT_CENTER,
                &name,
                egui::FontId::proportional(11.0),
                theme::TEXT,
            );
            if row.header.height() >= 32.0 {
                painter.text(
                    egui::pos2(name_rect.left(), row.header.top() + 24.0),
                    egui::Align2::LEFT_CENTER,
                    plays_summary(timeline, &active_ranges),
                    egui::FontId::proportional(10.0),
                    theme::SECONDARY,
                );
            }
            let name_hit = Rect::from_min_max(
                name_rect.left_top(),
                egui::pos2(
                    name_rect.left()
                        + ui.fonts_mut(|fonts| {
                            fonts
                                .layout_no_wrap(
                                    name.clone(),
                                    egui::FontId::proportional(11.0),
                                    theme::TEXT,
                                )
                                .size()
                                .x
                        }),
                    name_rect.bottom(),
                ),
            );
            let name_response = ui.interact(
                name_hit,
                ui.id().with(("take lane name hit", row.track, row.lane)),
                if enabled {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            );
            if name_response.double_clicked() {
                self.begin_take_rename(row.track, row.lane, name.clone());
            } else {
                let whole = Rect::from_min_max(
                    egui::pos2(name_hit.right() + 4.0, row.header.top()),
                    egui::pos2(
                        row.header.right() - 30.0,
                        row.header.bottom() - TAKE_LANE_SEAM,
                    ),
                );
                if enabled
                    && ui
                        .interact(
                            whole,
                            ui.id().with(("take lane whole", row.track, row.lane)),
                            Sense::click(),
                        )
                        .clicked()
                {
                    self.focused_lane = Some((row.track, row.lane));
                    action = Some(TakeAction::Comp {
                        track: row.track,
                        lane: row.lane,
                        edit: CompEdit::UseWholeTake,
                    });
                }
            }
            let menu_rect = Rect::from_center_size(
                egui::pos2(row.header.right() - 15.0, row.header.center().y),
                egui::vec2(26.0, 24.0),
            );
            let menu_response = ui.interact(
                menu_rect,
                ui.id().with(("take lane menu", row.track, row.lane)),
                if enabled {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            );
            if enabled && menu_response.hovered() {
                ui.painter().rect_filled(menu_rect, 2.0, theme::FIELD);
            }
            paint_kebab(ui, menu_rect);
            menu_response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Take lane menu")
            });
            menu_response.clone().on_hover_text("Take lane menu");
            if menu_response.clicked() {
                self.focused_lane = Some((row.track, row.lane));
            }
            if enabled {
                let keys = &self.keys;
                let item = |ui: &mut Ui, command: Command| {
                    let shortcut = keys.text(ui.ctx(), command);
                    menu::Item::new(command.title())
                        .shortcut(&shortcut)
                        .destructive(command.destructive())
                        .show(ui)
                };
                let picked = menu::show(menu::under(&menu_response), |ui| {
                    let picked = [Command::RenameTake, Command::EditTake]
                        .into_iter()
                        .find(|&command| item(ui, command));
                    menu::separator(ui);
                    picked.or([
                        Command::UseWholeTake,
                        Command::DeleteTake,
                        Command::FlattenComp,
                        Command::TakeToNewTrack,
                    ]
                    .into_iter()
                    .find(|&command| item(ui, command)))
                })
                .flatten();
                match picked {
                    Some(Command::RenameTake) => {
                        self.begin_take_rename(row.track, row.lane, name.clone());
                    }
                    Some(Command::EditTake) => {
                        self.focused_lane = Some((row.track, row.lane));
                        action = Some(TakeAction::EditTake {
                            track: row.track,
                            lane: row.lane,
                            clip: self.focused_take_clip,
                        });
                    }
                    Some(
                        command @ (Command::UseWholeTake
                        | Command::DeleteTake
                        | Command::FlattenComp),
                    ) => {
                        action = Some(TakeAction::Comp {
                            track: row.track,
                            lane: row.lane,
                            edit: match command {
                                Command::UseWholeTake => CompEdit::UseWholeTake,
                                Command::DeleteTake => CompEdit::DeleteTake,
                                _ => CompEdit::Flatten,
                            },
                        });
                    }
                    Some(Command::TakeToNewTrack) => {
                        action = Some(TakeAction::TakeToNewTrack {
                            track: row.track,
                            lane: row.lane,
                        });
                    }
                    _ => {}
                }
            }
        }
        let seam = Rect::from_min_max(
            egui::pos2(row.header.left(), row.band.bottom() - TAKE_LANE_SEAM),
            egui::pos2(row.band.right(), row.band.bottom()),
        );
        let seam_response = ui.interact(
            seam,
            ui.id().with(("take lane seam", row.track, row.lane)),
            if enabled {
                Sense::drag()
            } else {
                Sense::hover()
            },
        );
        if seam_response.hovered() || matches!(self.take_drag, Some(TakeDrag::Height { .. })) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        if seam_response.drag_started()
            && let Some(pos) = pointer
        {
            self.take_drag = Some(TakeDrag::Height {
                track: row.track,
                lane: row.lane,
                start: track.take_lanes.height(row.lane) as f32,
                height: track.take_lanes.height(row.lane),
                pointer: pos.y,
            });
        }
        if seam_response.dragged()
            && let Some(TakeDrag::Height {
                track,
                lane,
                start,
                pointer: anchor,
                ..
            }) = self.take_drag
            && let Some(pos) = pointer
        {
            let height = quantize_height(start + pos.y - anchor);
            self.lane_height_preview = Some((track, lane, height));
            self.take_drag = Some(TakeDrag::Height {
                track,
                lane,
                start,
                height,
                pointer: anchor,
            });
        }
        if seam_response.drag_stopped()
            && let Some(TakeDrag::Height {
                track: _track_id,
                lane,
                height,
                ..
            }) = self.take_drag.take()
        {
            self.lane_height_preview = None;
            let mut lanes = track.take_lanes.clone();
            lanes.set_height(lane, height);
            action = Some(TakeAction::View {
                track: row.track,
                lanes,
            });
        }
        action
    }
}

pub(crate) fn paint_kebab(ui: &Ui, rect: Rect) {
    let center = rect.center();
    for offset in [-4.0, 0.0, 4.0] {
        ui.painter().circle_filled(
            egui::pos2(center.x, center.y + offset),
            1.4,
            theme::SECONDARY,
        );
    }
}

fn take_note_rects(clip: &Clip, lane: usize, band: Rect, viewport: &Viewport) -> Vec<(Rect, bool)> {
    let Some(take) = clip.takes.get(lane) else {
        return Vec::new();
    };
    let mut notes = Vec::new();
    for event in &take.events {
        let Message::Note { key, end, .. } = event.message else {
            continue;
        };
        if event.tick >= clip.end || end <= clip.start {
            continue;
        }
        let active = clip.sections.iter().any(|section| {
            section.take == take.id && section.start <= event.tick && event.tick < section.end
        });
        notes.push((event.tick.max(clip.start), end.min(clip.end), key, active));
    }
    if notes.is_empty() {
        return Vec::new();
    }
    let use_active = notes.iter().any(|note| note.3);
    let (low, high) = notes
        .iter()
        .filter(|note| !use_active || note.3)
        .fold((127_u8, 0_u8), |(low, high), note| {
            (low.min(note.2), high.max(note.2))
        });
    let midpoint = (f32::from(low) + f32::from(high)) * 0.5;
    let half = ((f32::from(high) - f32::from(low)) * 0.5 + 1.0).max(6.0);
    let [pitch_low, pitch_high] = [midpoint - half, midpoint + half];
    let top = band.top() + 4.0;
    let bottom = band.bottom() - 4.0;
    let mut rects = Vec::new();
    for (start, end, key, active) in notes {
        let left = (band.left() + viewport.x(start)).clamp(band.left(), band.right());
        let right = (band.left() + viewport.x(end)).clamp(band.left(), band.right());
        if right <= left {
            continue;
        }
        let position = (f32::from(key) - pitch_low) / (pitch_high - pitch_low);
        let y = (bottom - position * (bottom - top)).clamp(top, bottom - 2.0);
        rects.push((
            Rect::from_min_max(egui::pos2(left, y), egui::pos2(right, y + 2.0)),
            active,
        ));
    }
    rects
}

fn snap(snap: Snap, timeline: &Timeline, tick: u64) -> u64 {
    if snap == Snap::Off {
        tick
    } else {
        snap.nearest(timeline, tick)
    }
}

fn quantize_height(height: f32) -> u16 {
    let step = TAKE_LANE_HEIGHT_STEP;
    let clamped = height.clamp(MODEL_MIN_HEIGHT as f32, MODEL_MAX_HEIGHT as f32);
    (clamped / step).round() as u16 * step as u16
}

fn plays_summary(timeline: &Timeline, ranges: &[[u64; 2]]) -> String {
    match ranges {
        [] => "Not in comp".into(),
        [range] => {
            let start = timeline.position(range[0]);
            let end = timeline.position(range[1]);
            format!(
                "Plays {}.{}–{}.{}",
                start.bar, start.beat, end.bar, end.beat
            )
        }
        ranges => format!("Plays {} ranges", ranges.len()),
    }
}

fn readout(painter: &egui::Painter, pos: Pos2, text: &str) {
    let galley = painter.layout_no_wrap(
        text.to_owned(),
        egui::FontId::proportional(10.0),
        theme::TEXT,
    );
    let rect = Rect::from_min_size(
        egui::pos2(pos.x + 12.0, pos.y - 20.0),
        galley.size() + egui::vec2(10.0, 4.0),
    );
    painter.rect_filled(rect, 2.0, theme::FIELD);
    painter.rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0_f32, theme::DIVIDER),
        egui::StrokeKind::Inside,
    );
    painter.galley(rect.min + egui::vec2(5.0, 2.0), galley, theme::TEXT);
}

fn dashed_line(painter: &egui::Painter, from: Pos2, to: Pos2, color: Color32) {
    let length = (to.y - from.y).abs();
    let mut offset = 0.0;
    while offset < length {
        let end = (offset + 3.0).min(length);
        painter.line_segment(
            [
                egui::pos2(from.x, from.y + offset),
                egui::pos2(to.x, from.y + end),
            ],
            egui::Stroke::new(1.0_f32, color),
        );
        offset += 6.0;
    }
}

#[derive(Clone, Copy)]
struct Cut {
    section: usize,
    clip: ClipId,
    tick: u64,
    x: f32,
}

fn cut_at(
    _timeline: &Timeline,
    clips: &[Clip],
    _track: u64,
    lane: usize,
    band: Rect,
    viewport: &Viewport,
    pos: Pos2,
) -> Option<Cut> {
    let mut best: Option<(f32, usize, ClipId, u64)> = None;
    for clip in clips {
        if lane >= clip.takes.len() {
            continue;
        }
        let take = clip.takes[lane].id;
        for (section, pair) in clip.sections.windows(2).enumerate() {
            if pair[0].take != take && pair[1].take != take {
                continue;
            }
            let x = band.left() + viewport.x(pair[1].start);
            let distance = (pos.x - x).abs();
            if distance <= CUT_TARGET && best.is_none_or(|(closest, _, _, _)| distance < closest) {
                best = Some((distance, section + 1, clip.id(), pair[1].start));
            }
        }
    }
    best.map(|(_, section, clip, tick)| Cut {
        section,
        clip,
        tick,
        x: band.left() + viewport.x(tick),
    })
}

impl App {
    pub(crate) fn apply_take_action(&mut self, action: TakeAction) {
        let track = match &action {
            TakeAction::Comp { track, .. }
            | TakeAction::TakeToNewTrack { track, .. }
            | TakeAction::View { track, .. }
            | TakeAction::EditTake { track, .. } => *track,
        };
        if self.view.selected != Some(track) {
            self.select_track(track);
        }
        match action {
            TakeAction::Comp { track, lane, edit } => {
                self.send(Action::CompEdit {
                    expected: self.view.project.midi.clone(),
                    track,
                    lane,
                    edit,
                });
            }
            TakeAction::TakeToNewTrack { track, lane } => {
                self.send(Action::TakeToNewTrack {
                    expected: self.view.project.midi.clone(),
                    track,
                    lane,
                });
            }
            TakeAction::View { track, lanes } => {
                self.send(Action::TrackView { track, lanes });
            }
            TakeAction::EditTake { track, lane, clip } => {
                let arrangement = &self.view.project.midi;
                let ids: Vec<(ClipId, u64)> = match clip {
                    Some(clip) => arrangement
                        .find_clip(clip)
                        .filter(|(owner, clip)| *owner == track && lane < clip.takes.len())
                        .map(|(_, clip)| vec![(clip.id(), clip.takes[lane].id)])
                        .unwrap_or_default(),
                    None => arrangement
                        .clips(track)
                        .iter()
                        .filter(|clip| lane < clip.takes.len())
                        .map(|clip| (clip.id(), clip.takes[lane].id))
                        .collect(),
                };
                if ids.is_empty() {
                    return;
                }
                let start = arrangement
                    .find_clip(ids[0].0)
                    .map_or(0, |(_, clip)| clip.start);
                self.arrangement_ui.selected = ids.iter().map(|(id, _)| *id).collect();
                self.piano_roll.open_take_lane(&ids, lane);
                self.piano_roll.reveal(start);
                self.piano_roll_open = true;
                self.select_track(track);
            }
        }
    }
}
