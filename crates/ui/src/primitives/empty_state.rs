//! Empty state (T41 S9): a large glyph over one line of text, centred horizontally and
//! vertically in the area it is given. Used where a view has nothing to show yet.
use super::icons::Icon;
use crate::theme;
use eframe::egui::{self, Rect, Ui, vec2};

/// Glyph size of an empty state, in points.
pub const GLYPH: f32 = 48.0;
/// Gap between the glyph and the text.
const GAP: f32 = 12.0;

/// Paints the empty state centred in `rect` and exposes the text as a label for assistive
/// technology. Paints only: it takes no input and moves no cursor.
pub fn paint(ui: &Ui, rect: Rect, glyph: Icon, text: &str) {
    let painter = ui.painter_at(rect);
    let galley = painter.layout_no_wrap(
        text.to_owned(),
        theme::text::font(theme::text::BODY),
        theme::SECONDARY,
    );
    let height = GLYPH + GAP + galley.size().y;
    let top = rect.center().y - height / 2.0;
    glyph.paint(
        &painter,
        egui::pos2(rect.center().x, top + GLYPH / 2.0),
        GLYPH,
        theme::SECONDARY,
    );
    let text_pos = egui::pos2(rect.center().x - galley.size().x / 2.0, top + GLYPH + GAP);
    let text_rect = Rect::from_min_size(text_pos, galley.size());
    painter.galley(text_pos, galley, theme::SECONDARY);
    let id = ui.id().with(("empty-state", text));
    let response = ui.interact(text_rect, id, egui::Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
}

/// Allocates the remaining space of `ui` (at least `min_height` tall) and paints the empty
/// state in it.
pub fn fill(ui: &mut Ui, min_height: f32, glyph: Icon, text: &str) {
    let size = vec2(ui.available_width(), ui.available_height().max(min_height));
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    paint(ui, rect, glyph, text);
}
