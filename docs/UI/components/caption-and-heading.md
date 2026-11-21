---
name: Caption and heading
kind: component
reference: composer::style::paintSectionHeading, composer::style::font
status: mixed
impl: gallery
reference-source: [Source/ChannelStrip.cpp (pushHeading, caption), Source/RackEditor.cpp, Source/SettingsPanel.h]
tokens: [text, secondary]
used-in: [channel-strip, fx-rack, settings, side-drawers, startup]
snapshot: snapshots/egui/t18-caption-1024.png
api: ui::primitives::text::{heading, section, body, caption, unavailable, danger}
evidence: [t18-caption]
---

> **Migrated to Rust/egui (T18, 2026-11-21).** The Clef section at the end of this file is the
> implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

# Caption and heading

- **Section heading**: 11-px uppercase `secondary` with spacing above, no rule (channel strip
  sections, browser groups, menu section headers at 12 bold).
- **Caption**: 10–11 px `secondary` beside or above a field or field group ("Zones", "Low"), or the
  label column of a labelled row (70 pixels in the channel strip, 160 in settings).
- **Readout**: a value with its unit in `secondary` small text ("Latency: 128 smp / 2.90 ms").
- **Page heading**: 16 px `text`.


## Kit (UI10.4)
Section headings are painted by `paintSectionHeading` (the channel strip uses it). Captions stay `juce::Label`s in `secondary` small text; the channel strip's caption column is 62 pixels.

## Alignment
Captions, readouts and names are `juce::Label`s with a zero border (`setBorderSize({})`), so their text starts on the same left edge as the section headings and the controls; JUCE's default label border would indent them by a few pixels.

## Clef Startup Variant — T07

`primitives::project::{heading, body, caption}` use the 16-, 13- and 11-point foundation
styles. The local font family loads macOS SF Pro. Captions use `secondary` and wrap within the
available width. See [recent project details](../snapshots/egui/t07-startup-recent-1024.png)
and [recovery with fallback](../snapshots/egui/t07-recovery-fallback-760.png).

## Clef Rust/egui — T18

**API** — one function per role, each a non-selectable `egui::Label` in SF Pro:

| Function | Style | Colour |
| --- | --- | --- |
| `heading` | `HEADING` 16, bold | `TEXT` |
| `section` | `SMALL` 11, uppercase | `SECONDARY` |
| `body` | `BODY` 13 | `TEXT` |
| `caption` | `SMALL` 11 | `SECONDARY` |
| `unavailable` | `SMALL` 11 | `DISABLED` |
| `danger` | `BODY` 13 | `DANGER` |

`theme::install_fonts` also maps egui's `Heading`, `Body`, `Button` and `Small` text styles to these
sizes, so a plain `ui.label` renders as `body`. `primitives::project::{heading, body, caption}`
delegate here.
