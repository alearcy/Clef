# T32 — Piano Roll Lane Id Stability

Status: Complete
Completed: 2026-11-19
Created: 2026-11-19
Surface: T10 piano roll, lane stack
Depends on: T10, T30, T31

## Problem

With the piano roll open, clicking one arrangement clip and then another painted a red
border around the editor for one frame. It was not an egui id clash: the border came from
egui's debug `warn_if_rect_changes_id` heuristic, which draws a 2-pixel red rectangle when
the same rect is occupied on consecutive frames by different widget ids, one of which
disappeared.

Every egui `Ui` is registered as a widget rect. The piano roll wrapped each clip lane in
`ui.push_id(clip.id())`. Switching the edited clip replaced that lane `Ui` with a new id at
the same rect, so the heuristic reported it even though the ids were intentionally
different.

## Requirement

The clip-lane slot must keep a stable id while the piano roll is open. Per-clip identity
must move into the widgets that actually edit clip data, so switching clips cannot make one
clip's note or value focus widget inherit another clip's identity.

## Approved Behavior

- A lane slot is keyed by its stack position, not by the clip id.
- Note and velocity/controller point focus ids include the clip id.
- Selection, editing, playback, and the visible layout do not change.
- The egui warning stays enabled; the instability is removed rather than suppressed.

## Acceptance Criteria

- Clicking clip A, then clip B, then clip A in the arrangement while the piano roll is open
  paints no widget-rect warning on any frame.
- Note and point focus ids are unique per clip even when two clips project the same
  `EventId` at the same tick.
- Existing piano roll tests and workspace gates pass.

## Verification — 2026-11-19

- New app-level regression `switching_arrangement_clip_does_not_flash_a_widget_rect_warning`
  renders the full surface, locates both arrangement clips from their painted rects,
  clicks A, B, A with real pointer events, and asserts that no 2-pixel red warning shape is
  painted. It fails when the lane uses `clip.id()` and passes with the slot id.
- `cargo fmt --all --check`, `cargo check --locked`, `cargo test --locked` (128 passed,
  12 ignored), and `cargo clippy --all-targets --locked -- -D warnings` passed.
- Not exercised by hand in the running app after the change; the regression reproduces the
  click sequence and the one-frame shape.
- Debug instrumentation used during the investigation (logging, extra egui options, clip
  prints, temporary tests, dev dependency) was removed.

## Implementation — 2026-11-19

- `PianoRoll::show` uses `ui.push_id(("piano-roll-lane", index))` for the lane slot.
- `note_focus` takes the clip id and includes it in its widget id.
- `point_id` takes the clip id; `ValueFocus` carries it for velocity and controller points.

## Decisions

- 2026-11-19: key the lane slot by stack position and salt data widgets with the clip id,
  instead of disabling `warn_if_rect_changes_id`.
