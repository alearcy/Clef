---
name: Selection edge
kind: component
reference: composer::style::paintChannelBackground (selected), shared edge in rows
status: shared
impl: gallery
tokens: [panel, field, accent]
used-in: [track-header, mixer, channel-strip, list-row, fx-rack, settings]
snapshot: snapshots/egui/t18-selection-edge-1024.png
api: ui::paint::selection_edge
evidence: [t18-selection-edge]
---

> **Migrated to Rust/egui (T18, 2026-11-21).** The Clef section at the end of this file is the
> implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

# Selection edge

The one cue for a selected header, strip, row, macro or node: a 1.5-pixel `accent` edge at 80 %
alpha drawn inside the bounds. Headers and strips keep their `panel` surface, because they carry
`field` controls (slots, chips, buttons) that a `field` fill would swallow; plain list rows, which
hold only text, add the `field` fill. Role accents stay at 10 %. A track colour never
washes the background (`trackWashAlpha` is reserved for parts and stripes, not headers or strips).

## Clef Rust/egui — T18

**API**

```rust
pub fn selection_edge(painter: &egui::Painter, rect: egui::Rect);
```

A `SELECTION_EDGE` (1.5) stroke of `ACCENT` at 80 % inside `rect`, corner `CORNER_CONTROL`.
[List row](list-row.md) calls it for selected rows and headers. Mixer strips keep their T11 edge
until the mixer is migrated.
