---
name: Project dialog
kind: component
status: app
impl: gallery
tokens: [panel, text, secondary, divider]
used-in: [startup]
snapshot: snapshots/egui/t07-recovery-ready-1024.png
---

# Project Dialog

## Purpose

Ask the user to recover, postpone or discard failed-session checkpoints. Use the same
modal to confirm removal of the selected session's copies.

## Anatomy

A centered egui modal with a title, a wrapping project identity and status area, and
a wrapping action row. The recovery state can include a scrolling choice of sessions.
The discard confirmation repeats the selected project identity.

## Metrics

The modal width uses up to 540 points of the available viewport, with an outer allowance
of 48 points. The panel uses the Composer Dark window style and its text uses the
startup typography roles.

## States

| State | Appearance and action |
| --- | --- |
| Recovery available | Project name and checkpoint time with Recover enabled. |
| Fallback | The usable generation's time and the error from the newest generation. |
| Unavailable | An error and disabled Recover. Later and Discard remain available. |
| Confirm discard | The selected project identity and Cancel or Discard Recovery. |

## Behaviour

The modal blocks background interaction. Escape returns to the chooser, or cancels the
discard confirmation. The surface owns the actions through the shared project commands.

## Usage

```rust
let dismissed = project::modal(ctx, |ui| {
    project::heading(ui, "Discard this recovery?");
    project::body(ui, "Clef will remove the autosave copies for this session.");
});
```

The [gallery](../snapshots/egui/) contains the four states at 1024 and 760 points.
`gallery::tests::startup_accessible_controls_fit_both_review_widths` checks the accessible
names and bounds of the rendered controls.
