//! Mute and solo (T18 Phase 3): the only filled lamps of headers and strips; mute `oxblood`,
//! solo yellow with a dark label. The lamp also changes its accessible state, never only colour.
use super::mixer::register_activation_focus;
use crate::{theme, theme::*};
use eframe::egui::{self, Ui, WidgetInfo, WidgetType, vec2};

/// Width of a mute or solo lamp (the header reserves this per lamp).
pub(crate) const LAMP_WIDTH: f32 = 25.0;

pub(crate) fn lamp(
    ui: &mut Ui,
    label: &str,
    hint: &str,
    solo: bool,
    lit: bool,
    enabled: bool,
) -> egui::Response {
    let (fill, color) = match (lit, solo) {
        (true, true) => (SOLO, theme::BACKGROUND),
        (true, false) => (OXBLOOD, theme::TEXT),
        _ => (theme::FIELD, theme::SECONDARY),
    };
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(label).color(color))
            .fill(fill)
            .min_size(vec2(LAMP_WIDTH, 20.0)),
    );
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, lit, hint));
    register_activation_focus(&response);
    response.on_hover_text(hint)
}
