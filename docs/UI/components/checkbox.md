---
name: Checkbox
kind: component
reference: composer::style::LookAndFeel::drawToggleButton
status: shared
impl: planned
tokens: [field, accent, text, controlEdge]
used-in: [fx-rack, settings]
snapshot: snapshots/settings.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Checkbox

## Purpose
A boolean setting with a visible label ("Skip first note", settings switches).

## Anatomy
`juce::ToggleButton`: a small `field` box with a `controlEdge` edge, an `accent` check when on, the label
to its right in `text`.

## Behaviour
Changing it invokes a registry command (one per row when it belongs to a row, as in the Articulation
Split), so it can be bound to a key; the tooltip shows the live binding.

## Do / Don't
- Don't use a checkbox for a mode with more than two values; use a segmented control.

The box keeps its 1-pixel `controlEdge` edge (`divider` while disabled). See control depth in [foundations](../foundations.md).
