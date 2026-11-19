use crate::colors;
use clef_model::timeline::{MAX_BAR, MAX_TICK, Position, Tempo};
use eframe::egui::{self, Context, Ui};

pub fn button(ui: &mut Ui, label: &str, name: &str, enabled: bool, on: bool) -> bool {
    let button = egui::Button::new(label).selected(on);
    let response = ui.add_enabled(enabled, button);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, on, name)
    });
    response.on_hover_text(name).clicked()
}

pub fn record(
    ui: &mut Ui,
    label: &str,
    name: &str,
    enabled: bool,
    active: bool,
    reason: Option<&str>,
) -> bool {
    let text = egui::RichText::new(label).color(if active { colors::DANGER } else { colors::TEXT });
    let response = ui.add_enabled(enabled, egui::Button::new(text));
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, active, name)
    });
    if let Some(reason) = reason {
        response
            .on_disabled_hover_text(reason)
            .on_hover_text(name)
            .clicked()
    } else {
        response.on_hover_text(name).clicked()
    }
}

pub fn arm(
    ui: &mut Ui,
    track: &str,
    enabled: bool,
    armed: bool,
    recording: bool,
    keyboard: bool,
) -> bool {
    let name = if recording {
        format!("Recording MIDI on {track}")
    } else if armed {
        format!("Disarm {track}")
    } else {
        format!("Arm {track} for MIDI recording")
    };
    let response = ui.add_enabled(
        enabled,
        egui::Button::new("")
            .frame(false)
            .min_size(egui::vec2(28.0, 44.0)),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, armed, &name)
    });
    let painter = ui.painter();
    let center = response.rect.center();
    let color = if armed {
        colors::DANGER
    } else if enabled {
        colors::SECONDARY
    } else {
        colors::DISABLED
    };
    painter.circle_stroke(center, 6.0, egui::Stroke::new(1.0_f32, color));
    if recording {
        painter.rect_filled(
            egui::Rect::from_center_size(center, egui::vec2(5.0, 5.0)),
            0.0,
            color,
        );
    } else if armed {
        painter.circle_filled(center, 3.0, color);
    } else {
        painter.circle_stroke(center, 2.5, egui::Stroke::new(1.0_f32, color));
    }
    if response.has_focus() && keyboard || response.hovered() && enabled {
        painter.rect_stroke(
            egui::Rect::from_center_size(center, egui::vec2(20.0, 20.0)),
            3.0,
            egui::Stroke::new(
                1.0_f32,
                if response.has_focus() && keyboard {
                    colors::ACCENT
                } else {
                    colors::FIELD
                },
            ),
            egui::StrokeKind::Inside,
        );
    }
    response.on_hover_text(name).clicked()
}

pub fn counter(ui: &mut Ui, position: Position, counting: bool) {
    let suffix = if counting { " · Count-in" } else { "" };
    ui.label(
        egui::RichText::new(format!(
            "{}.{:02}.{:04}{suffix}",
            position.bar, position.beat, position.tick
        ))
        .monospace(),
    )
    .on_hover_text("Bar, beat and tick");
}

pub fn bar(ui: &mut Ui, draw: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(colors::PANEL)
        .inner_margin(6.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.set_min_height(28.0);
                draw(ui);
            });
        });
}

pub fn navigation(ui: &mut Ui, offset: &mut u64, zoom: &mut f32, max_offset: u64) {
    ui.horizontal(|ui| {
        ui.spacing_mut().slider_width = 84.0;
        let scroll = ui.label("Scroll");
        ui.add(egui::Slider::new(offset, 0..=max_offset).show_value(false))
            .labelled_by(scroll.id);
        let scale = ui.label("Zoom");
        ui.add(
            egui::Slider::new(zoom, 8.0..=240.0)
                .logarithmic(true)
                .show_value(false),
        )
        .labelled_by(scale.id);
    });
}

pub fn window(ctx: &Context, title: &str, open: &mut bool, draw: impl FnOnce(&mut Ui)) {
    egui::Window::new(title)
        .open(open)
        .default_width(360.0)
        .show(ctx, draw);
}

pub fn tempo_fields(ui: &mut Ui, tempo: &mut Tempo) {
    egui::Grid::new("tempo-fields").show(ui, |ui| {
        let bar = ui.label("Bar");
        ui.add(egui::DragValue::new(&mut tempo.bar).range(1..=MAX_BAR))
            .labelled_by(bar.id);
        ui.end_row();
        let bpm = ui.label("Tempo");
        ui.add(
            egui::DragValue::new(&mut tempo.bpm)
                .range(20..=400)
                .suffix(" BPM"),
        )
        .labelled_by(bpm.id);
        ui.end_row();
        let meter = ui.label("Meter");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut tempo.beats).range(1..=16))
                .labelled_by(meter.id)
                .on_hover_text("Beats per bar");
            egui::ComboBox::from_id_salt("meter-unit")
                .selected_text(tempo.unit.to_string())
                .show_ui(ui, |ui| {
                    for unit in [1, 2, 4, 8, 16, 32] {
                        ui.selectable_value(&mut tempo.unit, unit, unit.to_string());
                    }
                })
                .response
                .on_hover_text("Beat unit");
        });
        ui.end_row();
    });
}

pub fn tick_field(ui: &mut Ui, label: &str, tick: &mut u64) {
    ui.horizontal(|ui| {
        let label = ui.label(label);
        ui.add(
            egui::DragValue::new(tick)
                .range(0..=MAX_TICK)
                .suffix(" ticks"),
        )
        .labelled_by(label.id);
    });
}
