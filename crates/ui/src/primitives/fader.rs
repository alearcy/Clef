//! Fader (T18 Phase 3): the channel fader travel and its dB scale, shared by the level meters.
//! The gesture lives in `mixer::Slider`, shared with the knobs.
use crate::{theme, theme::*};
use clef_model::mixer::Control;
use eframe::egui::{self, Rect, Stroke, StrokeKind, Ui, pos2, vec2};

pub(super) const SCALE: [(i16, f32); 8] = [
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

pub(super) fn fader_value(position: f32) -> i16 {
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

pub(super) fn axis(rect: Rect) -> Rect {
    rect.shrink2(vec2(0.0, MIX_CAP[1] * 0.5))
}

pub(super) fn paint_fader(ui: &Ui, rect: Rect, value: i16, focused: bool, master: bool) {
    let rail = axis(rect);
    let x = rect.center().x;
    let track = Rect::from_min_max(pos2(x - 3.0, rail.top()), pos2(x + 3.0, rail.bottom()));
    let painter = ui.painter();
    painter.rect(
        track,
        2,
        theme::BACKGROUND,
        Stroke::new(1.0_f32, theme::DIVIDER),
        StrokeKind::Inside,
    );
    let cap = Rect::from_center_size(pos2(x, meter_y(rail, value)), MIX_CAP.into());
    if ui.is_enabled() {
        let middle = if master { theme::ACCENT } else { CAP };
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
        painter.rect_filled(cap, 1, theme::DISABLED);
    }
    painter.hline(
        cap.x_range(),
        cap.center().y,
        Stroke::new(1.0_f32, theme::BACKGROUND),
    );
    if focused {
        painter.rect_stroke(
            cap,
            2,
            Stroke::new(1.0_f32, theme::ACCENT),
            StrokeKind::Outside,
        );
    }
}

pub(super) fn meter_y(rect: Rect, db: i16) -> f32 {
    egui::lerp(rect.bottom()..=rect.top(), fraction(db))
}
