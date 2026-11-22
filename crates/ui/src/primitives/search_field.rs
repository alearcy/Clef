//! Search field (T18 Phase 3): a text field for filtering a list as you type. The `search` glyph
//! sits in the left indent (`secondary`, `accent` while it filters), the hint in `secondary`, the
//! live binding right-aligned in `disabled` while empty, and a `close` glyph that clears the text
//! while it holds any. Escape clears the text and gives focus back.
use super::{
    icons::Icon,
    text_field::{SEARCH_GLYPH, TextField},
};
use crate::theme::{self, metric, text};
use eframe::egui::{self, Align2, Key, Response, Sense, Ui, pos2};

#[must_use]
pub struct SearchField<'a> {
    value: &'a mut String,
    name: &'a str,
    hint: &'a str,
    shortcut: &'a str,
    width: Option<f32>,
    id: Option<egui::Id>,
}

impl<'a> SearchField<'a> {
    /// `name` is the accessible label and the hint ("Search commands").
    pub fn new(value: &'a mut String, name: &'a str) -> Self {
        Self {
            value,
            name,
            hint: name,
            shortcut: "",
            width: None,
            id: None,
        }
    }

    /// The live binding that focuses the field, shown while it is empty.
    pub fn shortcut(mut self, shortcut: &'a str) -> Self {
        self.shortcut = shortcut;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn id(mut self, id: egui::Id) -> Self {
        self.id = Some(id);
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let filtering = !self.value.trim().is_empty();
        let mut field = TextField::new(&mut *self.value, self.name)
            .hint(self.hint)
            .search();
        if let Some(width) = self.width {
            field = field.width(width);
        }
        if let Some(id) = self.id {
            field = field.id(id);
        }
        let response = field.show(ui);
        // The editor sits inside the body's padding and the glyph indents.
        let body = response.rect.expand2(egui::vec2(
            metric::CONTROL_PADDING + SEARCH_GLYPH + 4.0,
            0.0,
        ));
        let painter = ui.painter().clone();
        let glyph = pos2(
            body.left() + metric::CONTROL_PADDING + SEARCH_GLYPH / 2.0,
            body.center().y,
        );
        Icon::Search.paint(
            &painter,
            glyph,
            SEARCH_GLYPH,
            if filtering {
                theme::ACCENT
            } else {
                theme::SECONDARY
            },
        );
        let right = pos2(
            body.right() - metric::CONTROL_PADDING - SEARCH_GLYPH / 2.0,
            body.center().y,
        );
        let mut cleared = false;
        if filtering {
            let clear = ui
                .interact(
                    egui::Rect::from_center_size(right, egui::Vec2::splat(SEARCH_GLYPH + 4.0)),
                    response.id.with("clear"),
                    Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Clear search");
            clear.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Clear search")
            });
            Icon::Close.paint(
                &painter,
                right,
                9.0,
                if clear.hovered() {
                    theme::TEXT
                } else {
                    theme::SECONDARY
                },
            );
            if clear.clicked() {
                cleared = true;
                response.request_focus();
            }
        } else if !self.shortcut.is_empty() {
            painter.text(
                pos2(body.right() - metric::CONTROL_PADDING, body.center().y),
                Align2::RIGHT_CENTER,
                self.shortcut,
                text::font(text::CAPTION),
                theme::DISABLED,
            );
        }
        // The editor gives up focus on Escape itself, so a field that just lost it counts too.
        if (response.has_focus() || response.lost_focus())
            && ui.input(|input| input.key_pressed(Key::Escape))
        {
            cleared = true;
            response.surrender_focus();
        }
        if cleared {
            self.value.clear();
        }
        response
    }
}
