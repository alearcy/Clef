//! Text field: a raised field for typing a name or a search; native editing is kept.
use crate::{
    paint::{self, BodyState, Depth},
    theme::{self, metric, text},
};
use eframe::egui::{self, CornerRadius, Response, Stroke, StrokeKind, Ui, vec2};

/// The search glyph's square, in points.
pub(super) const SEARCH_GLYPH: f32 = 12.0;

#[must_use]
pub struct TextField<'a> {
    value: &'a mut String,
    name: &'a str,
    hint: Option<&'a str>,
    width: Option<f32>,
    enabled: bool,
    id: Option<egui::Id>,
    /// Search form: glyph indent on the left, clear glyph on the right (`SearchField`).
    search: bool,
}

impl<'a> TextField<'a> {
    /// `name` is the accessible label ("Track name").
    pub fn new(value: &'a mut String, name: &'a str) -> Self {
        Self {
            value,
            name,
            hint: None,
            width: None,
            enabled: true,
            id: None,
            search: false,
        }
    }

    pub(super) fn search(mut self) -> Self {
        self.search = true;
        self
    }

    pub fn hint(mut self, hint: &'a str) -> Self {
        self.hint = Some(hint);
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn id(mut self, id: egui::Id) -> Self {
        self.id = Some(id);
        self
    }

    /// Extra room the search form keeps for its glyphs.
    fn indents(&self) -> [f32; 2] {
        if self.search {
            [SEARCH_GLYPH + 4.0, SEARCH_GLYPH + 4.0]
        } else {
            [0.0, 0.0]
        }
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let width = self.width.unwrap_or_else(|| ui.available_width());
        let (auto, bounds) = ui.allocate_space(vec2(width, metric::CONTROL_HEIGHT));
        let id = self.id.unwrap_or(auto);
        let body = paint::body_rect(bounds);
        let enabled = self.enabled && ui.is_enabled();
        // The body is painted before the editor so the text sits on it; hover comes from the
        // pointer and the focus edge is drawn after the editor reports focus.
        paint::control_body(
            ui.painter(),
            bounds,
            Depth::Raised,
            BodyState {
                hovered: enabled && ui.rect_contains_pointer(bounds),
                disabled: !enabled,
                ..BodyState::default()
            },
        );
        let indents = self.indents();
        let mut edit = egui::TextEdit::singleline(self.value)
            .id(id)
            .frame(egui::Frame::NONE)
            .font(text::font(text::BODY))
            .text_color(if enabled {
                theme::TEXT
            } else {
                theme::DISABLED
            })
            .margin(egui::Margin::ZERO)
            .vertical_align(egui::Align::Center)
            .desired_width(width - 2.0 * metric::CONTROL_PADDING - indents[0] - indents[1]);
        if let Some(hint) = self.hint {
            edit = edit.hint_text(egui::RichText::new(hint).color(theme::SECONDARY));
        }
        let response = ui
            .add_enabled_ui(enabled, |ui| {
                let [left, right] = indents;
                let mut inner = body.shrink2(vec2(metric::CONTROL_PADDING, 0.0));
                inner.min.x += left;
                inner.max.x -= right;
                ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                    ui.add_sized(inner.size(), edit)
                })
                .inner
            })
            .inner;
        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.set_label(self.name));
        if response.has_focus() {
            ui.painter().rect_stroke(
                body,
                CornerRadius::same(metric::CORNER_CONTROL),
                Stroke::new(1.0_f32, theme::ACCENT),
                StrokeKind::Inside,
            );
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focused_field_draws_the_accent_edge_and_carries_its_name() {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        ctx.enable_accesskit();
        let mut value = String::from("Strings");
        let id = egui::Id::new("field");
        let mut last = None;
        for _ in 0..3 {
            last = Some(ctx.run_ui(egui::RawInput::default(), |ui| {
                TextField::new(&mut value, "Track name")
                    .id(id)
                    .width(200.0)
                    .show(ui);
                ui.memory_mut(|memory| memory.request_focus(id));
            }));
        }
        let output = last.unwrap();
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Rect(rect) if rect.stroke.color == theme::ACCENT)));
        let tree = output.platform_output.accesskit_update.unwrap();
        assert!(
            tree.nodes
                .iter()
                .any(|(_, node)| node.label() == Some("Track name"))
        );
    }
}
