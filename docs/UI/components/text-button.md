---
name: Text button
kind: component
reference: composer::style::LookAndFeel::drawButtonBackground / drawButtonText
status: shared
impl: planned
tokens: [field, peacock, text, disabled, controlEdge]
used-in: [settings, startup, side-drawers, fx-rack, transport-bar, callout]
snapshot: snapshots/settings.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Text button

## Purpose
A flat labelled button for actions that need words (Open…, Replace…, Store A, TAP, READ).

## Anatomy
`juce::TextButton` under the kit `LookAndFeel`: `field` fill, 2-pixel corners, label in `text` at
body or small size. **Primary** variant: `peacock` fill with `foregroundFor` label (one per surface,
e.g. "Open ↩" in the chooser).

## Metrics
Height `controlHeight` (24), or 22–26 inside rows. Horizontal padding 8. TAP and READ are 44 × 24.

## States
hover lifts the fill; down darkens; disabled `disabled` label; a toggle-style text button is lit
with `peacock` fill (READ while automation read is on).

## Do / Don't
- Do use a [tool button](tool-button.md) when a glyph says it; a text button only when words are
  needed.
- Don't use it for an option among a few: that is a [segmented control](segmented-control.md).
- Don't colour it with a track colour.

Raised (UI10 Fix 7): a body 2 px shorter than its bounds with a 1-pixel top light and a soft 2-pixel shadow under it (`paintControlBody`, `ControlDepth::raised`); hover adds the `controlEdge` edge; lit or pressed keeps the body and shadow and drops the top light; disabled is flat `panel` with a `divider` edge. See control depth in [foundations](../foundations.md).

## Clef Startup Variant — T07

`primitives::project::button` uses the shared 24-point button metrics. The enabled primary
action uses `peacock`, secondary actions use `field`, and disabled actions expose their
disabled state through AccessKit. Each button dispatches a shared project command.
See [New Project](../snapshots/egui/t07-startup-empty-1024.png),
[disabled Open](../snapshots/egui/t07-startup-missing-760.png) and
[Recover](../snapshots/egui/t07-recovery-ready-1024.png).
