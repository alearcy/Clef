---
name: Colour picker
kind: component
reference: none (JUCE ColourSelector lives in juce_gui_extra, which the kit does not link)
status: app
impl: planned
reference-source: [Source/ColourPicker.h] (hosted by Arrangement::chooseColour for every surface)
tokens: [panel, field, text]
used-in: [track-header, arrangement, track-lanes, fx-rack, mixer]
snapshot: snapshots/colour-picker.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Colour picker

## Purpose
The one way to choose any colour: tracks, folders, markers, arrangement blocks, automation lanes,
articulation rows.

## Behaviour
OK commits once (one undo step); Cancel or Escape discards. The context menu of a coloured item
offers "Use track colour" (or the owner's default) to clear its own colour.

## Do / Don't
- Never a fixed swatch list or a second picker.

Since UI10.3 the picker content is its own component, `composer::ColourPicker` (`onApply`, `onCancel`), in `Source/ColourPicker.h`; `Arrangement::chooseColour` hosts it in one dialog window for every surface.
