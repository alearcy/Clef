use super::timeline::Viewport;
use crate::{colors, theme};
use clef_model::{
    midi::{Clip, ClipId, EventId, Message, RollEvent, continuous_cc, discrete_cc},
    timeline::Timeline,
};
use eframe::egui::{self, Align2, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};
use std::collections::BTreeSet;

pub struct PitchGrid<'a> {
    pub keys: Rect,
    pub grid: Rect,
    pub time: &'a Viewport,
    pub lowest: u8,
    pub rows: u8,
}

impl PitchGrid<'_> {
    fn row(&self) -> f32 {
        self.grid.height() / f32::from(self.rows)
    }

    pub fn key(&self, y: f32) -> u8 {
        self.lowest
            + ((self.grid.bottom() - y) / self.row())
                .floor()
                .clamp(0.0, f32::from(self.rows - 1)) as u8
    }

    pub fn note(&self, tick: u64, end: u64, pitch: u8) -> Rect {
        let x = self.grid.left() + self.time.x(tick);
        let y = self.grid.bottom() - (f32::from(pitch) - f32::from(self.lowest) + 1.0) * self.row();
        Rect::from_min_max(
            egui::pos2(x, y + theme::ROLL_NOTE_INSET),
            egui::pos2(
                (self.grid.left() + self.time.x(end)).max(x + theme::ROLL_NOTE_MIN),
                y + self.row() - theme::ROLL_NOTE_INSET,
            ),
        )
    }

    pub fn paint(&self, ui: &Ui, bounds: [u64; 2], playhead: u64) {
        let painter = ui.painter_at(self.grid);
        let keys = ui.painter_at(self.keys);
        for pitch in self.lowest..self.lowest + self.rows {
            let bottom = self.grid.bottom() - f32::from(pitch - self.lowest) * self.row();
            let top = bottom - self.row();
            let dark = matches!(pitch % 12, 1 | 3 | 6 | 8 | 10);
            let background = if dark { colors::CANVAS } else { colors::PANEL };
            painter.rect_filled(
                Rect::from_x_y_ranges(self.grid.x_range(), top..=bottom),
                0.0,
                background,
            );
            let width = self.keys.width() * if dark { 0.57 } else { 1.0 };
            keys.rect_filled(
                Rect::from_min_max(
                    egui::pos2(self.keys.left(), top),
                    egui::pos2(self.keys.left() + width, bottom - theme::ROLL_NOTE_INSET),
                ),
                0.0,
                if dark { colors::CANVAS } else { colors::FIELD },
            );
            if pitch % 12 == 0 {
                keys.text(
                    egui::pos2(self.keys.center().x, (bottom + top) * 0.5),
                    Align2::CENTER_CENTER,
                    format!("C{}", i16::from(pitch / 12) - 1),
                    FontId::proportional(theme::ROLL_CAPTION),
                    colors::TEXT,
                );
                painter.hline(
                    self.grid.x_range(),
                    bottom,
                    Stroke::new(theme::ROLL_LINE, colors::DIVIDER),
                );
            }
        }
        self.time.grid_lines(ui, self.grid);
        for tick in bounds {
            paint_marker(ui, self.grid, self.time, tick, colors::SECONDARY);
        }
        self.time.paint_playhead(ui, self.grid, playhead);
    }
}

pub fn note_focus(
    ui: &Ui,
    rect: Rect,
    clip: ClipId,
    note: &RollEvent,
    active: bool,
    enabled: bool,
) -> Option<egui::Id> {
    let Message::Note {
        key,
        channel,
        velocity,
        ..
    } = note.source.message
    else {
        return None;
    };
    let id = ui.id().with(("piano-roll-note", clip, note.id, note.tick));
    let response = ui.interact(rect, id, Sense::focusable_noninteractive());
    let pitch = [
        "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
    ][usize::from(key % 12)];
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            enabled,
            active,
            format!(
                "{pitch}{}, channel {}, velocity {}",
                i16::from(key / 12) - 1,
                channel + 1,
                velocity
            ),
        )
    });
    if response.has_focus() {
        ui.painter_at(rect).rect_stroke(
            rect,
            theme::ROLL_RADIUS,
            Stroke::new(theme::ROLL_LINE, colors::ACCENT),
            egui::StrokeKind::Inside,
        );
        Some(id)
    } else {
        None
    }
}

pub fn summary(ui: &mut Ui, name: &str, active: bool, repeated: bool) -> bool {
    let label = if repeated {
        format!("{name} · MIDI · Repeat source")
    } else {
        format!("{name} · MIDI")
    };
    ui.add(egui::Button::new(&label).selected(active).truncate())
        .on_hover_text(label)
        .clicked()
}

pub fn heading(ui: &mut Ui, title: &str) {
    ui.label(title);
}

pub fn button(ui: &mut Ui, text: &str, enabled: bool) -> bool {
    super::transport::button(ui, text, text, enabled, false)
}

pub fn close(ui: &mut Ui) -> bool {
    super::transport::button(ui, "Close", "Close Piano Roll", true, false)
}

pub fn toggle(ui: &mut Ui, text: &str, value: &mut bool) {
    if super::transport::button(ui, text, text, true, *value) {
        *value = !*value;
    }
}

pub fn number<T: egui::emath::Numeric + PartialOrd>(
    ui: &mut Ui,
    label: &str,
    value: &mut T,
    range: std::ops::RangeInclusive<T>,
) {
    let name = ui.label(label);
    ui.add(egui::DragValue::new(value).range(range))
        .labelled_by(name.id);
}

pub fn cc_picker(ui: &mut Ui, current: &mut u8) {
    let label = ui.label("Controller");
    egui::ComboBox::from_id_salt("piano-roll-controller")
        .selected_text(cc_name(*current))
        .show_ui(ui, |ui| {
            for cc in 0..128 {
                ui.selectable_value(current, cc, cc_name(cc));
            }
        })
        .response
        .labelled_by(label.id);
}

pub fn cc_name(cc: u8) -> String {
    match cc {
        1 => "Modulation · CC1".into(),
        7 => "Volume · CC7".into(),
        11 => "Expression · CC11".into(),
        64 => "Sustain · CC64".into(),
        _ => format!("CC{cc}"),
    }
}

pub fn ruler(ui: &mut Ui, time: &Viewport, width: f32, tick: u64, timeline: &Timeline) {
    ui.horizontal(|ui| {
        ui.allocate_space(Vec2::new(theme::ROLL_KEYBED, super::timeline::RULER_HEIGHT));
        time.ruler(ui, width, tick, timeline, false);
    });
}

pub struct Lane<'a> {
    pub time: &'a Viewport,
    pub width: f32,
    pub height: f32,
    pub lowest: u8,
    pub rows: u8,
    pub enabled: bool,
    pub name: &'a str,
}

pub fn pitch_lane<'a>(ui: &mut Ui, lane: Lane<'a>) -> (PitchGrid<'a>, Response, Response) {
    let Lane {
        time,
        width,
        height,
        lowest,
        rows,
        enabled,
        name,
    } = lane;
    let result = ui.horizontal(|ui| {
        let (keys, audition) =
            ui.allocate_exact_size(Vec2::new(theme::ROLL_KEYBED, height), Sense::click());
        let (grid, edit) =
            ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());
        labelled(&audition, enabled, &format!("{name} piano keyboard"));
        labelled(&edit, enabled, &format!("{name} MIDI note grid"));
        super::timeline::paint_focus(ui, &audition);
        super::timeline::paint_focus(ui, &edit);
        (
            PitchGrid {
                keys,
                grid,
                time,
                lowest,
                rows,
            },
            audition,
            edit,
        )
    });
    result.inner
}

pub fn labelled(response: &Response, enabled: bool, name: &str) {
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, enabled, name));
}

pub fn paint_note(
    ui: &Ui,
    grid: Rect,
    area: Rect,
    selected: bool,
    active: bool,
    primary: bool,
    preview: bool,
) {
    if !area.intersects(grid) {
        return;
    }
    let painter = ui.painter_at(grid);
    let fill = if preview {
        colors::ACCENT.gamma_multiply(0.54)
    } else if active {
        colors::PEACOCK
    } else {
        colors::PEACOCK.gamma_multiply(if selected { 0.5 } else { 0.35 })
    };
    painter.rect_filled(area, theme::ROLL_RADIUS, fill);
    if selected || preview {
        painter.rect_stroke(
            area,
            theme::ROLL_RADIUS,
            Stroke::new(theme::ROLL_SELECTION, colors::TEXT),
            egui::StrokeKind::Inside,
        );
    }
    if primary && area.width() >= theme::ROLL_HANDLE * 3.0 {
        for x in [
            area.left() + theme::ROLL_HANDLE,
            area.right() - theme::ROLL_HANDLE,
        ] {
            painter.vline(
                x,
                area.y_range(),
                Stroke::new(theme::ROLL_LINE, colors::TEXT),
            );
        }
    }
}

pub fn comp_strip(ui: &mut Ui, time: &Viewport, width: f32, ranges: &[[u64; 2]], playhead: u64) {
    let rect = Rect::from_min_size(ui.cursor().min, Vec2::new(width, 3.0));
    ui.allocate_space(rect.size());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, colors::CANVAS);
    for [start, end] in ranges {
        let left = rect.left() + time.x(*start);
        let right = rect.left() + time.x(*end);
        painter.rect_filled(
            Rect::from_min_max(
                egui::pos2(left.max(rect.left()), rect.top()),
                egui::pos2(right.min(rect.right()), rect.bottom()),
            ),
            0.0,
            colors::ACCENT.gamma_multiply(0.7),
        );
    }
    painter.vline(
        rect.left() + time.x(playhead),
        rect.y_range(),
        Stroke::new(1.0_f32, colors::TEXT),
    );
}

pub fn paint_marker(ui: &Ui, rect: Rect, time: &Viewport, tick: u64, color: egui::Color32) {
    ui.painter_at(rect).vline(
        rect.left() + time.x(tick),
        rect.y_range(),
        Stroke::new(theme::ROLL_LINE, color),
    );
}

pub fn repeat_markers(ui: &Ui, rect: Rect, time: &Viewport, clip: &Clip, window: [u64; 2]) {
    if let (Some(period), Some(end)) = (clip.cycle(), clip.repeat_end()) {
        let origin = clip.start + period - clip.repeat_phase();
        let minimum = time
            .tick_at(theme::ROLL_HIT)
            .saturating_sub(time.tick_at(0.0));
        let step = period.saturating_mul(minimum.div_ceil(period).max(1));
        let first = origin + window[0].saturating_sub(origin).div_ceil(step) * step;
        for tick in (first..end.min(window[1])).step_by(step as usize) {
            paint_marker(ui, rect, time, tick, colors::SECONDARY.gamma_multiply(0.6));
        }
    }
}

pub fn resize_gutter(ui: &mut Ui, height: f32, ratio: &mut f32) {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), theme::ROLL_GUTTER),
        Sense::drag(),
    );
    labelled(&response, true, "Resize Piano Roll");
    let hovering = response.hovered() || response.dragged();
    response
        .clone()
        .on_hover_cursor(egui::CursorIcon::ResizeVertical);
    ui.painter_at(rect).hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(
            theme::ROLL_LINE,
            if hovering {
                colors::ACCENT
            } else {
                colors::DIVIDER
            },
        ),
    );
    if response.dragged() {
        *ratio = (*ratio + response.drag_motion().y / height).clamp(0.3, 0.75);
    }
}

pub fn value_lane(ui: &mut Ui, name: &str, width: f32, enabled: bool) -> (Rect, Response) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [theme::ROLL_KEYBED, theme::ROLL_CONTROLLER_HEIGHT],
            egui::Label::new(name).truncate(),
        )
        .on_hover_text(name);
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(width, theme::ROLL_CONTROLLER_HEIGHT),
            Sense::click_and_drag(),
        );
        labelled(&response, enabled, name);
        ui.painter_at(rect).rect_filled(rect, 0.0, colors::CANVAS);
        super::timeline::paint_focus(ui, &response);
        (rect, response)
    })
    .inner
}

pub fn paint_key_cursor(ui: &Ui, keys: Rect, lowest: u8, rows: u8, key: u8) {
    if key < lowest || key >= lowest.saturating_add(rows) {
        return;
    }
    let row = keys.height() / f32::from(rows);
    let bottom = keys.bottom() - f32::from(key - lowest) * row;
    ui.painter_at(keys).rect_filled(
        Rect::from_min_max(
            egui::pos2(keys.left(), bottom - row),
            egui::pos2(keys.right(), bottom),
        ),
        0.0,
        colors::ACCENT.gamma_multiply(0.30),
    );
}

pub struct ValueFocus<'a> {
    pub rect: Rect,
    pub clip: ClipId,
    pub time: &'a Viewport,
    pub timeline: &'a Timeline,
    pub events: &'a [RollEvent],
    pub selection: &'a BTreeSet<EventId>,
    pub cc: Option<u8>,
    pub enabled: bool,
    pub name: &'a str,
}

/// Focus id for one projected value point.
///
/// A repeated clip projects one source event onto several ticks with the same
/// `EventId`, so the tick is part of the id. Without it, all projections of the
/// same event and controller share one widget id and egui reports an id clash.
pub(crate) fn point_id(
    base: egui::Id,
    clip: ClipId,
    event: EventId,
    tick: u64,
    cc: Option<u8>,
) -> egui::Id {
    base.with(("piano-roll-point", clip, event, tick, cc))
}

pub fn point_focus(ui: &Ui, focus: ValueFocus<'_>) -> Option<(EventId, egui::Id)> {
    let ValueFocus {
        rect,
        clip,
        time,
        timeline,
        events,
        selection,
        cc,
        enabled,
        name,
    } = focus;
    let mut found = None;
    for event in events {
        let value = match event.source.message {
            Message::Note { velocity, .. } => velocity,
            message => message.control().unwrap()[2],
        };
        let position = point(rect, time, event.tick, value);
        if !rect.contains(position) {
            continue;
        }
        let id = point_id(ui.id(), clip, event.id, event.tick, cc);
        let hit =
            Rect::from_center_size(position, Vec2::splat(theme::ROLL_HIT * 2.0)).intersect(rect);
        let response = ui.interact(hit, id, Sense::focusable_noninteractive());
        let at = timeline.position(event.tick);
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                enabled,
                selection.contains(&event.id),
                format!(
                    "{name}, bar {} beat {} tick {}, value {value}",
                    at.bar, at.beat, at.tick
                ),
            )
        });
        if response.has_focus() {
            ui.painter_at(hit).rect_stroke(
                hit,
                theme::ROLL_RADIUS,
                Stroke::new(theme::ROLL_LINE, colors::ACCENT),
                egui::StrokeKind::Inside,
            );
            found = Some((event.id, id));
        }
    }
    found
}

pub fn point(rect: Rect, time: &Viewport, tick: u64, value: u8) -> Pos2 {
    egui::pos2(
        rect.left() + time.x(tick),
        rect.bottom() - rect.height() * f32::from(value) / 127.0,
    )
}

pub fn velocity_at(rect: Rect, y: f32) -> u8 {
    ((rect.bottom() - y) / rect.height() * 127.0)
        .round()
        .clamp(0.0, 127.0) as u8
}

/// Value after a vertical drag, anchored at the value and pointer position where the
/// gesture started. The grabbed value stays under the pointer instead of jumping to it.
pub fn velocity_dragged(rect: Rect, start: u8, from_y: f32, to_y: f32) -> u8 {
    let delta = (from_y - to_y) / rect.height().max(1.0) * 127.0;
    (f32::from(start) + delta).round().clamp(0.0, 127.0) as u8
}

/// A velocity point and the end of its note extent line, clipped to the lane.
pub(crate) fn value_span(rect: Rect, time: &Viewport, tick: u64, end: u64, value: u8) -> [Pos2; 2] {
    let start = point(rect, time, tick, value);
    let end_x = (rect.left() + time.x(end.max(tick))).min(rect.right());
    [start, egui::pos2(end_x, start.y)]
}

pub fn paint_values(
    ui: &Ui,
    rect: Rect,
    time: &Viewport,
    events: &[RollEvent],
    selection: &BTreeSet<EventId>,
    cc: Option<u8>,
    clip: &Clip,
) {
    let painter = ui.painter_at(rect);
    let value = |event: &RollEvent| match event.source.message {
        Message::Note { velocity, .. } => velocity,
        message => message.control().unwrap()[2],
    };
    if let Some(cc) = cc {
        let stroke = Stroke::new(theme::ROLL_LINE, colors::PEACOCK);
        for pair in events.windows(2) {
            let [from, to] = [pair[0], pair[1]];
            if discrete_cc(cc)
                || (clip.cycle().is_some() && to.tick >= clip.playback_run(from.tick)[1])
            {
                continue;
            }
            let mut last = point(rect, time, from.tick, value(&from));
            if continuous_cc(cc)
                && from
                    .source
                    .message
                    .interpolated(to.source.message, 0.5)
                    .is_some()
            {
                for step in 1..=theme::ROLL_CURVE_STEPS {
                    let fraction = f64::from(step) / f64::from(theme::ROLL_CURVE_STEPS);
                    let tick = from.tick + ((to.tick - from.tick) as f64 * fraction).round() as u64;
                    let next = point(
                        rect,
                        time,
                        tick,
                        from.source
                            .message
                            .interpolated(to.source.message, fraction)
                            .unwrap(),
                    );
                    painter.line_segment([last, next], stroke);
                    if step * 2 == theme::ROLL_CURVE_STEPS {
                        painter.circle_stroke(
                            next,
                            theme::ROLL_POINT,
                            Stroke::new(theme::ROLL_LINE, colors::ACCENT),
                        );
                    }
                    last = next;
                }
            } else {
                let held = point(rect, time, to.tick, value(&from));
                painter.line_segment([last, held], stroke);
                painter.line_segment([held, point(rect, time, to.tick, value(&to))], stroke);
            }
        }
    }
    for event in events {
        let position = point(rect, time, event.tick, value(event));
        let color = if selection.contains(&event.id) {
            colors::ACCENT
        } else {
            colors::PEACOCK
        };
        let color = if event.active {
            color
        } else {
            color.gamma_multiply(0.35)
        };
        if cc.is_none() {
            let [start, end] = value_span(rect, time, event.tick, event.end, value(event));
            painter.line_segment([start, end], Stroke::new(theme::ROLL_LINE, color));
        }
        if cc.is_none() || cc.is_some_and(discrete_cc) {
            painter.vline(
                position.x,
                position.y..=rect.bottom(),
                Stroke::new(theme::ROLL_POINT, color),
            );
        } else {
            painter.circle_filled(position, theme::ROLL_POINT, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clef_model::midi::Snap;

    #[test]
    fn velocity_extent_runs_from_the_note_start_to_its_projected_end() {
        let timeline = Timeline::default();
        let time = Viewport::with_snap(&timeline, 0, 96.0, 960.0, Snap::Off);
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(960.0, 64.0));
        let [start, end] = value_span(rect, &time, 480, 960, 96);
        assert_eq!(start.x, time.x(480));
        assert_eq!(end.x, time.x(960));
        assert_eq!(start.y, end.y, "the extent stays at the velocity height");
        assert!(end.x > start.x);
        let [_, clipped] = value_span(rect, &time, 480, 10_000, 96);
        assert_eq!(clipped.x, rect.right(), "the extent is clipped to the lane");
    }
}
