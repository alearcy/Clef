---
name: Chip
kind: component
reference: composer::style::LookAndFeel (toggled TextButton), ModChip, TextToggle
status: shared
impl: planned
reference-source: [Source/Arrangement.cpp, Source/Mixer.cpp, Source/MainComponent.cpp] (callers)
tokens: [field, text, accent, secondary, background]
used-in: [track-header, mixer, fx-rack, arrangement, transport-bar, window]
snapshot: snapshots/shared-controls.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Chip

## Purpose
A small toggle that shows or hides something attached to its owner and says how much there is.

## Anatomy
Short text in 10–11 px on a kit `TextButton`, 20 high: "A" / "A n" (automation lanes, n = parameters
with a curve), "T n" (take lanes, two or more takes), "◆N" (pinned macro lane), "LIST" (playlist),
"MAP", and in the transport "1 BAR" (count-in) and "READ". Off: `field`. Lit while what it opens is
shown (or the mode is on): `peacock` fill (`buttonOnColourId`) with `text`. BROWSER and the
channel-strip toggle in the status row are flat text toggles (`text`, `accent` while shown). The modulation **~ chip** is the exception: solid `accent` with a
`background` glyph so it reads as a button.

## Metrics
Header widths: ◆N 30, A n 32, T n 30. Height 20.

## Behaviour
Each chip invokes a registry command; when its column is missing (narrow header) the overflow and
the track menu offer the same command.

## Do / Don't
- Don't use a colour other than peacock for "on"; oxblood and yellow belong to the mute and solo lamps.

## Kit
A toggle chip is a plain `juce::TextButton` under the kit `LookAndFeel` (field off, peacock while toggled); no separate class is needed. The modulation chip is `composer::style::ModChip`. Flat status toggles (BROWSER, EDITOR · MIXER · FX RACKS) are `composer::style::TextToggle`.
