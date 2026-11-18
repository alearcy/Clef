---
name: Transport bar
kind: view
reference: composer::style::paintTransportBar, ToolButton, paintSegmentDisplay, paintPerformance
status: mixed
impl: planned
reference-source: [Source/MainComponent.cpp]
components: [tool-button, segment-display, text-button, chip, performance-readout, menu-button]
snapshot: snapshots/transport-bar.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Transport bar

## Anatomy, left to right
Back to start, Play (with a caret in its own 16-pixel slot opening the play-start menu), Stop,
Record, Capture MIDI (back chevron + record dot: `disabled` when nothing is buffered, `accent` when a
phrase is ready, never a second red lamp), the global record-mode chip "REC Merge ▾" (24 high, D66
colours: Merge `secondary`, New take `accent`, Replace `danger`), Punch, Loop, Metronome, Click,
READ (44 × 24 text button, `peacock` while automation read is on), BPM, TAP, the Conductor glyph,
the bars and time [segment displays](../components/segment-display.md), CPU/Disk, Commands… and the
settings glyph.

## Lamps
Play accent green while running; Record `danger` while recording; everything else `accent` when
active. A lit lamp stays lit while disabled.

## Rules
- One row; it never grows a second row. The time counter yields first at narrow widths.
- Every control is a registry command with its live shortcut in the tooltip.
