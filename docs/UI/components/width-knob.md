---
name: Width knob
kind: component
reference: composer::style::WidthKnob (paintWidthKnob); app ChannelStrip::WidthKnob sets range and text
status: shared
impl: planned
reference-source: [Source/ChannelStrip.cpp] (range, default, description)
tokens: [cap, accent, capDark]
used-in: [channel-strip, mixer]
snapshot: snapshots/shared-controls.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Width knob

## Purpose
Stereo width, drawn as what it does.

## Anatomy
No pointer: a fan of sound from below the centre, one upright ray at mono, open to 60 degrees each
side at full width, with two wavefronts across it.

## Behaviour
The fan glides to a new value (vblank easing) instead of jumping. Reset on double-click. The master
has no width knob.
