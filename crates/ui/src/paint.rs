//! Shared painters for control depth, the selection edge and soft shadows (foundations, depth).
use crate::theme::{self, metric};
use eframe::egui::{Color32, CornerRadius, Id, Painter, Rect, Response, Stroke, StrokeKind, vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Depth {
    /// Controls you press or type into: top light and a soft shadow under the body.
    Raised,
    /// Travels and tracks other controls sit in: inner top shadow and a bottom light.
    Recessed,
}

/// What a control body shows this frame.
#[derive(Clone, Copy, Default, Debug)]
pub struct BodyState {
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    pub disabled: bool,
    /// A lit fill (peacock, mute, solo, accent): drops the top light, keeps the shadow.
    pub lit: Option<Color32>,
}

impl BodyState {
    /// The state of an interactive response; `focused` only counts under keyboard navigation.
    pub fn of(response: &Response) -> Self {
        Self {
            hovered: response.hovered(),
            pressed: response.is_pointer_button_down_on(),
            focused: response.has_focus() && keyboard(&response.ctx),
            disabled: !response.enabled(),
            lit: None,
        }
    }

    pub fn lit(mut self, fill: Color32) -> Self {
        self.lit = Some(fill);
        self
    }
}

const KEYBOARD: &str = "clef-keyboard-navigation";

/// Records whether the last input was keyboard navigation, so focus edges stay off after a click.
pub fn set_keyboard(ctx: &eframe::egui::Context, keyboard: bool) {
    ctx.data_mut(|data| data.insert_temp(Id::new(KEYBOARD), keyboard));
}

pub fn keyboard(ctx: &eframe::egui::Context) -> bool {
    ctx.data(|data| data.get_temp(Id::new(KEYBOARD)).unwrap_or(false))
}

/// The body rectangle of a control: 2 points shorter than its bounds, top-aligned.
pub fn body_rect(bounds: Rect) -> Rect {
    Rect::from_min_max(
        bounds.min,
        bounds.max - vec2(0.0, metric::CONTROL_BODY_INSET),
    )
}

/// The hover lift of a field control: `field` moved a third of the way to `controlEdge`.
pub fn hover_fill() -> Color32 {
    theme::FIELD.lerp_to_gamma(theme::CONTROL_EDGE, 0.35)
}

/// Paints a raised or recessed control body and returns the body rectangle.
pub fn control_body(painter: &Painter, bounds: Rect, depth: Depth, state: BodyState) -> Rect {
    let body = body_rect(bounds);
    let corner = CornerRadius::same(metric::CORNER_CONTROL);
    if state.disabled && state.lit.is_none() {
        painter.rect(
            body,
            corner,
            theme::PANEL,
            Stroke::new(1.0_f32, theme::DIVIDER),
            StrokeKind::Inside,
        );
        return body;
    }
    match depth {
        Depth::Raised => {
            under_shadow(painter, body);
            let fill = state.lit.unwrap_or(if state.hovered || state.pressed {
                hover_fill()
            } else {
                theme::FIELD
            });
            painter.rect_filled(body, corner, fill);
            if state.lit.is_none() && !state.pressed {
                painter.hline(
                    body.x_range().shrink(f32::from(metric::CORNER_CONTROL)),
                    body.top() + 0.5,
                    Stroke::new(1.0_f32, Color32::from_white_alpha(TOP_LIGHT)),
                );
            }
        }
        Depth::Recessed => {
            painter.rect_filled(body, corner, state.lit.unwrap_or(theme::FIELD));
            for step in 0..3 {
                let alpha = [128, 70, 26][step];
                painter.hline(
                    body.x_range(),
                    body.top() + step as f32 + 0.5,
                    Stroke::new(1.0_f32, Color32::from_black_alpha(alpha)),
                );
            }
            painter.hline(
                body.x_range().shrink(f32::from(metric::CORNER_CONTROL)),
                body.bottom() - 0.5,
                Stroke::new(1.0_f32, Color32::from_white_alpha(18)),
            );
        }
    }
    if state.focused {
        painter.rect_stroke(
            body,
            corner,
            Stroke::new(1.0_f32, theme::ACCENT),
            StrokeKind::Inside,
        );
    }
    body
}

/// The soft shadow under a raised body: a tight blurred drop shadow (black 35 %, 1 point down,
/// 3-point blur) that falls mostly in the 2-point strip under the body (foundations, control depth).
pub fn under_shadow(painter: &Painter, body: Rect) {
    painter.add(
        eframe::egui::epaint::Shadow {
            offset: [0, 1],
            blur: 3,
            spread: 0,
            color: Color32::from_black_alpha(RAISED_SHADOW),
        }
        .as_shape(body, CornerRadius::same(metric::CORNER_CONTROL)),
    );
}

/// Black alpha of the raised drop shadow (35 %).
const RAISED_SHADOW: u8 = 90;
/// White alpha of a raised body's 1-point top light (5 %).
const TOP_LIGHT: u8 = 12;

/// The one selected-row, header, strip or node cue: 1.5-point `accent` at 80 % inside `rect`.
pub fn selection_edge(painter: &Painter, rect: Rect) {
    painter.rect_stroke(
        rect,
        CornerRadius::same(metric::CORNER_CONTROL),
        Stroke::new(metric::SELECTION_EDGE, theme::ACCENT.gamma_multiply(0.8)),
        StrokeKind::Inside,
    );
}
