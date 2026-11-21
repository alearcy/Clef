//! Slot button (T18 Phase 3): an insert or instrument slot with its power button. `peacock` when
//! the processor runs, `field` when empty, missing or offline; Late and Failed add a `danger` edge
//! and say so in the accessible description.
use super::power_button::power_button;
use crate::{theme, theme::MIX_SMALL};
use eframe::egui::{
    self, Key, Modifiers, Response, RichText, Sense, Stroke, Ui, WidgetInfo, WidgetType,
};

pub const SLOT_HEIGHT: f32 = 20.0;
const POWER_WIDTH: f32 = 14.0;

pub enum State {
    Active,
    /// Active, but too many blocks missed their deadline recently (T37 L1).
    Late,
    Bypassed,
    Missing,
    Failed,
    Empty,
    Offline,
}

/// Tooltip and accessible description of a Late effect slot.
pub fn late_description(percent: u8) -> String {
    format!(
        "Late: {percent} % of blocks in the last 2 s. \
         Reduce the plugin's load (for example oversampling)."
    )
}

/// `edge_inset` is the parent's trailing padding after this row. The power glyph is
/// centred between the slot and the parent's outer edge, including that inset.
pub fn slot(
    ui: &mut Ui,
    name: &str,
    channel: &str,
    role: &str,
    state: State,
    description: &str,
    edge_inset: f32,
) -> (Response, bool, bool) {
    let (fill, ink, stroke) = match state {
        State::Active => (theme::PEACOCK, theme::TEXT, Stroke::NONE),
        State::Late => (
            theme::PEACOCK,
            theme::TEXT,
            Stroke::new(1.5_f32, theme::DANGER),
        ),
        State::Bypassed => (theme::PEACOCK, theme::DISABLED, Stroke::NONE),
        State::Missing => (theme::FIELD, theme::DISABLED, Stroke::NONE),
        State::Failed => (
            theme::FIELD,
            theme::DANGER,
            Stroke::new(1.5_f32, theme::DANGER),
        ),
        State::Empty | State::Offline => (theme::FIELD, theme::DISABLED, Stroke::NONE),
    };
    let ready = matches!(state, State::Active | State::Late | State::Bypassed);
    let lit = matches!(state, State::Active | State::Late);
    ui.horizontal(|ui| {
        let gap_before = edge_inset;
        let width = (ui.available_width() - POWER_WIDTH - gap_before).max(1.0);
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(width, SLOT_HEIGHT), Sense::click_and_drag());
        response.widget_info(|| {
            WidgetInfo::labeled(
                WidgetType::Button,
                ui.is_enabled(),
                format!("{channel}, {role} {name}, {description}"),
            )
        });
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, fill);
        if stroke != Stroke::NONE {
            painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);
        }
        if response.has_focus() {
            painter.rect_stroke(
                rect,
                0.0,
                Stroke::new(1.0_f32, theme::ACCENT),
                egui::StrokeKind::Inside,
            );
        }
        let text = egui::WidgetText::from(RichText::new(name).size(MIX_SMALL).color(ink));
        let galley = text.into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            (width - 8.0).max(1.0),
            egui::TextStyle::Body,
        );
        painter.galley(
            egui::pos2(rect.left() + 4.0, rect.center().y - galley.size().y / 2.0),
            galley,
            ink,
        );
        let response = response.on_hover_text(format!("{name} · {description}"));
        let activate = response.clicked()
            || response.has_focus()
                && ui.input_mut(|input| {
                    input.consume_key(Modifiers::NONE, Key::Enter)
                        || input.consume_key(Modifiers::NONE, Key::Space)
                });
        ui.add_space(gap_before);
        let label = format!(
            "{channel}, {} {role} {name}",
            if lit { "Bypass" } else { "Enable" }
        );
        let power = power_button(ui, POWER_WIDTH, SLOT_HEIGHT, &label, lit, ready);
        (response, activate, power)
    })
    .inner
}
