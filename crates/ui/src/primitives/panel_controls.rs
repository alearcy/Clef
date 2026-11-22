//! Lower-panel window controls (T41 S2): Detach (or Attach), Half / Full, Close, the same
//! three glyph buttons at the right end of the Editor's and the Mixer's tool row.
use super::icons::Icon;
use super::tool_button::tool_button;
use eframe::egui::Ui;

/// What a press on one of the controls asks for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PanelControl {
    /// Detach the panel into its own window, or attach a detached one again.
    Detach,
    /// Toggle the docked panel between half and full height.
    Full,
    /// Close the panel.
    Close,
}

/// The panel's display state, as the controls show it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PanelState {
    pub detached: bool,
    pub full: bool,
}

/// Draws the controls in `ui`, which must lay out right to left: the row then reads
/// Detach (or Attach), Half / Full, Close, with Close at the far right. Half / Full is lit
/// while Full and absent from a detached window, which has no height to share.
fn controls(ui: &mut Ui, state: PanelState, keyboard: bool) -> Option<PanelControl> {
    let mut picked = None;
    if tool_button(ui, Icon::Close, "Close", true, false, keyboard).clicked() {
        picked = Some(PanelControl::Close);
    }
    if !state.detached {
        let name = if state.full {
            "Half Height"
        } else {
            "Full Height"
        };
        if tool_button(ui, Icon::PanelFull, name, true, state.full, keyboard).clicked() {
            picked = Some(PanelControl::Full);
        }
    }
    let (glyph, name) = if state.detached {
        (Icon::Attach, "Attach to Main Window")
    } else {
        (Icon::Detach, "Detach into Own Window")
    };
    if tool_button(ui, glyph, name, true, false, keyboard).clicked() {
        picked = Some(PanelControl::Detach);
    }
    picked
}

/// Width of the three controls with their spacing.
pub const WIDTH: f32 = 3.0 * crate::theme::TOOL_TARGET + 2.0 * SPACING;
const SPACING: f32 = 4.0;

/// Draws the controls right-aligned in `row` (a rectangle the caller has allocated).
pub fn at_right(
    ui: &mut Ui,
    row: eframe::egui::Rect,
    state: PanelState,
    keyboard: bool,
) -> Option<PanelControl> {
    use eframe::egui::{Align, Layout, Rect, UiBuilder, pos2};
    let rect = Rect::from_min_max(pos2(row.right() - WIDTH, row.top()), row.max);
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::right_to_left(Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = SPACING;
            controls(ui, state, keyboard)
        },
    )
    .inner
}
