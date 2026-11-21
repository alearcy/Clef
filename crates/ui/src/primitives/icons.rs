//! Icons (T18 Phase 3): the one vector glyph set every glyph-only control draws. Glyphs carry no
//! tile, border or label; the host gives the colour (foundations, lamps and state colour).
use crate::theme;
use eframe::egui::{self, Color32, Painter, Pos2, Shape, Stroke, vec2};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Start,
    Play,
    Stop,
    Record,
    Loop,
    Metronome,
    /// Fit the content to the visible width: arrows out to two bounds.
    FitWidth,
    /// Fit the content to the visible height.
    FitHeight,
    /// A magnifier: the search field's leading glyph.
    Search,
    /// A cross: clear a field, close a panel.
    Close,
    /// A tick: the checkbox mark.
    Check,
    /// A bracket over a 3 (D102).
    Triplet,
    /// A note head with its dot (D102).
    Dotted,
}

impl Icon {
    /// Every glyph, in gallery order.
    pub const ALL: [Self; 13] = [
        Self::Start,
        Self::Play,
        Self::Stop,
        Self::Record,
        Self::Loop,
        Self::Metronome,
        Self::FitWidth,
        Self::FitHeight,
        Self::Search,
        Self::Close,
        Self::Check,
        Self::Triplet,
        Self::Dotted,
    ];
}

impl Icon {
    /// Strokes the glyph centred on `center` in a `size`-point square.
    pub fn paint(self, painter: &Painter, center: Pos2, size: f32, color: Color32) {
        let point = |x, y| center + vec2(x, y) * (size / 2.0);
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
                painter.circle_filled(center, size * 0.4, color);
            }
            Self::Loop => {
                line(&[(-0.84, -0.02), (-0.84, -0.5), (0.82, -0.5)]);
                line(&[(0.43, -0.89), (0.82, -0.5), (0.43, -0.11)]);
                line(&[(0.84, 0.02), (0.84, 0.5), (-0.82, 0.5)]);
                line(&[(-0.43, 0.89), (-0.82, 0.5), (-0.43, 0.11)]);
            }
            Self::FitWidth => {
                line(&[(-0.85, -0.7), (-0.85, 0.7)]);
                line(&[(0.85, -0.7), (0.85, 0.7)]);
                line(&[(-0.55, 0.0), (0.55, 0.0)]);
                line(&[(-0.3, -0.25), (-0.55, 0.0), (-0.3, 0.25)]);
                line(&[(0.3, -0.25), (0.55, 0.0), (0.3, 0.25)]);
            }
            Self::FitHeight => {
                line(&[(-0.7, -0.85), (0.7, -0.85)]);
                line(&[(-0.7, 0.85), (0.7, 0.85)]);
                line(&[(0.0, -0.55), (0.0, 0.55)]);
                line(&[(-0.25, -0.3), (0.0, -0.55), (0.25, -0.3)]);
                line(&[(-0.25, 0.3), (0.0, 0.55), (0.25, 0.3)]);
            }
            Self::Metronome => {
                polygon(
                    &[(-0.72, 0.78), (-0.25, -0.72), (0.25, -0.72), (0.72, 0.78)],
                    Color32::TRANSPARENT,
                    stroke,
                );
                line(&[(-0.04, 0.43), (0.74, -0.49)]);
            }
            Self::Search => {
                painter.circle_stroke(point(-0.18, -0.18), size * 0.3, stroke);
                line(&[(0.27, 0.27), (0.8, 0.8)]);
            }
            Self::Close => {
                line(&[(-0.6, -0.6), (0.6, 0.6)]);
                line(&[(0.6, -0.6), (-0.6, 0.6)]);
            }
            Self::Check => {
                line(&[(-0.7, 0.0), (-0.2, 0.55), (0.75, -0.6)]);
            }
            Self::Triplet => {
                line(&[(-0.85, -0.2), (-0.85, -0.6), (0.85, -0.6), (0.85, -0.2)]);
                painter.text(
                    point(0.0, 0.3),
                    egui::Align2::CENTER_CENTER,
                    "3",
                    crate::theme::text::font(size * 0.75),
                    color,
                );
            }
            Self::Dotted => {
                painter.circle_filled(point(-0.3, 0.45), size * 0.2, color);
                line(&[(-0.1, 0.45), (-0.1, -0.85)]);
                painter.circle_filled(point(0.55, 0.45), size * 0.09, color);
            }
        }
    }
}
