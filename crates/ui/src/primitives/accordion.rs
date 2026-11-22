//! Accordion section (T41 S6): a full-width header row with a disclosure chevron that expands
//! and collapses its body with a quick animation (120 ms ease-out). Several sections may be
//! open at once; the caller owns each section's open state.
use crate::{paint, theme};
use eframe::egui::{self, Rect, Sense, Stroke, Ui, UiBuilder, emath::easing, pos2, vec2};

/// Expand and collapse time, in seconds.
pub const ANIMATION: f32 = 0.12;

/// Draws the header and, while open or animating, the body clipped to the animated height.
/// Returns the body's result while it is drawn.
pub fn section<R>(
    ui: &mut Ui,
    title: &str,
    open: &mut bool,
    body: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    let id = ui.id().with(("accordion", title));
    let width = ui.available_width();
    let (header, response) =
        ui.allocate_exact_size(vec2(width, theme::metric::CONTROL_HEIGHT), Sense::click());
    if response.clicked() {
        *open = !*open;
    }
    let expanded = *open;
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::CollapsingHeader, true, expanded, title)
    });
    let painter = ui.painter_at(header);
    if response.hovered() {
        painter.rect_filled(header, 0.0, paint::hover_fill());
    }
    let openness =
        ui.ctx()
            .animate_bool_with_time_and_easing(id, *open, ANIMATION, easing::cubic_out);
    // The chevron turns from pointing right (closed) to pointing down (open).
    let centre = pos2(header.left() + 10.0, header.center().y);
    let angle = openness * std::f32::consts::FRAC_PI_2;
    let rotate = |x: f32, y: f32| {
        let (sin, cos) = angle.sin_cos();
        centre + vec2(x * cos - y * sin, x * sin + y * cos)
    };
    painter.add(egui::Shape::line(
        vec![rotate(-2.0, -4.0), rotate(2.0, 0.0), rotate(-2.0, 4.0)],
        Stroke::new(theme::TOOL_STROKE, theme::SECONDARY),
    ));
    painter.text(
        pos2(header.left() + 22.0, header.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        theme::text::font(theme::text::BODY),
        theme::TEXT,
    );
    if paint::keyboard(ui.ctx()) && response.has_focus() {
        painter.rect_stroke(
            header.shrink(1.0),
            0.0,
            Stroke::new(theme::TOOL_STROKE, theme::ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    if openness <= 0.0 {
        return None;
    }
    let height_id = id.with("height");
    let full = ui
        .data(|data| data.get_temp::<f32>(height_id))
        .unwrap_or(0.0);
    let visible = if openness >= 1.0 {
        full
    } else {
        full * openness
    };
    let top = ui.cursor().top();
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_size(
                pos2(header.left(), top),
                vec2(width, f32::INFINITY),
            ))
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(
        Rect::from_min_size(pos2(header.left(), top), vec2(width, visible))
            .intersect(ui.clip_rect()),
    );
    child.add_space(6.0);
    let result = body(&mut child);
    child.add_space(10.0);
    let measured = child.min_rect().height();
    if (measured - full).abs() > 0.5 {
        ui.data_mut(|data| data.insert_temp(height_id, measured));
        ui.ctx().request_repaint();
    }
    let shown = if openness >= 1.0 { measured } else { visible };
    ui.allocate_exact_size(vec2(width, shown), Sense::hover());
    Some(result)
}
