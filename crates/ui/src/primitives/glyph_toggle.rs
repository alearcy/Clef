//! Glyph toggle (T18 Phase 3): a glyph-only toggle on a raised body, where a word would waste
//! room. 24 × 22; `secondary` glyph at rest, `text` on hover, a `peacock` body with a `text` glyph
//! when lit, flat with a `disabled` glyph when disabled. The host owns the state.
use super::icons::Icon;
use crate::{
    paint::{self, BodyState, Depth},
    theme,
};
use eframe::egui::{Response, Sense, Ui, WidgetInfo, WidgetType, vec2};

const GLYPH: f32 = 14.0;

/// `name` is the accessible name and tooltip ("Triplet").
pub fn glyph_toggle(ui: &mut Ui, icon: Icon, name: &str, lit: bool, enabled: bool) -> Response {
    let response = ui.add_enabled(enabled, |ui: &mut Ui| {
        ui.allocate_response(vec2(24.0, 22.0), Sense::click())
    });
    response.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, enabled, lit, name));
    let mut state = BodyState::of(&response);
    if lit {
        state = state.lit(theme::PEACOCK);
    }
    let body = paint::control_body(ui.painter(), response.rect, Depth::Raised, state);
    let colour = if !enabled {
        theme::DISABLED
    } else if lit || response.hovered() {
        theme::TEXT
    } else {
        theme::SECONDARY
    };
    icon.paint(ui.painter(), body.center(), GLYPH, colour);
    response.on_hover_text(name)
}
