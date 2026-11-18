---
name: Tap button
kind: component
reference: composer::style::TapButton (Icon::sendPre / sendPost)
status: shared
impl: planned
tokens: [text, accent]
used-in: [channel-strip, mixer]
snapshot: snapshots/shared-controls.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Tap button

## Purpose
A send's pre/post-fader switch as a glyph: the tap branches above the fader cap for pre, below it
for post. Toggle on = pre-fader.

## Metrics
12 wide in mixer send rows, 22 in the channel strip.

## Do / Don't
