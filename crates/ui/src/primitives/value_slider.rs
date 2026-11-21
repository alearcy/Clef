//! Value slider (T18 Phase 2): a bounded number with a unit. Drag to change (Shift is fine),
//! double-click to type (Return commits, Escape cancels), ↑/↓ step with focus (Shift is a large
//! step; the track form also takes ←/→), Option-click resets to the default.
use crate::{
    paint::{self, BodyState, Depth},
    theme::{self, metric, text},
};
use eframe::egui::{
    self, Align2, CornerRadius, Key, Response, Sense, Stroke, StrokeKind, Ui, WidgetInfo,
    emath::Numeric, vec2,
};
use std::ops::RangeInclusive;

/// Points of drag per step; Shift makes it four times finer.
const DRAG_PER_STEP: f32 = 4.0;

#[must_use]
pub struct ValueSlider<'a, T: Numeric> {
    name: &'a str,
    value: &'a mut T,
    range: RangeInclusive<T>,
    step: f64,
    large_step: f64,
    default: Option<T>,
    unit: &'a str,
    width: f32,
    track: bool,
    enabled: bool,
}

impl<'a, T: Numeric> ValueSlider<'a, T> {
    /// `name` is the accessible name ("Tempo").
    pub fn new(name: &'a str, value: &'a mut T, range: RangeInclusive<T>) -> Self {
        Self {
            name,
            value,
            range,
            step: 1.0,
            large_step: 10.0,
            default: None,
            unit: "",
            width: 88.0,
            track: false,
            enabled: true,
        }
    }

    pub fn step(mut self, step: f64, large: f64) -> Self {
        self.step = step;
        self.large_step = large;
        self
    }

    /// Option-click restores `default`.
    pub fn default_value(mut self, default: T) -> Self {
        self.default = Some(default);
        self
    }

    /// Shown after the number ("BPM").
    pub fn unit(mut self, unit: &'a str) -> Self {
        self.unit = unit;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// The horizontal travel form: a recessed track filled to the value, set by position.
    pub fn track(mut self) -> Self {
        self.track = true;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    fn text(&self, value: f64) -> String {
        let number = if T::INTEGRAL {
            format!("{value:.0}")
        } else {
            format!("{value:.2}")
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_owned()
        };
        if self.unit.is_empty() {
            number
        } else {
            format!("{number} {}", self.unit)
        }
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let enabled = self.enabled && ui.is_enabled();
        let (low, high) = (self.range.start().to_f64(), self.range.end().to_f64());
        let clamp = |value: f64| {
            let value = value.clamp(low, high);
            if T::INTEGRAL { value.round() } else { value }
        };
        let (rect, _) =
            ui.allocate_exact_size(vec2(self.width, metric::CONTROL_HEIGHT), Sense::hover());
        let id = ui.make_persistent_id(("value-slider", self.name));
        let editing_id = id.with("editing");
        let mut editing: Option<String> = ui.data(|data| data.get_temp(editing_id));
        let mut response = ui.interact(
            rect,
            id,
            match (enabled, self.track) {
                (false, _) => Sense::hover(),
                // The travel follows the pointer from the press, like a fader.
                (true, true) => Sense::drag(),
                (true, false) => Sense::click_and_drag(),
            },
        );
        let start = self.value.to_f64();
        let mut value = start;
        if enabled && editing.is_none() {
            let shift = ui.input(|input| input.modifiers.shift);
            let alt = ui.input(|input| input.modifiers.alt);
            if response.clicked() && alt {
                if let Some(default) = self.default {
                    value = default.to_f64();
                }
            } else if response.double_clicked() {
                editing = Some(
                    self.text(value)
                        .trim_end_matches(self.unit)
                        .trim()
                        .to_owned(),
                );
            } else if self.track
                && response.dragged()
                && let Some(pointer) = response.interact_pointer_pos()
            {
                let t = ((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64;
                value = low + t * (high - low);
            } else if !self.track && response.dragged() {
                let delta = response.drag_delta();
                let moved = delta.x - delta.y;
                let fine = if shift { 4.0 } else { 1.0 };
                let accumulated_id = id.with("drag");
                let carried: f32 = ui.data(|data| data.get_temp(accumulated_id).unwrap_or(0.0));
                let total = carried + moved;
                let steps = (total / (DRAG_PER_STEP * fine)).trunc();
                ui.data_mut(|data| {
                    data.insert_temp(accumulated_id, total - steps * DRAG_PER_STEP * fine)
                });
                value += f64::from(steps) * self.step;
            }
            if response.drag_stopped() {
                ui.data_mut(|data| data.remove_temp::<f32>(id.with("drag")));
            }
            if response.has_focus() {
                // Shift first: an unmodified pattern also matches with Shift held.
                let (up, down) = ui.input_mut(|input| {
                    let mut pressed = |modifiers, key| input.consume_key(modifiers, key);
                    let large_up = pressed(egui::Modifiers::SHIFT, Key::ArrowUp);
                    let large_down = pressed(egui::Modifiers::SHIFT, Key::ArrowDown);
                    let up = pressed(egui::Modifiers::NONE, Key::ArrowUp)
                        || self.track && pressed(egui::Modifiers::NONE, Key::ArrowRight);
                    let down = pressed(egui::Modifiers::NONE, Key::ArrowDown)
                        || self.track && pressed(egui::Modifiers::NONE, Key::ArrowLeft);
                    (
                        if large_up { 2 } else { u8::from(up) },
                        if large_down { 2 } else { u8::from(down) },
                    )
                });
                let size = |kind: u8| match kind {
                    2 => self.large_step,
                    1 => self.step,
                    _ => 0.0,
                };
                value += size(up) - size(down);
            }
        }
        if let Some(mut typed) = editing.take() {
            let field_id = id.with("field");
            let field = ui.put(
                rect,
                egui::TextEdit::singleline(&mut typed)
                    .id(field_id)
                    .font(text::font(text::BODY))
                    .horizontal_align(egui::Align::Center)
                    .frame(egui::Frame::new()),
            );
            if !field.has_focus() && !field.lost_focus() {
                field.request_focus();
            }
            let cancel = ui.input(|input| input.key_pressed(Key::Escape));
            if field.lost_focus() || cancel {
                if !cancel && let Ok(parsed) = typed.trim().parse::<f64>() {
                    value = parsed;
                }
                response.request_focus();
            } else {
                editing = Some(typed);
            }
        }
        ui.data_mut(|data| match &editing {
            Some(text) => {
                data.insert_temp(editing_id, text.clone());
            }
            None => {
                data.remove_temp::<String>(editing_id);
            }
        });
        let value = clamp(value);
        if value != start {
            *self.value = T::from_f64(value);
            response.mark_changed();
        }
        let shown = self.text(value);
        let label = format!("{}: {shown}", self.name);
        response.widget_info(|| {
            let mut info = WidgetInfo::slider(enabled, value, self.name);
            info.label = Some(label.clone());
            info
        });
        if ui.is_rect_visible(rect) && editing.is_none() {
            let state = BodyState::of(&response);
            let body = paint::control_body(ui.painter(), rect, Depth::Recessed, state);
            let ink = if enabled {
                theme::TEXT
            } else {
                theme::DISABLED
            };
            if self.track {
                let t = if high > low {
                    ((value - low) / (high - low)) as f32
                } else {
                    0.0
                };
                let fill = egui::Rect::from_min_max(
                    body.min,
                    egui::pos2(body.left() + body.width() * t, body.bottom()),
                );
                if enabled {
                    ui.painter().rect_filled(
                        fill,
                        CornerRadius::same(metric::CORNER_CONTROL),
                        theme::PEACOCK,
                    );
                }
                ui.painter().vline(
                    fill.right(),
                    body.y_range(),
                    Stroke::new(
                        2.0_f32,
                        if enabled {
                            theme::TEXT
                        } else {
                            theme::DISABLED
                        },
                    ),
                );
            } else {
                ui.painter().text(
                    body.center(),
                    Align2::CENTER_CENTER,
                    &shown,
                    text::font(text::BODY),
                    ink,
                );
            }
            if response.has_focus() && paint::keyboard(ui.ctx()) {
                ui.painter().rect_stroke(
                    body,
                    CornerRadius::same(metric::CORNER_CONTROL),
                    Stroke::new(1.0_f32, theme::ACCENT),
                    StrokeKind::Inside,
                );
            }
        } else if editing.is_some() {
            ui.painter().rect_stroke(
                paint::body_rect(rect),
                CornerRadius::same(metric::CORNER_CONTROL),
                Stroke::new(1.0_f32, theme::ACCENT),
                StrokeKind::Inside,
            );
        }
        response.on_hover_text(label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Modifiers, PointerButton, Pos2, RawInput};

    struct Harness {
        ctx: egui::Context,
        value: u32,
    }

    impl Harness {
        fn frame(&mut self, modifiers: Modifiers, events: Vec<Event>) -> Response {
            let mut out = None;
            let value = &mut self.value;
            let _ = self.ctx.run_ui(
                RawInput {
                    modifiers,
                    events,
                    ..Default::default()
                },
                |ui| {
                    out = Some(
                        ValueSlider::new("Tempo", value, 20..=400)
                            .unit("BPM")
                            .default_value(120)
                            .show(ui),
                    );
                },
            );
            out.unwrap()
        }
    }

    fn key(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn button(pos: Pos2, pressed: bool, modifiers: Modifiers) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers,
        }
    }

    #[test]
    fn arrows_step_option_click_resets_and_typing_commits_or_cancels() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut h = Harness { ctx, value: 100 };
        let none = Modifiers::NONE;
        let response = h.frame(none, vec![]);
        let at = response.rect.center();
        h.ctx.memory_mut(|memory| memory.request_focus(response.id));
        h.frame(none, vec![key(Key::ArrowUp, none)]);
        assert_eq!(h.value, 101);
        h.frame(
            Modifiers::SHIFT,
            vec![key(Key::ArrowDown, Modifiers::SHIFT)],
        );
        assert_eq!(h.value, 91);

        let alt = Modifiers::ALT;
        h.frame(alt, vec![Event::PointerMoved(at), button(at, true, alt)]);
        h.frame(alt, vec![button(at, false, alt)]);
        assert_eq!(h.value, 120, "Option-click restores the default");

        // Double-click opens the editor; typed values clamp to the range on Return.
        for _ in 0..2 {
            h.frame(none, vec![button(at, true, none)]);
            h.frame(none, vec![button(at, false, none)]);
        }
        h.frame(none, vec![]);
        h.frame(Modifiers::COMMAND, vec![key(Key::A, Modifiers::COMMAND)]);
        h.frame(none, vec![Event::Text("999".into())]);
        h.frame(none, vec![key(Key::Enter, none)]);
        h.frame(none, vec![]);
        assert_eq!(h.value, 400);

        for _ in 0..2 {
            h.frame(none, vec![button(at, true, none)]);
            h.frame(none, vec![button(at, false, none)]);
        }
        h.frame(none, vec![]);
        h.frame(none, vec![Event::Text("5".into())]);
        h.frame(none, vec![key(Key::Escape, none)]);
        h.frame(none, vec![]);
        assert_eq!(h.value, 400, "Escape cancels typing");
    }
}
