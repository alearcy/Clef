use crate::colors;
use clef_model::{
    midi::{Clip, ClipId, Snap},
    timeline::{MAX_TICK, QUARTER, Timeline},
};
use clef_session::{NoteMark, NotePreview};
use eframe::egui::{
    self, Align2, FontId, Id, Key, Modifiers, Pos2, Rect, Response, Sense, Stroke, Ui,
};
use std::collections::BTreeSet;

pub const RULER_HEIGHT: f32 = 24.0;
pub const LOOP_HEIGHT: f32 = 10.0;
const MIN_GRID_PIXELS: f64 = 7.0;
const LOCATOR_TARGET: f32 = 5.0;
const NOTE_TOP: f32 = 17.0;
const NOTE_BOTTOM: f32 = 4.0;
const NOTE_STROKE: f32 = 1.5;

pub struct Viewport {
    offset: u64,
    pixels_per_tick: f64,
    lines: Vec<GridLine>,
}

struct GridLine {
    x: f32,
    bar: Option<u32>,
    label: bool,
}

impl Viewport {
    #[cfg(test)]
    pub fn new(timeline: &Timeline, offset: u64, zoom: f32, width: f32) -> Self {
        Self::with_snap(timeline, offset, zoom, width, Snap::Beat)
    }

    pub fn with_snap(timeline: &Timeline, offset: u64, zoom: f32, width: f32, snap: Snap) -> Self {
        let pixels_per_tick = f64::from(zoom) / QUARTER as f64;
        let mut viewport = Self {
            offset,
            pixels_per_tick,
            lines: Vec::new(),
        };
        let end = viewport.tick_at(width);
        let mut sections = timeline.sections().peekable();
        while let Some((start, tempo)) = sections.next() {
            let section_end = sections.peek().map_or(MAX_TICK + 1, |(tick, _)| *tick);
            let first = start.max(offset);
            let end = end.min(section_end - 1);
            if first > end {
                continue;
            }
            let bar = tempo.bar_ticks();
            let beat = if snap == Snap::Off {
                tempo.beat_ticks()
            } else {
                snap.step(timeline, start)
            };
            let bar_pixels = bar as f64 * pixels_per_tick;
            let label_stride = (36.0 / bar_pixels).ceil().max(1.0) as u64;
            let (step, origin) = if beat as f64 * pixels_per_tick >= MIN_GRID_PIXELS {
                (beat, start)
            } else {
                (
                    (MIN_GRID_PIXELS / bar_pixels).ceil().max(1.0) as u64 * bar,
                    start,
                )
            };
            let mut tick = origin + (first - origin).div_ceil(step) * step;
            while tick <= end {
                let relative = tick - start;
                let bar_index = relative / bar;
                viewport.lines.push(GridLine {
                    x: viewport.x(tick),
                    bar: relative
                        .is_multiple_of(bar)
                        .then_some(tempo.bar + bar_index as u32),
                    label: bar_index.is_multiple_of(label_stride) || tick == start,
                });
                tick += step;
            }
            if step < bar && !bar.is_multiple_of(step) {
                let mut tick = start + (first - start).div_ceil(bar) * bar;
                while tick <= end {
                    let bar_index = (tick - start) / bar;
                    if !(tick - start).is_multiple_of(step) {
                        viewport.lines.push(GridLine {
                            x: viewport.x(tick),
                            bar: Some(tempo.bar + bar_index as u32),
                            label: bar_index.is_multiple_of(label_stride),
                        });
                    }
                    tick += bar;
                }
            }
        }
        viewport
    }

    pub fn x(&self, tick: u64) -> f32 {
        ((tick as f64 - self.offset as f64) * self.pixels_per_tick) as f32
    }

    pub fn tick_at(&self, x: f32) -> u64 {
        (self.offset as f64 + f64::from(x) / self.pixels_per_tick)
            .round()
            .clamp(0.0, MAX_TICK as f64) as u64
    }

    pub fn ruler(
        &self,
        ui: &mut Ui,
        width: f32,
        tick: u64,
        timeline: &Timeline,
        enabled: bool,
    ) -> Option<u64> {
        let (rect, response) = allocate(ui, width, RULER_HEIGHT, enabled);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Timeline ruler")
        });
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, colors::FIELD);
        for line in &self.lines {
            let x = rect.left() + line.x;
            let y = if line.bar.is_some() {
                rect.center().y
            } else {
                rect.bottom() - 5.0
            };
            painter.vline(
                x,
                y..=rect.bottom(),
                Stroke::new(1.0_f32, colors::SECONDARY),
            );
            if let Some(bar) = line.bar.filter(|_| line.label) {
                painter.text(
                    Pos2::new(x + 3.0, rect.top() + 2.0),
                    Align2::LEFT_TOP,
                    bar,
                    FontId::proportional(11.0),
                    colors::TEXT,
                );
            }
        }
        let pointer = response.interact_pointer_pos();
        let preview = pointer
            .filter(|_| response.dragged())
            .map(|pos| self.tick_at(pos.x - rect.left()));
        self.paint_playhead(ui, rect, preview.unwrap_or(tick));
        paint_focus(ui, &response);
        if enabled && (response.clicked() || response.drag_stopped()) {
            return pointer
                .map(|pos| self.tick_at(pos.x - rect.left()))
                .or(Some(tick));
        }
        if enabled && response.has_focus() {
            return ui.input_mut(|input| {
                if input.consume_key(Modifiers::NONE, Key::ArrowLeft) {
                    let previous = tick.saturating_sub(1);
                    let (start, tempo) = timeline.section_at(previous);
                    Some(start + (previous - start) / tempo.beat_ticks() * tempo.beat_ticks())
                } else if input.consume_key(Modifiers::NONE, Key::ArrowRight) {
                    let (start, tempo) = timeline.section_at(tick);
                    Some(
                        (start + ((tick - start) / tempo.beat_ticks() + 1) * tempo.beat_ticks())
                            .min(MAX_TICK),
                    )
                } else {
                    None
                }
            });
        }
        None
    }

    pub fn loop_band(
        &self,
        ui: &mut Ui,
        width: f32,
        timeline: &Timeline,
        enabled: bool,
        gesture: &mut Option<LoopGesture>,
        pending: Option<[u64; 2]>,
    ) -> Option<[u64; 2]> {
        let (rect, response) = allocate(ui, width, LOOP_HEIGHT, enabled);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Loop locators")
        });
        let range = timeline.loop_range;
        let left = rect.left() + self.x(range[0]);
        let right = rect.left() + self.x(range[1]);
        let mut completed = None;
        if !enabled || ui.input(|i| i.key_pressed(Key::Escape)) {
            *gesture = None;
        } else {
            if response.drag_started()
                && let Some(origin) = ui.input(|i| i.pointer.press_origin())
            {
                let left_distance = (origin.x - left).abs();
                let right_distance = (origin.x - right).abs();
                let kind = if left_distance.min(right_distance) <= LOCATOR_TARGET {
                    if left_distance <= right_distance {
                        LoopEdit::Left
                    } else {
                        LoopEdit::Right
                    }
                } else if (left..right).contains(&origin.x) {
                    LoopEdit::Move
                } else {
                    LoopEdit::Draw
                };
                *gesture = Some(LoopGesture {
                    kind,
                    anchor: self.tick_at(origin.x - rect.left()),
                    range,
                    preview: range,
                });
            }
            if let Some(edit) = gesture.as_mut()
                && let Some(pos) = response.interact_pointer_pos()
            {
                let free = ui.input(|i| i.modifiers.alt);
                edit.update(self.tick_at(pos.x - rect.left()), timeline, free);
            }
            if response.drag_stopped() {
                completed = gesture.take().map(|edit| edit.preview);
            }
        }
        let completed = completed.filter(|[left, right]| left < right);
        let display = completed
            .or_else(|| gesture.as_ref().map(|edit| edit.preview))
            .or(pending)
            .unwrap_or(range);
        let area = Rect::from_min_max(
            Pos2::new(rect.left() + self.x(display[0]), rect.top()),
            Pos2::new(rect.left() + self.x(display[1]), rect.bottom()),
        );
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, colors::CANVAS);
        if area.intersects(rect) {
            painter.rect_filled(
                area.intersect(rect),
                0.0,
                colors::ACCENT.gamma_multiply(if timeline.looping { 0.6 } else { 0.25 }),
            );
        }
        for x in [area.left(), area.right()] {
            if x >= rect.left() && x <= rect.right() {
                painter.vline(x, rect.y_range(), Stroke::new(2.0_f32, colors::ACCENT));
            }
        }
        paint_focus(ui, &response);
        response.on_hover_text(
            "Loop locators. Drag to draw, move, or resize. Option bypasses beat snap.",
        );
        completed
    }

    pub fn lane(&self, ui: &Ui, rect: Rect, tick: u64) {
        self.lane_grid(ui, rect);
        self.paint_playhead(ui, rect, tick);
    }

    pub fn lane_grid(&self, ui: &Ui, rect: Rect) {
        ui.painter_at(rect).rect_filled(rect, 0.0, colors::CANVAS);
        self.grid_lines(ui, rect);
    }

    pub fn grid_lines(&self, ui: &Ui, rect: Rect) {
        let painter = ui.painter_at(rect);
        for line in &self.lines {
            painter.vline(
                rect.left() + line.x,
                rect.y_range(),
                Stroke::new(
                    1.0_f32,
                    colors::SECONDARY.gamma_multiply(if line.bar.is_some() { 0.25 } else { 0.12 }),
                ),
            );
        }
    }

    pub fn clip_rect(&self, lane: Rect, [start, end]: [u64; 2]) -> Rect {
        let left = lane.left() + self.x(start);
        let right = lane.left() + self.x(end);
        Rect::from_min_max(
            Pos2::new(left, lane.top() + 4.0),
            Pos2::new(right.max(left + 1.0), lane.bottom() - 4.0),
        )
        .intersect(lane)
    }

    fn notes(
        &self,
        painter: &egui::Painter,
        [lane, area]: [Rect; 2],
        marks: impl Iterator<Item = NoteMark>,
        pitch: [f32; 2],
        ink: egui::Color32,
        playhead: u64,
    ) {
        let top = area.top() + NOTE_TOP;
        let bottom = area.bottom() - NOTE_BOTTOM;
        for mark in marks {
            let left = lane.left() + self.x(mark.start);
            let end = mark.end.unwrap_or(playhead).max(mark.start + 1);
            let right = (lane.left() + self.x(end)).max(left + 1.0);
            if left >= area.right() || right <= area.left() {
                continue;
            }
            let position = (f32::from(mark.key) - pitch[0]) / (pitch[1] - pitch[0]);
            let y = bottom - position * (bottom - top);
            painter.hline(
                left.max(area.left())..=right.min(area.right()),
                y,
                Stroke::new(NOTE_STROKE, ink),
            );
        }
    }

    pub fn clips(
        &self,
        ui: &Ui,
        lane: Rect,
        timeline: &Timeline,
        track: &str,
        clips: &[Clip],
        selection: (&BTreeSet<ClipId>, bool),
    ) -> Option<(Id, ClipId)> {
        if !ui.is_rect_visible(lane) {
            return None;
        }
        let stop = self.tick_at(lane.width());
        let mut visible: Vec<_> = clips
            .iter()
            .take_while(|clip| clip.start <= stop)
            .filter(|clip| clip.end > self.offset)
            .collect();
        visible.sort_by_key(|clip| clip.layer());
        let mut focus = None;
        for clip in visible {
            let area = self.clip_rect(lane, [clip.start, clip.end]);
            if !area.is_positive() {
                continue;
            }
            let painter = ui.painter_at(area);
            painter.rect_filled(area, 3.0, colors::PEACOCK);
            let marks = || {
                clip.active_events()
                    .filter(|event| event.tick < clip.end)
                    .filter_map(|event| {
                        let mut mark = NoteMark::from_event(event)?;
                        let end = clip.playback_run(event.tick)[1];
                        mark.end = mark.end.map(|tick| tick.min(end));
                        Some(mark)
                    })
            };
            let pitch = pitch_range(marks());
            self.notes(
                &painter,
                [lane, area],
                marks(),
                pitch,
                colors::TEXT,
                clip.end,
            );
            if let (Some(period), Some(end)) = (clip.cycle(), clip.repeat_end()) {
                let minimum_step = (8.0 / (period as f64 * self.pixels_per_tick))
                    .ceil()
                    .max(1.0) as u64;
                let step = period * minimum_step;
                let origin = clip.start + period - clip.repeat_phase();
                let first = self.offset.saturating_sub(origin).div_ceil(step);
                for tick in (origin + first * step..end.min(stop + 1)).step_by(step as usize) {
                    let x = lane.left() + self.x(tick);
                    painter.vline(
                        x,
                        area.y_range(),
                        Stroke::new(1.0_f32, colors::TEXT.gamma_multiply(0.5)),
                    );
                }
            }
            let takes = clip.takes.len();
            if area.width() >= 32.0 {
                let font = FontId::proportional(11.0);
                let title = if takes == 1 {
                    "MIDI".into()
                } else {
                    format!("MIDI · {takes} takes")
                };
                let width = area.width() - 12.0;
                let mut text = painter.layout_no_wrap(title, font.clone(), colors::TEXT);
                if text.size().x > width && takes > 1 {
                    text = painter.layout_no_wrap("MIDI".into(), font, colors::TEXT);
                }
                if text.size().x <= width {
                    painter.galley(area.left_top() + egui::vec2(6.0, 5.0), text, colors::TEXT);
                }
            }
            let pos = |tick| {
                let pos = timeline.position(tick);
                format!("{}.{:02}.{:04}", pos.bar, pos.beat, pos.tick)
            };
            let name = format!(
                "{track}, MIDI clip, {} to {}, {takes} {}",
                pos(clip.start),
                pos(clip.end),
                if takes == 1 { "take" } else { "takes" }
            );
            let response = ui.interact(
                area,
                ui.id().with(("midi clip", clip.id().0)),
                Sense::focusable_noninteractive(),
            );
            let is_selected = selection.0.contains(&clip.id());
            if is_selected {
                painter.rect_stroke(
                    area,
                    3.0,
                    Stroke::new(1.0_f32, colors::TEXT),
                    egui::StrokeKind::Inside,
                );
                if selection.1 {
                    self.edge_handles(ui, lane, clip, None);
                }
            }
            if response.has_focus() {
                focus = Some((response.id, clip.id()));
                painter.rect_stroke(
                    area,
                    3.0,
                    Stroke::new(1.0_f32, colors::ACCENT),
                    egui::StrokeKind::Inside,
                );
            }
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::SelectableLabel,
                    true,
                    is_selected,
                    &name,
                )
            });
            response.on_hover_text(name);
        }
        focus
    }

    pub fn clip_at<'a>(&self, lane: Rect, clips: &'a [Clip], point: Pos2) -> Option<&'a Clip> {
        if !lane.contains(point) {
            return None;
        }
        clips
            .iter()
            .filter(|clip| self.clip_rect(lane, [clip.start, clip.end]).contains(point))
            .max_by_key(|clip| clip.layer())
    }

    pub fn edge_target(&self, lane: Rect, clip: &Clip, point: Pos2) -> Option<bool> {
        let right = lane.left() + self.x(clip.end);
        (right <= lane.right()
            && point.x > right - 6.0
            && right - (lane.left() + self.x(clip.start)).max(lane.left()) > 12.0)
            .then_some(point.y < lane.center().y)
    }

    pub fn edge_handles(&self, ui: &Ui, lane: Rect, clip: &Clip, hover: Option<bool>) {
        let area = self.clip_rect(lane, [clip.start, clip.end]);
        let right = lane.left() + self.x(clip.end);
        if area.width() <= 12.0 || right > lane.right() {
            return;
        }
        let painter = ui.painter_at(area);
        for (repeat, y) in [(true, area.top() + 5.0), (false, area.bottom() - 5.0)] {
            let fill = if hover == Some(repeat) {
                colors::ACCENT
            } else {
                colors::TEXT
            };
            painter.rect_filled(
                Rect::from_center_size(Pos2::new(right - 3.0, y), egui::vec2(4.0, 5.0)),
                1.0,
                fill,
            );
        }
    }

    pub fn edit_preview(
        &self,
        ui: &Ui,
        [lane, visible]: [Rect; 2],
        range: [u64; 2],
        tick: u64,
        snap: bool,
        label: &str,
    ) {
        let area = self.clip_rect(lane, range);
        if !area.is_positive() {
            return;
        }
        let painter = ui.painter_at(visible);
        painter.rect_filled(area, 3.0, colors::PEACOCK.gamma_multiply(0.65));
        painter.rect_stroke(
            area,
            3.0,
            Stroke::new(1.0_f32, colors::ACCENT),
            egui::StrokeKind::Inside,
        );
        if snap {
            let x = lane.left() + self.x(tick);
            painter.vline(x, lane.y_range(), Stroke::new(1.0_f32, colors::ACCENT));
        }
        let font = FontId::proportional(11.0);
        let text = painter.layout_no_wrap(label.to_owned(), font, colors::TEXT);
        let size = text.size() + egui::vec2(12.0, 4.0);
        if size.x <= visible.width() && size.y <= visible.height() {
            let x =
                (lane.left() + self.x(tick) + 4.0).clamp(visible.left(), visible.right() - size.x);
            let bubble = Rect::from_min_size(Pos2::new(x, visible.top() + 2.0), size);
            painter.rect_filled(bubble, 2.0, colors::PANEL);
            painter.galley(bubble.min + egui::vec2(6.0, 2.0), text, colors::TEXT);
        }
    }

    pub fn recording(
        &self,
        ui: &Ui,
        lane: Rect,
        range: [u64; 2],
        loop_range: [u64; 2],
        notes: Option<&NotePreview>,
    ) {
        if !ui.is_rect_visible(lane) {
            return;
        }
        let area = self.clip_rect(lane, range);
        let painter = ui.painter_at(lane);
        let pitch = pitch_range(
            notes
                .into_iter()
                .flat_map(|notes| notes.current.iter().chain(&notes.previous).copied()),
        );
        if let Some(notes) = notes.filter(|notes| !notes.previous.is_empty()) {
            let background = self.clip_rect(lane, loop_range);
            if background.is_positive() {
                painter.rect_filled(background, 3.0, colors::PEACOCK.gamma_multiply(0.5));
                self.notes(
                    &painter,
                    [lane, background],
                    notes.previous.iter().copied(),
                    pitch,
                    colors::SECONDARY,
                    loop_range[1],
                );
            }
        }
        if area.is_positive() {
            painter.rect_filled(area, 3.0, colors::DANGER.gamma_multiply(0.23));
            painter.rect_stroke(
                area,
                3.0,
                Stroke::new(1.0_f32, colors::DANGER),
                egui::StrokeKind::Inside,
            );
            if let Some(notes) = notes {
                self.notes(
                    &painter,
                    [lane, area],
                    notes.current.iter().copied(),
                    pitch,
                    colors::DANGER,
                    range[1],
                );
            }
        }
    }

    pub fn paint_playhead(&self, ui: &Ui, rect: Rect, tick: u64) {
        let x = rect.left() + self.x(tick);
        if (rect.left()..rect.right()).contains(&x) {
            ui.painter_at(rect)
                .vline(x, rect.y_range(), Stroke::new(1.0_f32, colors::ACCENT));
        }
    }
}

pub fn marquee(ui: &Ui, bounds: Rect, selection: Rect) {
    let painter = ui.painter_at(bounds);
    painter.rect_filled(selection, 0.0, colors::ACCENT.gamma_multiply(0.12));
    painter.rect_stroke(
        selection,
        0.0,
        Stroke::new(1.0_f32, colors::ACCENT),
        egui::StrokeKind::Inside,
    );
}

pub fn snap_control(ui: &mut Ui, value: &mut Snap) {
    let label = ui.label("Snap");
    egui::ComboBox::from_id_salt("arrangement-snap")
        .selected_text(value.label())
        .show_ui(ui, |ui| {
            for snap in Snap::ALL {
                ui.selectable_value(value, snap, snap.label());
            }
        })
        .response
        .labelled_by(label.id);
}

pub fn clip_menu(
    response: &Response,
    rows: impl IntoIterator<Item = (&'static str, String, bool)>,
    anchor: Option<Pos2>,
    open: bool,
) -> Option<usize> {
    let mut command = None;
    let mut popup = egui::Popup::context_menu(response);
    if let Some(anchor) = anchor {
        popup = popup.anchor(egui::PopupAnchor::Position(anchor));
    }
    if open {
        popup = popup.open_memory(Some(egui::SetOpenCommand::Bool(true)));
    }
    popup.show(|ui| {
        for (index, (label, shortcut, enabled)) in rows.into_iter().enumerate() {
            if ui
                .add_enabled(enabled, egui::Button::new(label).shortcut_text(shortcut))
                .clicked()
            {
                command = Some(index);
                ui.close();
            }
        }
    });
    command
}

fn pitch_range(marks: impl Iterator<Item = NoteMark>) -> [f32; 2] {
    let (low, high) = marks.fold((127_u8, 0_u8), |(low, high), mark| {
        (low.min(mark.key), high.max(mark.key))
    });
    let midpoint = (f32::from(low) + f32::from(high)) * 0.5;
    let half = ((f32::from(high) - f32::from(low)) * 0.5 + 1.0).max(6.0);
    [midpoint - half, midpoint + half]
}

fn allocate(ui: &mut Ui, width: f32, height: f32, enabled: bool) -> (Rect, Response) {
    ui.allocate_exact_size(
        egui::vec2(width, height),
        if enabled {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        },
    )
}

pub(super) fn paint_focus(ui: &Ui, response: &Response) {
    if response.has_focus() {
        ui.painter_at(response.rect).rect_stroke(
            response.rect,
            0.0,
            Stroke::new(1.0_f32, colors::ACCENT),
            egui::StrokeKind::Inside,
        );
    }
}

enum LoopEdit {
    Draw,
    Move,
    Left,
    Right,
}

pub struct LoopGesture {
    kind: LoopEdit,
    anchor: u64,
    range: [u64; 2],
    preview: [u64; 2],
}

impl LoopGesture {
    fn update(&mut self, tick: u64, timeline: &Timeline, free: bool) {
        let snap = |tick| {
            if free {
                tick
            } else {
                timeline.nearest_beat(tick)
            }
        };
        let [left, right] = self.range;
        self.preview = match self.kind {
            LoopEdit::Draw => {
                let anchor = snap(self.anchor);
                let tick = snap(tick);
                [anchor.min(tick), anchor.max(tick)]
            }
            LoopEdit::Move => {
                let offset = tick as i64 - self.anchor as i64;
                let length = right - left;
                let start = snap(left.saturating_add_signed(offset)).min(MAX_TICK - length);
                [start, start + length]
            }
            LoopEdit::Left => [snap(tick).min(right - 1), right],
            LoopEdit::Right => [left, snap(tick).max(left + 1)],
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, PointerButton, RawInput, Shape};

    #[test]
    fn current_note_grows_over_the_previous_pass_on_the_same_pitch() {
        let ctx = egui::Context::default();
        let viewport = Viewport::new(&Timeline::default(), 0, 96.0, 400.0);
        let preview = NotePreview {
            previous: vec![NoteMark {
                start: 0,
                end: Some(480),
                key: 60,
            }],
            current: vec![NoteMark {
                start: 240,
                end: None,
                key: 60,
            }],
        };
        let lane = Rect::from_min_size(Pos2::new(100.0, 100.0), egui::vec2(400.0, 44.0));
        let render = |end| {
            ctx.run_ui(egui::RawInput::default(), |ui| {
                viewport.recording(ui, lane, [0, end], [0, 960], Some(&preview));
            })
            .shapes
            .into_iter()
            .filter_map(|shape| match shape.shape {
                egui::Shape::LineSegment { points, stroke } if stroke.width == NOTE_STROKE => {
                    Some((points, stroke.color))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
        };
        let [previous, held]: [_; 2] = render(360).try_into().unwrap();
        let [same_previous, extended]: [_; 2] = render(720).try_into().unwrap();
        assert_eq!(previous, same_previous);
        assert_eq!(previous.1, colors::SECONDARY);
        assert_eq!(held.1, colors::DANGER);
        assert_eq!(held.0[0].y, previous.0[0].y);
        assert_eq!(held.0[0], extended.0[0]);
        assert_eq!(held.0[0].x, 124.0);
        assert_eq!(held.0[1].x, 136.0);
        assert_eq!(extended.0[1].x, 172.0);
    }

    #[test]
    fn committed_clip_draws_active_notes_and_clips_their_tails() {
        use clef_model::midi::{Arrangement, Event, Message, Mode};

        let ctx = egui::Context::default();
        let timeline = Timeline::default();
        let viewport = Viewport::new(&timeline, 240, 96.0, 400.0);
        let note = |tick, key| Event {
            tick,
            order: 0,
            message: Message::Note {
                channel: 0,
                key,
                velocity: 90,
                end: 1200,
                release: 0,
                release_order: 1,
            },
        };
        let mut arrangement = Arrangement::default();
        arrangement
            .record(
                1,
                Mode::NewTake,
                [0, 960],
                vec![vec![note(480, 42)], vec![note(120, 60)]],
                false,
            )
            .unwrap();
        let lane = Rect::from_min_size(Pos2::new(100.0, 100.0), egui::vec2(400.0, 44.0));
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            viewport.clips(
                ui,
                lane,
                &timeline,
                "Piano",
                arrangement.clips(1),
                (&BTreeSet::new(), true),
            );
        });
        let actual: Vec<_> = output
            .shapes
            .into_iter()
            .filter_map(|shape| match shape.shape {
                egui::Shape::LineSegment { points, stroke }
                    if stroke.width == NOTE_STROKE && stroke.color == colors::TEXT =>
                {
                    Some([points[0].x, points[1].x])
                }
                _ => None,
            })
            .collect();
        assert_eq!(actual, vec![[100.0, 172.0]]);
    }

    #[test]
    fn paints_new_locators_on_release_and_while_waiting_for_session() {
        let context = egui::Context::default();
        let timeline = Timeline {
            loop_range: [0, QUARTER],
            ..Timeline::default()
        };
        let viewport = Viewport::new(&timeline, 0, 48.0, 400.0);
        let start = Pos2::new(96.0, LOOP_HEIGHT / 2.0);
        let end = Pos2::new(192.0, LOOP_HEIGHT / 2.0);
        let mut gesture = None;
        let mut completed = None;
        let mut frame = |events, enabled, pending| {
            let output = context.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 60.0))),
                    events,
                    ..RawInput::default()
                },
                |ui| {
                    completed =
                        viewport.loop_band(ui, 400.0, &timeline, enabled, &mut gesture, pending);
                },
            );
            let locators: Vec<_> = output
                .shapes
                .into_iter()
                .filter_map(|shape| match shape.shape {
                    Shape::LineSegment { points, stroke } if stroke.color == colors::ACCENT => {
                        Some(points[0].x)
                    }
                    _ => None,
                })
                .collect();
            (completed, locators)
        };
        let button = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };

        let (_, original) = frame(vec![], true, None);
        frame(
            vec![Event::PointerMoved(start), button(start, true)],
            true,
            None,
        );
        let (_, preview) = frame(vec![Event::PointerMoved(end)], true, None);
        let (range, released) = frame(vec![button(end, false)], true, None);
        assert_eq!(range, Some([2 * QUARTER, 4 * QUARTER]));
        assert_eq!(preview, vec![start.x, end.x]);
        assert_eq!(released, preview);
        assert_eq!(frame(vec![], false, range).1, preview);
        assert_eq!(frame(vec![], true, None).1, original);
    }
}
