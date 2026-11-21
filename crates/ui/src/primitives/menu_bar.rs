//! Menu bar (T18 Phase 2): the 28-point in-window row of menus at the top of the project window.
//! Click or Return opens a menu; while one is open, hovering another title or ←/→ moves to it;
//! F10 focuses the first title from anywhere; Escape closes and returns focus to the title.
use super::menu;
use crate::{
    paint,
    theme::{self, metric, text},
};
use eframe::egui::{
    self, Align2, CornerRadius, Id, Key, Modifiers, Popup, PopupCloseBehavior, PopupKind,
    RectAlign, Sense, Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, vec2,
};

const FOCUS_FIRST: &str = "clef-menu-focus-first";

#[derive(Clone, Copy, Default)]
struct State {
    open: Option<usize>,
}

/// Ask the next enabled menu row to take keyboard focus (a menu opened from the keyboard).
fn focus_first_row(ctx: &egui::Context) {
    ctx.data_mut(|data| data.insert_temp(Id::new(FOCUS_FIRST), true));
}

/// Called by a menu row: true once, for the first enabled row after a keyboard open.
pub(super) fn take_first_focus(ctx: &egui::Context) -> bool {
    ctx.data_mut(|data| data.remove_temp::<bool>(Id::new(FOCUS_FIRST)))
        .unwrap_or(false)
}

/// The project window's bar. Other bars (the gallery specimen) pass their own id, so two bars on
/// screen never share widget ids.
pub fn main_id() -> Id {
    Id::new("clef-menu-bar")
}

/// Open menu `index` of the bar `id` (the gallery; the app opens menus from input).
pub fn open(ctx: &egui::Context, id: Id, index: usize) {
    ctx.data_mut(|data| data.insert_temp(id, State { open: Some(index) }));
}

/// Draw the bar. `menu(index, ui)` draws the menu under `titles[index]` and returns a value
/// when a row is picked, which closes the menu; `trailing` draws the right-aligned items.
pub fn menu_bar<R>(
    ui: &mut Ui,
    bar: Id,
    titles: &[&str],
    menu: impl FnOnce(usize, &mut Ui) -> Option<R>,
    trailing: impl FnOnce(&mut Ui) -> Option<R>,
) -> Option<R> {
    let ctx = ui.ctx().clone();
    let mut state = ctx.data(|data| data.get_temp::<State>(bar).unwrap_or_default());
    let mut result = None;
    let mut rects = Vec::with_capacity(titles.len());
    if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::F10)) {
        ctx.memory_mut(|memory| memory.request_focus(bar.with(0)));
        paint::set_keyboard(&ctx, true);
    }
    let row = ui.horizontal(|ui| {
        ui.set_height(metric::MENU_BAR_HEIGHT);
        ui.spacing_mut().item_spacing.x = 0.0;
        let font = text::font(text::BODY);
        for (index, title) in titles.iter().enumerate() {
            let width = ui
                .painter()
                .layout_no_wrap((*title).to_owned(), font.clone(), theme::TEXT)
                .size()
                .x
                + metric::CONTROL_PADDING * 2.0;
            let (rect, _) =
                ui.allocate_exact_size(vec2(width, metric::MENU_BAR_HEIGHT), Sense::hover());
            let response = ui.interact(rect, bar.with(index), Sense::click());
            response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, *title));
            let open = state.open == Some(index);
            if response.clicked() {
                state.open = (!open).then_some(index);
                if state.open.is_some() && !response.is_pointer_button_down_on() {
                    focus_first_row(&ctx);
                }
            } else if state.open.is_some() && !open && response.hovered() {
                state.open = Some(index);
            } else if response.has_focus()
                && ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::ArrowDown))
            {
                state.open = Some(index);
                focus_first_row(&ctx);
            }
            let open = state.open == Some(index);
            if ui.is_rect_visible(rect) {
                let body = rect.shrink2(vec2(0.0, 3.0));
                let corner = CornerRadius::same(metric::CORNER_CONTROL);
                if open {
                    ui.painter().rect_filled(body, corner, theme::PEACOCK);
                } else if response.hovered() {
                    ui.painter().rect_filled(body, corner, theme::FIELD);
                }
                if response.has_focus() && paint::keyboard(&ctx) {
                    ui.painter().rect_stroke(
                        body,
                        corner,
                        Stroke::new(1.0_f32, theme::ACCENT),
                        StrokeKind::Inside,
                    );
                }
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    *title,
                    font.clone(),
                    theme::TEXT,
                );
            }
            rects.push(rect);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            trailing(ui)
        })
        .inner
    });
    result = row.inner.or(result);
    if let Some(index) = state.open {
        let step = ctx.input_mut(|input| {
            if input.consume_key(Modifiers::NONE, Key::ArrowRight) {
                1
            } else if input.consume_key(Modifiers::NONE, Key::ArrowLeft) {
                -1
            } else {
                0
            }
        });
        let index = if step == 0 {
            index
        } else {
            focus_first_row(&ctx);
            (index as isize + step).rem_euclid(titles.len() as isize) as usize
        };
        state.open = Some(index);
        let shown = Popup::new(
            bar.with(index).with("menu"),
            ctx.clone(),
            rects[index],
            ui.layer_id(),
        )
        .kind(PopupKind::Menu)
        .align(RectAlign::BOTTOM_START)
        .gap(0.0)
        .open(true)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .frame(menu::frame())
        .show(|ui| {
            menu::begin(ui);
            menu(index, ui)
        });
        if let Some(shown) = shown {
            menu::name(&shown.response, titles[index]);
            let escape = ctx.input(|input| input.key_pressed(Key::Escape));
            if let Some(picked) = shown.inner {
                result = Some(picked);
                state.open = None;
            } else if shown.response.should_close() {
                state.open = None;
                if escape {
                    ctx.memory_mut(|memory| memory.request_focus(bar.with(index)));
                }
            }
        }
    }
    if state.open.is_none() {
        ctx.data_mut(|data| data.remove_temp::<bool>(Id::new(FOCUS_FIRST)));
    }
    ctx.data_mut(|data| data.insert_temp(bar, state));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn keyboard_opens_moves_between_menus_picks_and_escapes() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let frame = |events: Vec<egui::Event>| {
            let mut picked = None;
            let mut shown = None;
            let _ = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    picked = menu_bar(
                        ui,
                        main_id(),
                        &["File", "Edit"],
                        |index, ui| {
                            shown = Some(index);
                            [("New", 0), ("Open", 1)]
                                .into_iter()
                                .find(|(label, _)| menu::Item::new(label).show(ui))
                                .map(|(_, row)| (index, row))
                        },
                        |_| None,
                    );
                },
            );
            (picked, shown)
        };
        frame(vec![]);
        frame(vec![key(Key::F10)]);
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(Id::new("clef-menu-bar").with(0))
        );
        // ↓ opens File with its first row focused; → moves to Edit.
        assert_eq!(frame(vec![key(Key::ArrowDown)]).1, Some(0));
        frame(vec![]);
        assert_eq!(frame(vec![key(Key::ArrowRight)]).1, Some(1));
        frame(vec![]);
        // Return activates the focused first row of Edit.
        assert_eq!(frame(vec![key(Key::Enter)]).0, Some((1, 0)));
        assert_eq!(frame(vec![]).1, None);
        // Escape closes and gives focus back to the title.
        frame(vec![key(Key::F10)]);
        frame(vec![key(Key::ArrowDown)]);
        frame(vec![key(Key::Escape)]);
        assert_eq!(frame(vec![]).1, None);
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(Id::new("clef-menu-bar").with(0))
        );
    }
}
