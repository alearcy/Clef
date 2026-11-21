---
name: Range overlay
kind: component
reference: composer::style::paintRangeOverlay
status: shared
impl: gallery
tokens: [accent]
used-in: [arrangement, piano-roll, mixer, track-lanes, fx-rack]
snapshot: snapshots/egui/t18-range-overlay-1024.png
api: ui::primitives::range_overlay::range_overlay
evidence: [t18-range-overlay]
---

> **Migrated to Rust/egui (T18 Phase 3, 2026-11-21).** The Clef section at the end of this file is
> the implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

# Range overlay

The one marquee and range selection look: `accent` at about 12 % with an 80 % edge. Used for part
marquees, lane ranges, take comps, node marquees and mixer strip ranges. A readout chip (10 px,
`accent`) may ride on it ("Take 2 · 12.1.000 → 13.3.000"). Never a foreign selection hue.

## Clef Rust/egui — T18 Phase 3

**API**

```rust
pub fn range_overlay(painter: &egui::Painter, range: egui::Rect, readout: Option<&str>);
```

`accent` at 12 % with a 1-point `accent` edge at 80 %; an optional `caption` `accent` readout chip on a
`background` 85 % knock-out at the top-left corner. Paint-only: the host owns the gesture and the
accessible description of the selection.

**States:** marquee, with readout. **Adoption:** the arrangement and piano-roll marquees
(`timeline::marquee`). The loop band keeps `LOOP_ACTIVE` (foundations, lamps), so it is not a range
overlay.

**Evidence:** gallery `snapshots/egui/t18-range-overlay-{1024,760}.png` and `t18-range-overlay-1024@2x.png`
(Retina); accessible names checked by `gallery::tests::every_gallery_control_has_an_accessible_name`.
