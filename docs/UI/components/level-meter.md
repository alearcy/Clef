---
name: Level meter
kind: component
reference: composer::style::paintLevelMeter, LevelMeterState
status: shared
impl: gallery
tokens: [field, divider, accent, danger, secondary]
used-in: [track-header, channel-strip, mixer]
snapshot: snapshots/egui/t18-level-meter-1024.png
api: ui::primitives::level_meter
evidence: [t18-level-meter]
---

> **Migrated to Rust/egui (T18 Phase 3, 2026-11-21).** The Clef section at the end of this file is
> the implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

# Level meter

## Clef T11

*Superseded where it differs by the T18 Phase 3 section at the end of this file.*

`strip_meter` and `header_meter` in `crates/ui/src/primitives/mixer.rs` paint the same stereo
reading from the session. The mixer scale and fader share one mapping and draw ticks at +6, 0, −6,
−12, −24, −36 and −48 dB. The sequencer version is a frameless 9-point meter without labels.
The session owns the immediate attack, 20 dB/s decay, one-second hold and clip latch. Clear a
clipped channel by clicking its mixer meter or invoking its clip reset command. See the
[mixer](../snapshots/egui/t11-playing-1024.png) and [header](../snapshots/egui/t11-header-1024.png)
snapshots. T11 uses these primitives provisionally pending T18 design-system acceptance.

## Purpose
Post-fader level: peak, hold tick and a clipped flag.

## Anatomy
A `background` well with an `accent` fill, a 1-pixel `text` hold line and a 2-pixel `danger` clip bar at the top; stereo meters sit in a `divider` outline, except the track header's 9-pixel meter, which has none. The host supplies the facts, the kit paints them.

## Metrics
9 pixels wide at the right edge of every channel header, full row height; 14 in mixer strips beside
the fader, on the fader's own law (0 dB on the cap meets 0 dB on the meter).

## Scale
Strips draw a dB scale beside the meter (`ChannelStrip::paintMeterScale`), on the fader's own law:
6, 0, −6, −12, −24, −36, −48. The 0 dB tick is 6 pixels in `secondary`, the others 4 pixels in
`divider`; labels are 11 px `secondary` and a label that would collide with another is dropped
(ticks never are). In the inspector the scale sits between the fader and a wide stereo meter with
L / R under it; in the mixer between the fader and the 14-pixel meter.

## Behaviour
Smoothing, hold timing and dB conversion stay with the host on the message thread. A clip latches
until clicked.

## Clef Rust/egui — T18 Phase 3

**API**

```rust
pub(crate) fn strip_meter(ui: &mut egui::Ui, name: &str, reading: Meter, shortcut: &str) -> bool;
pub(crate) fn header_meter(ui: &mut egui::Ui, height: f32, name: &str, reading: Meter);
pub(crate) fn master_header(ui: &mut egui::Ui, width: f32, reading: Meter);
pub(crate) fn scale(ui: &mut egui::Ui);
```

Stereo bars on the fader's dB scale (`fader::fraction`, so meter and fader ticks line up), hold
ticks, and a `danger` clip strip. A clipped strip meter is a button named "Reset clip, <channel>"
that clears it; header meters are read-only. Moved out of `mixer.rs` unchanged in look.

**States:** silent, mid level with holds, clipped.

**Evidence:** gallery `snapshots/egui/t18-level-meter-{1024,760}.png` and `t18-level-meter-1024@2x.png`
(Retina); accessible names checked by `gallery::tests::every_gallery_control_has_an_accessible_name`.
