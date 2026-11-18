---
name: Performance readout
kind: component
reference: composer::style::paintPerformance
status: shared
impl: planned
tokens: [secondary, disabled]
used-in: [transport-bar]
snapshot: snapshots/kit-gallery.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Performance readout

Compact CPU / Disk near the top right, from real supplied data only. Missing data renders as a dash;
never fictional values.
