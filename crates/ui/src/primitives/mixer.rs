use crate::{colors, theme::*};
use clef_model::mixer::Control;
use clef_session::Meter;
use eframe::egui::{
    self, Align2, Color32, FontId, Key, Modifiers, Pos2, Rect, Response, Sense, Stroke, StrokeKind,
    Ui, WidgetInfo, WidgetType, pos2, vec2,
};

const SCALE: [(i16, f32); 8] = [
    (-600, 0.0),
    (-480, 0.09),
    (-360, 0.18),
    (-240, 0.30),
    (-120, 0.49),
    (-60, 0.65),
    (0, 0.82),
    (60, 1.0),
];

pub(super) fn fraction(value: i16) -> f32 {
    for pair in SCALE.windows(2) {
        let [(low, a), (high, b)] = [pair[0], pair[1]];
        if value <= high {
            return egui::lerp(
                a..=b,
                (f32::from(value - low) / f32::from(high - low)).max(0.0),
            );
        }
    }
    1.0
}

fn fader_value(position: f32) -> i16 {
    if position <= 0.0 {
        return Control::Fader.bounds().0;
    }
    for pair in SCALE.windows(2) {
        let [(low, a), (high, b)] = [pair[0], pair[1]];
        if position <= b {
            return egui::lerp(f32::from(low)..=f32::from(high), (position - a) / (b - a)).round()
                as i16;
        }
    }
    60
}

fn axis(rect: Rect) -> Rect {
    rect.shrink2(vec2(0.0, MIX_CAP[1] * 0.5))
}

fn meter_y(rect: Rect, db: i16) -> f32 {
    egui::lerp(rect.bottom()..=rect.top(), fraction(db))
}

pub(crate) fn strip(ui: &mut Ui, selected: bool, content: impl FnOnce(&mut Ui)) -> Response {
    let [left, right, top, bottom] = MIX_PADDING;
    egui::Frame::new()
        .fill(colors::PANEL)
        .stroke(if selected {
            Stroke::new(1.0_f32, colors::ACCENT)
        } else {
            Stroke::new(1.0_f32, Color32::TRANSPARENT)
        })
        .inner_margin(egui::Margin {
            left,
            right,
            top,
            bottom,
        })
        .show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                let width = MIX_CHANNEL - f32::from(left + right) - 2.0;
                ui.set_width(width);
                ui.spacing_mut().item_spacing = vec2(0.0, MIX_ROW);
                content(ui);
            });
        })
        .response
}

pub(crate) fn small_label(ui: &mut Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(MIX_SMALL)
            .color(colors::SECONDARY),
    );
}

pub(crate) fn footer(ui: &mut Ui, name: &str, master: bool, hint: &str) -> Response {
    let label = egui::Label::new(egui::RichText::new(name).size(MIX_SMALL))
        .truncate()
        .sense(Sense::click());
    let frame = egui::Frame::new().fill(if master {
        colors::PEACOCK
    } else {
        colors::FIELD
    });
    let response = frame
        .show(ui, |ui| {
            ui.add_sized([ui.available_width(), 20.0], label)
                .on_hover_text(hint)
        })
        .inner;
    register_activation_focus(&response);
    response
}

pub(crate) fn lamp(
    ui: &mut Ui,
    label: &str,
    hint: &str,
    solo: bool,
    lit: bool,
    enabled: bool,
) -> bool {
    let (fill, color) = match (lit, solo) {
        (true, true) => (MIX_SOLO, colors::BACKGROUND),
        (true, false) => (MIX_MUTE, colors::TEXT),
        _ => (colors::FIELD, colors::SECONDARY),
    };
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(label).color(color))
            .fill(fill)
            .min_size(vec2(25.0, 20.0)),
    );
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, lit, hint));
    register_activation_focus(&response);
    response.on_hover_text(hint).clicked()
}

fn register_activation_focus(response: &Response) {
    if response.has_focus() {
        response
            .ctx
            .data_mut(|data| data.insert_temp(egui::Id::new("mixer-activation"), response.id));
    }
}

pub(crate) fn activation_has_focus(ctx: &egui::Context) -> bool {
    let id = ctx.data(|data| data.get_temp::<egui::Id>(egui::Id::new("mixer-activation")));
    id.is_some() && id == ctx.memory(|memory| memory.focused())
}

#[derive(Clone, Copy)]
pub(crate) struct DragOrigin {
    value: i16,
    pointer: Pos2,
}

#[derive(Clone, Copy)]
pub(crate) enum Adjustment {
    Drag { origin: DragOrigin, value: i16 },
    Commit(i16),
}

pub(crate) struct Slider<'a> {
    pub control: Control,
    pub value: i16,
    pub drag_start: Option<DragOrigin>,
    pub name: &'a str,
    pub master: bool,
}

impl Slider<'_> {
    pub(crate) fn show(&self, ui: &mut Ui) -> Option<Adjustment> {
        let fader = self.control == Control::Fader;
        let size = if fader {
            vec2(MIX_FADER, MIX_FADER_HEIGHT)
        } else {
            vec2(MIX_KNOB, MIX_KNOB)
        };
        let (_, rect) = ui.allocate_space(size);
        let id = ui.id().with(("mixer-control", self.control.label()));
        let response = ui.interact(rect, id, Sense::click_and_drag());
        let enabled = ui.is_enabled();
        let (min, max) = self.control.bounds();
        let name = format!("{} {}", self.name, self.control.label());
        let displayed = text_value(self.control, self.value);
        let accessibility_value = match self.control {
            Control::Gain | Control::Fader => f64::from(self.value) / 10.0,
            _ => f64::from(self.value),
        };
        response.widget_info(|| WidgetInfo::slider(enabled, accessibility_value, &name));
        if fader {
            paint_fader(ui, rect, self.value, response.has_focus(), self.master);
        } else {
            let progress = f32::from(self.value - min) / f32::from(max - min);
            let progress = if self.control == Control::Width {
                ui.ctx()
                    .animate_value_with_time(id.with("width-fan"), progress, 0.12)
            } else {
                progress
            };
            paint_knob(ui, rect, self.control, progress, response.has_focus());
        }
        let origin = self.drag_start.or_else(|| {
            if !response.drag_started() {
                return None;
            }
            ui.input(|i| i.pointer.press_origin())
                .map(|pointer| DragOrigin {
                    value: self.value,
                    pointer,
                })
        });
        let read_drag = |origin: DragOrigin| {
            let delta = response
                .interact_pointer_pos()
                .map_or(egui::Vec2::ZERO, |at| at - origin.pointer);
            if fader {
                fader_value(fraction(origin.value) - delta.y / axis(rect).height())
            } else {
                (f32::from(origin.value)
                    + (delta.x - delta.y) * f32::from(max - min) / MIX_FADER_HEIGHT)
                    .round()
                    .clamp(f32::from(min), f32::from(max)) as i16
            }
        };
        let option = ui.input(|i| i.modifiers.alt);
        let event = if !enabled {
            None
        } else if response.double_clicked() || (option && response.clicked()) {
            Some(Adjustment::Commit(self.control.initial()))
        } else if response.drag_stopped()
            && let Some(origin) = origin
        {
            let value = response
                .interact_pointer_pos()
                .map_or(self.value, |_| read_drag(origin));
            Some(Adjustment::Commit(value))
        } else if response.dragged()
            && let Some(origin) = origin
        {
            Some(Adjustment::Drag {
                origin,
                value: read_drag(origin),
            })
        } else if response.has_focus() {
            let direction = ui.input_mut(|i| {
                let up = i.consume_key(Modifiers::NONE, Key::ArrowUp)
                    || i.consume_key(Modifiers::NONE, Key::ArrowRight);
                let down = i.consume_key(Modifiers::NONE, Key::ArrowDown)
                    || i.consume_key(Modifiers::NONE, Key::ArrowLeft);
                i16::from(up) - i16::from(down)
            });
            (direction != 0).then(|| {
                let step = if matches!(self.control, Control::Gain | Control::Fader) {
                    5
                } else {
                    1
                };
                Adjustment::Commit((self.value + direction * step).clamp(min, max))
            })
        } else {
            None
        };
        response.on_hover_text(format!("{name}: {displayed}"));
        event
    }
}

pub(crate) fn text_value(control: Control, value: i16) -> String {
    match control {
        Control::Fader if value == Control::Fader.bounds().0 => "−∞ dB".into(),
        Control::Fader | Control::Gain => format!("{:.1} dB", f32::from(value) / 10.0),
        Control::Pan if value == 0 => "C".into(),
        Control::Pan if value < 0 => format!("L {}", -value),
        Control::Pan => format!("R {value}"),
        _ => format!("{value}%"),
    }
}

fn paint_fader(ui: &Ui, rect: Rect, value: i16, focused: bool, master: bool) {
    let rail = axis(rect);
    let x = rect.center().x;
    let track = Rect::from_min_max(pos2(x - 3.0, rail.top()), pos2(x + 3.0, rail.bottom()));
    let painter = ui.painter();
    painter.rect(
        track,
        2,
        colors::BACKGROUND,
        Stroke::new(1.0_f32, colors::DIVIDER),
        StrokeKind::Inside,
    );
    let cap = Rect::from_center_size(pos2(x, meter_y(rail, value)), MIX_CAP.into());
    if ui.is_enabled() {
        let middle = if master { colors::ACCENT } else { CAP };
        let mut satin = egui::Mesh::default();
        for (y, color) in [
            (cap.top(), CAP_LIGHT),
            (cap.center().y, middle),
            (cap.bottom(), CAP_DARK),
        ] {
            satin.colored_vertex(pos2(cap.left(), y), color);
            satin.colored_vertex(pos2(cap.right(), y), color);
        }
        for index in [0, 2] {
            satin.add_triangle(index, index + 1, index + 2);
            satin.add_triangle(index + 1, index + 3, index + 2);
        }
        painter.add(egui::Shape::mesh(satin));
    } else {
        painter.rect_filled(cap, 1, colors::DISABLED);
    }
    painter.hline(
        cap.x_range(),
        cap.center().y,
        Stroke::new(1.0_f32, colors::BACKGROUND),
    );
    if focused {
        painter.rect_stroke(
            cap,
            2,
            Stroke::new(1.0_f32, colors::ACCENT),
            StrokeKind::Outside,
        );
    }
}

fn paint_knob(ui: &Ui, rect: Rect, control: Control, progress: f32, focused: bool) {
    let painter = ui.painter();
    let center = rect.center();
    let radius = rect.width() * 0.42;
    let cap = if ui.is_enabled() {
        CAP
    } else {
        colors::DISABLED
    };
    painter.circle(
        center,
        radius,
        colors::FIELD,
        Stroke::new(
            1.0_f32,
            if focused {
                colors::ACCENT
            } else {
                colors::DIVIDER
            },
        ),
    );
    if control == Control::Width {
        let origin = center + vec2(0.0, radius * 0.75);
        let angle = std::f32::consts::FRAC_PI_3 * progress;
        let edge = |angle: f32, length| origin + vec2(angle.sin(), -angle.cos()) * length;
        for direction in [-angle, angle] {
            painter.line_segment(
                [origin, edge(direction, radius * 1.4)],
                Stroke::new(1.0_f32, cap),
            );
        }
        for length in [radius * 0.84, radius * 1.4] {
            let points = (0..=8).map(|i| edge(egui::lerp(-angle..=angle, i as f32 / 8.0), length));
            painter.add(egui::Shape::line(
                points.collect(),
                Stroke::new(1.0_f32, colors::ACCENT),
            ));
        }
    } else {
        let angle = (progress * 1.5 - 0.75) * std::f32::consts::PI;
        painter.circle_filled(center, radius * 0.70, cap);
        let direction = vec2(angle.sin(), -angle.cos());
        painter.line_segment(
            [center + direction * 3.0, center + direction * radius * 0.68],
            Stroke::new(1.5_f32, colors::BACKGROUND),
        );
    }
}

pub(crate) fn scale(ui: &mut Ui) {
    let (_, stage) = ui.allocate_space(vec2(28.0, MIX_FADER_HEIGHT));
    let rail = axis(stage);
    let painter = ui.painter();
    for (db, _) in SCALE.iter().copied().skip(1) {
        let y = meter_y(rail, db);
        let label = if db > 0 {
            format!("+{}", db / 10)
        } else {
            format!("{}", db / 10)
        };
        let length = if db == 0 { 6.0 } else { 4.0 };
        painter.line_segment(
            [pos2(rail.right() - length, y), pos2(rail.right(), y)],
            Stroke::new(
                1.0_f32,
                if db == 0 {
                    colors::SECONDARY
                } else {
                    colors::DIVIDER
                },
            ),
        );
        painter.text(
            pos2(rail.left(), y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(MIX_SCALE_TEXT),
            colors::SECONDARY,
        );
    }
}

pub(crate) fn strip_meter(ui: &mut Ui, name: &str, reading: Meter, shortcut: &str) -> bool {
    let (_, stage) = ui.allocate_space(vec2(MIX_METER, MIX_FADER_HEIGHT));
    let rect = axis(stage);
    paint_meter(ui, rect, reading, true);
    let enabled = ui.is_enabled() && reading.clipped;
    let response = ui.interact(
        rect,
        ui.id().with("clip-reset"),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let label = format!("Clear {name} clip indicator");
    let name = format!("{name} stereo meter");
    response.widget_info(|| {
        if enabled {
            WidgetInfo::labeled(WidgetType::Button, true, &label)
        } else {
            WidgetInfo::labeled(WidgetType::Other, true, &name)
        }
    });
    register_activation_focus(&response);
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            0,
            Stroke::new(1.0_f32, colors::ACCENT),
            StrokeKind::Inside,
        );
    }
    response
        .on_hover_text(if !enabled {
            name
        } else if shortcut.is_empty() {
            label
        } else {
            format!("{label} ({shortcut})")
        })
        .clicked()
}

pub(crate) fn header_meter(ui: &mut Ui, height: f32, name: &str, reading: Meter) {
    let (id, rect) = ui.allocate_space(vec2(HEADER_METER, height));
    paint_header_meter(ui, id, rect, name, reading);
}

pub(crate) fn master_header(ui: &mut Ui, width: f32, reading: Meter) {
    let (id, rect) = ui.allocate_space(vec2(width, ARRANGEMENT_TRACK_HEIGHT));
    ui.painter().rect_filled(rect, 0, colors::PANEL);
    ui.painter().text(
        rect.left_center() + vec2(8.0, 0.0),
        Align2::LEFT_CENTER,
        "Master",
        egui::TextStyle::Body.resolve(ui.style()),
        colors::TEXT,
    );
    let meter = Rect::from_min_max(pos2(rect.right() - HEADER_METER, rect.top()), rect.max);
    paint_header_meter(ui, id.with("master-meter"), meter, "Master", reading);
}

fn paint_header_meter(ui: &Ui, id: egui::Id, rect: Rect, name: &str, reading: Meter) {
    paint_meter(ui, rect, reading, false);
    let clip = if reading.clipped { " · Clipped" } else { "" };
    let response = ui.interact(rect, id, Sense::hover());
    let label = format!("{name} stereo level{clip}");
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, &label));
    response.on_hover_text(label);
}

fn paint_meter(ui: &Ui, rect: Rect, reading: Meter, outlined: bool) {
    let painter = ui.painter();
    painter.rect_filled(rect, 0, colors::BACKGROUND);
    let inner = if outlined { rect.shrink(1.0) } else { rect };
    for channel in 0..2 {
        let half = inner.width() / 2.0;
        let left = inner.left() + channel as f32 * half;
        let right = left + half - 1.0;
        let point = |db| meter_y(rect, db).clamp(inner.top(), inner.bottom());
        if reading.level[channel] > MIX_SILENCE {
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(left, point(reading.level[channel])),
                    pos2(right, inner.bottom()),
                ),
                0,
                colors::ACCENT,
            );
        }
        if reading.hold[channel] > MIX_SILENCE {
            painter.line_segment(
                [
                    pos2(left, point(reading.hold[channel])),
                    pos2(right, point(reading.hold[channel])),
                ],
                Stroke::new(1.0_f32, colors::TEXT),
            );
        }
    }
    if outlined {
        painter.rect_stroke(
            rect,
            0,
            Stroke::new(1.0_f32, colors::DIVIDER),
            StrokeKind::Inside,
        );
    }
    if reading.clipped {
        painter.rect_filled(
            Rect::from_min_size(rect.min, vec2(rect.width(), 2.0)),
            0,
            colors::DANGER,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixer_slider_commits_the_release_position_as_one_gesture() {
        let ctx = egui::Context::default();
        let frame = |events, value, drag_start| {
            let mut event = None;
            let mut rect = Rect::NOTHING;
            let _ = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    rect = Rect::from_min_size(ui.next_widget_position(), vec2(MIX_KNOB, MIX_KNOB));
                    event = Slider {
                        control: Control::Width,
                        value,
                        drag_start,
                        name: "Piano",
                        master: false,
                    }
                    .show(ui);
                },
            );
            (rect, event)
        };
        let (rect, _) = frame(vec![], 100, None);
        let point = rect.center();
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let _ = frame(
            vec![egui::Event::PointerMoved(point), button(point, true)],
            100,
            None,
        );
        let moved = point + vec2(15.0, 0.0);
        let (_, event) = frame(vec![egui::Event::PointerMoved(moved)], 100, None);
        let Some(Adjustment::Drag { origin, value }) = event else {
            panic!("drag was not delivered")
        };
        assert_eq!(value, 114);
        let end = point + vec2(21.0, 0.0);
        let (_, event) = frame(
            vec![egui::Event::PointerMoved(end), button(end, false)],
            value,
            Some(origin),
        );
        assert!(matches!(event, Some(Adjustment::Commit(120))));
    }

    #[test]
    fn fader_and_meter_share_the_same_db_ticks() {
        for (db, at) in SCALE.iter().copied().skip(1) {
            assert_eq!(fraction(db), at);
            assert_eq!(fader_value(at), db);
        }
        assert_eq!(fader_value(0.0), Control::Fader.bounds().0);
    }
}
