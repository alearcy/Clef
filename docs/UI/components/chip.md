---
name: Chip
kind: component
reference: composer::style::LookAndFeel (toggled TextButton), ModChip, TextToggle
status: shared
impl: gallery
reference-source: [Source/Arrangement.cpp, Source/Mixer.cpp, Source/MainComponent.cpp] (callers)
tokens: [field, text, accent, secondary, background]
used-in: [track-header, mixer, fx-rack, arrangement, transport-bar, window]
snapshot: snapshots/egui/t18-chip-1024.png
api: ui::primitives::chip::chip
evidence: [t18-chip]
---

> **Migrated to Rust/egui (T18, 2026-11-21).** The Clef section at the end of this file is the
> implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

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

## Clef Rust/egui — T18

**API**

```rust
pub fn chip(ui: &mut egui::Ui, label: &str, name: &str, on: bool, enabled: bool) -> egui::Response;
```

20 high (`CHIP_HEIGHT`), `CAPTION` (10) text, raised `FIELD` body off, `PEACOCK` with
`foreground_on` label while on, disabled flat. `name` is the accessible name and tooltip; the state
is exposed as toggled. Used by the arrangement's take-lane chip ("T n").
