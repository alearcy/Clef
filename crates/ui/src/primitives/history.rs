use crate::colors;
use clef_session::HistoryView;
use eframe::egui;

pub(crate) fn edit_menu(ui: &mut egui::Ui, rows: [(&str, &str, bool); 3]) -> Option<usize> {
    ui.menu_button("Edit", |ui| {
        for (index, (label, shortcut, enabled)) in rows.into_iter().enumerate() {
            let button = egui::Button::new(label).shortcut_text(shortcut);
            if ui.add_enabled(enabled, button).clicked() {
                ui.close();
                return Some(index);
            }
        }
        None
    })
    .inner
    .flatten()
}

#[derive(Default)]
pub(crate) struct HistoryWindow {
    pub open: bool,
    preview: Option<(u64, usize)>,
    pub pending: bool,
    pub error: Option<String>,
}

impl HistoryWindow {
    pub(crate) fn show(
        &mut self,
        ctx: &egui::Context,
        history: &HistoryView,
        enabled: bool,
    ) -> Option<usize> {
        if self
            .preview
            .is_some_and(|(version, _)| !enabled || version != history.version)
        {
            self.preview = None;
            ctx.stop_dragging();
        }
        let mut target = None;
        let mut open = self.open;
        let content = ctx.content_rect();
        egui::Window::new("Edit History")
            .open(&mut open)
            .collapsible(false)
            .default_pos(egui::pos2(content.right() - 364.0, content.top() + 76.0))
            .default_width(340.0)
            .show(ctx, |ui| {
                if let Some(error) = &self.error {
                    ui.colored_label(colors::DANGER, error);
                }
                if history.labels.is_empty() {
                    ui.label("No edits in this session.");
                    return;
                }
                let mut revision = self.preview.map_or(history.cursor, |(_, index)| index);
                // Leave enough travel to distinguish the retained steps in a full journal.
                ui.spacing_mut().slider_width = 264.0;
                let slider = ui.add_enabled(
                    enabled,
                    egui::Slider::new(&mut revision, 0..=history.labels.len())
                        .text("Revision")
                        .integer()
                        .show_value(false),
                );
                if slider.drag_started() {
                    slider.request_focus();
                }
                if self.preview.is_some()
                    && ui.input_mut(|input| {
                        input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                    })
                {
                    self.preview = None;
                    if slider.dragged() {
                        ui.memory_mut(|memory| memory.surrender_focus(slider.id));
                        ui.ctx().stop_dragging();
                    }
                } else if slider.drag_stopped() {
                    if self.preview.take().is_some() && enabled && revision != history.cursor {
                        target = Some(revision);
                    }
                } else if slider.dragged() {
                    self.preview = Some((history.version, revision));
                } else if slider.changed() && enabled && revision != history.cursor {
                    target = Some(revision);
                }
                if let Some((_, index)) = self.preview {
                    ui.colored_label(
                        colors::SECONDARY,
                        format!("Preview: {}", revision_label(history, index)),
                    );
                } else if self.pending {
                    ui.label("Applying history…");
                } else if !enabled {
                    ui.colored_label(
                        colors::SECONDARY,
                        "History navigation is unavailable while Clef is busy.",
                    );
                }
                egui::ScrollArea::vertical()
                    .max_height(240.0)
                    .show(ui, |ui| {
                        for index in 0..=history.labels.len() {
                            let name = revision_label(history, index);
                            if index == history.cursor {
                                ui.label(format!("Current: {name}"));
                            } else {
                                ui.colored_label(colors::SECONDARY, name);
                            }
                        }
                    });
            });
        self.open = open;
        if !open {
            self.preview = None;
            return None;
        }
        target
    }
}

fn revision_label(history: &HistoryView, index: usize) -> &str {
    index
        .checked_sub(1)
        .and_then(|index| history.labels.get(index))
        .copied()
        .unwrap_or("Oldest retained state")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_slider_previews_then_commits_on_release_and_supports_cancel() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut window = HistoryWindow {
            open: true,
            ..Default::default()
        };
        let history = HistoryView {
            cursor: 1,
            labels: vec!["Add Track", "Rename Track", "Record MIDI"],
            available: true,
            ..Default::default()
        };
        let frame = |window: &mut HistoryWindow, events| {
            let mut target = None;
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(760.0, 480.0),
                    )),
                    events,
                    ..Default::default()
                },
                |_| {
                    target = window.show(&ctx, &history, true);
                },
            );
            (target, output)
        };
        frame(&mut window, vec![]);
        let (_, output) = frame(&mut window, vec![]);
        let slider = output
            .platform_output
            .accesskit_update
            .unwrap()
            .nodes
            .into_iter()
            .find(|(_, node)| node.role() == egui::accesskit::Role::Slider)
            .unwrap()
            .1;
        let bounds = slider.bounds().unwrap();
        assert!(bounds.x0 >= 0.0 && bounds.x1 <= 760.0 && bounds.y0 >= 0.0 && bounds.y1 <= 480.0);
        assert_eq!(slider.numeric_value(), Some(1.0));
        let early = egui::pos2(
            bounds.x0 as f32 + 3.0,
            ((bounds.y0 + bounds.y1) * 0.5) as f32,
        );
        let late = egui::pos2(bounds.x1 as f32 - 3.0, early.y);
        let pointer = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };

        assert_eq!(
            frame(
                &mut window,
                vec![egui::Event::PointerMoved(early), pointer(early, true)]
            )
            .0,
            None
        );
        assert_eq!(
            frame(&mut window, vec![egui::Event::PointerMoved(late)]).0,
            None
        );
        assert_eq!(window.preview, Some((0, 3)));
        assert_eq!(frame(&mut window, vec![pointer(late, false)]).0, Some(3));
        assert_eq!(
            frame(&mut window, vec![key(egui::Key::ArrowRight)]).0,
            Some(2)
        );

        assert_eq!(
            frame(
                &mut window,
                vec![egui::Event::PointerMoved(early), pointer(early, true)]
            )
            .0,
            None
        );
        assert_eq!(frame(&mut window, vec![key(egui::Key::Escape)]).0, None);
        assert!(window.preview.is_none());
        assert_eq!(frame(&mut window, vec![pointer(early, false)]).0, None);
    }
}
