use crate::{
    colors,
    theme::{CAP_LIGHT, MIX_SMALL},
};
use eframe::egui::{
    self, Button, Key, Modifiers, Response, RichText, Sense, Shape, Stroke, Ui, WidgetInfo,
    WidgetType, vec2,
};

pub const SLOT_HEIGHT: f32 = 20.0;
pub const GAP_HEIGHT: f32 = 5.0;
const POWER_WIDTH: f32 = 16.0;
const INSERT_LINE: f32 = 3.0;

pub enum State {
    Active,
    Bypassed,
    Missing,
    Failed,
    Empty,
    Offline,
}

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
            Stroke::new(INSERT_LINE, colors::ACCENT),
        );
    }
    response
}

pub fn slot(
    ui: &mut Ui,
    name: &str,
    channel: &str,
    role: &str,
    state: State,
    description: &str,
) -> (Response, bool, bool) {
    let (fill, ink, stroke) = match state {
        State::Active => (colors::PEACOCK, colors::TEXT, Stroke::NONE),
        State::Bypassed => (colors::PEACOCK, colors::DISABLED, Stroke::NONE),
        State::Missing => (colors::FIELD, colors::DISABLED, Stroke::NONE),
        State::Failed => (
            colors::FIELD,
            colors::DANGER,
            Stroke::new(1.5_f32, colors::DANGER),
        ),
        State::Empty | State::Offline => (colors::FIELD, colors::DISABLED, Stroke::NONE),
    };
    let ready = matches!(state, State::Active | State::Bypassed);
    let lit = matches!(state, State::Active);
    ui.horizontal(|ui| {
        let width = (ui.available_width() - POWER_WIDTH - ui.spacing().item_spacing.x).max(1.0);
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
                Stroke::new(1.0_f32, colors::ACCENT),
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
        let power = power_button(ui, channel, role, name, lit, ready);
        (response, activate, power)
    })
    .inner
}

fn power_button(
    ui: &mut Ui,
    channel: &str,
    role: &str,
    name: &str,
    lit: bool,
    ready: bool,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(POWER_WIDTH, SLOT_HEIGHT), Sense::click());
    let enabled = ready && ui.is_enabled();
    let label = format!(
        "{channel}, {} {role} {name}",
        if lit { "Bypass" } else { "Enable" }
    );
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, enabled, lit, &label));
    let color = if !enabled {
        colors::DISABLED
    } else if response.hovered() {
        CAP_LIGHT
    } else if lit {
        colors::ACCENT
    } else {
        colors::SECONDARY
    };
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let radius = 4.5;
    let stroke = Stroke::new(1.4_f32, color);
    let start = -std::f32::consts::FRAC_PI_2 + 0.55;
    let end = start + std::f32::consts::TAU - 1.1;
    let arc: Vec<_> = (0..=20)
        .map(|step| {
            let angle = start + (end - start) * step as f32 / 20.0;
            center + vec2(angle.cos(), angle.sin()) * radius
        })
        .collect();
    painter.add(Shape::line(arc, stroke));
    painter.line_segment(
        [center + vec2(0.0, -radius - 1.5), center + vec2(0.0, 0.5)],
        stroke,
    );
    if response.has_focus() {
        painter.circle_stroke(center, 7.0, Stroke::new(1.0_f32, colors::ACCENT));
    }
    if !enabled {
        return false;
    }
    let keyboard = response.has_focus()
        && ui.input_mut(|input| {
            input.consume_key(Modifiers::NONE, Key::Enter)
                || input.consume_key(Modifiers::NONE, Key::Space)
        });
    response.on_hover_text(label).clicked() || keyboard
}

pub fn append(ui: &mut Ui, channel: &str) -> Response {
    let response = ui.add_sized(
        [ui.available_width(), SLOT_HEIGHT],
        Button::new(RichText::new("Add effect…").size(MIX_SMALL))
            .truncate()
            .fill(colors::FIELD)
            .stroke(Stroke::new(1.0_f32, colors::DIVIDER)),
    );
    let label = format!("{channel}, Add audio effect…");
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), &label));
    response.on_hover_text(label)
}

pub fn menu_item(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    if ui
        .add_enabled(enabled, Button::new(label).frame(false))
        .clicked()
    {
        ui.close();
        true
    } else {
        false
    }
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
    let modal = egui::Modal::new(egui::Id::new("effect-picker")).show(ctx, |ui| {
        ui.set_min_width(340.0);
        ui.heading("Add Audio Effect");
        let query = ui.add(egui::TextEdit::singleline(search).hint_text("Search effects"));
        if ui.ctx().memory(|memory| memory.focused()).is_none() {
            query.request_focus();
        }
        if searching {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Scanning plugins…");
            });
        }
        egui::ScrollArea::vertical()
            .max_height(280.0)
            .show(ui, |ui| {
                let search = search.to_lowercase();
                let mut found = false;
                for (index, name, enabled) in entries {
                    let name = name.as_ref();
                    if name.to_lowercase().contains(&search) {
                        found = true;
                        if ui
                            .add_enabled(enabled, Button::new(name).frame(false))
                            .clicked()
                        {
                            selected = Some(index);
                        }
                    }
                }
                if !found && !searching {
                    ui.colored_label(colors::SECONDARY, "No matching audio effects.");
                }
            });
        ui.horizontal(|ui| {
            scan = ui.add_enabled(!searching, Button::new("Rescan")).clicked();
            cancel = ui.button("Cancel").clicked();
        });
    });
    (selected, scan, cancel || modal.should_close())
}
