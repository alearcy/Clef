//! List row: one entry in a vertical list, with an optional second-line detail.
use crate::{
    paint,
    theme::{self, metric, text},
};
use eframe::egui::{
    self, CornerRadius, Response, Sense, Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, vec2,
};

#[must_use]
pub struct ListRow<'a> {
    name: &'a str,
    detail: Option<&'a str>,
    trailing: Option<&'a str>,
    accessible: Option<&'a str>,
    height: f32,
    width: Option<f32>,
    selected: bool,
    enabled: bool,
    available: bool,
    header: bool,
}

impl<'a> ListRow<'a> {
    pub fn new(name: &'a str) -> Self {
        Self {
            name,
            detail: None,
            trailing: None,
            accessible: None,
            height: 22.0,
            width: None,
            selected: false,
            enabled: true,
            available: true,
            header: false,
        }
    }

    /// A second line in `secondary` (path, kind, status); the row should be 44 high.
    pub fn detail(mut self, detail: &'a str) -> Self {
        self.detail = Some(detail);
        self
    }

    /// Right-aligned `secondary` text on the first line: a command's live shortcut.
    pub fn trailing(mut self, trailing: &'a str) -> Self {
        self.trailing = (!trailing.is_empty()).then_some(trailing);
        self
    }

    /// Full accessible name and tooltip when the row text is truncated or partial.
    pub fn accessible(mut self, name: &'a str) -> Self {
        self.accessible = Some(name);
        self
    }

    /// 22 (browser, picker), 26 (sidebar) or 44 (two-line rows).
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// An unavailable entry (missing file) draws in `disabled` and stays selectable.
    pub fn available(mut self, available: bool) -> Self {
        self.available = available;
        self
    }

    /// A header row (track header): selection keeps `panel` and the row paints no edge; the
    /// header that owns the row draws the selection edge around its whole bounds (T41).
    pub fn header(mut self) -> Self {
        self.header = true;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let width = self.width.unwrap_or_else(|| ui.available_width());
        let response = ui.add_enabled(self.enabled, |ui: &mut Ui| {
            ui.allocate_response(vec2(width, self.height), Sense::click())
        });
        let enabled = response.enabled();
        let full = self.accessible.map_or_else(
            || match self.detail {
                Some(detail) => format!("{}, {detail}", self.name),
                None => self.name.to_owned(),
            },
            str::to_owned,
        );
        response.widget_info(|| {
            WidgetInfo::selected(WidgetType::SelectableLabel, enabled, self.selected, &full)
        });
        let rect = response.rect;
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let corner = CornerRadius::same(metric::CORNER_CONTROL);
            let fill = if self.selected && !self.header || response.hovered() && enabled {
                theme::FIELD
            } else {
                theme::PANEL
            };
            painter.rect_filled(rect, corner, fill);
            if self.selected && !self.header {
                paint::selection_edge(painter, rect);
            }
            if response.has_focus() && paint::keyboard(ui.ctx()) {
                painter.rect_stroke(
                    rect.shrink(3.0),
                    corner,
                    Stroke::new(1.0_f32, theme::ACCENT),
                    StrokeKind::Inside,
                );
            }
            let usable = enabled && self.available;
            let lines: &[(&str, f32, egui::Color32)] = match self.detail {
                Some(detail) => &[
                    (self.name, text::BODY, theme::TEXT),
                    (detail, text::SMALL, theme::SECONDARY),
                ],
                None => &[(self.name, text::BODY, theme::TEXT)],
            };
            let mut name_width = width - metric::CONTROL_PADDING * 2.0;
            if let Some(trailing) = self.trailing {
                let galley = painter.layout_no_wrap(
                    trailing.to_owned(),
                    text::font(text::BODY),
                    if usable {
                        theme::SECONDARY
                    } else {
                        theme::DISABLED
                    },
                );
                name_width -= galley.size().x + metric::CONTROL_PADDING;
                let y = match self.detail {
                    Some(_) => rect.center().y - (text::BODY + text::SMALL + 8.0) / 2.0,
                    None => rect.center().y - galley.size().y / 2.0,
                };
                painter.galley(
                    egui::pos2(rect.right() - metric::CONTROL_PADDING - galley.size().x, y),
                    galley,
                    theme::SECONDARY,
                );
            }
            let total: f32 = lines.iter().map(|(_, size, _)| size + 4.0).sum();
            let mut y = rect.center().y - total / 2.0;
            for &(line, size, colour) in lines {
                let colour = if usable { colour } else { theme::DISABLED };
                let mut job = egui::text::LayoutJob::simple_singleline(
                    line.to_owned(),
                    text::font(size),
                    colour,
                );
                job.wrap.max_width = name_width.max(1.0);
                job.wrap.max_rows = 1;
                let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
                painter.galley(
                    egui::pos2(rect.left() + metric::CONTROL_PADDING, y),
                    galley,
                    colour,
                );
                y += size + 4.0;
            }
        }
        response.on_hover_text(full)
    }
}
