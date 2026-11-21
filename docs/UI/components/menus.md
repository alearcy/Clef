---
name: Menus
kind: component
reference: composer::style::LookAndFeel (drawPopupMenu*, drawMenuBar*), isDestructiveMenuLabel
status: shared
impl: gallery
tokens: [field, divider, peacock, text, secondary, danger, disabled]
used-in: [every surface]
snapshot: snapshots/egui/t18-menu-1024.png
api: ui::primitives::{menu, menu_bar}
evidence: [t18-menu, t18-menu-bar]
---

> **Migrated to Rust/egui (T18 Phase 2, 2026-11-21).** The Clef section at the end of this
> file is the implementation contract. Earlier sections keep the legacy design intent; their C++
> symbols are not in this repository.

# Menus

## Purpose
Context menus, pop-up menus (from menu buttons and chips) and the Windows in-window menu bar. On
macOS the menus are the system menu bar (F05); settings entries live in the app menu.

## Anatomy
- `field` surface, 1-pixel `divider` edge, 4 pixels padding, no shadow.
- Rows 26 high, 14-px text; a 16-pixel tick column (stroked check in the row's ink); the live
  shortcut right-aligned in 13-px `secondary`, omitted when unbound; a solid ▸ (4 × 6, `secondary`)
  for a submenu.
- Separators: a 1-pixel `divider` line in a 9-pixel row. Section headers: 12-px bold `secondary` in a
  22-pixel caption row, sized by the kit (never JUCE's default 1.5 × row height and 1.25 × width).
- Labels with non-ASCII characters (…, ▸, ·) are built with `juce::String::fromUTF8`, or they render as mojibake.
- Highlighted row: `peacock` fill, 2-pixel corners, `text`.
- Destructive entries (label starts with Delete, Remove, Clear; or marked: Replace, Write) in
  `danger`; disabled entries `disabled`.

## Behaviour
Every menu is a projection of the command registry: same name, enablement and live shortcut. Menus
anchor to their invoking control (F04). Keep them short and relevant; command search covers the
rest. A menu that toggles several items in one visit (mixer Sections) stays open until Escape,
a click outside or its button: its rows are kit `ToggleMenuItem`s, painted as every menu row
(UI10.6).

T08 uses a provisional in-window egui Edit menu on macOS. Its entries and
states are documented in [Edit History](../views/edit-history.md).

## Do / Don't
- Don't add a second action list or a menu entry with private behaviour.
- Don't nest frequent actions more than one submenu deep.

## Clef Rust/egui — T18 Phase 2

**impl:** gallery. Context menus, the menus of menu buttons and chips, the REC mode popup, the
mixer's send, output and slot menus, and the project window's [menu bar](../views/window.md) all
use this primitive. Clef has no macOS system menu bar: native AppKit UI is not allowed, so the
in-window bar is used on every platform. Mixer Sections (a toggle menu that stays open) does not
exist in Clef yet; the primitive has no keep-open row until it does.

**API**

```rust
// ui::primitives::menu
pub fn frame() -> egui::Frame;                               // field, 1-pt divider edge, 4-pt padding
pub fn under(button: &Response) -> Popup<'_>;                // opens under a button on click
pub fn context(response: &Response) -> Popup<'_>;            // opens on a secondary click
pub fn show<R>(popup: Popup<'_>, content: impl FnOnce(&mut Ui) -> R) -> Option<R>;
pub fn begin(ui: &mut Ui);                                   // menu spacing inside a custom popup
pub struct Item<'a>;                                         // Item::new(label)
impl Item<'_> {
    fn shortcut(self, &str) -> Self;  fn ticked(self, bool) -> Self;
    fn enabled(self, bool) -> Self;   fn destructive(self, bool) -> Self;
    fn highlighted(self, bool) -> Self;                      // forced highlight, gallery only
    fn show(self, ui) -> bool;                               // picked; closes the menu
}
pub fn separator(ui: &mut Ui);
pub fn caption(ui: &mut Ui, label: &str);
// ui::primitives::menu_bar
pub fn menu_bar<R>(ui, titles: &[&str], menu: impl FnOnce(usize, &mut Ui) -> Option<R>,
                   trailing: impl FnOnce(&mut Ui) -> Option<R>) -> Option<R>;
```

Accessible name: the row label plus its shortcut ("Duplicate (⌘D)"); ticked rows are checkboxes
with their checked state; bar titles are buttons named by their title.

**Anatomy**

| Region | Size | Fill | Edge | Text |
| --- | --- | --- | --- | --- |
| Menu frame | content + `menuPadding` 4 | `field` | 1 pt `divider` | — |
| Row | `menuRow` 26 high, as wide as the widest row | none; `peacock` when highlighted | — | `MENU` 14, `text` |
| Tick column | `menuTick` 16 | — | 1.5-pt stroked check in the row's ink | — |
| Shortcut | right-aligned, `controlPadding` 8 from the edge | — | — | `BODY` 13, `secondary`; omitted when unbound |
| Separator | `menuSeparator` 9 | — | 1-pt `divider` line | — |
| Caption | `menuCaption` 22 | — | — | 12, `secondary` (one SF Pro weight is loaded, so not bold) |
| Bar title | text + 2 × `controlPadding`, `menuBarHeight` 28 | none; `field` on hover; `peacock` while its menu is open | `accent` focus edge under keyboard navigation | `BODY` 13, `text` |

**States**

| State | Fill | Text / glyph |
| --- | --- | --- |
| rest | none | `text`; shortcut `secondary` |
| hover / keyboard focus | `peacock` | `text`, shortcut `text` |
| destructive (Delete, Remove, Clear, or marked) | none | `danger`; `text` while highlighted |
| ticked | none | check in the row's ink |
| disabled | none | `disabled`, skipped by ↑/↓, not clickable |

**Behaviour**

- Every menu entry that runs a command is a projection of the [command registry](../views/command-palette.md):
  same name (Undo and Redo name the edit), availability and live shortcut.
- Rows are focusable: ↑/↓ move, Return or Space activates, Escape closes. A menu opened from the
  keyboard focuses its first enabled row.
- Menu bar: click or Return on a title opens it; ↓ on a focused title opens it; while a menu is
  open, hovering another title or ←/→ moves to it; F10 focuses the first title from anywhere;
  Escape closes and returns focus to the title; picking a row closes the menu.
- Rows are as wide as the widest row of their menu (remembered across frames), never the full
  space the popup could take.

**Tokens:** `field`, `divider`, `peacock`, `text`, `secondary`, `danger`, `disabled`, `accent`;
`menuRow`, `menuSeparator`, `menuCaption`, `menuTick`, `menuPadding`, `menuBarHeight`,
`controlPadding`, `cornerControl`; `MENU`, `BODY`.

**Usage** (`crates/ui/src/menus.rs`, `primitives/timeline.rs`)

```rust
super::menu::show(popup, |ui| {
    for (index, (label, shortcut, enabled)) in rows.into_iter().enumerate() {
        if super::menu::Item::new(label).shortcut(&shortcut).enabled(enabled).show(ui) {
            command = Some(index);
        }
    }
});
```

**Evidence:** gallery `t18-menu`, `t18-menu-bar` (`snapshots/egui/t18-menu-{1024,760}.png`,
`t18-menu-bar-{1024,760}.png`); `primitives::menu_bar::tests::keyboard_opens_moves_between_menus_picks_and_escapes`;
`arrangement::tests::take_lane_menu_opens_with_the_shared_popup`; surface snapshots T09 (clip
menu) and every project-window snapshot (menu bar).

**Do / Don't**

- Don't call `ui.menu_button`, `Popup::menu`, `Popup::context_menu` or `ui.button` inside a menu
  in a surface; use `menu::under`/`menu::context` and `menu::Item`.
- Don't hardcode a shortcut string in a menu row; read it from the key map.
- Don't nest submenus: groups are captioned sections with ticks.
