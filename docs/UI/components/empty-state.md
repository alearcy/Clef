---
name: Empty state
kind: component
status: shared
impl: gallery
tokens: [secondary]
used-in: [piano-roll, mixer, arrangement]
snapshot: snapshots/egui/t41-empty-1024.png
api: ui::primitives::empty_state
evidence: [t41-empty, t41-no-clip]
---

# Empty state

## Purpose
Tells the user a view has nothing to show yet, and what to do. [T41 round 3](../../specs/T41-session-window-layout.md#round-3--gui-review-2026-11-22) (S9).

## Anatomy
A 48-point glyph in `secondary` above one line of 13-point `secondary` body text, 12 points apart,
the pair centred horizontally and vertically in the area it is given.

## Clef Rust/egui

```rust
pub fn paint(ui: &egui::Ui, rect: egui::Rect, glyph: Icon, text: &str);
pub fn fill(ui: &mut egui::Ui, min_height: f32, glyph: Icon, text: &str);
```

`paint` draws into a rectangle the caller owns; `fill` allocates the rest of the `Ui` first. The text
is exposed as a label for assistive technology.

| Where | Glyph | Text |
| --- | --- | --- |
| Editor, no MIDI clip selected | `Icon::Keys` | Select a MIDI clip in the arrangement. |
| Mixer, no tracks or buses | `Icon::Faders` | No tracks yet. |
| Arrangement, no instrument tracks | `Icon::Lanes` | No tracks yet. |

**Evidence:** `snapshots/egui/t41-empty-{1024,760}.png`, `t41-no-clip-{1024,760}.png`.
