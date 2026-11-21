//! Segmented control (T18 Phase 2): an exclusive choice among two to four options shown at
//! once. A recessed `field` track; the lit segment is `peacock` with a `foreground_on` label.
//! Segments are focusable; ←/→ on a focused segment selects its neighbour.
use crate::{
    paint::{self, BodyState, Depth},
    theme::{self, metric, text},
};
use eframe::egui::{
    self, Align2, CornerRadius, Key, Modifiers, Response, Sense, Stroke, StrokeKind, Ui,
    WidgetInfo, WidgetType, vec2,
};

#[must_use]
pub struct Segmented<'a, T> {
    name: &'a str,
    value: &'a mut T,
    options: &'a [(T, &'a str)],
    enabled: bool,
}

impl<'a, T: Copy + PartialEq> Segmented<'a, T> {
    /// `name` is the group's accessible name ("Applies to").
    pub fn new(name: &'a str, value: &'a mut T, options: &'a [(T, &'a str)]) -> Self {
        debug_assert!(
            (2..=4).contains(&options.len()),
            "use a menu button beyond four"
        );
        Self {
            name,
            value,
            options,
            enabled: true,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Returns the response of the whole track; `changed()` is set when the value changed.
    pub fn show(self, ui: &mut Ui) -> Response {
        let font = text::font(text::BODY);
        let segment = self
            .options
            .iter()
            .map(|(_, label)| {
                ui.painter()
                    .layout_no_wrap((*label).to_owned(), font.clone(), theme::TEXT)
                    .size()
                    .x
            })
            .fold(0.0_f32, f32::max)
            + metric::CONTROL_PADDING * 2.0;
        let size = vec2(segment * self.options.len() as f32, metric::CONTROL_HEIGHT);
        let enabled = self.enabled && ui.is_enabled();
        let (rect, mut track) = ui.allocate_exact_size(size, Sense::hover());
        track.widget_info(|| WidgetInfo::labeled(WidgetType::Other, enabled, self.name));
        let body = paint::control_body(
            ui.painter(),
            rect,
            Depth::Recessed,
            BodyState {
                disabled: !enabled,
                ..Default::default()
            },
        );
        // Unique per control even when two share a name in one container.
        let base = track.id;
        let mut chosen = None;
        let mut focus = None;
        for (index, &(option, label)) in self.options.iter().enumerate() {
            let cell = egui::Rect::from_min_size(
                body.min + vec2(segment * index as f32, 0.0),
                vec2(segment, body.height()),
            );
            let response = ui.interact(
                cell,
                base.with(index),
                if enabled {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            );
            let lit = *self.value == option;
            let full = format!("{}: {label}", self.name);
            response
                .widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, enabled, lit, &full));
            if response.clicked() {
                chosen = Some(option);
            }
            if response.has_focus() {
                let step = ui.input_mut(|input| {
                    if input.consume_key(Modifiers::NONE, Key::ArrowRight) {
                        1
                    } else if input.consume_key(Modifiers::NONE, Key::ArrowLeft) {
                        -1
                    } else {
                        0
                    }
                });
                if step != 0 {
                    let next =
                        (index as isize + step).clamp(0, self.options.len() as isize - 1) as usize;
                    chosen = Some(self.options[next].0);
                    focus = Some(base.with(next));
                }
            }
            let corner = CornerRadius::same(metric::CORNER_CONTROL);
            let ink = if !enabled {
                theme::DISABLED
            } else if lit {
                theme::foreground_on(theme::PEACOCK)
            } else if response.hovered() {
                theme::TEXT
            } else {
                theme::SECONDARY
            };
            if lit {
                ui.painter().rect_filled(
                    cell,
                    corner,
                    if enabled {
                        theme::PEACOCK
                    } else {
                        theme::PANEL
                    },
                );
            }
            if response.has_focus() && paint::keyboard(ui.ctx()) {
                ui.painter().rect_stroke(
                    cell,
                    corner,
                    Stroke::new(1.0_f32, theme::ACCENT),
                    StrokeKind::Inside,
                );
            }
            ui.painter().text(
                cell.center(),
                Align2::CENTER_CENTER,
                label,
                font.clone(),
                ink,
            );
        }
        if let Some(id) = focus {
            ui.memory_mut(|memory| memory.request_focus(id));
        }
        if let Some(option) = chosen
            && option != *self.value
        {
            *self.value = option;
            track.mark_changed();
        }
        track
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_and_arrows_select_and_each_segment_is_a_named_radio() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        ctx.enable_accesskit();
        let mut value = false;
        let options = [(false, "All projects"), (true, "This project")];
        let mut frame = |events: Vec<egui::Event>| {
            let mut changed = false;
            let output = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    changed = Segmented::new("Applies to", &mut value, &options)
                        .show(ui)
                        .changed();
                },
            );
            (changed, output)
        };
        let (_, output) = frame(vec![]);
        let radios: Vec<_> = output
            .platform_output
            .accesskit_update
            .unwrap()
            .nodes
            .into_iter()
            .filter(|(_, node)| node.role() == egui::accesskit::Role::RadioButton)
            .map(|(_, node)| node)
            .collect();
        assert_eq!(radios.len(), 2);
        let second = radios
            .iter()
            .find(|node| node.label() == Some("Applies to: This project"))
            .and_then(|node| node.bounds())
            .unwrap();
        let at = egui::pos2(
            ((second.x0 + second.x1) / 2.0) as f32,
            ((second.y0 + second.y1) / 2.0) as f32,
        );
        let click = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        frame(vec![egui::Event::PointerMoved(at), click(true)]);
        assert!(frame(vec![click(false)]).0);
        assert!(value);
        let mut frame = |events: Vec<egui::Event>| {
            let mut changed = false;
            let _ = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    changed = Segmented::new("Applies to", &mut value, &options)
                        .show(ui)
                        .changed();
                },
            );
            changed
        };
        let left = egui::Event::Key {
            key: Key::ArrowLeft,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let tab = egui::Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        // Tab reaches the first segment; ← there selects it.
        frame(vec![tab]);
        assert!(frame(vec![left]));
        assert!(!value);
    }
}
