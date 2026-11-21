//! Menu button: shows one current value and opens a menu of choices under itself.
use crate::{
    paint::{self, BodyState, Depth},
    theme::{self, metric, text},
};
use eframe::egui::{
    self, Align2, Popup, PopupCloseBehavior, Response, Sense, Ui, WidgetInfo, WidgetType, vec2,
};

/// A raised `field` button showing `value` and ending in ▾. `name` is the accessible name
/// ("Snap"); `caption`, when given, is a `secondary` caption painted before the value.
/// `menu` draws the choices with [`choice`]; the menu closes when a choice is clicked.
pub fn menu_button<R>(
    ui: &mut Ui,
    id: impl std::hash::Hash,
    name: &str,
    caption: Option<&str>,
    value: &str,
    enabled: bool,
    menu: impl FnOnce(&mut Ui) -> R,
) -> (Response, Option<R>) {
    let font = text::font(text::BODY);
    let shown = format!("{value} ▾");
    let caption_width = caption.map_or(0.0, |caption| {
        ui.painter()
            .layout_no_wrap(caption.into(), font.clone(), theme::SECONDARY)
            .size()
            .x
            + 6.0
    });
    let value_width = ui
        .painter()
        .layout_no_wrap(shown.clone(), font.clone(), theme::TEXT)
        .size()
        .x;
    let width = caption_width + value_width + metric::CONTROL_PADDING * 2.0;
    let id = ui.make_persistent_id(id);
    let response = ui.add_enabled(enabled, |ui: &mut Ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(width, metric::CONTROL_HEIGHT), Sense::hover());
        ui.interact(rect, id, Sense::click())
    });
    let enabled = response.enabled();
    let label = format!("{name}: {value}");
    response.widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, enabled, &label));
    let open = Popup::is_id_open(ui.ctx(), Popup::default_response_id(&response));
    if ui.is_rect_visible(response.rect) {
        let mut state = BodyState::of(&response);
        state.pressed |= open;
        let body = paint::control_body(ui.painter(), response.rect, Depth::Raised, state);
        let mut x = body.left() + metric::CONTROL_PADDING;
        if let Some(caption) = caption {
            ui.painter().text(
                egui::pos2(x, body.center().y),
                Align2::LEFT_CENTER,
                caption,
                font.clone(),
                if enabled {
                    theme::SECONDARY
                } else {
                    theme::DISABLED
                },
            );
            x += caption_width;
        }
        ui.painter().text(
            egui::pos2(x, body.center().y),
            Align2::LEFT_CENTER,
            shown,
            font,
            if enabled {
                theme::TEXT
            } else {
                theme::DISABLED
            },
        );
    }
    let inner = super::menu::show(
        super::menu::under(&response).close_behavior(PopupCloseBehavior::CloseOnClick),
        |ui| {
            ui.set_min_width(width);
            menu(ui)
        },
    );
    (response.on_hover_text(label), inner)
}

/// One choice row in a menu button's menu, ticked when current; returns true when picked.
pub fn choice(ui: &mut Ui, label: &str, selected: bool) -> bool {
    super::menu::Item::new(label)
        .ticked(selected)
        .destructive(false)
        .show(ui)
}
