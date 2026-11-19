# T30 — Piano Roll Repeated Point Focus IDs

Status: Complete
Completed: 2026-11-19
Created: 2026-11-19
Surface: T10 piano roll, controller lanes
Depends on: T10

## Problem

In a repeated MIDI clip, the piano roll projects one source event onto every repeat.
All projections share the source `EventId`; only `tick` differs. The velocity and
controller lane focus widgets built their egui id from the event id and the controller
number, without the tick:

```rust
ui.id().with(("piano-roll-point", event.id, cc))
```

Projected points with the same velocity therefore received the same egui id in one
frame. In debug builds egui's id-clash detector painted the red "First use of widget
ID" / "Second use of widget ID" overlay across the lane, and keyboard focus could not
distinguish the repeat points. Note widgets were unaffected because their id already
includes the tick.

## Requirement

Each rendered velocity or controller point must have an egui id unique within its
lane. Repeated projections of one source event stay one editable source, but their
focus widgets must be distinct per tick.

## Scope

- Add the projected tick to the point widget id.
- Cover the repeated-clip projection with an automated regression.
- No change to selection, editing, playback, or the repeat model: selecting a repeat
  still selects the source event.

## Acceptance criteria

- A repeated clip whose source projects to several ticks produces one distinct focus
  id per projected point.
- Existing piano roll and controller lane behavior is unchanged.
- `cargo fmt`, `cargo check`, `cargo test`, and `cargo clippy` pass.

## Verification — 2026-11-19

- Regression `repeated_clip_projections_get_distinct_point_focus_ids` in
  `crates/ui/src/piano_roll.rs` builds a clip, resizes it with `repeat: true`, confirms
  that all four projections share one source `EventId`, and asserts that their point
  focus ids are distinct. With the tick removed again the same test fails
  (one id for four projections), so it guards the exact defect.
- `cargo fmt --all --check`, `cargo check --locked`, `cargo test --locked` (124 passed,
  12 ignored), and `cargo clippy --all-targets --locked -- -D warnings` passed.
- Not verified by hand in the running app: the fix removes the egui id clash by
  construction, and the regression proves the id uniqueness the detector checks.
- The code graph was refreshed with `graphify update .`; documentation nodes were not
  re-extracted because no semantic backend key is configured.

## Decisions

- 2026-11-19: fix at the id construction site instead of suppressing the egui
  warning or disabling `warn_on_id_clash`; the warning exposed a real focus-identity
  defect.

## Completion — 2026-11-19

The user asked to verify the red overlay in the velocity lane and then to proceed with
the proposed fix. `point_id` in `crates/ui/src/primitives/piano_roll.rs` now includes
the projected tick, and both velocity and controller lanes build their focus widgets
through it. The T10 follow-up note links back here.
