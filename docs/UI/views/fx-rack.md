---
name: FX rack
kind: view
status: app
impl: planned
reference-source: [Source/RackEditor.cpp, Source/RackEditor.h, Source/PinnedMacros.h]
components: [power-button, knob, knob-cell, fader, segmented-control, menu-button, text-field, chip, detach-button, list-row, callout, range-overlay, checkbox, colour-picker]
snapshot: snapshots/fx-rack.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# FX rack (D59)

## Tab
The lower panel's third tab. The focused rack is one `panel` box filling the width; the track's other
racks fold to 32-pixel spines (power, name, slot) in chain order; a dashed "+ New rack" box stays at
the end. Click a spine (or Focus next/previous rack) to swap; Shift-click opens beside (520 minimum
each, then the row scrolls). Box header: power, slot label, name (double-click renames), latency in
samples, ⋯ menu, [detach](../components/detach-button.md).
D100: a MIDI track's row starts with the PLUGIN rack (instrument rack, MIDI nodes) and its inserts;
its audio parts bypass the PLUGIN rack and enter at INSERT 1, with nothing drawn for that path. An
audio track's row holds insert racks only: no PLUGIN slot, New instrument rack and Wrap in rack
unavailable, its menus and picker list audio effects only, and an instrument or instrument preset
dragged over it shows no accent target (the drop reports "Audio tracks take audio effects only").

## Graph
`canvas` with a 16-pixel dot grid, larger than its box, scrolling both ways. Nodes are `field` boxes
with a `divider` edge and 2-pixel corners: power glyph, bold name, `secondary` kind tag. Selected:
the shared `accent` edge; bypassed 60 %, its cables 35 %. Pins straddle the edge: audio `accent`,
MIDI `midi`, sidechain `sidechain`, modulation hollow (`field` fill, `capDark` ring). Cables are
2-pixel horizontal-tangent curves in the pin colour (selected 3 px `text`); mod cables dashed
`capDark`. While playing, brightened dots travel active cables. Third-party plugin nodes show a large
centred format badge (VST3, AU) and their bus names. Empty nodes: dashed `barLine` edge.
Quarantined plugin nodes (D88): the slot's 1.5-pixel `danger` outline, name and kind tag
("quarantined") in `danger`, the warning glyph in a `danger`-tinted badge in the format's place; pins
and cables stay as saved. The glyph sits in the badge, not before the name as on the board, because
the power glyph already leads the name. The node menu carries the same quarantine commands as a slot.

## Node row grammar (board 9)
Mode row (segments up to four, a prefixed menu beyond), the display, one knob row of
[knob cells](../components/knob-cell.md) at a 52/56 pitch, then targets or steps; rows 8 apart, none
wraps. Every value says what it is; nothing is truncated — the node grows. A one-mode setting swaps
in place in the mode row; a knob dims instead of jumping.

## Folding
Any node folds to one line (power, bold name, ▸) keeping every pin on its edge; the graph never
reflows. Fold state is view state saved with the rack. Commands: fold/unfold selected, fold all,
unfold all, fold rack, focus next/previous rack.

## Macros and modulators
Macro strip under the box header: count menu and Map at left, macros in rows of eight, then the
snapshot row (Store A, A, morph fader, B, Store B, "Morph n %"). Map mode: dashed accent edges on
mappable controls, solid edge + accent arc on mapped ones; the mapping editor is a
[callout](../components/callout.md). LFO, Envelope and Pump nodes: shape segments, a `canvas` shape
box (`capDark` curve, `accent` live dot), knob cells, one target row each ("→ Gain › Pan").
