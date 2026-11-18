---
name: Selection edge
kind: component
reference: composer::style::paintChannelBackground (selected), shared edge in rows
status: shared
impl: planned
tokens: [panel, field, accent]
used-in: [track-header, mixer, channel-strip, list-row, fx-rack, settings]
snapshot: snapshots/kit-gallery.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Selection edge

The one cue for a selected header, strip, row, macro or node: a 1.5-pixel `accent` edge at 80 %
alpha drawn inside the bounds. Headers and strips keep their `panel` surface, because they carry
`field` controls (slots, chips, buttons) that a `field` fill would swallow; plain list rows, which
hold only text, add the `field` fill. Role accents stay at 10 %. A track colour never
washes the background (`trackWashAlpha` is reserved for parts and stripes, not headers or strips).
