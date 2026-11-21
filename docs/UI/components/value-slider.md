---
name: Value slider
kind: component
status: shared
impl: gallery
tokens: [field, peacock, text, disabled, accent]
used-in: [transport-bar, edit-history, piano-roll, midi-functions]
snapshot: snapshots/egui/t18-value-slider-1024.png
api: ui::primitives::value_slider::ValueSlider
evidence: [t18-value-slider]
---

# Value slider

## Purpose

A bounded number with a unit that the user nudges, drags or types: tempo, bar and beats, tick
positions, the Edit History revision, MIDI function amounts. Created for Clef in T18 Phase 2;
there is no legacy reference.

## Clef Rust/egui — T18 Phase 2

**impl:** gallery. Adopted by the tempo fields (Bar, Tempo, Beats per bar), the tick fields of
the transport windows, the MIDI functions' numbers (`primitives::piano_roll::number`) and the
Edit History revision slider (track form).

**API**

```rust
pub struct ValueSlider<'a, T: egui::emath::Numeric>;
impl<'a, T> ValueSlider<'a, T> {
    pub fn new(name: &'a str, value: &'a mut T, range: RangeInclusive<T>) -> Self;
    pub fn step(self, step: f64, large: f64) -> Self;   // default 1 and 10
    pub fn default_value(self, default: T) -> Self;      // Option-click restores it
    pub fn unit(self, unit: &'a str) -> Self;            // "BPM", shown after the number
    pub fn width(self, width: f32) -> Self;              // default 88
    pub fn track(self) -> Self;                          // horizontal travel form
    pub fn enabled(self, enabled: bool) -> Self;
    pub fn show(self, ui: &mut Ui) -> Response;          // changed(), dragged(), drag_stopped()
}
```

Accessible node: a slider named `name` with its numeric value; the label and tooltip read
"{name}: {value} {unit}".

**Anatomy**

| Region | Size | Fill | Edge | Text |
| --- | --- | --- | --- | --- |
| Box form | `width` × `controlHeight` 24 | recessed `field` | `accent` focus edge | `BODY`, centred, `text` |
| Track form | `width` × `controlHeight` | recessed `field`, `peacock` up to the value | 2-pt `text` value line | — |
| Typing | the box | — | 1-pt `accent` | native text editing |

**States**

| State | Change |
| --- | --- |
| rest | value in `text` |
| hover / dragging | body unchanged; the value updates as it moves |
| focused (keyboard) | 1-pt `accent` edge |
| typing | the box becomes a text field with an `accent` edge |
| disabled | flat `panel` body, `divider` edge, value `disabled`; the track keeps only its value line |

**Behaviour**

- Box form: drag horizontally or vertically (right/up increases), one step per 4 points; Shift
  makes it four times finer. Double-click types a value: Return or a click away commits, Escape
  cancels; typed values clamp to the range. ↑/↓ with focus step (Shift is the large step).
  Option-click restores the default when one is set.
- Track form: the value follows the pointer from the press (preview on drag, the owner commits on
  release); ↑/→ and ↓/← step with focus. It has no typing or reset.
- Values are clamped to the range and rounded for integer types.

**Tokens:** `field`, `peacock`, `text`, `disabled`, `panel`, `divider`, `accent`;
`controlHeight`, `cornerControl`; `BODY`.

**Usage** (`crates/ui/src/primitives/transport.rs`)

```rust
ValueSlider::new("Tempo", &mut tempo.bpm, 20..=400)
    .unit("BPM")
    .default_value(120)
    .show(ui);
```

**Evidence:** gallery `t18-value-slider`; `primitives::value_slider::tests::arrows_step_option_click_resets_and_typing_commits_or_cancels`;
`primitives::history::tests::history_slider_previews_then_commits_on_release_and_supports_cancel`.

**Do / Don't:** don't use `egui::DragValue` or `egui::Slider` in a surface; don't show a value
without its unit when it has one.
