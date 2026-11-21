---
name: Text button
kind: component
reference: composer::style::LookAndFeel::drawButtonBackground / drawButtonText
status: shared
impl: gallery
tokens: [field, peacock, text, disabled, controlEdge]
used-in: [settings, startup, side-drawers, fx-rack, transport-bar, callout]
snapshot: snapshots/egui/t18-text-button-1024.png
api: ui::primitives::text_button::TextButton
evidence: [t18-text-button, primitives::text_button::tests]
---

> **Migrated to Rust/egui (T18, 2026-11-21).** The Clef section at the end of this file is the
> implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

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

Raised (UI10 Fix 7): a body 2 px shorter than its bounds with a 1-pixel top light and a soft 2-pixel shadow under it (`paintControlBody`, `ControlDepth::raised`); hover lifts the body to `field` without an edge; lit or pressed keeps the body and shadow and drops the top light; disabled is flat `panel` with a `divider` edge. See control depth in [foundations](../foundations.md).

## Clef Startup Variant — T07

`primitives::project::button` uses the shared 24-point button metrics. The enabled primary
action uses `peacock`, secondary actions use `field`, and disabled actions expose their
disabled state through AccessKit. Each button dispatches a shared project command.
See [New Project](../snapshots/egui/t07-startup-empty-1024.png),
[disabled Open](../snapshots/egui/t07-startup-missing-760.png) and
[Recover](../snapshots/egui/t07-recovery-ready-1024.png).

## Clef Rust/egui — T18

**API**

```rust
pub struct TextButton<'a>; // builder
impl<'a> TextButton<'a> {
    pub fn new(label: &'a str) -> Self;
    pub fn primary(self, primary: bool) -> Self; // peacock fill, one per surface
    pub fn enabled(self, enabled: bool) -> Self;
    pub fn toggle(self, on: bool) -> Self;       // lit with peacock while on
    pub fn tooltip(self, tooltip: &'a str) -> Self;
    pub fn shortcut(self, shortcut: &'a str) -> Self; // "{tooltip} ({shortcut})", omitted when empty
    pub fn small(self) -> Self;                  // SMALL (11) text inside dense rows
    pub fn min_width(self, width: f32) -> Self;
    pub fn accessible(self, name: &'a str) -> Self;
    pub fn show(self, ui: &mut egui::Ui) -> egui::Response;
}
```

**States** (all through `paint::control_body`, raised)

| State | Body | Label |
| --- | --- | --- |
| rest | `FIELD`, top light, under-shadow | `TEXT` |
| hover | `paint::hover_fill()` (field lifted toward `CONTROL_EDGE`) | `TEXT` |
| pressed | lifted fill, top light dropped | `TEXT` |
| keyboard focus | 1-point `ACCENT` edge (keyboard navigation only) | unchanged |
| disabled | flat `PANEL`, `DIVIDER` edge | `DISABLED`; AccessKit keeps the name |
| primary / toggle on | `PEACOCK`, top light dropped | `foreground_on(PEACOCK)` |

Width hugs the label plus `CONTROL_PADDING` (8) on each side; height `CONTROL_HEIGHT` (24), body 22.
Allocation happens in the caller's `Ui` (no child `Ui`), so wrapped rows wrap the button.

**Usage**

```rust
TextButton::new(title)
    .enabled(self.command_available(command, false))
    .tooltip(&description)
    .shortcut(&hint)
    .show(ui)
    .clicked()
```

**Evidence** — [gallery](../snapshots/egui/t18-text-button-1024.png); tests
`disabled_button_keeps_its_accessible_name`, `toggle_reports_its_state`,
`width_hugs_the_label_with_padding`. Used by every command button, dialogs, pickers and the startup
chooser (`primitives::project::button` delegates to it).
