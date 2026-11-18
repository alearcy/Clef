---
name: Text field
kind: component
reference: LookAndFeel (TextEditor colours); app AmountField, inline rename editors
status: mixed
impl: planned
reference-source: [Source/RackEditor.cpp (AmountField), Source/Arrangement.cpp, Source/PianoRoll.cpp, Source/MarkerList.cpp]
tokens: [field, text, accent, controlEdge]
used-in: [track-header, track-lanes, fx-rack, settings, transport-bar, channel-strip, command-palette]
snapshot: snapshots/kit-gallery.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Text field

## Purpose
Typing a name or an exact value: inline renames (tracks, parts, lanes, takes, racks), the position
counter, typed knob values, node fields, search fields.

## Anatomy
A native `juce::TextEditor` with a `field` fill and `controlEdge` edge; focus shows the `accent` edge.
Search fields carry the `search` glyph at the left.

## Behaviour
- Native editing is kept: selection, clipboard, IME, undo inside the field.
- Return commits, Escape restores the previous value; an invalid value is rejected, the previous
  complete value restored and the reason sent to the status line.
- Inline rename opens on double-click over the name, with the name's own font and position; Tab and
  Shift-Tab walk the next names in row order.
- A typed value replaces a readout in place (knob value line, counter); no popup, no second field.

## Do / Don't
- Don't open a dialog to rename.
- Don't restyle a text field with a track colour.
- Don't use a text field to enter a numeric parameter: every number is a [value slider](fader.md#value-slider), whose value text is typed in place on double-click (composer, 2026-11-13, global rule).

Raised (UI10 Fix 7, variant B for fields): the same body, top light and under-shadow as a raised button (`ControlDepth::raised`); hover adds the `controlEdge` edge, focus the `accent` edge while typing. See control depth in [foundations](../foundations.md).
