---
name: Segmented control
kind: component
reference: composer::style::SegmentedControl
status: mixed
impl: planned
reference-source: [Source/AutomationControls.cpp (Region | Track), Source/RackEditor.cpp (node modes)]
tokens: [field, peacock, text, secondary, controlEdge]
used-in: [side-drawers, settings, track-lanes, fx-rack]
snapshot: snapshots/shared-controls.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Segmented control

## Purpose
An exclusive choice among two to four options shown at once: Tracks | Racks | Plugins, Region |
Track, Show chooser | Reopen last | Start empty, LFO shapes, Arpeggiator modes.

## Anatomy
A `field` well holding touching segments; the lit segment is `peacock` with a `foregroundFor`
label, the others `secondary` text. Segments may be glyphs (LFO shapes) with the name in the tooltip.

## Metrics
Row height of its context (22–24). Equal-width segments unless labels differ a lot.

## States
hover lifts the label to `text`; disabled segments dim and are skipped by the keyboard; a segment
that applies only in another mode stays in place, dimmed, with a hint ("Notes mode only").

## Behaviour
Each segment is a registry command or a session edit with undo, as its owner defines.

## Do / Don't
- Don't use more than four segments; use a [menu button](menu-button.md).

## Adoption
The Browser segments and Settings › Startup use the kit control since UI10.3 (Startup used to light `field` instead of peacock). The automation lane's Region | Track pair and the rack node mode rows still lay out their own TextButtons, painted by the same kit LookAndFeel colours; they move with their surfaces.

## Code
```cpp
composer::style::SegmentedControl segments { { "Tracks", "Racks", "Plugins" } };
segments.onSelect = [this](int i) { commands.invoke(segmentCommands[i]); };
segments.setSelected(shown); // the host reports the value back
```

Recessed (UI10 Fix 7): the track has the same body height as a raised button; the lit segment is a flat `peacock` fill inside it with an inner top shadow and a 1-pixel bottom light (`ControlDepth::recessed`); hover lifts the body to `field`, focus keeps the `accent` edge. See control depth in [foundations](../foundations.md).
