---
name: Mixer
kind: view
reference: paintChannelBackground, ChannelRole, paintLevelMeter, channelWidth, channelGap
status: mixed
impl: planned
reference-source: [Source/Mixer.cpp, Source/Mixer.h, Source/ProjectSession.h (MixerSection)]
components: [search-field, slot-button, power-button, tap-button, knob, width-knob, fader, level-meter, mute-solo, track-state-button, automation-mode-chip, chip, knob-cell, range-overlay, menus]
snapshot: snapshots/mixer.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Mixer

## Search row
D101: a 30-pixel row above the bank with the mixer's own [search field](../components/search-field.md)
(200 wide, at the first strip's edge, "Find channel", ⌘F right-aligned) and "N of M channels · Esc clears". It filters the bank only (folders
that enclose a match show open; a D54-hidden match shows with the `hidden` glyph and a dimmed name);
the dock never filters. Independent of the arrangement's search.

## Bank
Empty bank space (after or between the strips, never the dock): double-click creates a MIDI track,
Cmd+click an audio track, right-click opens the shared creation menu (D100, see
[track header](track-header.md)).
Strips 92 wide, touching, separated only by the `background` gap; a small bank is never stretched.
Folder children sit under their folder bar (no group rail). FX returns, buses and master dock right
with their role accents; a bus folder's bus leads its members. Send and bus-output cables hang in a
band under the strips in role accents, lit when they touch the selection. Range and marquee use the
[range overlay](../components/range-overlay.md).

## Strip, top to bottom
Scrolling part, by the **Sections** toggles (saved view state, default Inserts + Output): TRACK
(arm, monitor: MIDI monitoring on a MIDI track, audio monitoring on an audio track), Input (2 rows
tall; one input of the track's kind, D100), Inserts start with the Plugin slot on a MIDI track only, Trim (34 knob + value), Inserts (slot + power, 20-pixel rows), Sends
(two rows each: tap glyph, level knob and dB above; slot and power below; the empty "+" keeps the
upper row disabled). Fixed bottom, never scrolled: automation mode chip, Pan and Width (master: pan
only), the fader 38 wide with its dB scale and a 14-pixel meter on its law, the dB readout, M/S (MONO
on master) and ◆, output, and the name box tinted by the track or role colour (UI10.4). A macro column (72 wide, 3-pixel cap,
◂ to close) can open right of a strip.

## Rules
- Spacing (approved 2026-11-11): the strip has 8 px top, 5 px side and 6 px bottom padding; its
  sections (TRACK, Input, Trim, Inserts, Sends, Automation, Pan / Width, Fader, M/S, Output + name)
  are groups 12 px apart with 3 px between rows inside a group. Never rules between them.
- A strip's colour tints only its name box, never its background; selection uses the
  [selection edge](../components/selection-edge.md).
- Fader caps of routing channels take their role colour.
- The Sections menu stays open while toggling (`ToggleMenuItem` rows, UI10.6): click, Return or
  Space toggles a section and the strips relayout at once; each toggle is a registry command.
