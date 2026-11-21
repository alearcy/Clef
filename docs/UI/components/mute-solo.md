---
name: Mute and solo
kind: component
reference: composer::style::MuteSoloButton (colours::solo, colours::oxblood)
status: shared
impl: gallery
reference-source: [Source/Arrangement.cpp, Source/Mixer.cpp] (subclasses for sweep gestures)
tokens: [oxblood, solo, field, text, controlEdge]
used-in: [track-header, channel-strip, mixer]
snapshot: snapshots/egui/t18-mute-solo-1024.png
api: ui::primitives::mute_solo::lamp
evidence: [t18-mute-solo]
---

> **Migrated to Rust/egui (T18 Phase 3, 2026-11-21).** The Clef section at the end of this file is
> the implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

# Mute and solo

## Clef T11

*Superseded where it differs by the T18 Phase 3 section at the end of this file.*

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

## Clef Rust/egui — T18 Phase 3

**API**

```rust
pub(crate) fn lamp(ui: &mut egui::Ui, label: &str, hint: &str, solo: bool, lit: bool, enabled: bool) -> bool;
```

The only filled lamps: mute lit is `oxblood` with a `text` label, solo lit is `solo` with a
`background` label (both ≥ 4.5:1, foundations contrast table); off is `field` with a `secondary`
label. Lit also changes the label colour and the accessible checked state, so it never depends on
the fill alone. Accessible: checkbox role named by `hint` ("Mute Strings").

**States:** off, lit, disabled, disabled lit (a lit lamp stays lit while disabled).

**Evidence:** gallery `snapshots/egui/t18-mute-solo-{1024,760}.png` and `t18-mute-solo-1024@2x.png`
(Retina); accessible names checked by `gallery::tests::every_gallery_control_has_an_accessible_name`.
