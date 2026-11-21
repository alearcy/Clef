//! Menus (T18 Phase 2): pop-up, context and menu-bar menus. A `field` surface with a 1-point
//! `divider` edge and no shadow; 26-point rows with a tick column, the live shortcut right-aligned
//! in `secondary`, a `peacock` highlight, destructive rows in `danger`. Rows are focusable, so ↑/↓
//! move between them, Return or Space activates and Escape closes (egui's focus and popup rules).
use crate::theme::{self, metric, text};
use eframe::egui::{
    self, Align2, CornerRadius, Popup, Response, Sense, Stroke, Ui, WidgetInfo, WidgetType, vec2,
};

/// The frame of every Clef menu.
pub fn frame() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::FIELD)
        .stroke(Stroke::new(1.0_f32, theme::DIVIDER))
        .corner_radius(metric::CORNER_CONTROL)
        .inner_margin(metric::MENU_PADDING)
}

/// A menu that opens under `button` on click.
pub fn under(button: &Response) -> Popup<'_> {
    Popup::menu(button).frame(frame())
}

/// A context menu that opens on a secondary click of `response`.
pub fn context(response: &Response) -> Popup<'_> {
    Popup::context_menu(response).frame(frame())
}

/// Show `popup` with menu spacing; returns the content's value while it is open.
pub fn show<R>(popup: Popup<'_>, content: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    popup
        .show(|ui| {
            begin(ui);
            content(ui)
        })
        .map(|inner| {
            name(&inner.response, "Menu");
            inner.inner
        })
}

/// Gives a menu's popup area the menu role, so screen readers announce it rather than an
/// unnamed group.
pub fn name(area: &Response, label: &str) {
    area.ctx.accesskit_node_builder(area.id, |node| {
        node.set_role(egui::accesskit::Role::Menu);
        node.set_label(label);
    });
}

/// Menu spacing for a popup's `Ui`: rows touch, at least 180 wide.
pub fn begin(ui: &mut Ui) {
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    ui.set_min_width(180.0);
}

/// The width every row of this menu shares: the widest row seen so far, so a menu is as wide
/// as its longest entry rather than the space its popup could take.
fn row_width(ui: &Ui, natural: f32) -> f32 {
    let id = ui.id().with("clef-menu-width");
    let widest = ui.data(|data| data.get_temp::<f32>(id)).unwrap_or(0.0);
    let width = natural.max(widest).max(ui.min_rect().width());
    if width > widest {
        ui.ctx().data_mut(|data| data.insert_temp(id, width));
        ui.ctx().request_discard("menu width grew");
    }
    width
}

/// One menu row. `show` returns true when it is picked and closes the menu.
#[must_use]
pub struct Item<'a> {
    label: &'a str,
    shortcut: &'a str,
    tick: Option<bool>,
    enabled: bool,
    destructive: bool,
    highlighted: bool,
}

impl<'a> Item<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            shortcut: "",
            tick: None,
            enabled: true,
            // A label starting with Delete, Remove or Clear is destructive by default.
            destructive: ["Delete", "Remove", "Clear"]
                .iter()
                .any(|word| label.starts_with(word)),
            highlighted: false,
        }
    }

    /// The live shortcut; omitted when empty (unbound).
    pub fn shortcut(mut self, shortcut: &'a str) -> Self {
        self.shortcut = shortcut;
        self
    }

    /// A checkable row: the tick shows when `on`.
    pub fn ticked(mut self, on: bool) -> Self {
        self.tick = Some(on);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn destructive(mut self, destructive: bool) -> Self {
        self.destructive = destructive;
        self
    }

    /// Paint the highlight regardless of pointer and focus (gallery specimens).
    pub fn highlighted(mut self, on: bool) -> Self {
        self.highlighted = on;
        self
    }

    pub fn show(self, ui: &mut Ui) -> bool {
        let response = self.response(ui);
        let picked = response.clicked();
        if picked {
            ui.close();
        }
        picked
    }

    pub fn response(&self, ui: &mut Ui) -> Response {
        let font = text::font(text::MENU);
        let shortcut_font = text::font(text::BODY);
        let label = ui
            .painter()
            .layout_no_wrap(self.label.to_owned(), font.clone(), theme::TEXT);
        let shortcut = ui.painter().layout_no_wrap(
            self.shortcut.to_owned(),
            shortcut_font.clone(),
            theme::SECONDARY,
        );
        let gap = if self.shortcut.is_empty() { 0.0 } else { 24.0 };
        let width = (metric::MENU_TICK
            + label.size().x
            + gap
            + shortcut.size().x
            + metric::CONTROL_PADDING * 2.0)
            .max(row_width(ui, 0.0));
        let width = row_width(ui, width);
        let response = ui.add_enabled(self.enabled, |ui: &mut Ui| {
            ui.allocate_response(vec2(width, metric::MENU_ROW), Sense::click())
        });
        let enabled = response.enabled();
        if enabled && super::menu_bar::take_first_focus(ui.ctx()) {
            response.request_focus();
        }
        let info_label = if self.shortcut.is_empty() {
            self.label.to_owned()
        } else {
            format!("{} ({})", self.label, self.shortcut)
        };
        response.widget_info(|| match self.tick {
            Some(on) => WidgetInfo::selected(WidgetType::Checkbox, enabled, on, &info_label),
            None => WidgetInfo::labeled(WidgetType::Button, enabled, &info_label),
        });
        let rect = response.rect;
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let highlighted =
                enabled && (self.highlighted || response.hovered() || response.has_focus());
            if highlighted {
                painter.rect_filled(
                    rect,
                    CornerRadius::same(metric::CORNER_CONTROL),
                    theme::PEACOCK,
                );
            }
            let ink = if !enabled {
                theme::DISABLED
            } else if highlighted {
                theme::TEXT
            } else if self.destructive {
                theme::DANGER
            } else {
                theme::TEXT
            };
            let left = rect.left() + metric::CONTROL_PADDING;
            if self.tick == Some(true) {
                let c = egui::pos2(left + metric::MENU_TICK / 2.0 - 2.0, rect.center().y);
                painter.line(
                    vec![
                        c + vec2(-4.0, 0.0),
                        c + vec2(-1.0, 3.5),
                        c + vec2(4.5, -4.0),
                    ],
                    Stroke::new(1.5_f32, ink),
                );
            }
            painter.text(
                egui::pos2(left + metric::MENU_TICK, rect.center().y),
                Align2::LEFT_CENTER,
                self.label,
                font,
                ink,
            );
            if !self.shortcut.is_empty() {
                painter.text(
                    egui::pos2(rect.right() - metric::CONTROL_PADDING, rect.center().y),
                    Align2::RIGHT_CENTER,
                    self.shortcut,
                    shortcut_font,
                    if highlighted || !enabled {
                        ink
                    } else {
                        theme::SECONDARY
                    },
                );
            }
        }
        response
    }
}

/// A 1-point `divider` line in a 9-point row.
pub fn separator(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(
        vec2(row_width(ui, 0.0), metric::MENU_SEPARATOR),
        Sense::hover(),
    );
    ui.painter().hline(
        rect.x_range().shrink(metric::CONTROL_PADDING / 2.0),
        rect.center().y,
        Stroke::new(1.0_f32, theme::DIVIDER),
    );
}

/// A section caption: 12-point `secondary` in a 22-point row (SF Pro is loaded in one weight).
pub fn caption(ui: &mut Ui, label: &str) {
    let width = ui
        .painter()
        .layout_no_wrap(label.to_owned(), text::font(12.0), theme::SECONDARY)
        .size()
        .x
        + metric::CONTROL_PADDING * 2.0;
    let (rect, _) = ui.allocate_exact_size(
        vec2(row_width(ui, width), metric::MENU_CAPTION),
        Sense::hover(),
    );
    ui.painter().text(
        egui::pos2(rect.left() + metric::CONTROL_PADDING, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        text::font(12.0),
        theme::SECONDARY,
    );
}
