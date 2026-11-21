---
name: Segment display
kind: component
reference: composer::style::paintSegmentDisplay, segmentsFor
status: shared
impl: gallery
tokens: [text, danger]
used-in: [transport-bar]
snapshot: snapshots/egui/t18-segment-display-1024.png
api: ui::primitives::segment_display::segment_display
evidence: [t18-segment-display]
---

> **Migrated to Rust/egui (T18 Phase 3, 2026-11-21).** The Clef section at the end of this file is
> the implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

# Segment display

## Purpose
The transport's position counters, read as one hardware counter (D57).

## Anatomy
Soft-white seven-segment digits over faint unlit "8" ghosts in a dark recessed well, right-aligned;
narrow separators for `.` `:` `;`.

## States
`text` normally, `danger` only while the transport records. While typed into, the primary slot
shows a plain [text field](text-field.md).

## Behaviour
At the minimum window width the secondary (time) counter yields to the glyphs and returns when it
fits; the bars counter always stays.

## Clef Rust/egui — T18 Phase 3

**API**

```rust
pub fn segment_display(ui: &mut egui::Ui, name: &str, text: &str, recording: bool, min_width: f32) -> egui::Response;
```

Seven-segment digits (9 × 16, 1.8-point strokes, 12-point advance) in `text` over unlit "8" ghosts at
8 %, right-aligned in a recessed well (`paint::control_body`, `CONTROL_HEIGHT` high); `.` and `:` are
6-point separators, `-` the middle segment. `danger` replaces `text` only while the transport
records. Accessible: a label node named `name` whose value is the plain text.

**States:** stopped, running, recording, time. **Adoption:** the transport's bar.beat.tick counter
(`primitives::transport::counter`); the count-in state is a `Count-in` caption beside it, not a colour.
Typing into the counter is not part of this phase.

**Evidence:** gallery `snapshots/egui/t18-segment-display-{1024,760}.png` and `t18-segment-display-1024@2x.png`
(Retina); accessible names checked by `gallery::tests::every_gallery_control_has_an_accessible_name`.
