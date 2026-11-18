---
name: Segment display
kind: component
reference: composer::style::paintSegmentDisplay, segmentsFor
status: shared
impl: planned
tokens: [text, danger]
used-in: [transport-bar]
snapshot: snapshots/transport-bar.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Segment display

## Purpose
The transport's position counters, read as one hardware counter (D57).

## Anatomy
Soft-white seven-segment digits over faint unlit "8" ghosts in a dark recessed well, right-aligned;
narrow separators for `.` `:` `;`.

## States
`text` normally, `danger` only while the transport records. While typed into, the primary slot
shows a plain [text field](text-field.md).

## Behaviour
At the minimum window width the secondary (time) counter yields to the glyphs and returns when it
fits; the bars counter always stays.
