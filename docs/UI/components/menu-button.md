---
name: Menu button
kind: component
reference: composer::style::LookAndFeel::drawComboBox (ComboBox); app ShapeMenuButton and slot-style TextButtons
status: mixed
impl: planned
reference-source: [Source/AutomationControls.h, Source/RackEditor.cpp, Source/ChannelStrip.cpp]
tokens: [field, text, secondary, controlEdge]
used-in: [tool-row, channel-strip, fx-rack, plugin-window, settings]
snapshot: snapshots/kit-gallery.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

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

Raised (UI10 Fix 7): a body 2 px shorter than its bounds with a 1-pixel top light and a soft 2-pixel shadow under it (`paintControlBody`, `ControlDepth::raised`); hover adds the `controlEdge` edge; lit or pressed keeps the body and shadow and drops the top light; disabled is flat `panel` with a `divider` edge. See control depth in [foundations](../foundations.md).
