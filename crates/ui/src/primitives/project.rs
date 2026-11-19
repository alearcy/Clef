//! Project chooser and confirmation primitives using the Composer Dark roles.
use crate::colors;
use eframe::egui::{self, Context, Ui};

const SF_PRO: &str = "Clef SF Pro";

pub fn install_font(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut family = fonts.families[&egui::FontFamily::Proportional].clone();
    if let Ok(data) = std::fs::read("/System/Library/Fonts/SFNS.ttf") {
        fonts
            .font_data
            .insert(SF_PRO.into(), egui::FontData::from_owned(data).into());
        family.insert(0, SF_PRO.into());
    }
    fonts
        .families
        .insert(egui::FontFamily::Name(SF_PRO.into()), family);
    ctx.set_fonts(fonts);
}

fn typography(ui: &mut Ui) {
    for (style, size) in [
        (egui::TextStyle::Heading, 16.0),
        (egui::TextStyle::Body, 13.0),
        (egui::TextStyle::Button, 13.0),
        (egui::TextStyle::Small, 11.0),
    ] {
        ui.style_mut().text_styles.insert(
            style,
            egui::FontId::new(size, egui::FontFamily::Name(SF_PRO.into())),
        );
    }
}

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
            .fill(colors::PANEL)
            .stroke(egui::Stroke::new(1.0_f32, colors::DIVIDER))
            .corner_radius(4)
            .inner_margin(egui::Margin::symmetric(22, 20))
            .show(ui, |ui| {
                typography(ui);
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
    ui.label(egui::RichText::new(text).heading().strong());
}

pub fn body(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).text_style(egui::TextStyle::Body));
}

pub fn caption(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).small().color(colors::SECONDARY));
}

pub fn row(
    ui: &mut Ui,
    name: &str,
    detail: &str,
    accessible_detail: &str,
    selected: bool,
    available: bool,
) -> egui::Response {
    let response = ui.add_sized(
        [ui.available_width(), 44.0],
        egui::Button::new("").selected(selected).fill(if selected {
            colors::FIELD
        } else {
            colors::PANEL
        }),
    );
    if response.hovered() && !selected {
        ui.painter()
            .rect_filled(response.rect.shrink(1.0), 2, colors::FIELD);
    }
    if selected || response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.shrink(1.0),
            2,
            egui::Stroke::new(1.0_f32, colors::ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    for (text, style, color, offset) in [
        (name, egui::TextStyle::Body, colors::TEXT, 7.0),
        (detail, egui::TextStyle::Small, colors::SECONDARY, 24.0),
    ] {
        let mut layout = egui::text::LayoutJob::simple(
            text.into(),
            style.resolve(ui.style()),
            if available { color } else { colors::DISABLED },
            (response.rect.width() - 16.0).max(1.0),
        );
        layout.wrap.max_rows = 1;
        let position = response.rect.left_top() + egui::vec2(8.0, offset);
        ui.painter()
            .galley(position, ui.painter().layout_job(layout), color);
    }
    let full = format!("{name}, {accessible_detail}");
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, &full)
    });
    response.on_hover_text(full)
}

pub fn button(ui: &mut Ui, text: &str, enabled: bool, primary: bool) -> egui::Response {
    let mut button = egui::Button::new(text);
    if primary && enabled {
        button = button.fill(colors::PEACOCK);
    }
    ui.add_enabled(enabled, button)
}

pub fn modal(ctx: &Context, draw: impl FnOnce(&mut Ui)) -> bool {
    egui::Modal::new(egui::Id::new("startup recovery"))
        .show(ctx, |ui| {
            typography(ui);
            ui.set_width(ui.ctx().content_rect().width().min(540.0) - 48.0);
            draw(ui);
        })
        .should_close()
}
