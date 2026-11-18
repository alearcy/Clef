---
name: Range overlay
kind: component
reference: composer::style::paintRangeOverlay
status: shared
impl: planned
tokens: [accent]
used-in: [arrangement, piano-roll, mixer, track-lanes, fx-rack]
snapshot: snapshots/range-overlay.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Range overlay

The one marquee and range selection look: `accent` at about 12 % with an 80 % edge. Used for part
marquees, lane ranges, take comps, node marquees and mixer strip ranges. A readout chip (10 px,
`accent`) may ride on it ("Take 2 · 12.1.000 → 13.3.000"). Never a foreign selection hue.
