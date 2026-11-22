pub use super::{
    power_button::power_button as bypass,
    slot_button::{SLOT_HEIGHT, State, late_description, slot},
};
use crate::{theme, theme::MIX_SMALL};
use eframe::egui::{self, Button, Response, RichText, Sense, Stroke, Ui, WidgetInfo, WidgetType};

pub const GAP_HEIGHT: f32 = 5.0;
const INSERT_LINE: f32 = 3.0;

pub fn gap(ui: &mut Ui, label: &str, focus: bool) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), GAP_HEIGHT), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), label));
    if focus {
        response.request_focus();
    }
    if ui.is_enabled()
        && (response.hovered() || response.contains_pointer() || response.has_focus())
    {
        ui.painter().hline(
            rect.x_range(),
            rect.center().y,
            Stroke::new(INSERT_LINE, theme::ACCENT),
        );
    }
    response
}

pub fn append(ui: &mut Ui, channel: &str) -> Response {
    append_text(ui, channel, "Add effect…", "Add audio effect…")
}

pub fn append_send(ui: &mut Ui, channel: &str) -> Response {
    append_text(ui, channel, "Add send…", "Add send…")
}

fn append_text(ui: &mut Ui, channel: &str, text: &str, action: &str) -> Response {
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().interact_size.y = SLOT_HEIGHT;
            ui.spacing_mut().button_padding.y = 0.0;
            ui.add_sized(
                [ui.available_width(), SLOT_HEIGHT],
                Button::new(RichText::new(text).size(MIX_SMALL))
                    .truncate()
                    .fill(theme::FIELD)
                    .stroke(Stroke::new(1.0_f32, theme::DIVIDER)),
            )
        })
        .inner;
    let label = format!("{channel}, {action}");
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), &label));
    response.on_hover_text(label)
}

pub fn menu_item(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    super::menu::Item::new(label).enabled(enabled).show(ui)
}

pub fn picker(
    ctx: &egui::Context,
    search: &mut String,
    searching: bool,
    entries: impl Iterator<Item = (usize, impl AsRef<str>, bool)>,
) -> (Option<usize>, bool, bool) {
    let mut selected = None;
    let mut scan = false;
    let mut cancel = false;
    let modal = egui::Modal::new(egui::Id::new("effect-picker"))
        .frame(super::window_frame::frame().inner_margin(16))
        .show(ctx, |ui| {
            ui.set_min_width(340.0);
            super::text::heading(ui, "Add Audio Effect");
            // T41 round 6: the shared search list; ↑/↓ and Return pick, Escape cancels.
            let (indices, rows): (Vec<usize>, Vec<_>) = entries
                .map(|(index, name, enabled)| {
                    (
                        index,
                        super::search_list::Entry::new(name.as_ref()).enabled(enabled),
                    )
                })
                .unzip();
            match super::search_list::show(
                ui,
                egui::Id::new("effect-picker-list"),
                search,
                "Search effects",
                &rows,
                280.0,
                if searching {
                    ""
                } else {
                    "No matching audio effects."
                },
            ) {
                Some(super::search_list::Outcome::Picked(row)) => selected = Some(indices[row]),
                Some(super::search_list::Outcome::Cancelled) => cancel = true,
                None => {}
            }
            if searching {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Scanning plugins…");
                });
            }
            ui.horizontal(|ui| {
                scan = super::text_button::TextButton::new("Rescan")
                    .enabled(!searching)
                    .show(ui)
                    .clicked();
                cancel |= super::text_button::TextButton::new("Cancel")
                    .show(ui)
                    .clicked();
            });
        });
    (selected, scan, cancel || modal.should_close())
}
