---
name: Hierarchy rail
kind: component
reference: composer::style::paintGroupRail
status: shared
impl: planned
tokens: [accent]
used-in: [track-header]
snapshot: snapshots/kit-gallery.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Hierarchy rail

In the header gutter, one continuous 3-pixel rail per enclosing folder, at that folder's indent and
in its colour (accent when uncoloured). No other connector lines. Nested headers step right 12 pixels
per folder depth as a whole, so no control crosses the gutter. The mixer uses no group rail.
