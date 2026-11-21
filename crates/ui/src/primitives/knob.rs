//! Knob (T18 Phase 3): gain, pan, send and width knobs (the width knob is the fanned variant).
//! The gesture lives in `mixer::Slider`, shared with the fader.
use crate::{theme, theme::*};
use clef_model::mixer::Control;
use eframe::egui::{self, Rect, Stroke, Ui, vec2};

pub(super) fn paint_knob(ui: &Ui, rect: Rect, control: Control, progress: f32, focused: bool) {
    let painter = ui.painter();
    let center = rect.center();
    let radius = rect.width() * 0.42;
    let cap = if ui.is_enabled() {
        CAP
    } else {
        theme::DISABLED
    };
    painter.circle(
        center,
        radius,
        theme::FIELD,
        Stroke::new(
            1.0_f32,
            if focused {
                theme::ACCENT
            } else {
                theme::DIVIDER
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
                Stroke::new(1.0_f32, theme::ACCENT),
            ));
        }
    } else {
        let angle = (progress * 1.5 - 0.75) * std::f32::consts::PI;
        painter.circle_filled(center, radius * 0.70, cap);
        let direction = vec2(angle.sin(), -angle.cos());
        painter.line_segment(
            [center + direction * 3.0, center + direction * radius * 0.68],
            Stroke::new(1.5_f32, theme::BACKGROUND),
        );
    }
}
