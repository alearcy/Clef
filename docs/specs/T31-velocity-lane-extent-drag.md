# T31 — Velocity Lane Note Extent and Drag Feel

Status: Complete
Completed: 2026-11-19
Created: 2026-11-19
Surface: T10 piano roll, velocity lane
Depends on: T10, T30

## Problem

The velocity lane draws one vertical stem per note, so a note's extent is invisible.
Dragging a velocity bar also felt wrong: the value snapped to the pointer height at
drag start, the previous bar stayed painted under the preview, and the first drag after
opening the piano roll was the worst because the grabbed value was not the pointer's.

## Interview Decisions — 2026-11-19

- The horizontal line in the velocity lane marks the **note duration**: it runs from the
  note start to the note end at the note's velocity height, like the supplied reference.
- Drag symptoms reported by the user: the value jumps as soon as the drag starts, the old
  and new bar are visible together, and the value does not follow the pointer
  continuously. The jump is worst on the first drag after opening.

## Approved Behavior

- Velocity keeps its vertical bar. Each note also paints a thin horizontal segment from
  its start to its projected end, at the velocity height, in the same color as the bar
  (accent when selected).
- The horizontal segment is clipped to the lane. It follows the note's projected end, so
  trimmed, split, and repeated notes end where they actually sound.
- Only the velocity lane gets the extent line. CC lanes keep their current curves, points,
  and steps.
- Dragging a velocity point changes the value relative to the grab point: the value does
  not change until the pointer moves, then follows the vertical pointer delta. The
  grabbed value stays under the pointer instead of jumping to the pointer height.
- During a drag the lane paints one set of values, the preview, not the old values plus
  the preview.
- The lane repaints while a drag gesture is active so the preview tracks the pointer
  continuously.
- Values clamp to MIDI bounds (velocity 1–127, controllers 0–127). One drag is still one
  undoable edit on release.

## Scope

- `paint_values` velocity rendering and the velocity drag path in the piano roll.
- Regression tests for the note extent geometry and relative drag.
- Documentation and the T10 gallery snapshots for the velocity lane.

## Out of Scope

Changing the velocity scale, colors, lane height, selection model, CC editing semantics,
or adding insertion handles.

## Acceptance Criteria

- Grabbing a velocity bar at any height leaves the value unchanged until the pointer
  moves; moving up increases and moving down decreases by the pointer delta.
- While dragging, only the preview bar and its extent line are drawn.
- Each velocity bar has a horizontal extent line ending at the note end, clipped to the
  visible lane.
- CC lanes are unchanged.
- Automated tests, workspace gates, and regenerated T10 snapshots pass.

## Verification — 2026-11-19

- `velocity_extent_runs_from_the_note_start_to_its_projected_end` checks that the
  extent starts at the note tick, ends at the projected note end, stays at the velocity
  height, and clips at the lane edge.
- `velocity_drag_keeps_the_grabbed_value_until_the_pointer_moves` checks that grabbing
  below the tip does not change the value, that a 10-point move changes it by the
  pointer delta, and that it clamps to the MIDI range.
- `velocity_drag_previews_relative_to_the_grab_and_edits_once_on_release` drives the
  real lane with egui pointer events: no action is dispatched while dragging, and the
  release edit moves velocity 83 to 103 after a 10-point upward drag from a mid-stem
  grab, proving the value follows the grab offset instead of the pointer height.
- `cargo fmt --all --check`, `cargo check --locked`, `cargo test --locked` (127 passed,
  12 ignored), and `cargo clippy --all-targets --locked -- -D warnings` passed.
- The T10 gallery was regenerated at 760 and 1024 points; the velocity snapshots show
  the extent lines.
- Not exercised by hand in the running app: the drag feel and the first-drag behavior
  remain a user acceptance check.

## Implementation — 2026-11-19

- `paint_values` draws a one-pixel extent line from the note start to `event.end` for
  velocity only, in the bar color, clipped by the lane painter.
- `Gesture::Value` stores the press height; `velocity_dragged` turns the vertical pointer
  delta into a value anchored at the grabbed value. CC points use the same relative
  value path while keeping their absolute tick movement.
- The lane paints once per frame: the preview replaces the bar set instead of drawing
  over it. An active gesture requests a repaint.

## Decisions

- 2026-11-19: relative dragging instead of absolute pointer height; it preserves the
  grabbed point and removes the first-drag jump without changing the value mapping.

## Completion — 2026-11-19

The user asked for a note-extent line in the velocity lane and for a smoother velocity
drag after reporting the first-drag jump and jitter. Both landed with three focused
tests, regenerated snapshots, and the documentation update.
