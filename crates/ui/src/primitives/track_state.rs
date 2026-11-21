//! Track-state button (T18 Phase 3): record arm, identical on every surface. No filled tile: the
//! glyph is the lamp (UI10). Off is a `secondary` target; armed turns it `danger` with a filled
//! centre; a recording take squares it; disabled is `disabled`.
use crate::theme;
use eframe::egui::{self, Ui};

/// Returns true when clicked. The accessible name says what a click does.
pub fn record_arm(
    ui: &mut Ui,
    track: &str,
    enabled: bool,
    armed: bool,
    recording: bool,
    keyboard: bool,
) -> bool {
    let name = if recording {
        format!("Recording MIDI on {track}")
    } else if armed {
        format!("Disarm {track}")
    } else {
        format!("Arm {track} for MIDI recording")
    };
    let response = ui.add_enabled(
        enabled,
        egui::Button::new("")
            .frame(false)
            .min_size(egui::vec2(28.0, ui.available_height().clamp(20.0, 44.0))),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, armed, &name)
    });
    let painter = ui.painter();
    let center = response.rect.center();
    let color = if armed {
        theme::DANGER
    } else if enabled {
        theme::SECONDARY
    } else {
        theme::DISABLED
    };
    painter.circle_stroke(center, 6.0, egui::Stroke::new(1.0_f32, color));
    if recording {
        painter.rect_filled(
            egui::Rect::from_center_size(center, egui::vec2(5.0, 5.0)),
            0.0,
            color,
        );
    } else if armed {
        painter.circle_filled(center, 3.0, color);
    } else {
        painter.circle_stroke(center, 2.5, egui::Stroke::new(1.0_f32, color));
    }
    if response.has_focus() && keyboard || response.hovered() && enabled {
        painter.rect_stroke(
            egui::Rect::from_center_size(center, egui::vec2(20.0, 20.0)),
            3.0,
            egui::Stroke::new(
                1.0_f32,
                if response.has_focus() && keyboard {
                    theme::ACCENT
                } else {
                    theme::FIELD
                },
            ),
            egui::StrokeKind::Inside,
        );
    }
    response.on_hover_text(name).clicked()
}
