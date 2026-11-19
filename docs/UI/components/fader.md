---
name: Fader
kind: component
reference: composer::style::LookAndFeel::drawLinearSlider
status: shared
impl: app
tokens: [cap, capLight, capDark, field, controlEdge]
used-in: [channel-strip, mixer, fx-rack, arrangement]
snapshot: snapshots/slider-travel.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Fader

## Clef T11

The provisional egui mixer fader uses a 38 × 210-point stage and a 22 × 40-point cap. Its travel
uses the same dB mapping as the meter. The bottom stop is silent and the top is +6 dB. Drag and
arrow keys adjust the value, double-click or Option-click resets to 0 dB, and Escape cancels a
drag. See the [T11 mixer](../views/mixer.md#clef-t11) for layout and evidence.

## Purpose
Level and other linear values with a physical feel; the character element of the UI.

## Anatomy
A recessed 6-pixel travel slot (`field` groove with a `divider` rim and an inner shadow along its top or left, readable on `background` and
`panel`), a vector satin cap (vertical gradient `capLight` → `capDark`) with a central index line in
`background` at 65 % and a short contained shadow; keyboard focus adds an `accent` edge. A host-set thumb colour
tints the satin (routing channels use their role colour).

## Scale
Always paired with the [level meter](level-meter.md) scale: 6, 0, −6, −12, −24, −36, −48 between fader and meter.

## Metrics
Vertical cap 22 × 40 (thumb radius 21); horizontal thumb radius 14. Channel strip fader at least 208
high, mixer at least 210; mixer fader column 38 wide with a 14-pixel meter beside it.

## States
Dragging: cap follows; disabled: cap dims. Modulated: a 2-pixel `capDark` bar with a `text` dot at
the live value under the fader. Mapped to a macro: an accent bar under it.

## Behaviour
Standard JUCE `Slider` semantics. Double-click and Option-click reset to the default (0 dB). The
application owns the gain law, automation and history; one drag is one undo step
(`beginChannelGesture` / `endChannelGesture`).

## Do / Don't
- Don't draw a fader by hand; use a `juce::Slider` with the kit `LookAndFeel`.

## Value slider

`style::makeValueSlider(slider, default, bipolar)` (D102): a compact horizontal slider for parameter
rows in drawers — the same recessed field groove as a fader travel, a `peacock` fill from the minimum
(or from zero when bipolar), a 12-pixel round satin cap (`capLight` → `capDark`, cap shadow) and the
value with its unit as plain `text` to the right (46 wide). A double-click on the value types it, a
double-click on the travel returns to the default, ⇧-drag moves it finely (relative, slow), the wheel
steps (composer, 2026-11-13). It is the one control for every numeric parameter in a drawer, call-out, dialog or settings row (global rule, composer 2026-11-13): never a typed field, never − + steppers; a long range (times, ratios) uses a logarithmic travel. Disabled: `divider` fill and a flat `disabled` cap. Used in the MIDI
functions drawer. Snapshot: `snapshots/midi-functions-sections.png`.
