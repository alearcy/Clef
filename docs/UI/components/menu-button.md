---
name: Menu button
kind: component
reference: composer::style::LookAndFeel::drawComboBox (ComboBox); app ShapeMenuButton and slot-style TextButtons
status: mixed
impl: gallery
reference-source: [Source/AutomationControls.h, Source/RackEditor.cpp, Source/ChannelStrip.cpp]
tokens: [field, text, secondary, controlEdge]
used-in: [tool-row, channel-strip, fx-rack, plugin-window, settings]
snapshot: snapshots/egui/t18-menu-button-1024.png
api: ui::primitives::menu_button::{menu_button, choice}
evidence: [t18-menu-button]
---

> **Migrated to Rust/egui (T18, 2026-11-21).** The Clef section at the end of this file is the
> implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

# Menu button

## Purpose
Shows one current value and opens a [menu](menus.md) of choices.

## Anatomy
A `field` button. The value always says what it is:
- with a prefix: "Shape: Sine · Grid ▾", "X-fade: Equal power", "Key: C", "Sidechain: Bus 1";
- or with a `secondary` caption before it ("Rate");
- a value without prefix ends in ▾ ("Violin ▾").

## Metrics
24 high, 12-pixel text in tool rows; row height inside strips and nodes.

## States
Open: the menu anchors under the button (see F04 menu anchoring). Disabled: `disabled` text.

## Do / Don't
- Do use it beyond four choices; up to four exclusive choices are a [segmented control](segmented-control.md).
- Don't truncate the value; the button or its column grows.

## Decision (UI10.3)
No new class: a menu button is a kit-painted `juce::TextButton` or `juce::ComboBox` whose text follows the value convention above. `ShapeMenuButton` stays the one subclass because it draws the shape glyph. A new menu button needs no new painter.

Raised (UI10 Fix 7): a body 2 px shorter than its bounds with a 1-pixel top light and a soft 2-pixel shadow under it (`paintControlBody`, `ControlDepth::raised`); hover lifts the body to `field` without an edge; lit or pressed keeps the body and shadow and drops the top light; disabled is flat `panel` with a `divider` edge. See control depth in [foundations](../foundations.md).

## Clef Rust/egui — T18

**API**

```rust
pub fn menu_button<R>(
    ui: &mut egui::Ui,
    id: impl Hash,
    name: &str,              // accessible name: "{name}: {value}"
    caption: Option<&str>,   // secondary caption before the value
    value: &str,             // shown with a trailing ▾
    enabled: bool,
    menu: impl FnOnce(&mut egui::Ui) -> R,
) -> (egui::Response, Option<R>); // Some(..) while the menu is open this frame
pub fn choice(ui: &mut egui::Ui, label: &str, selected: bool) -> bool;
```

Raised body like the text button; while its menu is open the body stays pressed. The menu anchors
under the button and is a [menu](menus.md) (`menu::under`, T18 Phase 2); it closes on a click.
`choice` is a `menu::Item` ticked when it is the current value. Never truncates: width hugs caption
and value.

**Usage**

```rust
let (_, snap) = menu_button(ui, "arrangement-snap", "Snap", Some("Snap"), current.label(), true, |ui| {
    Snap::ALL.into_iter().find(|&snap| choice(ui, snap.label(), snap == current))
});
if let Some(Some(snap)) = snap { *value = snap; }
```

Replaces every `egui::ComboBox`: snap (arrangement and roll), track record mode, Audio/MIDI
settings, beat unit and the controller-lane picker.
