//! Text button: a raised labelled button for actions that need words.
use crate::{
    paint::{self, BodyState, Depth},
    theme::{self, metric, text},
};
use eframe::egui::{Align2, Response, Sense, Ui, WidgetInfo, WidgetType, vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Kind {
    #[default]
    Secondary,
    /// The one primary action of a surface: `peacock` fill.
    Primary,
}

#[must_use]
pub struct TextButton<'a> {
    label: &'a str,
    kind: Kind,
    enabled: bool,
    lit: Option<bool>,
    tooltip: Option<&'a str>,
    shortcut: Option<&'a str>,
    size: f32,
    min_width: f32,
    accessible: Option<&'a str>,
}

impl<'a> TextButton<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            kind: Kind::Secondary,
            enabled: true,
            lit: None,
            tooltip: None,
            shortcut: None,
            size: text::BODY,
            min_width: 0.0,
            accessible: None,
        }
    }

    pub fn primary(mut self, primary: bool) -> Self {
        self.kind = if primary {
            Kind::Primary
        } else {
            Kind::Secondary
        };
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// A toggle-style text button: lit with `peacock` while `on`.
    pub fn toggle(mut self, on: bool) -> Self {
        self.lit = Some(on);
        self
    }

    pub fn tooltip(mut self, tooltip: &'a str) -> Self {
        self.tooltip = Some(tooltip);
        self
    }

    /// The live binding, shown after the tooltip; omitted when empty.
    pub fn shortcut(mut self, shortcut: &'a str) -> Self {
        self.shortcut = (!shortcut.is_empty()).then_some(shortcut);
        self
    }

    /// Small (11) text, for buttons inside dense rows and strips.
    pub fn small(mut self) -> Self {
        self.size = text::SMALL;
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }

    /// The accessible name when it says more than the label.
    pub fn accessible(mut self, name: &'a str) -> Self {
        self.accessible = Some(name);
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let font = text::font(self.size);
        let galley = ui
            .painter()
            .layout_no_wrap(self.label.into(), font, theme::TEXT);
        let width = (galley.size().x + metric::CONTROL_PADDING * 2.0).max(self.min_width);
        let response = ui.add_enabled(self.enabled, |ui: &mut Ui| {
            ui.allocate_response(vec2(width, metric::CONTROL_HEIGHT), Sense::click())
        });
        let enabled = response.enabled();
        let name = self.accessible.unwrap_or(self.label);
        response.widget_info(|| match self.lit {
            Some(on) => WidgetInfo::selected(WidgetType::SelectableLabel, enabled, on, name),
            None => WidgetInfo::labeled(WidgetType::Button, enabled, name),
        });
        if ui.is_rect_visible(response.rect) {
            let mut state = BodyState::of(&response);
            let lit = match (self.kind, self.lit) {
                (_, Some(true)) | (Kind::Primary, None) if enabled => Some(theme::PEACOCK),
                _ => None,
            };
            if let Some(fill) = lit {
                state = state.lit(fill);
            }
            let body = paint::control_body(ui.painter(), response.rect, Depth::Raised, state);
            let colour = if !enabled {
                theme::DISABLED
            } else {
                lit.map_or(theme::TEXT, theme::foreground_on)
            };
            ui.painter().text(
                body.center(),
                Align2::CENTER_CENTER,
                self.label,
                text::font(self.size),
                colour,
            );
        }
        match (self.tooltip, self.shortcut) {
            (Some(tip), Some(key)) => response.on_hover_text(format!("{tip} ({key})")),
            (Some(tip), None) => response.on_hover_text(tip),
            (None, Some(key)) => response.on_hover_text(format!("{} ({key})", name)),
            (None, None) if self.accessible.is_some() => response.on_hover_text(name),
            (None, None) => response,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;

    fn run(draw: impl FnMut(&mut Ui)) -> egui::FullOutput {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        ctx.enable_accesskit();
        let mut draw = draw;
        ctx.run_ui(egui::RawInput::default(), |ui| draw(ui))
    }

    #[test]
    fn disabled_button_keeps_its_accessible_name() {
        let output = run(|ui| {
            TextButton::new("Save").enabled(false).show(ui);
        });
        let tree = output.platform_output.accesskit_update.unwrap();
        assert!(
            tree.nodes
                .iter()
                .any(|(_, node)| { node.label() == Some("Save") && node.is_disabled() })
        );
    }

    #[test]
    fn toggle_reports_its_state() {
        let output = run(|ui| {
            TextButton::new("REC Merge").toggle(true).show(ui);
        });
        let tree = output.platform_output.accesskit_update.unwrap();
        assert!(tree.nodes.iter().any(|(_, node)| {
            node.label() == Some("REC Merge")
                && node.toggled() == Some(egui::accesskit::Toggled::True)
        }));
    }

    #[test]
    fn width_hugs_the_label_with_padding() {
        let mut width = 0.0;
        run(|ui| width = TextButton::new("Open").show(ui).rect.width());
        assert!(width > metric::CONTROL_PADDING * 2.0);
        let mut min = 0.0;
        run(|ui| min = TextButton::new("A").min_width(44.0).show(ui).rect.width());
        assert_eq!(min, 44.0);
    }
}
