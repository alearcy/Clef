# T33 — Piano Roll Selection Reveals the Edited Clip

Status: Complete
Completed: 2026-11-19
Created: 2026-11-19
Surface: T10 piano roll, arrangement selection
Depends on: T10, T32

## Problem

Double-clicking a clip opened the piano roll on it. A later single click on another clip
updated the lane name but left the grid empty.

Opening a clip set the horizontal offset to that clip's start. When the arrangement
selection replaced the edited clip afterwards, `PianoRoll::show` switched the active clip
through `activate` but kept the old offset. The new clip's notes were outside the visible
time range, so the grid looked empty even though the lane had changed.

## Requirement

When the arrangement selection replaces the clip being edited, the piano roll must show
that clip from its start. The same must hold when switching between clips far apart on the
timeline.

## Approved Behavior

- Selecting a different clip in the arrangement makes it the edit target and sets the
  horizontal offset to its start.
- The ruler, notes, velocity, and controller lanes all follow the same offset.
- Selection changes that keep the current clip active do not move the view; scroll position
  is preserved while editing.
- Double-click, keyboard open, and lane-target switching keep their existing behavior.

## Scope

- The selection-driven active-clip change in `PianoRoll::show`.
- A regression for two clips at different timeline positions.

## Out of Scope

Changing zoom, follow mode, multi-clip selection rules, or arrangement selection behavior.

## Acceptance Criteria

- Opening a clip at bar 7 and then selecting a clip at bar 2 shows the bar-2 clip from its
  start with its notes visible.
- The test fails when the offset is not reset and passes with the fix.
- Workspace gates pass.

## Verification — 2026-11-19

- New regression `switching_the_edited_clip_reveals_its_start` opens a clip at tick 1920,
  then selects a clip at tick 0, and asserts the active clip and offset both move to the
  new clip. With the offset reset removed it fails with offset 1920.
- `cargo fmt --all --check`, `cargo check --locked`, `cargo test --locked` (129 passed,
  12 ignored), and `cargo clippy --all-targets --locked -- -D warnings` passed.
- Not exercised by hand in the running app; the regression covers the state transition, and
  the reported empty grid was reproduced from the screenshot's ruler offset.

## Implementation — 2026-11-19

- When `show` activates the first selected clip because the previous active clip is no
  longer selected, it also sets `self.offset` to that clip's start.

## Decisions

- 2026-11-19: recenter on the new clip's start instead of preserving the old scroll offset;
  an edited clip whose notes are off-screen is not usable.
