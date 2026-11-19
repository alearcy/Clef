---
name: Transport bar
kind: view
reference: composer::style::paintTransportBar, ToolButton, paintSegmentDisplay, paintPerformance
status: mixed
impl: app
reference-source: [Source/MainComponent.cpp]
components: [tool-button, segment-display, text-button, chip, performance-readout, menu-button]
snapshot: snapshots/transport-bar.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Transport bar

T05 provides a provisional egui transport with Play, Stop, Return to Start, Loop, Metronome,
one-bar count-in, tempo/meter and a musical position counter. Transport Settings contains the
project's Stop Mode and exact-position and loop controls. See the [T05 specification](../../specs/T05-transport-timeline.md).
The bar draws from the current workspace state and dispatches its action after the workspace
has drawn. Session and audio transport acknowledgments do not dim otherwise available buttons. See
[T25](../../specs/T25-ui-flicker.md).
Stop is available during playback, count-in, or a pending Play request while audio is enabled.

## T06 MIDI recording

Record and the project-wide REC menu sit after Stop. The menu offers Merge, Replace, and New take
and identifies the current choice. Record is available while stopped with audio enabled and a ready
armed instrument. The recording lamp uses danger and stays lit during a take. Stop remains available
during preparation, count-in, and capture. The mode and locate controls lock for the take. At compact
widths, Metronome, Count-In, and Tempo and Meter Map appear in Transport Settings so the transport
stays on one row. Record and the mode options use the command registry and assignable shortcuts.

See the [T06 specification](../../specs/T06-midi-recording.md) for recording behavior and verification.

## Reference anatomy, left to right
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
