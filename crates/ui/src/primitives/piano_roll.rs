use super::timeline::Viewport;
use crate::theme;
use clef_model::{
    midi::{Clip, ClipId, EventId, Message, RollEvent, continuous_cc, discrete_cc},
    timeline::Timeline,
};
use eframe::egui::{self, Align2, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};
use std::collections::BTreeSet;

/// The one pitch/time grid of the piano roll (T40): all 128 pitches, scrolled vertically.
pub struct PitchGrid<'a> {
    pub keys: Rect,
    pub grid: Rect,
    pub time: &'a Viewport,
    /// Pitch-row height in points.
    pub row: f32,
    /// Points scrolled down from the top of pitch 127.
    pub scroll: f32,
}

/// White keys within an octave, as pitch classes.
const WHITE: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];
/// Black-key width as a share of the keybed (foundations, piano roll).
const BLACK_WIDTH: f32 = 0.57;
/// Key names hide below this row height.
const NAME_ROW: f32 = 9.0;

pub fn is_black(pitch: u8) -> bool {
    matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
}

impl PitchGrid<'_> {
    /// Top edge of `pitch`'s row.
    pub fn top(&self, pitch: u8) -> f32 {
        self.grid.top() + f32::from(127 - pitch) * self.row - self.scroll
    }

    /// The pitch row under `y`.
    pub fn key(&self, y: f32) -> u8 {
        let index = ((y - self.grid.top() + self.scroll) / self.row).floor();
        127 - index.clamp(0.0, 127.0) as u8
    }

    /// The pitch under a point on the keybed: a black key where it is drawn, otherwise the white
    /// key whose (wider) body is under `y`.
    pub fn keybed_key(&self, pos: Pos2) -> u8 {
        let row_key = self.key(pos.y);
        if pos.x <= self.keys.left() + self.keys.width() * BLACK_WIDTH && is_black(row_key) {
            return row_key;
        }
        let octave_base = row_key - row_key % 12;
        let octave_bottom = self.top(octave_base) + self.row;
        let white = 12.0 * self.row / 7.0;
        let index = ((octave_bottom - pos.y) / white).floor().clamp(0.0, 6.0) as usize;
        (octave_base + WHITE[index]).min(127)
    }

    /// The pitches with a row at least partly visible, lowest first.
    pub fn visible(&self) -> std::ops::RangeInclusive<u8> {
        let high = self.key(self.grid.top());
        let low = self.key(self.grid.bottom() - 0.01);
        low..=high
    }

    pub fn note(&self, tick: u64, end: u64, pitch: u8) -> Rect {
        let x = self.grid.left() + self.time.x(tick);
        let y = self.top(pitch);
        Rect::from_min_max(
            egui::pos2(x, y + theme::ROLL_NOTE_INSET),
            egui::pos2(
                (self.grid.left() + self.time.x(end)).max(x + theme::ROLL_NOTE_MIN),
                y + self.row - theme::ROLL_NOTE_INSET,
            ),
        )
    }

    /// Grid rows (white-key rows lighter, B–C the octave rule) and the time grid.
    pub fn paint_grid(&self, ui: &Ui) {
        let painter = ui.painter_at(self.grid);
        painter.rect_filled(self.grid, 0.0, theme::CANVAS);
        for pitch in self.visible() {
            let top = self.top(pitch);
            if !is_black(pitch) {
                painter.rect_filled(
                    Rect::from_x_y_ranges(self.grid.x_range(), top..=top + self.row),
                    0.0,
                    theme::PANEL,
                );
            }
            let seam = match pitch % 12 {
                0 => Some(theme::BAR_LINE),
                5 => Some(theme::GRID),
                _ => None,
            };
            if let Some(colour) = seam {
                painter.hline(
                    self.grid.x_range(),
                    top + self.row,
                    Stroke::new(theme::ROLL_LINE, colour),
                );
            }
        }
        self.time.grid_lines(ui, self.grid);
    }

    /// A real piano keybed: seven equal white keys per octave, black keys over them.
    pub fn paint_keybed(&self, ui: &Ui, cursor: Option<u8>) {
        let painter = ui.painter_at(self.keys);
        painter.rect_filled(self.keys, 0.0, theme::BACKGROUND);
        let white = 12.0 * self.row / 7.0;
        let range = self.visible();
        let first = range.start() - range.start() % 12;
        for octave in (first..=*range.end()).step_by(12) {
            let bottom = self.top(octave) + self.row;
            for (index, class) in WHITE.iter().enumerate() {
                let pitch = octave + class;
                if pitch > 127 {
                    break;
                }
                let key_bottom = bottom - index as f32 * white;
                let key = Rect::from_min_max(
                    egui::pos2(self.keys.left(), key_bottom - white + 0.5),
                    egui::pos2(self.keys.right(), key_bottom - 0.5),
                );
                painter.rect_filled(key, 1.0, theme::CAP);
                // T41 round 3 (S8): a faint shade toward the key's front edge gives depth.
                paint_front_shade(&painter, key, theme::KEY_FRONT_SHADE);
                if cursor == Some(pitch) {
                    painter.rect_filled(key, 1.0, theme::ACCENT.gamma_multiply(0.45));
                }
                if *class == 0 && self.row >= NAME_ROW {
                    painter.text(
                        egui::pos2(self.keys.right() - 3.0, key.center().y),
                        Align2::RIGHT_CENTER,
                        format!("C{}", i16::from(pitch / 12) - 1),
                        theme::text::font(theme::ROLL_CAPTION),
                        theme::BACKGROUND,
                    );
                }
            }
            for class in [1u8, 3, 6, 8, 10] {
                let pitch = octave + class;
                if pitch > 127 {
                    break;
                }
                let top = self.top(pitch);
                let key = Rect::from_min_max(
                    egui::pos2(self.keys.left(), top),
                    egui::pos2(
                        self.keys.left() + self.keys.width() * BLACK_WIDTH,
                        top + self.row,
                    ),
                );
                // T41 round 3 (S8): a short soft drop shadow onto the white keys, and a lit
                // front edge on the black key.
                for (offset, alpha) in [(2.5, 28), (1.5, 48), (0.75, 64)] {
                    painter.rect_filled(
                        key.translate(egui::vec2(offset, offset)),
                        1.5,
                        egui::Color32::from_black_alpha(alpha),
                    );
                }
                painter.rect_filled(key, 1.0, theme::BACKGROUND);
                painter.rect_filled(
                    Rect::from_min_max(
                        egui::pos2(key.right() - 2.0, key.top() + 1.0),
                        egui::pos2(key.right() - 1.0, key.bottom() - 1.0),
                    ),
                    0.0,
                    egui::Color32::from_white_alpha(22),
                );
                if cursor == Some(pitch) {
                    painter.rect_filled(key, 1.0, theme::ACCENT.gamma_multiply(0.45));
                }
            }
        }
    }

    /// Clip start and end markers.
    pub fn paint_bounds(&self, ui: &Ui, bounds: [u64; 2]) {
        for tick in bounds {
            paint_marker(ui, self.grid, self.time, tick, theme::SECONDARY);
        }
    }
}

/// T41 round 3 (S8): a shade over the last fifth of a white key, darker toward its front edge.
fn paint_front_shade(painter: &egui::Painter, key: Rect, shade: egui::Color32) {
    let band = (key.width() * 0.2).max(4.0);
    let steps = 4;
    for step in 0..steps {
        let fraction = (step + 1) as f32 / steps as f32;
        let left = key.right() - band * (1.0 - step as f32 / steps as f32);
        painter.rect_filled(
            Rect::from_min_max(egui::pos2(left, key.top()), key.max),
            egui::CornerRadius::ZERO,
            shade.gamma_multiply(0.35 * fraction),
        );
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
            Stroke::new(theme::ROLL_LINE, theme::ACCENT),
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
    super::text_button::TextButton::new(&label)
        .toggle(active)
        .tooltip(&label)
        .show(ui)
        .clicked()
}

pub fn button(ui: &mut Ui, text: &str, enabled: bool) -> bool {
    super::transport::button(ui, text, text, enabled, false)
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
    ui.label(label);
    super::value_slider::ValueSlider::new(label, value, range)
        .width(64.0)
        .show(ui);
}

pub fn cc_picker(ui: &mut Ui, current: &mut u8) {
    let selected = *current;
    let (_, cc) = super::menu_button::menu_button(
        ui,
        "piano-roll-controller",
        "Controller",
        Some("Controller"),
        &cc_name(selected),
        true,
        |ui| {
            egui::ScrollArea::vertical()
                .max_height(320.0)
                .show(ui, |ui| {
                    (0..128)
                        .find(|&cc| super::menu_button::choice(ui, &cc_name(cc), cc == selected))
                })
                .inner
        },
    );
    if let Some(Some(cc)) = cc {
        *current = cc;
    }
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

/// The roll's ruler beside a keybed-wide spacer; returns the playhead tick it shows.
pub fn ruler(ui: &mut Ui, time: &Viewport, width: f32, tick: u64, timeline: &Timeline) -> u64 {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        ui.allocate_space(Vec2::new(theme::ROLL_KEYBED, super::timeline::RULER_HEIGHT));
        time.ruler(ui, width, tick, timeline, false).1
    })
    .inner
}

/// Allocates the keybed and the grid side by side at `height`.
pub fn pitch_area<'a>(
    ui: &mut Ui,
    time: &'a Viewport,
    [width, height]: [f32; 2],
    [row, scroll]: [f32; 2],
    enabled: bool,
) -> (PitchGrid<'a>, Response, Response) {
    let result = ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let (keys, audition) =
            ui.allocate_exact_size(Vec2::new(theme::ROLL_KEYBED, height), Sense::click());
        let (grid, edit) =
            ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());
        labelled(&audition, enabled, "Piano keyboard");
        labelled(&edit, enabled, "MIDI note grid");
        super::timeline::paint_focus(ui, &audition);
        super::timeline::paint_focus(ui, &edit);
        (
            PitchGrid {
                keys,
                grid,
                time,
                row,
                scroll,
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

pub struct NoteStyle {
    pub colour: egui::Color32,
    pub selected: bool,
    pub active: bool,
    pub primary: bool,
    pub preview: bool,
}

pub fn paint_note(ui: &Ui, grid: Rect, area: Rect, style: NoteStyle) {
    let NoteStyle {
        colour,
        selected,
        active,
        primary,
        preview,
    } = style;
    if !area.intersects(grid) {
        return;
    }
    let painter = ui.painter_at(grid);
    let fill = if preview {
        theme::ACCENT.gamma_multiply(0.54)
    } else if active {
        colour
    } else {
        colour.gamma_multiply(if selected { 0.5 } else { 0.35 })
    };
    painter.rect_filled(area, theme::ROLL_RADIUS, fill);
    if selected || preview {
        painter.rect_stroke(
            area,
            theme::ROLL_RADIUS,
            Stroke::new(theme::ROLL_SELECTION, theme::CAP_LIGHT),
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
                Stroke::new(theme::ROLL_LINE, theme::TEXT),
            );
        }
    }
}

pub fn comp_strip(ui: &mut Ui, time: &Viewport, width: f32, ranges: &[[u64; 2]]) {
    let rect = Rect::from_min_size(ui.cursor().min, Vec2::new(width, 3.0));
    ui.allocate_space(rect.size());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, theme::CANVAS);
    for [start, end] in ranges {
        let left = rect.left() + time.x(*start);
        let right = rect.left() + time.x(*end);
        painter.rect_filled(
            Rect::from_min_max(
                egui::pos2(left.max(rect.left()), rect.top()),
                egui::pos2(right.min(rect.right()), rect.bottom()),
            ),
            0.0,
            theme::ACCENT.gamma_multiply(0.7),
        );
    }
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
            paint_marker(ui, rect, time, tick, theme::SECONDARY.gamma_multiply(0.6));
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
                theme::ACCENT
            } else {
                theme::DIVIDER
            },
        ),
    );
    if response.dragged() {
        *ratio = (*ratio + response.drag_motion().y / height).clamp(0.3, 0.75);
    }
}

pub fn value_lane(ui: &mut Ui, name: &str, width: f32, enabled: bool) -> (Rect, Response) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
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
        ui.painter_at(rect).rect_filled(rect, 0.0, theme::CANVAS);
        super::timeline::paint_focus(ui, &response);
        (rect, response)
    })
    .inner
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
                Stroke::new(theme::ROLL_LINE, theme::ACCENT),
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
    (clip, colour): (&Clip, egui::Color32),
) {
    let painter = ui.painter_at(rect);
    let value = |event: &RollEvent| match event.source.message {
        Message::Note { velocity, .. } => velocity,
        message => message.control().unwrap()[2],
    };
    if let Some(cc) = cc {
        let stroke = Stroke::new(theme::ROLL_LINE, colour);
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
                            Stroke::new(theme::ROLL_LINE, theme::ACCENT),
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
            theme::ACCENT
        } else {
            colour
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
            // T41 round 3 (S7): a lollipop head on every velocity stem, the grab target; a
            // selected value carries an `accent` outline around the note colour.
            if cc.is_none() {
                let head = if event.active {
                    colour
                } else {
                    colour.gamma_multiply(0.35)
                };
                painter.circle_filled(position, theme::ROLL_VELOCITY_HEAD, head);
                if selection.contains(&event.id) {
                    painter.circle_stroke(
                        position,
                        theme::ROLL_VELOCITY_HEAD + 0.5,
                        Stroke::new(theme::ROLL_LINE, theme::ACCENT),
                    );
                }
            }
        } else {
            painter.circle_filled(position, theme::ROLL_POINT, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clef_model::midi::Snap;

    fn grid(time: &Viewport) -> PitchGrid<'_> {
        PitchGrid {
            keys: Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(56.0, 240.0)),
            grid: Rect::from_min_size(egui::pos2(56.0, 0.0), Vec2::new(400.0, 240.0)),
            time,
            row: 12.0,
            // Pitch 71 (B4) at the top edge.
            scroll: 12.0 * f32::from(127_u8 - 71),
        }
    }

    #[test]
    fn rows_map_pitches_and_back_through_the_scroll() {
        let timeline = Timeline::default();
        let time = Viewport::with_snap(&timeline, 0, 96.0, 400.0, Snap::Off);
        let grid = grid(&time);
        assert_eq!(grid.top(71), 0.0);
        assert_eq!(grid.key(0.5), 71);
        assert_eq!(grid.key(12.5), 70);
        assert_eq!(grid.visible(), 52..=71);
        for pitch in grid.visible() {
            assert_eq!(grid.key(grid.top(pitch) + 6.0), pitch);
        }
    }

    #[test]
    fn keybed_hits_black_keys_where_drawn_and_white_keys_by_their_wide_body() {
        let timeline = Timeline::default();
        let time = Viewport::with_snap(&timeline, 0, 96.0, 400.0, Snap::Off);
        let grid = grid(&time);
        let middle = |pitch: u8| grid.top(pitch) + 6.0;
        // C♯4 is drawn 57 % wide: inside it is black, beside it the white key under it.
        assert_eq!(grid.keybed_key(egui::pos2(10.0, middle(61))), 61);
        assert!(!is_black(grid.keybed_key(egui::pos2(50.0, middle(61)))));
        // Seven equal white keys per octave: each white key spans 12/7 rows.
        let c4_bottom = grid.top(60) + 12.0;
        let white = 12.0 * 12.0 / 7.0;
        for (index, pitch) in [60, 62, 64, 65, 67, 69, 71].into_iter().enumerate() {
            let y = c4_bottom - white * (index as f32 + 0.5);
            assert_eq!(grid.keybed_key(egui::pos2(50.0, y)), pitch);
        }
    }

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
