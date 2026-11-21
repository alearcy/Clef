---
name: Tabs
kind: component
reference: composer::style::TextToggle (since UI10.3; drawTabButton kept for JUCE tab bars)
status: shared
impl: gallery
tokens: [panel, field, text, secondary, accent]
used-in: [window]
snapshot: snapshots/egui/t18-tabs-1024.png
api: ui::primitives::tabs::tabs
evidence: [t18-tabs]
---

> **Migrated to Rust/egui (T18 Phase 3, 2026-11-21).** The Clef section at the end of this file is
> the implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

# Tabs

## Purpose
Switches the lower panel between Editor, Mixer and FX Racks. The three tabs sit centred in the
status row at the bottom of the window.

## Metrics
Text width plus 20 pixels × 26 each, 13-px (`body`) text.

## Current (UI10.3, 2026-11-11)
The tabs become flat text like BROWSER: EDITOR · MIXER · FX RACKS in 13-px (`body`) uppercase, `secondary`
off, `accent` for the shown panel, `text` on hover, a 1 × 12-pixel `divider` between items, 26-pixel
hit targets with 10-pixel padding, keyboard focus an `accent` edge.

## Legacy tab painter (drawTabButton)
Front tab `peacock` with `text`; others `panel` with `secondary`, `field` on hover; keyboard focus an
`accent` edge. Each tab is a registry command.

## Clef Rust/egui — T18 Phase 3

**API**

```rust
pub fn tabs(ui: &mut egui::Ui, name: &str, labels: &[&str], selected: &mut usize) -> bool;
```

Flat uppercase `body` text, `secondary` off, `accent` for the shown item, `text` on hover; a 1 × 12
`divider` between items; `TAB_HEIGHT` targets with 10 points of padding; keyboard focus an `accent`
edge. ←/→ on a focused tab move the focus and select the neighbour (wrapping). Returns true when
the selection changed. Each tab is a selectable node named by its label with its selected state;
selection is text colour plus the accessible state, never colour alone in assistive output.

**States:** first shown, middle shown, hover, focus. **Adoption:** none yet — no Clef window has a
tab row (T18 Phase 3 decision: gallery only).

**Evidence:** gallery `snapshots/egui/t18-tabs-{1024,760}.png` and `t18-tabs-1024@2x.png`
(Retina); accessible names checked by `gallery::tests::every_gallery_control_has_an_accessible_name`.
