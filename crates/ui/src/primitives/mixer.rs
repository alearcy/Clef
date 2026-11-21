#[cfg(test)]
use super::fader::SCALE;
pub(super) use super::fader::{axis, fader_value, fraction, paint_fader};
pub(super) use super::knob::paint_knob;
pub(crate) use super::level_meter::{header_meter, master_header, scale, strip_meter};
pub(crate) use super::mute_solo::lamp;
use crate::{theme, theme::*};
use clef_model::mixer::Control;
use eframe::egui::{
    self, Color32, Key, Modifiers, Pos2, Rect, Response, Sense, Stroke, Ui, WidgetInfo, pos2, vec2,
};

pub(crate) fn strip(ui: &mut Ui, selected: bool, content: impl FnOnce(&mut Ui)) -> Response {
    let [left, right, top, bottom] = MIX_PADDING;
    egui::Frame::new()
        .fill(theme::PANEL)
        .stroke(if selected {
            Stroke::new(1.0_f32, theme::ACCENT)
        } else {
            Stroke::new(1.0_f32, Color32::TRANSPARENT)
        })
        .inner_margin(egui::Margin {
            left,
            right,
            top,
            bottom,
        })
        .show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                let width = MIX_CHANNEL - f32::from(left + right) - 2.0;
                ui.set_width(width);
                ui.set_min_width(width);
                ui.set_max_width(width);
                ui.spacing_mut().item_spacing = vec2(0.0, MIX_ROW);
                content(ui);
            });
        })
        .response
}

pub(crate) fn small_label(ui: &mut Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(MIX_SMALL)
            .color(theme::SECONDARY),
    );
}

pub(crate) fn footer(ui: &mut Ui, name: &str, master: bool, hint: &str) -> Response {
    let label = egui::Label::new(egui::RichText::new(name).size(MIX_SMALL))
        .truncate()
        .sense(Sense::click());
    let frame = egui::Frame::new().fill(if master { theme::PEACOCK } else { theme::FIELD });
    let response = frame
        .show(ui, |ui| {
            ui.add_sized([ui.available_width(), 20.0], label)
                .on_hover_text(hint)
        })
        .inner;
    register_activation_focus(&response);
    response
}

pub(super) fn register_activation_focus(response: &Response) {
    if response.has_focus() {
        response
            .ctx
            .data_mut(|data| data.insert_temp(egui::Id::new("mixer-activation"), response.id));
    }
}

pub(crate) fn activation_has_focus(ctx: &egui::Context) -> bool {
    let id = ctx.data(|data| data.get_temp::<egui::Id>(egui::Id::new("mixer-activation")));
    id.is_some() && id == ctx.memory(|memory| memory.focused())
}

/// Where a drag started. Shift (fine) re-bases the origin at the pointer so changing it mid-drag
/// never makes the value jump.
#[derive(Clone, Copy)]
pub(crate) struct DragOrigin {
    value: i16,
    pointer: Pos2,
    fine: bool,
}

#[cfg(test)]
impl DragOrigin {
    pub(crate) fn at(value: i16) -> Self {
        Self {
            value,
            pointer: Pos2::ZERO,
            fine: false,
        }
    }
}

/// Shift-drag moves a quarter as far.
const FINE: f32 = 0.25;

#[derive(Clone, Copy)]
pub(crate) enum Adjustment {
    Drag { origin: DragOrigin, value: i16 },
    Commit(i16),
}

pub(crate) struct Slider<'a> {
    pub control: Control,
    pub value: i16,
    pub drag_start: Option<DragOrigin>,
    pub name: &'a str,
    pub master: bool,
    pub compact: bool,
}

impl Slider<'_> {
    pub(crate) fn show(&self, ui: &mut Ui) -> Option<Adjustment> {
        let fader = self.control == Control::Fader;
        let knob = if self.compact {
            crate::theme::MIX_SEND_KNOB
        } else {
            MIX_KNOB
        };
        let size = if fader {
            vec2(MIX_FADER, MIX_FADER_HEIGHT)
        } else {
            vec2(knob, knob)
        };
        let (_, rect) = ui.allocate_space(size);
        let id = ui.id().with(("mixer-control", self.control.label()));
        let response = ui.interact(rect, id, Sense::click_and_drag());
        let enabled = ui.is_enabled();
        let (min, max) = self.control.bounds();
        let name = format!("{} {}", self.name, self.control.label());
        let fine = ui.input(|i| i.modifiers.shift);
        let read_drag = |origin: DragOrigin| {
            let delta = response
                .interact_pointer_pos()
                .map_or(egui::Vec2::ZERO, |at| at - origin.pointer)
                * if origin.fine { FINE } else { 1.0 };
            if fader {
                fader_value(fraction(origin.value) - delta.y / axis(rect).height())
            } else {
                (f32::from(origin.value)
                    + (delta.x - delta.y) * f32::from(max - min) / MIX_FADER_HEIGHT)
                    .round()
                    .clamp(f32::from(min), f32::from(max)) as i16
            }
        };
        let origin = match self.drag_start {
            Some(origin) if origin.fine != fine => {
                response.interact_pointer_pos().map(|pointer| DragOrigin {
                    value: read_drag(origin),
                    pointer,
                    fine,
                })
            }
            Some(origin) => Some(origin),
            None if response.drag_started() => {
                ui.input(|i| i.pointer.press_origin())
                    .map(|pointer| DragOrigin {
                        value: self.value,
                        pointer,
                        fine,
                    })
            }
            None => None,
        };
        let live = origin
            .filter(|_| response.dragged() || response.drag_stopped())
            .map(read_drag);
        let shown = live.unwrap_or(self.value);
        let displayed = text_value(self.control, shown);
        let accessibility_value = match self.control {
            Control::Gain | Control::Fader => f64::from(shown) / 10.0,
            _ => f64::from(shown),
        };
        response.widget_info(|| WidgetInfo::slider(enabled, accessibility_value, &name));
        if fader {
            paint_fader(ui, rect, shown, response.has_focus(), self.master);
        } else {
            let progress = f32::from(shown - min) / f32::from(max - min);
            let progress = if self.control == Control::Width {
                ui.ctx()
                    .animate_value_with_time(id.with("width-fan"), progress, 0.12)
            } else {
                progress
            };
            paint_knob(ui, rect, self.control, progress, response.has_focus());
        }
        let option = ui.input(|i| i.modifiers.alt);
        let typing_id = id.with("typing");
        if enabled && response.double_clicked() {
            ui.data_mut(|data| data.insert_temp(typing_id, text_value(self.control, self.value)));
        }
        let typed = self.typing(ui, typing_id, rect);
        let event = if !enabled {
            None
        } else if typed.is_some() {
            typed
        } else if option && response.clicked() {
            Some(Adjustment::Commit(self.control.initial()))
        } else if response.drag_stopped() && origin.is_some() {
            Some(Adjustment::Commit(live.unwrap_or(self.value)))
        } else if response.dragged()
            && let Some(origin) = origin
        {
            Some(Adjustment::Drag {
                origin,
                value: live.unwrap_or(self.value),
            })
        } else if response.has_focus() {
            let (direction, large) = ui.input_mut(|i| {
                let mut direction = 0;
                let mut large = false;
                // egui ignores an extra Shift when matching, so the Shift form is tried first.
                for modifiers in [Modifiers::SHIFT, Modifiers::NONE] {
                    let up = i.consume_key(modifiers, Key::ArrowUp)
                        || i.consume_key(modifiers, Key::ArrowRight);
                    let down = i.consume_key(modifiers, Key::ArrowDown)
                        || i.consume_key(modifiers, Key::ArrowLeft);
                    let step = i16::from(up) - i16::from(down);
                    if step != 0 && direction == 0 {
                        direction = step;
                        large = modifiers.shift;
                    }
                }
                (direction, large)
            });
            (direction != 0).then(|| {
                let [small, big] = steps(self.control);
                let step = if large { big } else { small };
                Adjustment::Commit((self.value + direction * step).clamp(min, max))
            })
        } else {
            None
        };
        if self.compact {
            if response.hovered() || response.dragged() {
                response.show_tooltip_text(displayed);
            }
        } else {
            response.on_hover_text(format!("{name}: {displayed}"));
        }
        event
    }
}

impl Slider<'_> {
    /// The inline value field a double-click opens under the control. Return commits the typed
    /// value, Escape or a click elsewhere cancels.
    fn typing(&self, ui: &mut Ui, id: egui::Id, rect: Rect) -> Option<Adjustment> {
        let mut text = ui.data(|data| data.get_temp::<String>(id))?;
        let field = id.with("field");
        let mut result = None;
        let mut done = false;
        egui::Area::new(id.with("area"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos2(rect.center().x - 36.0, rect.bottom() + 2.0))
            .show(ui.ctx(), |ui| {
                let response = super::text_field::TextField::new(
                    &mut text,
                    &format!("{} {}", self.name, self.control.label()),
                )
                .id(field)
                .width(72.0)
                .show(ui);
                if !response.has_focus() && !response.lost_focus() {
                    response.request_focus();
                }
                let (enter, escape) =
                    ui.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
                if response.lost_focus() || enter || escape {
                    done = true;
                    if enter {
                        let (min, max) = self.control.bounds();
                        result = parse_value(self.control, &text)
                            .map(|value| Adjustment::Commit(value.clamp(min, max)));
                    }
                }
            });
        ui.data_mut(|data| {
            if done {
                data.remove_temp::<String>(id);
            } else {
                data.insert_temp(id, text);
            }
        });
        result
    }
}

/// Small and Shift (large) arrow steps: 0.5 and 3 dB for levels, 1 and 10 otherwise.
fn steps(control: Control) -> [i16; 2] {
    if matches!(control, Control::Gain | Control::Fader | Control::Send) {
        [5, 30]
    } else {
        [1, 10]
    }
}

/// Reads a typed value back into the control's units: the inverse of `text_value`, tolerant of a
/// missing unit, a plain minus sign and "inf".
pub(crate) fn parse_value(control: Control, text: &str) -> Option<i16> {
    let text = text.trim().replace('−', "-");
    let lower = text.to_lowercase();
    match control {
        Control::Fader | Control::Gain | Control::Send => {
            let number = lower.trim_end_matches("db").trim();
            if number.contains("inf") {
                return Some(control.bounds().0);
            }
            let db: f32 = number.parse().ok()?;
            Some((db * 10.0).round() as i16)
        }
        Control::Pan => {
            let (sign, number) = if let Some(rest) = lower.strip_prefix('l') {
                (-1, rest)
            } else if let Some(rest) = lower.strip_prefix('r') {
                (1, rest)
            } else if lower == "c" {
                return Some(0);
            } else {
                (1, lower.as_str())
            };
            let value: i16 = number.trim().parse().ok()?;
            Some(sign * value)
        }
        _ => lower.trim_end_matches('%').trim().parse().ok(),
    }
}

pub(crate) fn text_value(control: Control, value: i16) -> String {
    match control {
        Control::Fader | Control::Send if value == Control::Fader.bounds().0 => "−∞ dB".into(),
        Control::Fader | Control::Gain | Control::Send => {
            format!("{:.1} dB", f32::from(value) / 10.0)
        }
        Control::Pan if value == 0 => "C".into(),
        Control::Pan if value < 0 => format!("L {}", -value),
        Control::Pan => format!("R {value}"),
        _ => format!("{value}%"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixer_slider_commits_the_release_position_as_one_gesture() {
        let ctx = egui::Context::default();
        let frame = |events, value, drag_start| {
            let mut event = None;
            let mut rect = Rect::NOTHING;
            let _ = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    rect = Rect::from_min_size(ui.next_widget_position(), vec2(MIX_KNOB, MIX_KNOB));
                    event = Slider {
                        control: Control::Width,
                        value,
                        drag_start,
                        name: "Piano",
                        master: false,
                        compact: false,
                    }
                    .show(ui);
                },
            );
            (rect, event)
        };
        let (rect, _) = frame(vec![], 100, None);
        let point = rect.center();
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let _ = frame(
            vec![egui::Event::PointerMoved(point), button(point, true)],
            100,
            None,
        );
        let moved = point + vec2(15.0, 0.0);
        let (_, event) = frame(vec![egui::Event::PointerMoved(moved)], 100, None);
        let Some(Adjustment::Drag { origin, value }) = event else {
            panic!("drag was not delivered")
        };
        assert_eq!(value, 114);
        let end = point + vec2(21.0, 0.0);
        let (_, event) = frame(
            vec![egui::Event::PointerMoved(end), button(end, false)],
            value,
            Some(origin),
        );
        assert!(matches!(event, Some(Adjustment::Commit(120))));
    }

    /// One frame of a Width knob (0–200 %, default 100) with focus requested.
    struct Harness {
        ctx: egui::Context,
        rect: Rect,
        modifiers: Modifiers,
    }

    impl Harness {
        fn new() -> Self {
            let mut harness = Self {
                ctx: egui::Context::default(),
                rect: Rect::NOTHING,
                modifiers: Modifiers::NONE,
            };
            theme::install_fonts(&harness.ctx);
            let _ = harness.frame(vec![], Control::Width, 100, None);
            harness
        }

        fn frame(
            &mut self,
            events: Vec<egui::Event>,
            control: Control,
            value: i16,
            drag_start: Option<DragOrigin>,
        ) -> Option<Adjustment> {
            let mut event = None;
            let mut rect = Rect::NOTHING;
            let _ = self.ctx.run_ui(
                egui::RawInput {
                    events,
                    modifiers: self.modifiers,
                    ..Default::default()
                },
                |ui| {
                    rect = Rect::from_min_size(ui.next_widget_position(), vec2(MIX_KNOB, MIX_KNOB));
                    event = Slider {
                        control,
                        value,
                        drag_start,
                        name: "Piano",
                        master: false,
                        compact: false,
                    }
                    .show(ui);
                },
            );
            self.rect = rect;
            event
        }

        fn click(&mut self, modifiers: Modifiers, value: i16) -> Option<Adjustment> {
            let at = self.rect.center();
            let button = |pressed| egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers,
            };
            self.modifiers = modifiers;
            let _ = self.frame(
                vec![egui::Event::PointerMoved(at), button(true)],
                Control::Width,
                value,
                None,
            );
            let event = self.frame(vec![button(false)], Control::Width, value, None);
            self.modifiers = Modifiers::NONE;
            event
        }
    }

    fn key(key: Key, modifiers: Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn shift_drag_moves_a_quarter_as_far_without_a_jump() {
        let mut harness = Harness::new();
        let point = harness.rect.center();
        let press = egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        };
        let _ = harness.frame(
            vec![egui::Event::PointerMoved(point), press],
            Control::Width,
            100,
            None,
        );
        let at = point + vec2(21.0, 0.0);
        let Some(Adjustment::Drag { origin, value }) = harness.frame(
            vec![egui::Event::PointerMoved(at)],
            Control::Width,
            100,
            None,
        ) else {
            panic!("drag was not delivered")
        };
        assert_eq!(value, 120);
        // Shift goes down: the origin re-bases at the pointer and the value holds.
        let shift = egui::Event::Key {
            key: Key::A,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::SHIFT,
        };
        let mut input = vec![shift, egui::Event::PointerMoved(at)];
        let ctx = harness.ctx.clone();
        ctx.input_mut(|_| {});
        input.insert(0, egui::Event::PointerMoved(at));
        let event = {
            let mut event = None;
            let _ = ctx.run_ui(
                egui::RawInput {
                    events: input,
                    modifiers: Modifiers::SHIFT,
                    ..Default::default()
                },
                |ui| {
                    event = Slider {
                        control: Control::Width,
                        value: 100,
                        drag_start: Some(origin),
                        name: "Piano",
                        master: false,
                        compact: false,
                    }
                    .show(ui);
                },
            );
            event
        };
        let Some(Adjustment::Drag {
            origin: fine,
            value,
        }) = event
        else {
            panic!("fine drag was not delivered")
        };
        assert_eq!(value, 120);
        let further = at + vec2(28.0, 0.0);
        let mut event = None;
        let _ = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::PointerMoved(further)],
                modifiers: Modifiers::SHIFT,
                ..Default::default()
            },
            |ui| {
                event = Slider {
                    control: Control::Width,
                    value: 100,
                    drag_start: Some(fine),
                    name: "Piano",
                    master: false,
                    compact: false,
                }
                .show(ui);
            },
        );
        // 28 points at full speed would be +27; a quarter is +7.
        assert!(matches!(event, Some(Adjustment::Drag { value: 127, .. })));
    }

    #[test]
    fn double_click_types_a_value_and_option_click_resets() {
        let mut harness = Harness::new();
        let _ = harness.click(Modifiers::NONE, 40);
        let double = harness.click(Modifiers::NONE, 40);
        assert!(double.is_none(), "a double-click no longer resets");
        // The field opened with the current text; replace it and press Return.
        let select_all = key(Key::A, Modifiers::COMMAND);
        let _ = harness.frame(vec![], Control::Width, 40, None);
        let typed = harness.frame(
            vec![
                select_all,
                egui::Event::Text("75%".into()),
                key(Key::Enter, Modifiers::NONE),
            ],
            Control::Width,
            40,
            None,
        );
        assert!(
            matches!(typed, Some(Adjustment::Commit(75))),
            "{:?}",
            typed.map(|_| ())
        );
        // Escape cancels a new field without a change.
        let _ = harness.click(Modifiers::NONE, 75);
        let _ = harness.click(Modifiers::NONE, 75);
        let _ = harness.frame(vec![], Control::Width, 75, None);
        let cancelled = harness.frame(
            vec![
                egui::Event::Text("9".into()),
                key(Key::Escape, Modifiers::NONE),
            ],
            Control::Width,
            75,
            None,
        );
        assert!(cancelled.is_none());
        let reset = harness.click(Modifiers::ALT, 75);
        assert!(matches!(reset, Some(Adjustment::Commit(100))));
    }

    #[test]
    fn arrows_step_and_shift_arrows_take_the_large_step() {
        let mut harness = Harness::new();
        let mut id = egui::Id::NULL;
        let _ = harness.ctx.run_ui(Default::default(), |ui| {
            id = ui.id().with(("mixer-control", Control::Fader.label()));
        });
        harness.ctx.memory_mut(|memory| memory.request_focus(id));
        let mut step = |modifiers, value| {
            let _ = harness.frame(vec![], Control::Fader, value, None);
            harness.frame(
                vec![key(Key::ArrowUp, modifiers)],
                Control::Fader,
                value,
                None,
            )
        };
        assert!(matches!(
            step(Modifiers::NONE, -60),
            Some(Adjustment::Commit(-55))
        ));
        assert!(matches!(
            step(Modifiers::SHIFT, -60),
            Some(Adjustment::Commit(-30))
        ));
    }

    #[test]
    fn typed_values_parse_back_in_each_unit() {
        assert_eq!(parse_value(Control::Fader, "−6.0 dB"), Some(-60));
        assert_eq!(
            parse_value(Control::Fader, "-inf"),
            Some(Control::Fader.bounds().0)
        );
        assert_eq!(parse_value(Control::Gain, "3"), Some(30));
        assert_eq!(parse_value(Control::Pan, "L 20"), Some(-20));
        assert_eq!(parse_value(Control::Pan, "C"), Some(0));
        assert_eq!(parse_value(Control::Pan, "r5"), Some(5));
        assert_eq!(parse_value(Control::Width, "150%"), Some(150));
        assert_eq!(parse_value(Control::Width, "wide"), None);
        for value in [-600, -60, 0, 60] {
            assert_eq!(
                parse_value(Control::Fader, &text_value(Control::Fader, value)),
                Some(value)
            );
        }
    }

    #[test]
    fn fader_and_meter_share_the_same_db_ticks() {
        for (db, at) in SCALE.iter().copied().skip(1) {
            assert_eq!(fraction(db), at);
            assert_eq!(fader_value(at), db);
        }
        assert_eq!(fader_value(0.0), Control::Fader.bounds().0);
    }

    fn popup_texts(output: &egui::FullOutput) -> Vec<String> {
        fn collect(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => out.push(text.galley.text().to_owned()),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        collect(shape, out);
                    }
                }
                _ => {}
            }
        }
        let mut texts = Vec::new();
        for clipped in &output.shapes {
            collect(&clipped.shape, &mut texts);
        }
        texts
    }

    #[test]
    fn compact_send_knob_keeps_a_value_only_popup_visible_while_dragging() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let rect = std::cell::Cell::new(Rect::NOTHING);
        let frame = |ctx: &egui::Context, events, drag_start| {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(240.0, 160.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    rect.set(Rect::from_min_size(
                        ui.next_widget_position(),
                        vec2(MIX_SEND_KNOB, MIX_SEND_KNOB),
                    ));
                    Slider {
                        control: Control::Send,
                        value: -60,
                        drag_start,
                        name: "Strings",
                        master: false,
                        compact: true,
                    }
                    .show(ui);
                },
            )
        };
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let _ = frame(&ctx, vec![], None);
        let point = rect.get().center();
        let _ = frame(
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
            None,
        );
        let origin = DragOrigin {
            value: -60,
            pointer: point,
            fine: false,
        };
        let _ = frame(
            &ctx,
            vec![egui::Event::PointerMoved(point + vec2(10.0, 0.0))],
            Some(origin),
        );
        let output = frame(
            &ctx,
            vec![egui::Event::PointerMoved(point + vec2(10.0, 0.0))],
            Some(origin),
        );
        assert!(
            popup_texts(&output).iter().any(|label| label == "-2.9 dB"),
            "send popup did not show only the live value: {:?}",
            popup_texts(&output)
        );
        let output = frame(
            &ctx,
            vec![egui::Event::PointerMoved(point + vec2(20.0, 0.0))],
            Some(origin),
        );
        assert!(
            popup_texts(&output).iter().any(|label| label == "0.3 dB"),
            "send popup did not keep a live value while dragging: {:?}",
            popup_texts(&output)
        );
    }
}
