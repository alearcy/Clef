---
name: Mute and solo
kind: component
reference: composer::style::MuteSoloButton (colours::solo, colours::oxblood)
status: shared
impl: app
reference-source: [Source/Arrangement.cpp, Source/Mixer.cpp] (subclasses for sweep gestures)
tokens: [oxblood, solo, field, text, controlEdge]
used-in: [track-header, channel-strip, mixer]
snapshot: snapshots/shared-controls.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Mute and solo

## Clef T11

The provisional mixer uses independent labelled M and S buttons on instrument tracks. They and the
selected-channel commands use the same toggle. Muted tracks are silent even when soloed. Master
reserves the button row to align the strips. See the [T11 mixer](../views/mixer.md#clef-t11).

## Purpose
The only two lamps of headers and strips.

## Anatomy
Small "M" and "S" buttons. Off: `field`/`panel` fill, `text` letter. On: mute `oxblood` fill with a
light letter, solo `solo` yellow with a dark letter. The master has MONO (44 wide) instead of solo.

## Metrics
Track header 22 × 20 (16-pixel targets below 30-pixel rows); mixer strip row 22; channel strip 28.

## Behaviour
Registry commands per track; mixer strips support a sweep gesture across strips. Never repeated
beside the track title when M/S are already shown below.

## Do / Don't
- Don't use oxblood or yellow for anything else in a header or strip.

Raised (UI10 Fix 7): a body 2 px shorter than its bounds with a 1-pixel top light and a soft 2-pixel shadow under it (`paintControlBody`, `ControlDepth::raised`); hover lifts the body to `field` without an edge; lit or pressed keeps the body and shadow and drops the top light; disabled is flat `panel` with a `divider` edge. See control depth in [foundations](../foundations.md).
