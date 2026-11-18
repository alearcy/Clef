---
name: Automation mode chip
kind: component
reference: composer::style::paintAutomationModeChip; app AutomationModeChip (Source/AutomationControls.h)
status: mixed
impl: planned
tokens: [field, secondary, accent, danger, disabled, controlEdge]
used-in: [track-header, mixer, channel-strip]
snapshot: snapshots/automation-mode-chip.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Automation mode chip

## Purpose
A channel's automation mode, Read / Touch / Latch / Write.

## Anatomy
A `field` chip with the mode's full word and ▾ in 10-px text, never a letter. Read `secondary`,
Touch and Latch `accent`, Write `danger`; text colour only, no fill. While writing, a 6-pixel
`danger` dot leads the word. With global read off it dims to `disabled`.

## Metrics
58 × 20 in a track header (from a 240-pixel column); full strip width in the mixer, under sends and
above pan; a labelled "Automation" row in the channel strip.

## Behaviour
Click opens the mode menu: Read, Touch, Latch, Write (danger), current ticked, then "All channels
to Read"; each a registry command with its live binding.

Raised (UI10 Fix 7): a body 2 px shorter than its bounds with a 1-pixel top light and a soft 2-pixel shadow under it (`paintControlBody`, `ControlDepth::raised`); hover adds the `controlEdge` edge; lit or pressed keeps the body and shadow and drops the top light; disabled is flat `panel` with a `divider` edge. See control depth in [foundations](../foundations.md).

The mode word and its ▾ are 11-px `metrics::small` text on chips 16 px and taller (9 px below).
