---
name: Segmented control
kind: component
reference: composer::style::SegmentedControl
status: mixed
impl: gallery
reference-source: [Source/AutomationControls.cpp (Region | Track), Source/RackEditor.cpp (node modes)]
tokens: [field, peacock, text, secondary, controlEdge]
used-in: [side-drawers, settings, track-lanes, fx-rack]
snapshot: snapshots/egui/t18-segmented-1024.png
api: ui::primitives::segmented::Segmented
evidence: [t18-segmented]
---

> **Migrated to Rust/egui (T18 Phase 2, 2026-11-21).** The Clef section at the end of this
> file is the implementation contract. Earlier sections keep the legacy design intent; their C++
> symbols are not in this repository.

# Segmented control

## Purpose
An exclusive choice among two to four options shown at once: Tracks | Racks | Plugins, Region |
Track, Show chooser | Reopen last | Start empty, LFO shapes, Arpeggiator modes.

## Anatomy
A `field` well holding touching segments; the lit segment is `peacock` with a `foregroundFor`
label, the others `secondary` text. Segments may be glyphs (LFO shapes) with the name in the tooltip.

## Metrics
Row height of its context (22–24). Equal-width segments unless labels differ a lot.

## States
hover lifts the label to `text`; disabled segments dim and are skipped by the keyboard; a segment
that applies only in another mode stays in place, dimmed, with a hint ("Notes mode only").

## Behaviour
Each segment is a registry command or a session edit with undo, as its owner defines.

## Do / Don't
- Don't use more than four segments; use a [menu button](menu-button.md).

## Adoption
The Browser segments and Settings › Startup use the kit control since UI10.3 (Startup used to light `field` instead of peacock). The automation lane's Region | Track pair and the rack node mode rows still lay out their own TextButtons, painted by the same kit LookAndFeel colours; they move with their surfaces.

## Code
```cpp
composer::style::SegmentedControl segments { { "Tracks", "Racks", "Plugins" } };
segments.onSelect = [this](int i) { commands.invoke(segmentCommands[i]); };
segments.setSelected(shown); // the host reports the value back
```

Recessed (UI10 Fix 7): the track has the same body height as a raised button; the lit segment is a flat `peacock` fill inside it with an inner top shadow and a 1-pixel bottom light (`ControlDepth::recessed`); hover lifts the body to `field`, focus keeps the `accent` edge. See control depth in [foundations](../foundations.md).

## Clef Rust/egui — T18 Phase 2

**impl:** gallery. Adopted by Settings › Applies to (All projects | This project). Settings ›
Startup does not exist in Clef yet.

**API**

```rust
pub struct Segmented<'a, T: Copy + PartialEq>;
impl<'a, T> Segmented<'a, T> {
    pub fn new(name: &'a str, value: &'a mut T, options: &'a [(T, &'a str)]) -> Self; // 2–4 options
    pub fn enabled(self, enabled: bool) -> Self;
    pub fn show(self, ui: &mut Ui) -> Response; // changed() when the value changed
}
```

Accessible name: the group is `name`; each segment is a radio button named "{name}: {label}"
with its selected state.

**Anatomy**

| Region | Size | Fill | Edge | Text |
| --- | --- | --- | --- | --- |
| Track | equal segments × widest label + 2 × `controlPadding`, `controlHeight` 24 | recessed `field` (control depth) | — | — |
| Lit segment | one cell | `peacock` | — | `BODY`, `foreground_on(peacock)` |
| Other segments | one cell | none | — | `BODY`, `secondary` |

**States**

| State | Change |
| --- | --- |
| rest | unlit labels `secondary` |
| hover | the hovered label lifts to `text` |
| lit | `peacock` cell, `foreground_on` label |
| focused (keyboard) | 1-pt `accent` edge on the focused cell |
| disabled | flat `panel` body with `divider` edge, labels `disabled`, lit cell `panel` |

**Behaviour:** click selects; segments are in the Tab order; ←/→ on a focused segment selects
its neighbour and moves focus. Each owner decides whether a change is a session edit, a setting
or a command.

**Tokens:** `field`, `peacock`, `text`, `secondary`, `disabled`, `panel`, `divider`, `accent`;
`controlHeight`, `controlPadding`, `cornerControl`; `BODY`.

**Usage** (`crates/ui/src/lib.rs`, Audio/MIDI Settings)

```rust
primitives::segmented::Segmented::new(
    "Applies to",
    &mut self.settings_project,
    &[(false, "All projects"), (true, "This project")],
)
.show(ui);
```

**Evidence:** gallery `t18-segmented`; `primitives::segmented::tests::click_and_arrows_select_and_each_segment_is_a_named_radio`.

**Do / Don't:** don't exceed four segments (debug assertion); don't use `selectable_value` or
`selectable_label` for an exclusive choice.
