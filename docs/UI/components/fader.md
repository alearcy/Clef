---
name: Fader
kind: component
reference: composer::style::LookAndFeel::drawLinearSlider
status: shared
impl: gallery
tokens: [cap, capLight, capDark, field, controlEdge]
used-in: [channel-strip, mixer, fx-rack, arrangement]
snapshot: snapshots/egui/t18-fader-1024.png
api: ui::primitives::mixer::Slider (Control::Fader), ui::primitives::fader
evidence: [t18-fader]
---

> **Migrated to Rust/egui (T18 Phase 3, 2026-11-21).** The Clef section at the end of this file is
> the implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

# Fader

## Clef T11

*Superseded where it differs by the T18 Phase 3 section at the end of this file (the gesture: double-click now types the value; Option-click resets).*

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

## Clef Rust/egui — T18 Phase 3

**API**

```rust
Slider { control: Control::Fader, value, drag_start, name, master, compact: false }.show(ui) -> Option<Adjustment>
```

Travel, cap and dB scale are painted by `primitives::fader` (`paint_fader`, the shared `SCALE` and
`fraction` that the [level meter](level-meter.md) uses). The gesture is shared with the
[knobs](knob.md) in `mixer::Slider` and aligned with the [value slider](value-slider.md) (T18 Phase 3):

- drag moves the cap; **Shift** drags a quarter as far, and pressing or releasing Shift mid-drag
  re-bases the origin so the value never jumps; the release commits one undo step;
- **double-click opens an inline value field** under the control ("−6.0 dB"; "-inf", a missing unit and
  a plain minus are accepted): Return commits, Escape or a click elsewhere cancels;
- ↑/→ and ↓/← step 0.5 dB with focus, **Shift** steps 3 dB;
- **Option-click resets** to 0 dB. Double-click no longer resets.

Accessible: slider role named "<channel> Fader" with the dB value; tooltip "<name>: <value>".

**States:** default, low, high, disabled.

**Evidence:** gallery `snapshots/egui/t18-fader-{1024,760}.png` and `t18-fader-1024@2x.png`
(Retina); accessible names checked by `gallery::tests::every_gallery_control_has_an_accessible_name`.
