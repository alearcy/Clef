//! Project chooser and confirmation primitives using the Composer Dark roles.
use crate::theme;
use eframe::egui::{self, Context, Ui};

pub fn panel(ui: &mut Ui, draw: impl FnOnce(&mut Ui)) {
    const MAX_WIDTH: f32 = 760.0;
    const OUTER_MARGIN: f32 = 20.0;
    let available = ui.available_rect_before_wrap();
    let width = (available.width() - OUTER_MARGIN * 2.0).clamp(0.0, MAX_WIDTH);
    let left = available.center().x - width / 2.0;
    let height_id = ui.id().with("startup panel height");
    let height = ui
        .ctx()
        .data(|data| data.get_temp::<f32>(height_id))
        .unwrap_or(410.0);
    let top = available.top() + ((available.height() - height) / 2.0).max(OUTER_MARGIN);
    let area = egui::Rect::from_min_max(
        egui::pos2(left, top),
        egui::pos2(left + width, available.bottom() - OUTER_MARGIN),
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
        let response = egui::Frame::new()
            .fill(theme::PANEL)
            .stroke(egui::Stroke::new(1.0_f32, theme::DIVIDER))
            .corner_radius(4)
            .inner_margin(egui::Margin::symmetric(22, 20))
            .show(ui, |ui| {
                ui.set_width((width - 46.0).max(0.0));
                egui::ScrollArea::vertical().show(ui, draw);
            })
            .response;
        let measured = response.rect.height();
        if (height - measured).abs() > 0.5 {
            ui.ctx()
                .data_mut(|data| data.insert_temp(height_id, measured));
            ui.ctx().request_repaint();
        }
    });
}

pub fn heading(ui: &mut Ui, text: &str) {
    super::text::heading(ui, text);
}

pub fn body(ui: &mut Ui, text: &str) {
    super::text::body(ui, text);
}

pub fn caption(ui: &mut Ui, text: &str) {
    super::text::caption(ui, text);
}

pub fn row(
    ui: &mut Ui,
    name: &str,
    detail: &str,
    accessible_detail: &str,
    selected: bool,
    available: bool,
) -> egui::Response {
    let full = format!("{name}, {accessible_detail}");
    super::list_row::ListRow::new(name)
        .detail(detail)
        .accessible(&full)
        .height(44.0)
        .selected(selected)
        .available(available)
        .show(ui)
}

pub fn button(ui: &mut Ui, text: &str, enabled: bool, primary: bool) -> egui::Response {
    super::text_button::TextButton::new(text)
        .primary(primary)
        .enabled(enabled)
        .show(ui)
}

pub fn modal(ctx: &Context, draw: impl FnOnce(&mut Ui)) -> bool {
    egui::Modal::new(egui::Id::new("startup recovery"))
        .frame(super::window_frame::frame().inner_margin(16))
        .show(ctx, |ui| {
            ui.set_width(ui.ctx().content_rect().width().min(540.0) - 48.0);
            draw(ui);
        })
        .should_close()
}
