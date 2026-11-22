---
name: Accordion section
kind: component
status: shared
impl: gallery
tokens: [text, secondary, accent]
used-in: [midi-functions]
snapshot: snapshots/egui/t41-functions-1024.png
api: ui::primitives::accordion::section
evidence: [t41-functions]
---

# Accordion section

## Purpose
A titled group that expands and collapses in place. [T41 round 3](../../specs/T41-session-window-layout.md#round-3--gui-review-2026-11-22) (S6).

## Anatomy
A full-width header `controlHeight` (24) high: a `secondary` chevron that turns from right (closed)
to down (open), then the title in 13-point `text`; hover fill on the header, an `accent` focus ring
in keyboard mode. The body follows, inset 6 points above and 10 below.

## Behaviour
A click toggles the section. Opening and closing animate the body's height over 120 ms with an
ease-out curve, the body clipped to the animated height. Several sections may be open at once; the
caller owns each section's open state.

## Clef Rust/egui

```rust
pub fn section<R>(ui: &mut egui::Ui, title: &str, open: &mut bool, body: impl FnOnce(&mut egui::Ui) -> R) -> Option<R>;
```

Accessible: a collapsing-header node named by the title, selected while open.
