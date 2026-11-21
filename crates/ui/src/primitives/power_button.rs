//! Power button (T18 Phase 3): the bypass glyph of insert slots and send rows. `accent` when the
//! processor runs, `secondary` when bypassed, `CAP_LIGHT` on hover, `disabled` when not ready; the
//! accessible state carries on/off, never only the colour.
use crate::{theme, theme::CAP_LIGHT};
use eframe::egui::{Key, Modifiers, Sense, Shape, Stroke, Ui, WidgetInfo, WidgetType, vec2};

/// Draw the shared bypass glyph. The caller supplies the accessible label and the
/// target size, so insert slots and compact send rows use the same control.
pub fn power_button(
    ui: &mut Ui,
    width: f32,
    height: f32,
    label: &str,
    lit: bool,
    ready: bool,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let enabled = ready && ui.is_enabled();
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, enabled, lit, label));
    let color = if !enabled {
        theme::DISABLED
    } else if response.hovered() {
        CAP_LIGHT
    } else if lit {
        theme::ACCENT
    } else {
        theme::SECONDARY
    };
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let radius = 4.5;
    let stroke = Stroke::new(1.4_f32, color);
    let start = -std::f32::consts::FRAC_PI_2 + 0.55;
    let end = start + std::f32::consts::TAU - 1.1;
    let arc: Vec<_> = (0..=20)
        .map(|step| {
            let angle = start + (end - start) * step as f32 / 20.0;
            center + vec2(angle.cos(), angle.sin()) * radius
        })
        .collect();
    painter.add(Shape::line(arc, stroke));
    painter.line_segment(
        [center + vec2(0.0, -radius - 1.5), center + vec2(0.0, 0.5)],
        stroke,
    );
    if response.has_focus() {
        painter.circle_stroke(center, 7.0, Stroke::new(1.0_f32, theme::ACCENT));
    }
    if !enabled {
        return false;
    }
    let keyboard = response.has_focus()
        && ui.input_mut(|input| {
            input.consume_key(Modifiers::NONE, Key::Enter)
                || input.consume_key(Modifiers::NONE, Key::Space)
        });
    response.on_hover_text(label).clicked() || keyboard
}
