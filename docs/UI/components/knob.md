---
name: Knob
kind: component
reference: composer::style::LookAndFeel::drawRotarySlider
status: shared
impl: planned
tokens: [cap, capDark, accent, panel, field, controlEdge]
used-in: [channel-strip, mixer, fx-rack, knob-cell]
snapshot: snapshots/kit-gallery.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Knob

## Purpose
Rotary values: trim, pan, send level, node parameters, macros.

## Anatomy
A `field` disc with a 1-pixel `divider` rim (`controlEdge` on hover) on a short contained cap shadow (`paintKnobBody`), and a
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
