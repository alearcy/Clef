use crate::{
    Action, App, Command, Confirm,
    primitives::timeline::{self, Viewport},
};
use clef_model::{
    midi::{Arrangement, ClipEdit, ClipId, Snap},
    project::{ArrangementView, Track},
    timeline::{MAX_TICK, Timeline},
};
use eframe::egui::{self, Id, Key, Modifiers, Pos2, Rect, Sense, Ui};
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
            &track.name,
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
                        range,
                        tick,
                        !free,
                        &label,
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
                let tracks = &self.view.project.tracks;
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
                    id,
                    name: format!("Track {id}"),
                    plugin: None,
                    mix: Default::default(),
                    state: vec![],
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
}
