//! Range overlay (T18 Phase 3): the one marquee and range selection look, `accent` at 12 % with an
//! 80 % edge, optionally carrying a 10-point `accent` readout chip.
use crate::theme::{self, text};
use eframe::egui::{Align2, Painter, Rect, Stroke, StrokeKind, vec2};

/// Paints `range` clipped to `painter`; `readout` rides at its top-left corner.
pub fn range_overlay(painter: &Painter, range: Rect, readout: Option<&str>) {
    painter.rect_filled(range, 0.0, theme::ACCENT.gamma_multiply(0.12));
    painter.rect_stroke(
        range,
        0.0,
        Stroke::new(1.0_f32, theme::ACCENT.gamma_multiply(0.8)),
        StrokeKind::Inside,
    );
    if let Some(readout) = readout {
        let galley =
            painter.layout_no_wrap(readout.to_owned(), text::font(text::CAPTION), theme::ACCENT);
        let chip = Rect::from_min_size(range.min + vec2(2.0, 2.0), galley.size() + vec2(8.0, 4.0));
        painter.rect_filled(chip, 2.0, theme::BACKGROUND.gamma_multiply(0.85));
        painter.text(
            chip.center(),
            Align2::CENTER_CENTER,
            readout,
            text::font(text::CAPTION),
            theme::ACCENT,
        );
    }
}
