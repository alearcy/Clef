---
name: Glyph toggle
kind: component
reference: composer::style::GlyphToggle
status: shared
impl: gallery
tokens: [field, peacock, text, secondary, disabled, controlEdge]
used-in: [midi-functions, tool-row]
snapshot: snapshots/egui/t18-glyph-toggle-1024.png
api: ui::primitives::glyph_toggle::glyph_toggle
evidence: [t18-glyph-toggle]
---

> **Migrated to Rust/egui (T18 Phase 3, 2026-11-21).** The Clef section at the end of this file is
> the implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

# Glyph toggle

## Purpose
A glyph-only toggle on a raised control body, where a word would waste room: the triplet and dotted
feels (D102, D103). Two of them side by side replace a Straight · Triplet · Dotted choice: at most one
lit, neither lit means straight (composer, 2026-11-12).

## Anatomy
A raised `field` body (`paintControlBody`) with a 14-pixel [icon](icons.md) centred: `Icon::triplet`
(a bracket over a 3) or `Icon::dotted` (a note with its dot).

## Metrics
24 × 22 inside a `controlHeight` row; 4 pixels between a pair.

## States
At rest `secondary` glyph; hover lifts the glyph to `text` and the body to `field`, without an edge; lit is a
`peacock` body with a `text` glyph (no top light); disabled is flat with a `disabled` glyph.

## Behaviour
The host owns the state and the exclusivity of a pair. The accessible name is the tooltip ("Triplet",
"Dotted"); hosts with a command add its live binding to the tooltip.

## Do / Don't
- Do use it for a feel or another binary choice with a well-known glyph.
- Don't use it where a word is clearer; never paint the glyph inline in an app component.

## Code
`style::GlyphToggle triplet { style::Icon::triplet, "Triplet" };` in `UI/ComposerStyleKit/Source/ComposerStyle.h`.

## Clef Rust/egui — T18 Phase 3

**API**

```rust
pub fn glyph_toggle(ui: &mut egui::Ui, icon: Icon, name: &str, lit: bool, enabled: bool) -> egui::Response;
```

24 × 22 raised body through `paint::control_body`, a 14-point [icon](icons.md) centred: `secondary`
at rest, `text` on hover, `peacock` body with `text` glyph when lit, flat with a `disabled` glyph when
disabled. `name` is the accessible name and tooltip; the lit state is exposed as selected. The host
owns the state and the exclusivity of a pair.

**States:** rest, lit, disabled, disabled lit (triplet and dotted). **Adoption:** none — Clef has no
triplet/dotted feel yet, and its tool-row toggles (Follow, MIDI Functions) are words, which this
component's own rule keeps as words (T18 Phase 3 finding).

**Evidence:** gallery `snapshots/egui/t18-glyph-toggle-{1024,760}.png` and `t18-glyph-toggle-1024@2x.png`
(Retina); accessible names checked by `gallery::tests::every_gallery_control_has_an_accessible_name`.
