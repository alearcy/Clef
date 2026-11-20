---
name: Mixer
kind: view
reference: paintChannelBackground, ChannelRole, paintLevelMeter, channelWidth, channelGap
status: mixed
impl: app
reference-source: [Source/Mixer.cpp, Source/Mixer.h, Source/ProjectSession.h (MixerSection)]
components: [search-field, slot-button, power-button, tap-button, knob, width-knob, fader, level-meter, mute-solo, track-state-button, automation-mode-chip, chip, knob-cell, range-overlay, menus]
snapshot: snapshots/mixer.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Mixer

## Clef T11

The [T11 mixer](../../specs/T11-mixer.md#ui-proposal) is a provisional egui workspace view. The
Mixer and Sequencer buttons switch the workspace through the Show Mixer command. The transport
stays available. The bank follows project track order and Master stays docked on the right. The
bank scrolls horizontally when its strips exceed the available width. If the window is too short,
the entire mixer scrolls vertically, keeping the bank and Master aligned.

```text
workspace
  toolbar: Mixer / Sequencer
  transport
  vertical scroll
    row
      horizontal scroll: instrument strip × track count
      fixed channel width: Master strip
strip
  Gain and readout
  row: Pan and readout, Width and readout
  row: fader, shared dB ticks, stereo meter
  fader readout
  row: Mute, Solo
  route caption
  channel name and selection edge
```

Master carries Pan, the fader and meter, and an Output caption. The strip reserves the Gain and
Mute/Solo rows to align its fader and name with the bank. When the project is empty, Master is the
only strip. With audio stopped, readings are idle and controls remain available. Meter and clip
states are the same readings used by the sequencer's instrument headers. Long names truncate and
retain a full accessible label. Use Tab to move between controls, arrows to adjust a focused knob
or fader, and Left/Right at a focused channel name to move between strips. Selecting a strip shares
instrument selection with the sequencer. Selecting Master retains the instrument selection.

Evidence at [760 points](../snapshots/egui/t11-playing-760.png) and
[1024 points](../snapshots/egui/t11-long-bank-1024.png).

## Clef T12

T12 adds one ordered insert chain to every instrument strip and to Master. The provisional
egui UI keeps the T11 strip layout and places the chain between Gain and Pan/Width:

```text
strip
  Gain
  Inserts
    Plugin slot + Power (instrument tracks only)
    5-pixel insertion gap, then Effect slot + Power
    more Effect slots + Power
    Add effect…
  Pan / Width (Master: Pan)
  Fader and meter
  Mute / Solo (track) · Output (Master)
  Name
```

T35 adds the fixed **Plugin** row before the effect chain on instrument tracks. Master has no
Plugin row: its effects and Add effect start directly under the Inserts caption, and the
reserved alignment space sits below them so the lower controls stay aligned (see
[T35](#clef-t35)).

The chain has no fixed slot count. **Add effect…** appends an effect and remains visible on
an empty chain. Hovering the gap between two populated slots draws a 3-point `accent`
insertion line; clicking the gap opens the effect picker for that position, so choosing an
effect between `A` and `B` produces `A → new → B` without changing either existing slot.
The picker is a modal list of `catalog.effects` with a live search field, a Rescan action and
Cancel; cancelling leaves the project unchanged.

Each slot shows the plugin name truncated to the column, with the full name and state in the
tooltip and accessible name. Activating the slot opens the original plugin editor through the
existing hosted-editor path. The power glyph bypasses the slot. Active slots use the peacock
fill with `accent` power; bypassed slots dim the name and use `secondary` power; missing and
failed slots use the field surface, with a `danger` edge and cause on failure. The menu on a
slot provides **Insert Effect After…**, **Move Up**, **Move Down** and **Remove**; dragging
reorders slots. Every action is available through the shared insert commands, and each
edit is one undo step.

The insert area reserves up to four rows across the bank and Master: the Plugin row, up to two
effects and Add effect. Pan, fader, meter and name stay aligned across strips. Longer chains
scroll vertically inside
that area; the whole mixer still scrolls vertically when the window is too short, as in T11.
Keyboard focus on a slot targets the insert commands in the command palette; Enter or Space
opens the focused slot and toggles its power. The gap and slots have channel-and-plugin
accessible names.

Evidence at [1024 points](../snapshots/egui/t12-populated-1024.png) and
[760 points](../snapshots/egui/t12-populated-760.png); the full state set is
[empty](../snapshots/egui/t12-empty-1024.png),
[hovered insertion gap](../snapshots/egui/t12-insertion-1024.png),
[bypassed](../snapshots/egui/t12-bypassed-1024.png),
[missing](../snapshots/egui/t12-missing-1024.png),
[failed](../snapshots/egui/t12-failed-1024.png) and
[long chain](../snapshots/egui/t12-long-chain-760.png). The `t12_gallery` example regenerates
them at both review widths.

## Clef T35

The first row of an instrument strip's Inserts section is the fixed **Plugin** slot. It is the
track's source, not an effect: it cannot be removed, reordered, or preceded by an effect. Master
has no Plugin row; its effect rows start directly under Inserts and the equivalent alignment
space is below them, so Pan, fader, meter and name still align with the instrument strips.

```text
Plugin slot                     active: name + power (accent)
                                bypassed: dim name + secondary power
                                offline: field surface, dim name, disabled power
                                empty: "No instrument", opens the picker
```

Activating the Plugin slot opens the original instrument editor. An empty slot opens the
instrument picker. The context menu provides **Open Instrument Editor**, **Replace
Instrument…**, **Bypass Instrument** / **Enable Instrument**, and **Take Instrument Offline** /
**Bring Instrument Online**; the same state commands are in the command registry for the focused
channel.

Power is normal plugin bypass: the instrument worker stays online and processes MIDI and state,
but its output is replaced with silence before Gain and the effect chain. Effect tails may
continue. **Offline** is separate: the worker is released after its state is captured into the
project, the track does not load on open, selection, Enable Audio, playback or record preparation,
and the saved identity and state are retained. Bring Online reloads from that state. Both flags
persist and each change is one undo step.

The insert area reserves the Plugin row, up to two effect rows and the Add row: four rows. All
strips reserve that height; longer effect chains scroll inside the effect area while the Plugin
row stays visible. Evidence: [online](../snapshots/egui/t12-populated-1024.png),
[VI bypassed](../snapshots/egui/t12-vi-bypassed-1024.png),
[VI offline](../snapshots/egui/t12-vi-offline-1024.png),
[empty VI](../snapshots/egui/t12-vi-empty-1024.png), and
[long chain](../snapshots/egui/t12-long-chain-760.png). The `t12_gallery` example regenerates
the states at 1024 and 760 points.

## Legacy reference

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
