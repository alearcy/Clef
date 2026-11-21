//! Command palette (T18 Phase 2): Spotlight-style search over the whole command registry.
//! The owner builds the rows (live names, shortcuts and availability) and runs the command.
use crate::{
    commands::Command,
    primitives::{list_row::ListRow, search_field::SearchField, window_frame},
    theme,
};
use eframe::egui::{self, Align2, Id, Key, Modifiers};

/// One registry command as the palette shows it.
pub(crate) struct Row {
    pub command: Command,
    pub label: String,
    pub shortcut: String,
    pub enabled: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Close,
    Run(Command),
}

/// The open palette and the context it was opened from.
pub(crate) struct Palette {
    pub query: String,
    highlighted: Option<Command>,
    /// The widget that had keyboard focus; it gets it back before a command runs.
    pub origin: Option<Id>,
    /// A piano-roll lane had focus, so roll commands are available.
    pub roll: bool,
    reveal: bool,
    /// The modal's id: the app's palette and the gallery specimen must not share widget ids.
    id: Id,
}

impl Palette {
    pub(crate) fn new(origin: Option<Id>, roll: bool) -> Self {
        Self {
            query: String::new(),
            highlighted: None,
            origin,
            roll,
            reveal: false,
            id: Id::new("command-palette"),
        }
    }

    /// A palette with its own widget ids (the gallery specimen).
    pub(crate) fn with_id(mut self, id: Id) -> Self {
        self.id = id;
        self
    }

    /// Case-insensitive substring match on the label, in registry order.
    pub(crate) fn matches<'a>(&self, rows: &'a [Row]) -> Vec<&'a Row> {
        let query = self.query.trim().to_lowercase();
        rows.iter()
            .filter(|row| row.label.to_lowercase().contains(&query))
            .collect()
    }

    pub(crate) fn show(&mut self, ctx: &egui::Context, rows: &[Row]) -> Option<Outcome> {
        let shown = self.matches(rows);
        let enabled: Vec<Command> = shown
            .iter()
            .filter(|row| row.enabled)
            .map(|row| row.command)
            .collect();
        if self
            .highlighted
            .is_none_or(|command| !enabled.contains(&command))
        {
            self.highlighted = enabled.first().copied();
        }
        let step = ctx.input_mut(|input| {
            if input.consume_key(Modifiers::NONE, Key::ArrowDown) {
                1
            } else if input.consume_key(Modifiers::NONE, Key::ArrowUp) {
                -1
            } else {
                0
            }
        });
        if step != 0 && !enabled.is_empty() {
            let at = self
                .highlighted
                .and_then(|command| enabled.iter().position(|&c| c == command))
                .unwrap_or(0) as isize;
            let next = (at + step).rem_euclid(enabled.len() as isize) as usize;
            self.highlighted = Some(enabled[next]);
            self.reveal = true;
        }
        let run = ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Enter));
        let mut outcome = run.then_some(self.highlighted.map(Outcome::Run)).flatten();
        let content = ctx.content_rect();
        let id = self.id;
        let modal = egui::Modal::new(id)
            .frame(window_frame::frame())
            .area(
                egui::Modal::default_area(id)
                    .pivot(Align2::CENTER_TOP)
                    .fixed_pos(egui::pos2(
                        content.center().x,
                        content.top() + content.height() / 6.0,
                    )),
            )
            .show(ctx, |ui| {
                ui.set_width(560.0_f32.min(content.width() - 32.0));
                let field = SearchField::new(&mut self.query, "Search commands")
                    .id(id.with("query"))
                    .show(ui);
                field.request_focus();
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .max_height(360.0_f32.min(content.height() / 2.0))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        if shown.is_empty() {
                            ui.colored_label(theme::SECONDARY, "No matching commands.");
                        }
                        for row in &shown {
                            let highlighted = self.highlighted == Some(row.command);
                            let response = ListRow::new(&row.label)
                                .trailing(&row.shortcut)
                                .selected(highlighted)
                                .enabled(row.enabled)
                                .show(ui);
                            if highlighted && std::mem::take(&mut self.reveal) {
                                response.scroll_to_me(None);
                            }
                            if response.clicked() {
                                outcome = Some(Outcome::Run(row.command));
                            }
                        }
                    });
            });
        if modal.should_close() && outcome.is_none() {
            outcome = Some(Outcome::Close);
        }
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> Vec<Row> {
        [
            (Command::PlayStop, "Play / Stop", true),
            (Command::DuplicateClips, "Duplicate", false),
            (Command::ReturnToStart, "Return to Start", true),
            (Command::Record, "Record MIDI", true),
        ]
        .into_iter()
        .map(|(command, label, enabled)| Row {
            command,
            label: label.into(),
            shortcut: String::new(),
            enabled,
        })
        .collect()
    }

    fn frame(ctx: &egui::Context, palette: &mut Palette, keys: &[Key]) -> Option<Outcome> {
        let rows = rows();
        let mut outcome = None;
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1024.0, 640.0),
                )),
                events: keys
                    .iter()
                    .map(|&key| egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers::NONE,
                    })
                    .collect(),
                ..Default::default()
            },
            |_| outcome = palette.show(ctx, &rows),
        );
        outcome
    }

    #[test]
    fn filters_skips_unavailable_rows_runs_with_return_and_closes_with_escape() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut palette = Palette::new(None, false);
        assert_eq!(frame(&ctx, &mut palette, &[]), None);
        // Down skips the unavailable Duplicate row.
        assert_eq!(
            frame(&ctx, &mut palette, &[Key::ArrowDown, Key::Enter]),
            Some(Outcome::Run(Command::ReturnToStart))
        );
        // Up from the first row wraps to the last available one.
        let mut palette = Palette::new(None, false);
        assert_eq!(
            frame(&ctx, &mut palette, &[Key::ArrowUp, Key::Enter]),
            Some(Outcome::Run(Command::Record))
        );
        let mut palette = Palette::new(None, false);
        palette.query = "RET".into();
        let rows = rows();
        assert_eq!(
            palette
                .matches(&rows)
                .iter()
                .map(|row| row.command)
                .collect::<Vec<_>>(),
            [Command::ReturnToStart]
        );
        assert_eq!(
            frame(&ctx, &mut palette, &[Key::Enter]),
            Some(Outcome::Run(Command::ReturnToStart))
        );
        let mut palette = Palette::new(None, false);
        frame(&ctx, &mut palette, &[]);
        assert_eq!(
            frame(&ctx, &mut palette, &[Key::Escape]),
            Some(Outcome::Close)
        );
    }
}
