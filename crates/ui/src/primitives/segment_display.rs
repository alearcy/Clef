//! Segment display (T18 Phase 3): the transport counter read as one hardware counter. Seven-segment
//! digits in `text` (`danger` while recording) over faint unlit "8" ghosts in a recessed well;
//! narrow separators for `.` and `:`. The accessible value is the plain text.
use crate::{
    paint::{self, BodyState, Depth},
    theme::{self, metric},
};
use eframe::egui::{
    Color32, Painter, Pos2, Response, Sense, Stroke, Ui, WidgetInfo, WidgetType, pos2, vec2,
};

const DIGIT: [f32; 2] = [9.0, 16.0];
const ADVANCE: f32 = 12.0;
const SEPARATOR: f32 = 6.0;
const STROKE: f32 = 1.8;
const INSET: f32 = 7.0;

/// Segments a, b, c, d, e, f, g lit for each digit.
const SEGMENTS: [u8; 10] = [
    0b011_1111, 0b000_0110, 0b101_1011, 0b100_1111, 0b110_0110, 0b110_1101, 0b111_1101, 0b000_0111,
    0b111_1111, 0b110_1111,
];

fn width(text: &str) -> f32 {
    text.chars()
        .map(|c| {
            if c.is_ascii_digit() || c == '-' {
                ADVANCE
            } else {
                SEPARATOR
            }
        })
        .sum::<f32>()
        + 2.0 * INSET
}

fn digit(painter: &Painter, origin: Pos2, mask: u8, colour: Color32) {
    digit_sized(painter, origin, DIGIT, STROKE, mask, colour);
}

fn digit_sized(
    painter: &Painter,
    origin: Pos2,
    [w, h]: [f32; 2],
    stroke: f32,
    mask: u8,
    colour: Color32,
) {
    let half = h / 2.0;
    let p = |x: f32, y: f32| origin + vec2(x, y);
    let lines = [
        (p(1.0, 0.0), p(w - 1.0, 0.0)),
        (p(w, 1.0), p(w, half - 1.0)),
        (p(w, half + 1.0), p(w, h - 1.0)),
        (p(1.0, h), p(w - 1.0, h)),
        (p(0.0, half + 1.0), p(0.0, h - 1.0)),
        (p(0.0, 1.0), p(0.0, half - 1.0)),
        (p(1.0, half), p(w - 1.0, half)),
    ];
    for (index, (a, b)) in lines.into_iter().enumerate() {
        if mask & (1 << index) != 0 {
            painter.line_segment([a, b], Stroke::new(stroke, colour));
        }
    }
}

/// Draws `text` (digits, `-`, `.` and `:`) right-aligned in a recessed well of at least
/// `min_width`. `name` is the accessible name ("Position").
pub fn segment_display(
    ui: &mut Ui,
    name: &str,
    text: &str,
    recording: bool,
    min_width: f32,
) -> Response {
    let size = vec2(width(text).max(min_width), metric::CONTROL_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| {
        let mut info = WidgetInfo::labeled(WidgetType::Label, true, name);
        info.current_text_value = Some(text.to_owned());
        info
    });
    // egui reads a label's text as its value only; the name says what the number is.
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_label(name));
    let body = paint::control_body(ui.painter(), rect, Depth::Recessed, BodyState::default());
    let painter = ui.painter_at(body);
    let ink = if recording {
        theme::DANGER
    } else {
        theme::TEXT
    };
    let ghost = ink.gamma_multiply(0.08);
    let mut x = body.right() - INSET;
    let top = body.center().y - DIGIT[1] / 2.0;
    for character in text.chars().rev() {
        if let Some(value) = character.to_digit(10) {
            x -= ADVANCE;
            let origin = pos2(x + (ADVANCE - DIGIT[0]) / 2.0, top);
            digit(&painter, origin, SEGMENTS[8], ghost);
            digit(&painter, origin, SEGMENTS[value as usize], ink);
        } else if character == '-' {
            x -= ADVANCE;
            let origin = pos2(x + (ADVANCE - DIGIT[0]) / 2.0, top);
            digit(&painter, origin, SEGMENTS[8], ghost);
            digit(&painter, origin, 0b100_0000, ink);
        } else {
            x -= SEPARATOR;
            let centre = x + SEPARATOR / 2.0;
            let dots: &[f32] = if character == ':' {
                &[0.3, 0.75]
            } else {
                &[1.0]
            };
            for y in dots {
                painter.circle_filled(pos2(centre, top + DIGIT[1] * y), 1.2, ink);
            }
        }
    }
    response
}

/// Small digits of the two-row readout (T41 S12).
const SMALL_DIGIT: [f32; 2] = [5.5, 8.0];
const SMALL_ADVANCE: f32 = 8.0;
const SMALL_SEPARATOR: f32 = 3.5;
const SMALL_STROKE: f32 = 1.4;
const UNIT_WIDTH: f32 = 22.0;

/// T41 S12: two rows of small segment digits in one recessed well as tall as a tool target
/// (28 points, the transport row's height), each
/// row right-aligned before its unit caption (`48.0 kHz` over `24 bit`). `name` is the
/// accessible name; the value read out is both rows with their units.
pub fn two_row_readout(ui: &mut Ui, name: &str, rows: [(&str, &str); 2]) -> Response {
    let digits = rows
        .iter()
        .map(|(text, _)| {
            text.chars()
                .map(|c| {
                    if c.is_ascii_digit() || c == '-' {
                        SMALL_ADVANCE
                    } else {
                        SMALL_SEPARATOR
                    }
                })
                .sum::<f32>()
        })
        .fold(0.0_f32, f32::max);
    let size = vec2(digits + UNIT_WIDTH + 2.0 * 6.0, metric::TOOL_TARGET);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let value = format!("{} {}, {} {}", rows[0].0, rows[0].1, rows[1].0, rows[1].1);
    response.widget_info(|| {
        let mut info = WidgetInfo::labeled(WidgetType::Label, true, name);
        info.current_text_value = Some(value.clone());
        info
    });
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_label(name));
    let body = paint::control_body(ui.painter(), rect, Depth::Recessed, BodyState::default());
    let painter = ui.painter_at(body);
    let ink = theme::TEXT;
    // Unlit ghosts would crowd digits this small: only lit segments are drawn.
    let gap = (body.height() - 2.0 * SMALL_DIGIT[1]) / 3.0;
    for (index, (text, unit)) in rows.iter().enumerate() {
        let top = body.top() + gap + index as f32 * (SMALL_DIGIT[1] + gap);
        let unit_left = body.right() - 6.0 - UNIT_WIDTH;
        painter.text(
            pos2(unit_left + 3.0, top + SMALL_DIGIT[1] / 2.0),
            eframe::egui::Align2::LEFT_CENTER,
            *unit,
            theme::text::font(theme::text::TINY),
            theme::SECONDARY,
        );
        let mut x = unit_left;
        for character in text.chars().rev() {
            if let Some(value) = character.to_digit(10) {
                x -= SMALL_ADVANCE;
                let origin = pos2(x + (SMALL_ADVANCE - SMALL_DIGIT[0]) / 2.0, top);
                digit_sized(
                    &painter,
                    origin,
                    SMALL_DIGIT,
                    SMALL_STROKE,
                    SEGMENTS[value as usize],
                    ink,
                );
            } else if character == '-' {
                x -= SMALL_ADVANCE;
                let origin = pos2(x + (SMALL_ADVANCE - SMALL_DIGIT[0]) / 2.0, top);
                digit_sized(&painter, origin, SMALL_DIGIT, SMALL_STROKE, 0b100_0000, ink);
            } else {
                x -= SMALL_SEPARATOR;
                painter.circle_filled(
                    pos2(x + SMALL_SEPARATOR / 2.0, top + SMALL_DIGIT[1]),
                    0.9,
                    ink,
                );
            }
        }
    }
    response
}
