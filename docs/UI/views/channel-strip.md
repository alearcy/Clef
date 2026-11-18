---
name: Channel strip
kind: view
reference: paintChannelBackground, paintLevelMeter, inspectorWidth
status: mixed
impl: planned
reference-source: [Source/ChannelStrip.cpp, Source/ChannelStrip.h]
components: [slot-button, power-button, tap-button, knob, width-knob, fader, level-meter, mute-solo, track-state-button, automation-mode-chip, menu-button, text-field, caption-and-heading]
snapshot: snapshots/channel-strip.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Channel strip

## Purpose
The selected channel's full strip in the left drawer (`inspectorWidth` 208), one scrolling column.

## Anatomy (top to bottom, since UI10.4)
Name row with arm and monitor (audio monitoring on an audio track, whose arm is unavailable); TRACK (record mode "Global (Merge) ▾" for MIDI tracks, the "Delay"
caption with the track offset field and − / +, Chain and PDC wait readouts, Lanes); then the signal chain: INPUT (D100: the MIDI
input on a MIDI track, the physical audio input on an audio track), TRIM, INSERTS (the Plugin slot on
MIDI tracks only; an audio track's inserts start at FX 1; inserts with power and latency rows, "Add
audio effect…"), SENDS (tap glyph, name, level knob, dB readout, power), AUTOMATION (the mode chip,
full width), PAN / WIDTH, the fader with its dB scale and stereo meter and the dB readout, Mute /
Solo (Mono on master), OUTPUT. Section headings are drawn by `paintSectionHeading`; sections are
separated by spacing only; captions use a 62-pixel column. Insert and send rows keep 4 pixels
between them, so stacked slots never touch.

## Rules
- Neutral surface: no stripe, no colour wash; the [selection edge](../components/selection-edge.md)
  only. MIDI tracks label the first slot **Plugin**. M/S appear once.
- Every section stays reachable by scrolling; a hidden drawer gives its width back.
