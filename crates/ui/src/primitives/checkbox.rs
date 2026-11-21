//! Checkbox (T18 Phase 3): a boolean setting with a visible label. A 14-point `field` box with a
//! `controlEdge` edge (`divider` while disabled), an `accent` tick when on, the label in `text`.
use super::icons::Icon;
use crate::{
    paint,
    theme::{self, metric, text},
};
use eframe::egui::{
    self, CornerRadius, Response, Sense, Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, vec2,
};

const BOX: f32 = 14.0;
const GAP: f32 = 6.0;

/// Draws the checkbox; a click flips `value`. The label is also the accessible name.
pub fn checkbox(ui: &mut Ui, label: &str, value: &mut bool, enabled: bool) -> Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        text::font(text::BODY),
        if enabled {
            theme::TEXT
        } else {
            theme::DISABLED
        },
    );
    let size = vec2(BOX + GAP + galley.size().x, metric::CONTROL_HEIGHT);
    let mut response = ui.add_enabled(enabled, |ui: &mut Ui| {
        ui.allocate_response(size, Sense::click())
    });
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    let on = *value;
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, on, label));
    let rect = response.rect;
    let tile = egui::Rect::from_center_size(
        egui::pos2(rect.left() + BOX / 2.0, rect.center().y),
        vec2(BOX, BOX),
    );
    let painter = ui.painter();
    let hovered = enabled && response.hovered();
    painter.rect(
        tile,
        CornerRadius::same(metric::CORNER_CONTROL),
        if hovered {
            paint::hover_fill()
        } else {
            theme::FIELD
        },
        Stroke::new(
            1.0_f32,
            if enabled {
                theme::CONTROL_EDGE
            } else {
                theme::DIVIDER
            },
        ),
        StrokeKind::Inside,
    );
    if on {
        let colour = if enabled {
            theme::ACCENT
        } else {
            theme::DISABLED
        };
        Icon::Check.paint(painter, tile.center(), BOX - 4.0, colour);
    }
    if response.has_focus() && paint::keyboard(ui.ctx()) {
        painter.rect_stroke(
            tile.expand(1.0),
            CornerRadius::same(metric::CORNER_CONTROL),
            Stroke::new(1.0_f32, theme::ACCENT),
            StrokeKind::Outside,
        );
    }
    painter.galley(
        egui::pos2(
            rect.left() + BOX + GAP,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        theme::TEXT,
    );
    response
}
