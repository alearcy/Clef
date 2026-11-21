//! Notice box: the once-on-open notice over the workspace (quarantined plugins, missing audio,
//! stopped recording) and its question form. Not a general dialog.
use super::{text as label, text_button::TextButton};
use crate::theme::{self, text};
use eframe::egui::{self, Context, Key, Stroke, Ui, vec2};

const WIDTH: f32 = 560.0;
const PADDING: i8 = 16;
const MAX_ROWS: usize = 12;
const ROW_HEIGHT: f32 = 22.0;

/// What the composer chose.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Primary,
    Secondary,
    Dismissed,
}

#[must_use]
pub struct NoticeBox<'a> {
    id: &'a str,
    heading: &'a str,
    body: &'a str,
    question: bool,
    columns: &'a [&'a str],
    rows: &'a [[String; 3]],
    primary: &'a str,
    secondary: Option<&'a str>,
}

impl<'a> NoticeBox<'a> {
    pub fn new(id: &'a str, heading: &'a str, body: &'a str, primary: &'a str) -> Self {
        Self {
            id,
            heading,
            body,
            question: false,
            columns: &[],
            rows: &[],
            primary,
            secondary: None,
        }
    }

    /// The question form: no warning glyph, no columns or rows.
    pub fn question(mut self) -> Self {
        self.question = true;
        self
    }

    /// Up to twelve entries in three columns: first `danger`, second `text`, third `secondary`.
    pub fn rows(mut self, columns: &'a [&'a str], rows: &'a [[String; 3]]) -> Self {
        self.columns = columns;
        self.rows = rows;
        self
    }

    pub fn secondary(mut self, label: &'a str) -> Self {
        self.secondary = Some(label);
        self
    }

    /// Shows the box this frame; `None` while it stays open. Return runs the primary button,
    /// Escape or a click outside dismisses it.
    pub fn show(self, ctx: &Context) -> Option<Outcome> {
        let mut outcome = None;
        let modal = egui::Modal::new(egui::Id::new(self.id))
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .stroke(Stroke::new(1.0_f32, theme::DIVIDER))
                    .corner_radius(4)
                    .inner_margin(PADDING),
            )
            .show(ctx, |ui| {
                ui.set_width(
                    WIDTH.min(ctx.content_rect().width() - 48.0) - 2.0 * f32::from(PADDING),
                );
                ui.horizontal(|ui| {
                    if !self.question {
                        warning(ui);
                    }
                    label::heading(ui, self.heading);
                });
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(self.body)
                            .font(text::font(text::BODY))
                            .color(theme::SECONDARY),
                    )
                    .wrap()
                    .selectable(false),
                );
                if !self.question && !self.rows.is_empty() {
                    ui.add_space(8.0);
                    self.table(ui);
                }
                ui.add_space(12.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if TextButton::new(self.primary)
                        .primary(true)
                        .show(ui)
                        .clicked()
                    {
                        outcome = Some(Outcome::Primary);
                    }
                    if let Some(secondary) = self.secondary
                        && TextButton::new(secondary).show(ui).clicked()
                    {
                        outcome = Some(Outcome::Secondary);
                    }
                });
                if ui.input(|input| input.key_pressed(Key::Enter)) {
                    outcome = Some(Outcome::Primary);
                }
            });
        outcome.or_else(|| modal.should_close().then_some(Outcome::Dismissed))
    }

    fn table(&self, ui: &mut Ui) {
        let width = ui.available_width() / 3.0;
        egui::Grid::new((self.id, "rows"))
            .min_col_width(width - 8.0)
            .min_row_height(ROW_HEIGHT)
            .show(ui, |ui| {
                for column in self.columns {
                    label::section(ui, column);
                }
                ui.end_row();
                for row in self.rows.iter().take(MAX_ROWS) {
                    for (cell, colour) in
                        row.iter()
                            .zip([theme::DANGER, theme::TEXT, theme::SECONDARY])
                    {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(cell)
                                    .font(text::font(text::BODY))
                                    .color(colour),
                            )
                            .truncate(),
                        );
                    }
                    ui.end_row();
                }
            });
        if self.rows.len() > MAX_ROWS {
            label::caption(ui, &format!("… and {} more", self.rows.len() - MAX_ROWS));
        }
    }
}

/// The 16-point warning glyph in `danger`.
fn warning(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), egui::Sense::hover());
    let painter = ui.painter();
    let stroke = Stroke::new(theme::metric::TOOL_STROKE, theme::DANGER);
    let [top, left, right] = [
        rect.center_top() + vec2(0.0, 1.0),
        rect.left_bottom() + vec2(0.5, -1.0),
        rect.right_bottom() + vec2(-0.5, -1.0),
    ];
    painter.add(egui::Shape::closed_line(vec![top, right, left], stroke));
    painter.line_segment(
        [
            rect.center() + vec2(0.0, -2.5),
            rect.center() + vec2(0.0, 2.5),
        ],
        stroke,
    );
    painter.circle_filled(rect.center() + vec2(0.0, 5.0), 1.0, theme::DANGER);
}
