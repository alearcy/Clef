//! Tool button: a borderless glyph button for transport controls, editing tools and zoom.
use super::icons::Icon;
use crate::{paint, theme};
use eframe::egui::{self, Response, Stroke, Ui, vec2};

pub fn tool_button(
    ui: &mut Ui,
    glyph: Icon,
    name: &str,
    enabled: bool,
    active: bool,
    keyboard_focus: bool,
) -> Response {
    let response = ui.add_enabled(enabled, |ui: &mut Ui| {
        ui.allocate_response(
            vec2(theme::TOOL_TARGET, theme::TOOL_TARGET),
            egui::Sense::click(),
        )
    });
    let enabled = response.enabled();
    response.widget_info(|| {
        if matches!(
            glyph,
            Icon::Play | Icon::Record | Icon::Loop | Icon::Metronome
        ) {
            egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, active, name)
        } else {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name)
        }
    });
    let down = response.is_pointer_button_down_on();
    let color = if active || glyph == Icon::Record && (down || response.clicked()) {
        match glyph {
            Icon::Record => theme::DANGER,
            Icon::Loop => theme::LOOP_ACTIVE,
            _ => theme::ACCENT,
        }
    } else if !enabled {
        theme::DISABLED
    } else if down {
        theme::SECONDARY
    } else if response.hovered() {
        theme::CAP_LIGHT
    } else {
        theme::TEXT
    };
    let painter = ui.painter_at(response.rect);
    let center = response.rect.center();
    glyph.paint(&painter, center, theme::TOOL_GLYPH, color);
    if (keyboard_focus || paint::keyboard(ui.ctx())) && response.has_focus() {
        painter.circle_stroke(
            center,
            (theme::TOOL_TARGET + theme::TOOL_GLYPH) / 4.0,
            Stroke::new(theme::TOOL_STROKE, theme::ACCENT),
        );
    }
    response.on_hover_text(name)
}

/// T41 create glyph: a `CREATE` plus as tall as a field control (`controlHeight`), the only
/// orange control (foundations, `CREATE`). Hover lifts it on the field hover fill.
pub fn create_button(ui: &mut Ui, name: &str, enabled: bool) -> Response {
    let size = theme::metric::CONTROL_HEIGHT;
    let response = ui.add_enabled(enabled, |ui: &mut Ui| {
        ui.allocate_response(vec2(size, size), egui::Sense::click())
    });
    let enabled = response.enabled();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name));
    let painter = ui.painter_at(response.rect);
    if enabled && response.hovered() {
        painter.rect_filled(
            response.rect,
            egui::CornerRadius::same(theme::metric::CORNER_CONTROL),
            paint::hover_fill(),
        );
    }
    let color = if enabled {
        theme::CREATE
    } else {
        theme::DISABLED
    };
    Icon::Plus.paint(&painter, response.rect.center(), size * 0.75, color);
    if paint::keyboard(ui.ctx()) && response.has_focus() {
        painter.rect_stroke(
            response.rect,
            egui::CornerRadius::same(theme::metric::CORNER_CONTROL),
            Stroke::new(1.0_f32, theme::ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    response.on_hover_text(name)
}
