---
name: Scroll and zoom bars
kind: component
reference: composer::style::LookAndFeel::drawScrollbar; app ZoomScrollBars (Source/ZoomScrollBars.h)
status: mixed
impl: gallery
tokens: [field, secondary, text]
used-in: [arrangement, piano-roll, mixer]
snapshot: snapshots/egui/t18-scroll-zoom-bars-1024.png
api: ui::primitives::scroll_zoom_bars::{horizontal, vertical, Axis, Moved}
evidence: [t18-scroll-zoom-bars, primitives::scroll_zoom_bars::tests]
---

> **Migrated to Rust/egui (T18, 2026-11-21).** The Clef section at the end of this file is the
> implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

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

## Clef Rust/egui — T18

**API**

```rust
pub struct Axis { pub offset: f32, pub visible: f32, pub total: f32 } // owner's units
pub enum Zoom { In, Out }
pub struct Moved { pub offset: Option<f32>, pub zoom: Option<Zoom> }
pub fn horizontal(ui: &mut egui::Ui, name: &str, axis: Axis, enabled: bool) -> Moved;
pub fn vertical(ui: &mut egui::Ui, name: &str, axis: Axis, enabled: bool) -> Moved;
```

`SCROLL_BAR` (12) thick. The track is a recessed `control_body`; the thumb is `SECONDARY` at 55 %
(full on hover or drag), at least two buttons long; dragging keeps the grab point, a click on the
track centres the thumb there. −/+ glyphs (`TOOL_STROKE`) sit at the right end (horizontal) or
stacked at the bottom (vertical). The owner applies `Moved`; the primitive holds no view state.
`primitives::transport::navigation` wraps the horizontal bar for a time axis (×1.25 per zoom step,
8–240 points per quarter). Since [T40](../../specs/T40-editing-surfaces-cleanup.md) the
arrangement places the horizontal bar under its lanes and a vertical bar beside its track rows
(track height steps); the piano roll places the horizontal bar under its controller lanes and a
vertical bar beside its grid (pitch-row height steps).

`scroll_zoom_bars::wheel(ui, rect) -> Wheel` reads the gesture over a surface and consumes its
scroll: `scroll` (points; egui already turns Shift-wheel into horizontal scroll),
`zoom_horizontal` (continuous factor from ⌘-wheel or pinch) and `zoom_vertical` (whole ×1.25
steps from ⌘⇧-wheel, accumulated across frames), plus the pointer as the zoom anchor.
