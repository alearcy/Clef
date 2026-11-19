use crate::{colors, theme};
use clef_model::timeline::{MAX_BAR, MAX_TICK, Position, Tempo};
use eframe::egui::{self, Color32, Context, Painter, Pos2, Response, Shape, Stroke, Ui, vec2};

pub fn button(ui: &mut Ui, label: &str, name: &str, enabled: bool, on: bool) -> bool {
    let button = egui::Button::new(label).selected(on);
    let response = ui.add_enabled(enabled, button);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, on, name)
    });
    response.on_hover_text(name).clicked()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    Start,
    Play,
    Stop,
    Record,
    Loop,
    Metronome,
}

pub fn tool_button(
    ui: &mut Ui,
    glyph: Glyph,
    name: &str,
    enabled: bool,
    active: bool,
    keyboard_focus: bool,
) -> Response {
    let response = ui
        .add_enabled_ui(enabled, |ui| {
            ui.allocate_response(
                vec2(theme::TOOL_TARGET, theme::TOOL_TARGET),
                egui::Sense::click(),
            )
        })
        .inner;
    let enabled = response.enabled();
    response.widget_info(|| {
        if matches!(
            glyph,
            Glyph::Play | Glyph::Record | Glyph::Loop | Glyph::Metronome
        ) {
            egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, active, name)
        } else {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name)
        }
    });
    let down = response.is_pointer_button_down_on();
    let color = if active || glyph == Glyph::Record && (down || response.clicked()) {
        match glyph {
            Glyph::Record => colors::DANGER,
            Glyph::Loop => theme::LOOP_ACTIVE,
            _ => colors::ACCENT,
        }
    } else if !enabled {
        colors::DISABLED
    } else if down {
        colors::SECONDARY
    } else if response.hovered() {
        theme::CAP_LIGHT
    } else {
        colors::TEXT
    };
    let painter = ui.painter_at(response.rect);
    let center = response.rect.center();
    glyph.paint(&painter, center, color);
    if keyboard_focus && response.has_focus() {
        painter.circle_stroke(
            center,
            (theme::TOOL_TARGET + theme::TOOL_GLYPH) / 4.0,
            Stroke::new(theme::TOOL_STROKE, colors::ACCENT),
        );
    }
    response.on_hover_text(name)
}

impl Glyph {
    fn paint(self, painter: &Painter, center: Pos2, color: Color32) {
        let point = |x, y| center + vec2(x, y) * (theme::TOOL_GLYPH / 2.0);
        let stroke = Stroke::new(theme::TOOL_STROKE, color);
        let line = |points: &[(f32, f32)]| {
            painter.add(Shape::line(
                points.iter().map(|&(x, y)| point(x, y)).collect(),
                stroke,
            ));
        };
        let polygon = |points: &[(f32, f32)], fill, edge| {
            painter.add(Shape::convex_polygon(
                points.iter().map(|&(x, y)| point(x, y)).collect(),
                fill,
                edge,
            ));
        };
        match self {
            Self::Start => {
                line(&[(-0.8, -0.7), (-0.8, 0.7)]);
                polygon(
                    &[(-0.55, 0.0), (0.65, -0.75), (0.65, 0.75)],
                    color,
                    Stroke::NONE,
                );
            }
            Self::Play => {
                polygon(
                    &[(-0.55, -0.8), (0.8, 0.0), (-0.55, 0.8)],
                    color,
                    Stroke::NONE,
                );
            }
            Self::Stop => {
                painter.rect_filled(
                    egui::Rect::from_two_pos(point(-0.65, -0.65), point(0.65, 0.65)),
                    0.0,
                    color,
                );
            }
            Self::Record => {
                painter.circle_filled(center, theme::TOOL_GLYPH * 0.4, color);
            }
            Self::Loop => {
                line(&[(-0.84, -0.02), (-0.84, -0.5), (0.82, -0.5)]);
                line(&[(0.43, -0.89), (0.82, -0.5), (0.43, -0.11)]);
                line(&[(0.84, 0.02), (0.84, 0.5), (-0.82, 0.5)]);
                line(&[(-0.43, 0.89), (-0.82, 0.5), (-0.43, 0.11)]);
            }
            Self::Metronome => {
                polygon(
                    &[(-0.72, 0.78), (-0.25, -0.72), (0.25, -0.72), (0.72, 0.78)],
                    Color32::TRANSPARENT,
                    stroke,
                );
                line(&[(-0.04, 0.43), (0.74, -0.49)]);
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Modifiers, PointerButton, RawInput};

    #[test]
    fn record_glyph_stays_red_from_press_through_release_and_recording() {
        let ctx = Context::default();
        let mut center = Pos2::ZERO;
        let mut frame = |events, enabled, active| {
            let output = ctx.run_ui(
                RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    center = tool_button(ui, Glyph::Record, "Record MIDI", enabled, active, false)
                        .rect
                        .center();
                },
            );
            output
                .shapes
                .into_iter()
                .filter_map(|shape| match shape.shape {
                    Shape::Circle(circle) if circle.center == center => Some(circle.fill),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(frame(vec![], true, false), vec![colors::TEXT]);
        let target = Pos2::new(theme::TOOL_TARGET / 2.0, theme::TOOL_TARGET / 2.0);
        let button = |pressed| Event::PointerButton {
            pos: target,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(
            frame(vec![Event::PointerMoved(target), button(true)], true, false),
            vec![colors::DANGER],
        );
        assert_eq!(
            frame(vec![button(false)], true, false),
            vec![colors::DANGER]
        );
        assert_eq!(frame(vec![], false, true), vec![colors::DANGER]);
        assert_eq!(
            frame(vec![Event::PointerGone], true, false),
            vec![colors::TEXT]
        );
        assert_eq!(frame(vec![], false, false), vec![colors::DISABLED]);
    }
}
