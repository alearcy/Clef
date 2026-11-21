//! Chip: a small toggle that shows or hides something attached to its owner.
use crate::{
    paint::{self, BodyState, Depth},
    theme::{self, metric, text},
};
use eframe::egui::{Align2, Response, Sense, Ui, WidgetInfo, WidgetType, vec2};

/// A 20-point chip with `caption` text: `field` off, `peacock` while what it opens is shown.
/// `name` is the accessible name and tooltip ("Show 3 take lanes for Strings").
pub fn chip(ui: &mut Ui, label: &str, name: &str, on: bool, enabled: bool) -> Response {
    let font = text::font(text::CAPTION);
    let galley = ui
        .painter()
        .layout_no_wrap(label.into(), font.clone(), theme::TEXT);
    let width = (galley.size().x + metric::CONTROL_PADDING).max(metric::CHIP_HEIGHT + 10.0);
    let response = ui.add_enabled(enabled, |ui: &mut Ui| {
        ui.allocate_response(vec2(width, metric::CHIP_HEIGHT), Sense::click())
    });
    let enabled = response.enabled();
    response.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, enabled, on, name));
    if ui.is_rect_visible(response.rect) {
        let mut state = BodyState::of(&response);
        if on && enabled {
            state = state.lit(theme::PEACOCK);
        }
        let body = paint::control_body(ui.painter(), response.rect, Depth::Raised, state);
        let colour = if !enabled {
            theme::DISABLED
        } else if on {
            theme::foreground_on(theme::PEACOCK)
        } else {
            theme::TEXT
        };
        ui.painter()
            .text(body.center(), Align2::CENTER_CENTER, label, font, colour);
    }
    response.on_hover_text(name)
}
