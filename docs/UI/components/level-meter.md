---
name: Level meter
kind: component
reference: composer::style::paintLevelMeter, LevelMeterState
status: shared
impl: app
tokens: [field, divider, accent, danger, secondary]
used-in: [track-header, channel-strip, mixer]
snapshot: snapshots/kit-gallery.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Level meter

## Clef T11

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
