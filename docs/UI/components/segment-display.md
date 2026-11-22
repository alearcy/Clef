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

## Clef T41 round 3 — two-row readout

[T41 round 3](../../specs/T41-session-window-layout.md#round-3--gui-review-2026-11-22) (S12):

```rust
pub fn two_row_readout(ui: &mut egui::Ui, name: &str, rows: [(&str, &str); 2]) -> egui::Response;
```

Two rows of small seven-segment digits (5.5 × 8, 1.4-point strokes, 8-point advance) in one
recessed well 28 points high (`toolTarget`, the transport row), each row right-aligned before a
9-point `secondary` unit caption. Unlit ghosts are omitted at this size. The transport uses it for
the project format: sample rate on top (`48.0 kHz`, dashes while audio is off), bit depth below
(`24 bit`, `32 float`); a click opens Project Format. Accessible: a label named `name` whose value
reads both rows with their units.

**Evidence:** `snapshots/egui/t41-*-{1024,760}.png`; unit test
`the_format_readout_reads_kilohertz_and_bits_as_digits`.
