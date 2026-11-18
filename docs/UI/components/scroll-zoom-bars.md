---
name: Scroll and zoom bars
kind: component
reference: composer::style::LookAndFeel::drawScrollbar; app ZoomScrollBars (Source/ZoomScrollBars.h)
status: mixed
impl: planned
tokens: [field, secondary, text]
used-in: [arrangement, piano-roll, mixer]
snapshot: snapshots/window.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Scroll and zoom bars

## Purpose
Always-visible scroll bars on editing surfaces, carrying their zoom buttons.

## Anatomy
Horizontal bar ending with `minus`/`plus` horizontal zoom glyphs at its right; vertical bar with
vertical `minus`/`plus` stacked at its bottom. The corner between them stays empty, except in the
arrangement, where `setCorner` places the waveform zoom glyph (D05 refinement, see
[arrangement](../views/arrangement.md)).

## Behaviour
⌘-wheel zooms horizontally and ⌘⇧-wheel vertically at the pointer. A horizontal wheel zoom follows each event's
delta continuously (×1.25 per 0.05 of delta); the buttons and the vertical zoom move in whole steps. An owner
whose content is a window of a longer axis (the arrangement, D105) gives the bar the whole width and the view's
position through `horizontalAxis` and takes its moves through `onHorizontalScroll`. Fit-to-width and fit-to-height
live as glyph buttons in the [tool row](../views/tool-row.md), never zoom in a toolbar.
