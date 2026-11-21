---
name: Knob
kind: component
reference: composer::style::LookAndFeel::drawRotarySlider
status: shared
impl: gallery
tokens: [cap, capDark, accent, panel, field, controlEdge]
used-in: [channel-strip, mixer, fx-rack, knob-cell]
snapshot: snapshots/egui/t18-knob-1024.png
api: ui::primitives::mixer::Slider (Gain, Pan, Send, Width), ui::primitives::knob
evidence: [t18-knob]
---

> **Migrated to Rust/egui (T18 Phase 3, 2026-11-21).** The Clef section at the end of this file is
> the implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

# Knob

## Clef T11

*Superseded where it differs by the T18 Phase 3 section at the end of this file (the gesture: double-click now types the value; Option-click resets).*

The provisional mixer uses the knob for independent Gain trim and Pan. The control shows its
value, supports horizontal and vertical drag and arrow-key adjustment, and resets on double-click
or Option-click. See the [T11 mixer](../views/mixer.md#clef-t11) for layout and evidence.

## Purpose
Rotary values: trim, pan, send level, node parameters, macros.

## Anatomy
A `field` disc with a 1-pixel `divider` rim on a short contained cap shadow (`paintKnobBody`), and a
1.5-pixel `text` pointer from the centre to 38 % of the diameter, swept −144° to +144° (JUCE's
default rotary range). No value arc: the pointer is the value. Keyboard focus turns the rim `accent`.
On a node (a `field` surface) the disc takes the darker `panel` through the slider's fill colour.

## Metrics
44 pixels in the channel strip (trim, pan), about 30 in mixer strips, 34 in macros and nodes; 20–22 for send levels.

## States
Mapped to a macro: solid accent edge and an accent arc over the mapped range; map mode: dashed accent
edge. Modulated: base value stays, a 2-pixel `capDark` arc shows the swing with a `text` dot at the
live value, and a [~ chip](chip.md) sits in the sweep's gap.

## Behaviour
Vertical/horizontal drag; double-click and Option-click reset to the default. No value popup or
tooltip repeating the value while it turns: the value lives in the [knob cell](knob-cell.md).

## Do / Don't
- Pan knobs are quieter than faders. Width is its own [width knob](width-knob.md).

## Clef Rust/egui — T18 Phase 3

**API**

```rust
Slider { control: Control::Pan, value, drag_start, name, master: false, compact }.show(ui) -> Option<Adjustment>
```

Painted by `primitives::knob::paint_knob`; `compact` is the 20-point send knob that shows its value
in a tooltip while hovered or dragged. The gesture is the [fader](fader.md)'s: drag (horizontal or
vertical, Shift fine), double-click to type ("L 20", "C", "R 5", "-3 dB", "150%"), arrows step 1
(0.5 dB for gain and send) and Shift-arrows 10 (3 dB), Option-click resets to the control's default.
Accessible: slider named "<channel> <Control>" with its value.

**States:** gain, pan and width at default, low, high and disabled. The [width knob](width-knob.md)
is the fanned variant drawn by the same painter.

**Evidence:** gallery `snapshots/egui/t18-knob-{1024,760}.png` and `t18-knob-1024@2x.png`
(Retina); accessible names checked by `gallery::tests::every_gallery_control_has_an_accessible_name`.
