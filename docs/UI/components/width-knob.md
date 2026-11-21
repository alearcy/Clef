---
name: Width knob
kind: component
reference: composer::style::WidthKnob (paintWidthKnob); app ChannelStrip::WidthKnob sets range and text
status: shared
impl: gallery
reference-source: [Source/ChannelStrip.cpp] (range, default, description)
tokens: [cap, accent, capDark]
used-in: [channel-strip, mixer]
snapshot: snapshots/egui/t18-knob-1024.png
api: ui::primitives::knob (Control::Width)
evidence: [t18-knob]
---

> **Migrated to Rust/egui (T18 Phase 3, 2026-11-21).** The Clef section at the end of this file is
> the implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

# Width knob

## Clef T11

*Superseded where it differs by the T18 Phase 3 section at the end of this file (the gesture: double-click now types the value; Option-click resets).*

The provisional mixer places the 30-point Width knob beside Pan. The fan opens over 0–200%, with
100% restoring the original stereo width. The control shows its percentage and accepts pointer
drag, arrow keys, double-click or Option-click to reset, and Escape to cancel a drag. See the
[T11 mixer](../views/mixer.md#clef-t11) for layout and evidence.

## Purpose
Stereo width, drawn as what it does.

## Anatomy
No pointer: a fan of sound from below the centre, one upright ray at mono, open to 60 degrees each
side at full width, with two wavefronts across it.

## Behaviour
The fan glides to a new value (vblank easing) instead of jumping. Reset on double-click. The master
has no width knob.

## Clef Rust/egui — T18 Phase 3

**API**

The width knob is `Control::Width` of the [knob](knob.md) primitive: the same `mixer::Slider` gesture
(drag with Shift fine, double-click to type a percentage, arrows 1 and Shift-arrows 10, Option-click
resets to 100 %) and the fanned painter in `primitives::knob::paint_knob`, animated over 0.12 s.
Evidence: the width column of `t18-knob`.

**Evidence:** gallery `snapshots/egui/t18-knob-{1024,760}.png` and `t18-knob-1024@2x.png`
(Retina); accessible names checked by `gallery::tests::every_gallery_control_has_an_accessible_name`.
