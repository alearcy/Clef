---
name: Checkbox
kind: component
reference: composer::style::LookAndFeel::drawToggleButton
status: shared
impl: gallery
tokens: [field, accent, text, controlEdge]
used-in: [fx-rack, settings]
snapshot: snapshots/egui/t18-checkbox-1024.png
api: ui::primitives::checkbox::checkbox
evidence: [t18-checkbox]
---

> **Migrated to Rust/egui (T18 Phase 3, 2026-11-21).** The Clef section at the end of this file is
> the implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

# Checkbox

## Purpose
A boolean setting with a visible label ("Skip first note", settings switches).

## Anatomy
`juce::ToggleButton`: a small `field` box with a `controlEdge` edge, an `accent` check when on, the label
to its right in `text`.

## Behaviour
Changing it invokes a registry command (one per row when it belongs to a row, as in the Articulation
Split), so it can be bound to a key; the tooltip shows the live binding.

## Do / Don't
- Don't use a checkbox for a mode with more than two values; use a segmented control.

The box keeps its 1-pixel `controlEdge` edge (`divider` while disabled). See control depth in [foundations](../foundations.md).

## Clef Rust/egui — T18 Phase 3

**API**

```rust
pub fn checkbox(ui: &mut egui::Ui, label: &str, value: &mut bool, enabled: bool) -> egui::Response;
```

A 14-point `field` box with a `controlEdge` edge (`divider` while disabled), the [check icon](icons.md)
in `accent` when on (`disabled` while disabled), the label in `body` `text` 6 points to the right; hover
lifts the box to the hover fill; keyboard focus draws an `accent` edge around the box. A click or
Space/Return flips `value`. Accessible: checkbox role, the label as name, the checked state.

**States:** off, on, disabled on. **Adoption:** none yet — no Clef surface has a boolean option
(T18 Phase 3 decision: gallery only).

**Evidence:** gallery `snapshots/egui/t18-checkbox-{1024,760}.png` and `t18-checkbox-1024@2x.png`
(Retina); accessible names checked by `gallery::tests::every_gallery_control_has_an_accessible_name`.
