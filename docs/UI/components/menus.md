---
name: Menus
kind: component
reference: composer::style::LookAndFeel (drawPopupMenu*, drawMenuBar*), isDestructiveMenuLabel
status: shared
impl: planned
tokens: [field, divider, peacock, text, secondary, danger, disabled]
used-in: [every surface]
snapshot: snapshots/menus.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

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
