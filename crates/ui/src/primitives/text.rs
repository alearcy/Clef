//! Caption and heading: the text roles used inside surfaces (page heading, section heading,
//! body, caption, readout).
use crate::theme::{self, text};
use eframe::egui::{self, Response, RichText, Ui};

fn label(ui: &mut Ui, text: RichText) -> Response {
    ui.add(egui::Label::new(text).selectable(false))
}

/// Page or window heading: 16-point bold `text`.
pub fn heading(ui: &mut Ui, value: &str) -> Response {
    label(
        ui,
        RichText::new(value)
            .font(text::font(text::HEADING))
            .strong()
            .color(theme::TEXT),
    )
}

/// Section heading inside a panel: 11-point uppercase `secondary`, no rule.
pub fn section(ui: &mut Ui, value: &str) -> Response {
    label(
        ui,
        RichText::new(value.to_uppercase())
            .font(text::font(text::SMALL))
            .color(theme::SECONDARY),
    )
}

/// Default row and control text: 13-point `text`.
pub fn body(ui: &mut Ui, value: &str) -> Response {
    label(
        ui,
        RichText::new(value)
            .font(text::font(text::BODY))
            .color(theme::TEXT),
    )
}

/// Caption beside or above a field, or a secondary detail line: 11-point `secondary`.
pub fn caption(ui: &mut Ui, value: &str) -> Response {
    label(
        ui,
        RichText::new(value)
            .font(text::font(text::SMALL))
            .color(theme::SECONDARY),
    )
}

/// Unavailable data or a disabled description: 11-point `disabled`.
pub fn unavailable(ui: &mut Ui, value: &str) -> Response {
    label(
        ui,
        RichText::new(value)
            .font(text::font(text::SMALL))
            .color(theme::DISABLED),
    )
}

/// Error or destructive text: 13-point `danger`.
pub fn danger(ui: &mut Ui, value: &str) -> Response {
    label(
        ui,
        RichText::new(value)
            .font(text::font(text::BODY))
            .color(theme::DANGER),
    )
}

/// T41 status-row text: one 11-point line, `secondary` or `danger`, truncated with the full text
/// on hover.
pub fn status(ui: &mut Ui, value: &str, danger: bool) -> Response {
    let color = if danger {
        theme::DANGER
    } else {
        theme::SECONDARY
    };
    let response = ui.add(
        egui::Label::new(
            RichText::new(value)
                .font(text::font(text::SMALL))
                .color(color),
        )
        .selectable(false)
        .truncate(),
    );
    if value.is_empty() {
        response
    } else {
        response.on_hover_text(value)
    }
}
