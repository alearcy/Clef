//! Tabs (T18 Phase 3): flat uppercase `body` text items, `secondary` off, `accent` for the shown
//! panel, `text` on hover, a 1 × 12 `divider` between items, 26-point targets with 10 points of
//! padding and an `accent` keyboard-focus edge. ←/→ move the focused tab and select it.
use crate::{
    paint,
    theme::{self, metric, text},
};
use eframe::egui::{
    self, Align2, CornerRadius, Key, Sense, Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, vec2,
};

const PADDING: f32 = 10.0;

/// Draws the tab row named `name`; returns true when `selected` changed.
pub fn tabs(ui: &mut Ui, name: &str, labels: &[&str], selected: &mut usize) -> bool {
    let before = *selected;
    let id = ui.id().with(("clef-tabs", name));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let mut focused = None;
        for (index, label) in labels.iter().enumerate() {
            if index > 0 {
                let (rect, _) =
                    ui.allocate_exact_size(vec2(1.0, metric::TAB_HEIGHT), Sense::hover());
                ui.painter().vline(
                    rect.center().x,
                    egui::Rangef::point(rect.center().y).expand(6.0),
                    Stroke::new(1.0_f32, theme::DIVIDER),
                );
            }
            let upper = label.to_uppercase();
            let galley =
                ui.painter()
                    .layout_no_wrap(upper.clone(), text::font(text::BODY), theme::TEXT);
            let (rect, _) = ui.allocate_exact_size(
                vec2(galley.size().x + 2.0 * PADDING, metric::TAB_HEIGHT),
                Sense::hover(),
            );
            let response = ui.interact(rect, id.with(index), Sense::click());
            let shown = *selected == index;
            response.widget_info(|| {
                WidgetInfo::selected(WidgetType::SelectableLabel, true, shown, *label)
            });
            if response.clicked() {
                *selected = index;
            }
            if response.has_focus() {
                focused = Some(index);
            }
            let colour = if shown {
                theme::ACCENT
            } else if response.hovered() {
                theme::TEXT
            } else {
                theme::SECONDARY
            };
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                upper,
                text::font(text::BODY),
                colour,
            );
            if response.has_focus() && paint::keyboard(ui.ctx()) {
                ui.painter().rect_stroke(
                    rect,
                    CornerRadius::same(metric::CORNER_CONTROL),
                    Stroke::new(1.0_f32, theme::ACCENT),
                    StrokeKind::Inside,
                );
            }
        }
        if let Some(index) = focused {
            let step = ui.input_mut(|input| {
                i32::from(input.consume_key(egui::Modifiers::NONE, Key::ArrowRight))
                    - i32::from(input.consume_key(egui::Modifiers::NONE, Key::ArrowLeft))
            });
            if step != 0 {
                let next = (index as i32 + step).rem_euclid(labels.len() as i32) as usize;
                *selected = next;
                ui.memory_mut(|memory| memory.request_focus(id.with(next)));
            }
        }
    });
    *selected != before
}
