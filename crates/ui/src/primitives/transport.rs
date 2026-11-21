use super::{
    menu_button::{choice, menu_button},
    scroll_zoom_bars::{self, Axis, Zoom},
    text_button::TextButton,
    value_slider::ValueSlider,
    window_frame::ToolWindow,
};
use crate::theme;
use clef_model::timeline::{MAX_BAR, MAX_TICK, Position, Tempo};
use eframe::egui::{self, Context, Ui};

pub fn button(ui: &mut Ui, label: &str, name: &str, enabled: bool, on: bool) -> bool {
    TextButton::new(label)
        .toggle(on)
        .enabled(enabled)
        .accessible(name)
        .show(ui)
        .clicked()
}

pub use super::{icons::Icon, tool_button::tool_button};

/// The bar.beat.tick counter as a segment display; the count-in state is a caption beside it,
/// never only a colour.
pub fn counter(ui: &mut Ui, position: Position, counting: bool, recording: bool) {
    let text = format!("{}.{:02}.{:04}", position.bar, position.beat, position.tick);
    super::segment_display::segment_display(ui, "Bar, beat and tick", &text, recording, 120.0)
        .on_hover_text("Bar, beat and tick");
    if counting {
        super::text::caption(ui, "Count-in");
    }
}

pub fn bar(ui: &mut Ui, draw: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(theme::PANEL)
        .inner_margin(6.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.set_min_height(28.0);
                draw(ui);
            });
        });
}

/// Zoom range of the editing surfaces, in pixels per quarter note.
pub const ZOOM_RANGE: std::ops::RangeInclusive<f32> = 8.0..=240.0;
/// One zoom button step.
const ZOOM_STEP: f32 = 1.25;

/// The horizontal scroll and zoom bar of a time axis. `offset` is the first visible tick,
/// `zoom` pixels per quarter, `max_offset` the furthest first tick, `view_width` the visible
/// lane width in points and `bar_width` the bar's own width.
pub fn navigation(
    ui: &mut Ui,
    name: &str,
    offset: &mut u64,
    zoom: &mut f32,
    max_offset: u64,
    view_width: f32,
    bar_width: f32,
) {
    let visible = f64::from(view_width) / f64::from(*zoom) * clef_model::timeline::QUARTER as f64;
    let axis = Axis {
        offset: *offset as f32,
        visible: visible as f32,
        total: max_offset as f32 + visible as f32,
    };
    let moved = ui
        .allocate_ui(egui::vec2(bar_width, theme::metric::SCROLL_BAR), |ui| {
            scroll_zoom_bars::horizontal(ui, name, axis, true)
        })
        .inner;
    if let Some(next) = moved.offset {
        *offset = (next.round() as u64).min(max_offset);
    }
    match moved.zoom {
        Some(Zoom::In) => *zoom = (*zoom * ZOOM_STEP).clamp(*ZOOM_RANGE.start(), *ZOOM_RANGE.end()),
        Some(Zoom::Out) => {
            *zoom = (*zoom / ZOOM_STEP).clamp(*ZOOM_RANGE.start(), *ZOOM_RANGE.end())
        }
        None => {}
    }
}

pub fn window(ctx: &Context, title: &str, open: &mut bool, draw: impl FnOnce(&mut Ui)) {
    ToolWindow::new(title).show(ctx, open, draw);
}

pub fn tempo_fields(ui: &mut Ui, tempo: &mut Tempo) {
    egui::Grid::new("tempo-fields").show(ui, |ui| {
        ui.label("Bar");
        ValueSlider::new("Bar", &mut tempo.bar, 1..=MAX_BAR)
            .default_value(1)
            .show(ui);
        ui.end_row();
        ui.label("Tempo");
        ValueSlider::new("Tempo", &mut tempo.bpm, 20..=400)
            .unit("BPM")
            .default_value(120)
            .show(ui);
        ui.end_row();
        ui.label("Meter");
        ui.horizontal(|ui| {
            ValueSlider::new("Beats per bar", &mut tempo.beats, 1..=16)
                .default_value(4)
                .width(48.0)
                .show(ui);
            let current = tempo.unit;
            let (_, unit) = menu_button(
                ui,
                "meter-unit",
                "Beat unit",
                None,
                &current.to_string(),
                true,
                |ui| {
                    [1, 2, 4, 8, 16, 32]
                        .into_iter()
                        .find(|&unit| choice(ui, &unit.to_string(), unit == current))
                },
            );
            if let Some(Some(unit)) = unit {
                tempo.unit = unit;
            }
        });
        ui.end_row();
    });
}

pub fn tick_field(ui: &mut Ui, label: &str, tick: &mut u64) {
    ui.horizontal(|ui| {
        ui.label(label);
        ValueSlider::new(label, tick, 0..=MAX_TICK)
            .unit("ticks")
            .step(1.0, 960.0)
            .default_value(0)
            .width(140.0)
            .show(ui);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Shape};

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
                    center = tool_button(ui, Icon::Record, "Record MIDI", enabled, active, false)
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
        assert_eq!(frame(vec![], true, false), vec![theme::TEXT]);
        let target = Pos2::new(theme::TOOL_TARGET / 2.0, theme::TOOL_TARGET / 2.0);
        let button = |pressed| Event::PointerButton {
            pos: target,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(
            frame(vec![Event::PointerMoved(target), button(true)], true, false),
            vec![theme::DANGER],
        );
        assert_eq!(frame(vec![button(false)], true, false), vec![theme::DANGER]);
        assert_eq!(frame(vec![], false, true), vec![theme::DANGER]);
        assert_eq!(
            frame(vec![Event::PointerGone], true, false),
            vec![theme::TEXT]
        );
        assert_eq!(frame(vec![], false, false), vec![theme::DISABLED]);
    }
}
