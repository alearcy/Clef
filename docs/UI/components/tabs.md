---
name: Tabs
kind: component
reference: composer::style::TextToggle (since UI10.3; drawTabButton kept for JUCE tab bars)
status: shared
impl: planned
tokens: [panel, field, text, secondary, accent]
used-in: [window]
snapshot: snapshots/shared-controls.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Tabs

## Purpose
Switches the lower panel between Editor, Mixer and FX Racks. The three tabs sit centred in the
status row at the bottom of the window.

## Metrics
Text width plus 20 pixels × 26 each, 13-px (`body`) text.

## Current (UI10.3, 2026-11-11)
The tabs become flat text like BROWSER: EDITOR · MIXER · FX RACKS in 13-px (`body`) uppercase, `secondary`
off, `accent` for the shown panel, `text` on hover, a 1 × 12-pixel `divider` between items, 26-pixel
hit targets with 10-pixel padding, keyboard focus an `accent` edge.

## Legacy tab painter (drawTabButton)
Front tab `peacock` with `text`; others `panel` with `secondary`, `field` on hover; keyboard focus an
`accent` edge. Each tab is a registry command.
