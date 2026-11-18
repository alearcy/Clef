---
name: Knob cell
kind: component
status: app
impl: planned
reference-source: [Source/PinnedMacros.h (Cell), Source/RackEditor.cpp (MacroCell, node knobs), Source/ChannelStrip.cpp, Source/Mixer.cpp]
tokens: [text, secondary]
used-in: [fx-rack, track-lanes, mixer, channel-strip]
snapshot: snapshots/knob-cell.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Knob cell

## Purpose
The one way a knob is shown with its meaning: knob, then caption, then value, stacked.

## Anatomy
A [knob](knob.md); under it the caption (11 px `text`) and under that the value with its unit
(10 px `secondary`). A toggle or hold button can replace the knob in macro cells.

## Metrics
Fixed pitch 52 pixels (56 for dB and ms values); pinned macro cells 56 × 64; rows 8 pixels apart.

## Behaviour
Double-click the value line to type a value (a [text field](text-field.md) in place). Values read in
the parameter's units; a macro mapping several parameters reads as a percentage.

## Do / Don't
- Don't put the caption beside the knob or the value beside the caption.

## Decision (UI10.4)
No component: a knob cell is a kit knob plus `juce::Label`s laid out by its host, because every host wraps the slider (automation menu, reset, mapping). The channel strip and mixer show knob + value (the section heading names trim, pan and width); rack nodes and macros show knob + caption + value at the 52/56 pitch.
