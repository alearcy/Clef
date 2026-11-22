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

## Current Clef layout

```text
row
  Return to Start · Play · Stop · Record
  REC mode menu · Loop
  wide only: Metronome · 1 Bar · BPM and meter
  musical position · Transport Settings
```

Return to Start, Play, Stop, Record, Loop and Metronome are individual
[tool buttons](../components/tool-button.md). Their accessible names and shortcut tooltips come
from the command registry. Below 860 points of available width, Metronome and 1 Bar are available
in Transport Settings. See [T34](../../specs/T34-transport-glyphs.md) for state and gallery evidence.

Since T18 Phase 3 the musical position is a [segment display](../components/segment-display.md)
(`danger` only while a take records) and the count-in state is a `Count-in` caption beside it.

## T06 MIDI recording

Record and the project-wide REC menu sit after Stop. The menu offers Merge, Replace, and New take
and identifies the current choice. Record is available while stopped with audio enabled and a ready
armed instrument. The Record glyph uses `DANGER` from the initial press through preparation,
count-in, and capture. Stop remains available
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
Play and Metronome use `ACCENT` while active. Loop uses `LOOP_ACTIVE` alongside the
[loop band](timeline-rulers.md). Lit glyphs keep their active colours while disabled.

## Rules
- One row; it never grows a second row. The time counter yields first at narrow widths.
- Every control is a registry command with its live shortcut in the tooltip.

## Clef T41 band

The bar is a full-width band: `panel`, 6 points top and bottom, 12 at the sides, with the region
[shadow](../components/shadows.md) below it (`primitives::transport::band`). After Transport…
a `secondary` caption shows the device sample rate (`48 kHz`, `44.1 kHz`), hidden while audio is
off. The recording bit depth joins it once audio recording exists ([T14](../../specs/T14-audio-recording.md)).
Snap and the fit glyphs are not part of the bar: they are in the [tool row](tool-row.md) under it.

## Clef T41 round 2

Left to right after tempo: the bar.beat.tick [segment display](../components/segment-display.md),
the **second counter** (time `h:mm:ss.mmm`, samples, or SMPTE `hh:mm:ss:ff` at 24 / 25 / 30 fps,
picked from a menu on click and saved with the project; hidden below 860 points), the
**format readout** (sample rate over bit depth in two `secondary` caption rows; a click opens
Project Format…), **Snap**, then Transport…. The band keeps the window's 12-point side margins.

## Clef T41 round 3

[T41 round 3](../../specs/T41-session-window-layout.md#round-3--gui-review-2026-11-22) (S12): the project format readout beside the counters is a
[two-row segment readout](../components/segment-display.md#clef-t41-round-3--two-row-readout):
sample rate on top, bit depth below. A click still opens Project Format.
