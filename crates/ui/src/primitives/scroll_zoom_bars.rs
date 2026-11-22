//! Scroll and zoom bars: always-visible scroll bars on editing surfaces, carrying their zoom
//! buttons (horizontal −/+ at the right end, vertical −/+ stacked at the bottom).
use crate::{
    paint,
    theme::{self, metric},
};
use eframe::egui::{
    CornerRadius, Id, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, WidgetInfo,
    WidgetType, pos2, vec2,
};

/// One axis of a scrolled view, in the owner's content units (ticks, pixels, rows).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Axis {
    /// First visible unit.
    pub offset: f32,
    /// Units visible at the current zoom.
    pub visible: f32,
    /// Units the content spans.
    pub total: f32,
}

impl Axis {
    fn max_offset(self) -> f32 {
        (self.total - self.visible).max(0.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Zoom {
    In,
    Out,
}

/// What the composer did with a bar this frame.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Moved {
    pub offset: Option<f32>,
    pub zoom: Option<Zoom>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Horizontal,
    Vertical,
}

/// The horizontal bar across the available width, with −/+ at its right.
pub fn horizontal(ui: &mut Ui, name: &str, axis: Axis, enabled: bool) -> Moved {
    let size = vec2(ui.available_width(), metric::SCROLL_BAR);
    bar(ui, name, axis, enabled, size, Direction::Horizontal, None)
}

/// The vertical bar down the available height, with −/+ stacked at its bottom.
pub fn vertical(ui: &mut Ui, name: &str, axis: Axis, enabled: bool) -> Moved {
    let size = vec2(metric::SCROLL_BAR, ui.available_height());
    bar(ui, name, axis, enabled, size, Direction::Vertical, None)
}

/// The vertical bar down the available height, with its −/+ stacked further down so that they
/// end at `bottom` (T41 round 4): an owner with rows under the scrolled area (the piano roll's
/// controller lanes) keeps the −/+ at the corner beside the horizontal bar's −/+. The column
/// between the bar and the buttons stays empty and belongs to the owner.
pub fn vertical_to(ui: &mut Ui, name: &str, axis: Axis, enabled: bool, bottom: f32) -> Moved {
    let size = vec2(metric::SCROLL_BAR, ui.available_height());
    bar(
        ui,
        name,
        axis,
        enabled,
        size,
        Direction::Vertical,
        Some(bottom),
    )
}

fn bar(
    ui: &mut Ui,
    name: &str,
    axis: Axis,
    enabled: bool,
    size: Vec2,
    direction: Direction,
    buttons_bottom: Option<f32>,
) -> Moved {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let button = metric::SCROLL_BAR;
    let (track, minus, plus) = match (direction, buttons_bottom) {
        (Direction::Vertical, Some(bottom)) => (
            rect,
            Rect::from_min_size(
                pos2(rect.left(), bottom - 2.0 * button),
                Vec2::splat(button),
            ),
            Rect::from_min_size(pos2(rect.left(), bottom - button), Vec2::splat(button)),
        ),
        (Direction::Horizontal, _) => (
            Rect::from_min_max(rect.min, pos2(rect.right() - 2.0 * button, rect.bottom())),
            Rect::from_min_size(
                pos2(rect.right() - 2.0 * button, rect.top()),
                Vec2::splat(button),
            ),
            Rect::from_min_size(pos2(rect.right() - button, rect.top()), Vec2::splat(button)),
        ),
        (Direction::Vertical, None) => (
            Rect::from_min_max(rect.min, pos2(rect.right(), rect.bottom() - 2.0 * button)),
            Rect::from_min_size(
                pos2(rect.left(), rect.bottom() - 2.0 * button),
                Vec2::splat(button),
            ),
            Rect::from_min_size(
                pos2(rect.left(), rect.bottom() - button),
                Vec2::splat(button),
            ),
        ),
    };
    let mut moved = Moved::default();
    let scrollable = enabled && axis.max_offset() > 0.0;
    let id = ui.make_persistent_id((name, "scroll"));
    let response = ui.interact(
        track,
        id,
        if scrollable {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        },
    );
    response.widget_info(|| {
        let mut info = WidgetInfo::slider(scrollable, f64::from(axis.offset), name);
        info.typ = WidgetType::Slider;
        info
    });
    let length = |rect: Rect| match direction {
        Direction::Horizontal => rect.width(),
        Direction::Vertical => rect.height(),
    };
    let along = |pos: Pos2| match direction {
        Direction::Horizontal => pos.x - track.left(),
        Direction::Vertical => pos.y - track.top(),
    };
    let span = length(track);
    let fraction = if axis.total > 0.0 {
        (axis.visible / axis.total).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let thumb_length = (span * fraction).max(button * 2.0).min(span);
    let travel = (span - thumb_length).max(1.0);
    let start = if axis.max_offset() > 0.0 {
        travel * (axis.offset / axis.max_offset()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    if (response.dragged() || response.clicked())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        {
            let grab = ui.ctx().data_mut(|data| {
                if response.drag_started() || response.clicked() {
                    let at = along(pointer);
                    let inside = at >= start && at <= start + thumb_length;
                    let grab = if inside {
                        at - start
                    } else {
                        thumb_length / 2.0
                    };
                    data.insert_temp(id, grab);
                }
                data.get_temp::<f32>(id).unwrap_or(thumb_length / 2.0)
            });
            let position = ((along(pointer) - grab) / travel).clamp(0.0, 1.0);
            moved.offset = Some(position * axis.max_offset());
        }
    }
    let thumb = match direction {
        Direction::Horizontal => Rect::from_min_size(
            pos2(track.left() + start, track.top()),
            vec2(thumb_length, track.height()),
        ),
        Direction::Vertical => Rect::from_min_size(
            pos2(track.left(), track.top() + start),
            vec2(track.width(), thumb_length),
        ),
    };
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let corner = CornerRadius::same(metric::CORNER_CONTROL);
        paint::control_body(
            painter,
            track,
            paint::Depth::Recessed,
            paint::BodyState {
                disabled: !enabled,
                ..paint::BodyState::default()
            },
        );
        if scrollable {
            let colour = if response.dragged() || response.hovered() {
                theme::SECONDARY
            } else {
                theme::SECONDARY.gamma_multiply(0.55)
            };
            painter.rect_filled(thumb.shrink(2.0), corner, colour);
        }
    }
    let zoom_name = match direction {
        Direction::Horizontal => "horizontally",
        Direction::Vertical => "vertically",
    };
    if zoom_button(
        ui,
        id,
        minus,
        false,
        &format!("Zoom out {zoom_name}"),
        enabled,
    )
    .clicked()
    {
        moved.zoom = Some(Zoom::Out);
    }
    if zoom_button(ui, id, plus, true, &format!("Zoom in {zoom_name}"), enabled).clicked() {
        moved.zoom = Some(Zoom::In);
    }
    moved
}

fn zoom_button(
    ui: &mut Ui,
    bar: Id,
    rect: Rect,
    plus: bool,
    name: &str,
    enabled: bool,
) -> Response {
    let id = bar.with(plus);
    let response = ui.interact(
        rect,
        id,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, name));
    let colour = if !enabled {
        theme::DISABLED
    } else if response.is_pointer_button_down_on() {
        theme::SECONDARY
    } else if response.hovered() {
        theme::CAP_LIGHT
    } else {
        theme::TEXT
    };
    let painter = ui.painter();
    let centre = rect.center();
    let arm = rect.width() * 0.3;
    let stroke = Stroke::new(metric::TOOL_STROKE, colour);
    painter.line_segment([centre - vec2(arm, 0.0), centre + vec2(arm, 0.0)], stroke);
    if plus {
        painter.line_segment([centre - vec2(0.0, arm), centre + vec2(0.0, arm)], stroke);
    }
    if response.has_focus() && paint::keyboard(ui.ctx()) {
        painter.rect_stroke(
            rect,
            2,
            Stroke::new(1.0_f32, theme::ACCENT),
            StrokeKind::Inside,
        );
    }
    response.on_hover_text(name)
}

/// A wheel or trackpad gesture over an editing surface, read from this frame's input.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Wheel {
    /// Horizontal zoom factor at the pointer (⌘-wheel or pinch), continuous with the delta.
    pub zoom_horizontal: Option<f32>,
    /// Whole vertical zoom steps of ×1.25 (⌘⇧-wheel); positive zooms in.
    pub zoom_vertical: i32,
    /// Scroll delta in points; egui already turns Shift-wheel into horizontal scroll. Positive
    /// values move the content right or down, as egui reports them.
    pub scroll: Vec2,
    /// The pointer at the time of the gesture: the zoom anchor.
    pub pointer: Option<Pos2>,
}

/// Reads the wheel over `rect` and consumes its scroll, so no outer scroll area also moves.
pub fn wheel(ui: &Ui, rect: Rect) -> Wheel {
    let pointer = ui.input(|input| input.pointer.hover_pos());
    if !pointer.is_some_and(|pointer| rect.contains(pointer)) {
        return Wheel::default();
    }
    let accumulator = ui.make_persistent_id("vertical-zoom-accumulator");
    let mut wheel = Wheel {
        pointer,
        ..Wheel::default()
    };
    let (zoom, vertical) = ui.input_mut(|input| {
        let zoom = input.zoom_delta();
        wheel.scroll = input.smooth_scroll_delta;
        input.smooth_scroll_delta = Vec2::ZERO;
        (zoom, input.modifiers.command && input.modifiers.shift)
    });
    if (zoom - 1.0).abs() > f32::EPSILON {
        if vertical {
            let steps = ui.ctx().data_mut(|data| {
                let total = data.get_temp_mut_or(accumulator, 0.0_f32);
                *total += zoom.ln() / 1.25_f32.ln();
                let steps = total.trunc();
                *total -= steps;
                steps
            });
            wheel.zoom_vertical = steps as i32;
        } else {
            wheel.zoom_horizontal = Some(zoom);
        }
    }
    wheel
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;

    #[test]
    fn plus_button_zooms_in() {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let axis = Axis {
            offset: 0.0,
            visible: 100.0,
            total: 400.0,
        };
        let mut result = Moved::default();
        let plus = pos2(400.0 - metric::SCROLL_BAR / 2.0, metric::SCROLL_BAR / 2.0);
        for events in [
            vec![],
            vec![egui::Event::PointerMoved(plus)],
            vec![egui::Event::PointerButton {
                pos: plus,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
            vec![egui::Event::PointerButton {
                pos: plus,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        ] {
            let _ = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    let moved = horizontal(ui, "Time", axis, true);
                    if moved.zoom.is_some() {
                        result = moved;
                    }
                },
            );
        }
        assert_eq!(result.zoom, Some(Zoom::In));
    }

    #[test]
    fn thumb_fills_the_track_when_everything_is_visible() {
        let axis = Axis {
            offset: 0.0,
            visible: 400.0,
            total: 300.0,
        };
        assert_eq!(axis.max_offset(), 0.0);
    }
}
