//! Level meter (T18 Phase 3): strip, header and master meters on the fader's dB scale.
use super::{
    fader::{SCALE, axis, meter_y},
    mixer::register_activation_focus,
};
use crate::{theme, theme::*};
use clef_session::Meter;
use eframe::egui::{
    self, Align2, FontId, Rect, Sense, Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, pos2, vec2,
};

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
                    theme::SECONDARY
                } else {
                    theme::DIVIDER
                },
            ),
        );
        painter.text(
            pos2(rail.left(), y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(MIX_SCALE_TEXT),
            theme::SECONDARY,
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
            Stroke::new(1.0_f32, theme::ACCENT),
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
    ui.painter().rect_filled(rect, 0, theme::PANEL);
    ui.painter().text(
        rect.left_center() + vec2(8.0, 0.0),
        Align2::LEFT_CENTER,
        "Master",
        egui::TextStyle::Body.resolve(ui.style()),
        theme::TEXT,
    );
    let meter = Rect::from_min_max(pos2(rect.right() - HEADER_METER, rect.top()), rect.max);
    paint_header_meter(ui, id.with("master-meter"), meter, "Master", reading);
}

pub(super) fn paint_header_meter(ui: &Ui, id: egui::Id, rect: Rect, name: &str, reading: Meter) {
    paint_meter(ui, rect, reading, false);
    let clip = if reading.clipped { " · Clipped" } else { "" };
    let response = ui.interact(rect, id, Sense::hover());
    let label = format!("{name} stereo level{clip}");
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, &label));
    response.on_hover_text(label);
}

pub(super) fn paint_meter(ui: &Ui, rect: Rect, reading: Meter, outlined: bool) {
    let painter = ui.painter();
    painter.rect_filled(rect, 0, theme::BACKGROUND);
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
                theme::ACCENT,
            );
        }
        if reading.hold[channel] > MIX_SILENCE {
            painter.line_segment(
                [
                    pos2(left, point(reading.hold[channel])),
                    pos2(right, point(reading.hold[channel])),
                ],
                Stroke::new(1.0_f32, theme::TEXT),
            );
        }
    }
    if outlined {
        painter.rect_stroke(
            rect,
            0,
            Stroke::new(1.0_f32, theme::DIVIDER),
            StrokeKind::Inside,
        );
    }
    if reading.clipped {
        painter.rect_filled(
            Rect::from_min_size(rect.min, vec2(rect.width(), 2.0)),
            0,
            theme::DANGER,
        );
    }
}
