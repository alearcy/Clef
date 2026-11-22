---
name: Detach button
kind: component
reference: composer::style::DetachButton
status: shared
impl: gallery
api: ui::primitives::panel_controls
tokens: [text, accent]
used-in: [fx-rack, ara-editor, piano-roll, mixer]
snapshot: snapshots/shared-controls.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Detach button

## Purpose
Opens the current panel in its own resizable window: a frame with an arrow leaving its top right.

## States
Toggled on (the panel is detached, D87's ARA editor): the arrow's head is at the frame's centre, pointing back
in: the button attaches the panel again. The tooltip says which ("Open the editor in its own window", "Attach the
editor to the panel").

## Clef Rust/egui — T41 round 3 lower-panel controls

[T41 round 3](../../specs/T41-session-window-layout.md#round-3--gui-review-2026-11-22) (S2). `primitives::panel_controls::at_right(ui, row, state, keyboard) -> Option<PanelControl>`
draws three [tool buttons](tool-button.md) right-aligned in the Editor's and the Mixer's tool row:
**Detach** (`Icon::Detach`, "Detach into Own Window"; in the detached window `Icon::Attach`,
"Attach to Main Window"), **Half / Full** (`Icon::PanelFull`, lit `accent` while full; absent in the
detached window) and **Close** (`Icon::Close`). The same actions are registry commands
`view.panel-detach` and `view.panel-full` (View menu). The detached window is an egui viewport
(`show_viewport_immediate`); closing it closes the panel. Panel state is window-session only.
