---
name: Shadows
kind: component
reference: composer::style::paintLiftShadow, paintRegionShadow; slider LookAndFeel
status: shared
impl: planned
tokens: [background]
used-in: [piano-roll, arrangement, transport-bar, fader, knob]
snapshot: snapshots/kit-gallery.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Shadows

- **Lift**: under notes or parts while a move drag displaces them; paint it, then the lifted shapes.
- **Region**: the short falloff a chrome region casts into the margin on one side (transport bar).
- **Contained**: caps and knobs carry a short shadow inside their own bounds; black keys on the deck.

Never on menus, popups or cards; never as a separator.
