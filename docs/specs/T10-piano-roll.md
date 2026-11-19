# T10 — Piano Roll and MIDI Editing

**Status:** Done — 2026-11-19. Specification and UI proposal approved on 2026-11-19;
implementation, automated verification, and the two product decisions recorded below are
complete.
**Priority:** P1.
**Date:** 2026-11-17
**Task:** [T10](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T06 note/clip model; T08 undo/redo.

## Goal

Provide the core MIDI writing surface: edit the notes of a clip with pointer and keyboard,
hear the result through the track instrument, and undo every change. MIDI writing is the
product's first priority, so this surface needs an approved design before implementation.

## Interview Decisions — 2026-11-19

- Dock the piano roll below the arrangement. When several clips are selected, show all of
  them on separate lanes stacked in that lower panel.
- Include Velocity, all MIDI CC numbers (0–127), adjustable-strength quantize, humanize,
  and transpose.
- Support linear interpolation and adjustable curvature between continuous CC points.
- Edit the repeat source. Changes apply to existing and future repetitions.
- Audition notes and keybed pitches through the owning instrument track while transport
  is stopped.

## Approved Behavior

- Open the selected clip set with double-click or Enter from the arrangement. Keep the
  clip and track identity visible in each editor lane, including overlapping clips and
  clips from the same track. Use project time for the grid and playhead.
- Add, delete, move, resize, and velocity-edit notes. Support single selection, additive
  selection, marquee, copy/paste, and duplicate. Scope pointer edits to their clip lane
  and keyboard commands to the focused editor. Keep arrangement clip commands available
  when focus returns to the arrangement.
- Provide snap, temporary Option bypass, horizontal and pitch zoom and scroll, and
  playhead following. Propose a chromatic keybed covering MIDI pitches 0–127.
- Quantize selected note starts toward the chosen subdivision at 0–100% strength and
  preserve duration. Transpose in semitones. Humanize timing and velocity with separately
  adjustable amounts. Apply each invocation as one undoable edit. Keep generated changes
  within the editable source and valid MIDI bounds.
- Keep recorded and edited controller events per clip and channel. Support creation,
  movement, value changes, and removal of points. Offer all 128 controller numbers in the
  lane picker and allow several controller lanes for the focused clip. Show the MIDI
  channel of the events being edited.
- Preserve the controller number, channel, tick, value, and order of simultaneous
  messages through save/reopen and playback. Expand the current recording filter to cover
  the same CC range. Preserve existing pitch-bend recording and playback.
- Offer a straight segment and adjustable curvature per continuous CC segment. Propose
  linear interpolation for newly authored segments and preserve held playback for points
  from existing recordings until interpolation is explicitly enabled. Curve shape is
  undoable, saved in the project, and heard in playback. Anchor the curve at its endpoint
  values, keep the curve within those values, and restore its value at a locate or loop
  boundary.
- Show switch CCs as steps and CC120–127 as discrete events. Treat channel-mode messages
  as timed commands during playback and keep them out of ordinary value chase, except that
  a message whose channel still carries notes from another clip is replaced by note-offs
  for the clip's own voices, so one clip cannot silence another. Preserve
  the order of related messages when several CCs share a tick. Keep bank selection,
  parameter selection, and increment/decrement as discrete messages.
- Keep note and controller changes in the existing session undo/redo history. One gesture
  or command is one step. Cancel restores the pre-gesture state. Apply validated edits to
  playback and save the resulting clip data with the project.

## Out of Scope

Chord/scale lanes and chord tools; articulation maps; notation; ARA; audio editing; step
input unless explicitly agreed; automation lanes.

## Boundary Behavior

Apply editing commands to the focused clip lane. Clamp edits to its editable source
bounds. Preserve hidden material when a clip has been trimmed or split through a repeat.

## Existing Contracts

- [T06](T06-midi-recording.md) provides the persisted MIDI note, controller, clip, and take
  model. T10 must account for projects recorded before the editor exists.
- [T08](T08-undo-redo.md) provides session-wide project edit history. The arrangement's
  edit path rejects stale revisions and updates the engine before publishing an accepted
  edit. T10 can use that transaction pattern for note edits.
- [T09](T09-arrangement-editing.md) preserves hidden MIDI when a clip is trimmed and
  distinguishes a repeat source from the events already projected into the arrangement.
  The repeat-edit decision must cover both existing and newly extended occurrences.

## Written UI Proposal

This layout uses [Piano roll](../UI/views/piano-roll.md),
[Controller lanes](../UI/views/controller-lanes.md), [Timeline rulers](../UI/views/timeline-rulers.md)
and [MIDI functions](../UI/views/midi-functions.md) as its references.

```text
surface arrangement
  column
    arrangement timeline fill
    resize gutter
    column Piano Roll fill
      row hug: focused clip + snap + follow + MIDI functions
      row hug: keybed column + shared bars and beats ruler
      column vertical scroll
        row per selected clip: clip and track label + keybed + note grid
      column controller stack for focused clip
        row: Velocity header + velocity bars
        row per open CC: controller and channel header + events
        row hug: Add lane
```

| Primitive or view | Role and states |
| --- | --- |
| [Note](../UI/components/note.md) | Track-colored notes, hover lift, selected outline, primary resize handles, and drag preview |
| [Range overlay](../UI/components/range-overlay.md) | Marquee within the originating clip lane |
| [Scroll and zoom bars](../UI/components/scroll-zoom-bars.md) | Shared time-axis scroll and zoom, with pitch navigation per clip |
| [Timeline rulers](../UI/views/timeline-rulers.md) | Musical grid, playhead, clip boundaries, repeat boundaries, and snap guide |
| [Controller lanes](../UI/views/controller-lanes.md) | Velocity bars; linear CC segments and bend handles, stepped switches, and discrete event markers; empty, focused, selected, and collapsed states |
| [MIDI functions](../UI/views/midi-functions.md) | Compact controls for Quantize, Humanize, and Transpose with selection-dependent availability |

- With one clip, the pitch grid fills the editor. With several, keep all clip lanes
  available in a vertical scroll region and visually mark the lane targeted by commands.
  With an empty clip, show the editable pitch grid and empty controller lanes.
- During drag, show snap or free edit and retain the preview until the session responds.
  On cancel or rejection, restore the committed notes or points and surface the reason
  for rejection. Mark the repeat source and show affected visible repetitions during edits.
- Give continuous segments a bend handle and a keyboard-accessible curvature control
  with a linear reset. Keep segment handles off stepped lanes and discrete events.
- At narrow widths, keep the keybed, ruler, and grid aligned. Put secondary commands
  behind the MIDI functions control. Truncate long names with a tooltip. Provide named
  controls, visible keyboard focus, and keyboard access to the main edit commands.
- Add gallery evidence at 1024 and 760 points for one clip, several clips, note selection
  and snap preview, Velocity, linear and curved CC, switch CC, and empty or unavailable states.

## Verification Intent

- Verify note commands, snap and quantize rounding, MIDI bounds, deterministic playback
  of the resulting edit, and one-step undo/redo for each operation.
- Verify that selecting overlapping clips opens distinct lanes, that changing focus targets
  the intended clip and track, and that switching lanes preserves committed edits.
- Verify that editing the repeat source updates already visible and newly extended cycles,
  including clips that were trimmed or split. Exercise undo/redo and save/reopen.
- Verify representative continuous, switch, paired, and channel-mode CCs end to end, with
  an automated range test covering CC0–127 and channel isolation. Check ordering, seek,
  loop boundaries and stopped audition cleanup. Check linear and curved interpolation
  at interior positions, endpoint stability, compatibility with existing recordings,
  and bounded realtime scheduling without allocations or waits. Check VST3 delivery
  where an instrument exposes the controller mapping and CLAP delivery through its MIDI input.
- Verify pointer and keyboard workflows in egui and listen through an instrument for
  stopped audition and subsequent playback.

## Approval Gate

The user approved this specification and the written UI proposal on 2026-11-19.

## Implementation

The piano roll is a lower dock of the multitrack surface. It reads the arrangement
selection, sorts the selected clips by track order, start and layer, and gives every clip
its own pitch lane, keybed and grid inside one vertical scroll region. Lane names show the
track and start bar and append an ordinal when two lanes would otherwise read the same, so
co-located clips stay distinguishable on screen and to accessibility.

Note and controller edits are built as `EventChange` batches against the clip's editable
source and sent as one `ClipEdit::EditEvents` transaction: the session checks the expected
arrangement revision, validates a candidate, publishes the playback plan and records one
history entry. Rejected or cancelled gestures restore the committed events and report the
reason. The piano roll keeps a pending batch until the session answers it.

Pointers add, delete, move, resize, marquee-select and audition events. Option resolves to
a free grid for pointer and keyboard gestures. Double-click or Enter opens the dock and
moves keyboard focus to the active grid, so arrangement commands stop firing as soon as
the editor is open. Tab reaches notes, the keybed, both grids and the value lanes; focused
surfaces paint a focus ring.

MIDI functions offer strength-limited quantize, transpose and humanized timing and
velocity, each dispatched as one undoable edit, and each disabled when it cannot change
anything. Velocity and every CC lane are part of the same editor. Continuous segments
default to a linear interpolation; selecting a point loads its curvature into the control,
and the bend handle, Curvature, Linear and Apply Curvature all write the same value.

Playback uses prepared event edges: authored curves expand into bounded value edges, legacy
recorded points keep their held value, and a curved lane is not reset to neutral at an
internal repeat seam. The isolated scheduler panic path reuses a preallocated buffer, so an
event overflow on a worker does not allocate.

## Verification

- On 2026-11-19, `cargo check --locked`, `cargo build --locked`, `cargo test --locked`,
  `cargo fmt --all --check`, and `cargo clippy --all-targets --locked -- -D warnings`
  passed. The default run reported 123 passes and 12 ignored tests.
- Model and engine tests cover curved and legacy CC replay, repeat seams, all-CC capture,
  split and repeat edits, whole-cycle loop extension, legacy tail splits, trim hiding, and
  the one-step piano-roll transaction. UI tests
  cover pointer selection and audition, keyboard movement, Option bypass, quantize
  strength, source and MIDI bounds, trimmed-note resizing, and co-located lane names.
- The explicitly selected native regression
  `native_t10_delivers_mapped_and_raw_cc_to_isolated_instruments` passed with the rebuilt
  TestSynth and the CLAP fixture. It compares VST3 mapped controller output at two values
  and drives the CLAP input, so controller delivery is measured rather than assumed.
- Eleven gallery states are rendered at 760 and 1024 points: docked arrangement, one clip,
  several clips, selection, snap preview, Velocity, linear CC, curved CC, switch CC, empty
  clip, and disabled MIDI functions. Inspect the
  [docked arrangement](../UI/snapshots/egui/t10-dock-760.png),
  [several clips](../UI/snapshots/egui/t10-multiple-1024.png),
  [curved CC](../UI/snapshots/egui/t10-curved-cc-1024.png),
  [switch CC](../UI/snapshots/egui/t10-switch-cc-760.png), and
  [disabled functions](../UI/snapshots/egui/t10-unavailable-1024.png). Regenerate them
  with the `t10_gallery` example in `clef-ui` at each width.

## Decisions — 2026-11-19

- A repeated clip has no tail: dragging its lower edge past the repetition rounds up to
  whole cycles and repeats the source into the new interval. The repeat source is the only
  editable source, so what plays and what the editor shows stay the same. A clip stored
  with a tail by an older build still plays unchanged; splitting inside it turns the
  remainder into an ordinary clip instead of an empty cycle.
- Channel-mode messages stay protective: when another clip still owns notes on the same
  channel, All Notes Off and All Sound Off are replaced by note-offs for the sending
  clip's own voices. Sending the raw message would let one clip silence another on a
  shared channel.

## Accessibility — 2026-11-19

The two gaps recorded when the task was first closed are fixed.

- Every controller and velocity point is focusable. Tab reaches the points, the focused
  point becomes the selection and paints a focus ring, arrows move or change the selected
  points, and Delete removes them. With the lane focused, Enter or Insert adds a point at
  the playhead holding the value in force there, on the lane's channel and controller.
- The keybed keeps a per-clip pitch cursor while it has focus. Arrows move the cursor, a
  30 % accent band marks it, and Enter or Space auditions it on the clip channel, matching
  the pointer audition. Clicking the keybed also focuses it.

The regressions `the_value_lane_inserts_a_point_at_the_playhead_from_the_keyboard` and
`the_keybed_auditions_the_focused_pitch_from_the_keyboard` cover both paths.

## Known Cross-Task Gaps

Both were fixed on 2026-11-19 and are recorded in the
[T09 correction](T09-arrangement-editing.md#split-state-correction--2026-11-19):
the split now carries switch and zero-valued controllers, and a note crossing a mid-cycle
seam is clipped instead of vanishing.

## Progress — 2026-11-19

Implementation, automated verification, the two product decisions and the two
accessibility follow-ups above are complete. The approved behavior, the docked UI, project
persistence, undo/redo, controller playback and the ignored native delivery test pass.

## Completion — 2026-11-19

The user asked to recover and finish the task after the previous agent stopped, then chose
the two product decisions recorded above: repeated clips carry no tail, and channel-mode
messages stay protective. Every defect the verification passes found in the T10 surface was
fixed with a focused test; the two defects that belong to the T09 split and loop path are
recorded under Known Cross-Task Gaps and need a separate task, as do the two accessibility
gaps under Open Questions.

Evidence categories: automated model, engine, session and egui tests, plus the ignored
native VST3/CLAP delivery test and gallery snapshots. The pointer and keyboard workflows
were exercised in egui tests and through the gallery states, not by hand in the running
app, and no acoustic or listening check was performed; both remain user acceptance.

On 2026-11-19 the user asked for the T09 split defects and the accessibility gaps to be
fixed. Both T09 corrections landed with regressions, the controller points and the keybed
gained their keyboard paths with egui regressions, and the workspace gates and the native
delivery test passed again.

## Follow-up — 2026-11-19

A repeated clip's projected points shared one egui focus id, so debug builds painted the
id-clash overlay in the velocity lane and keyboard focus could not distinguish repeats.
Fixed under [T30](T30-piano-roll-repeat-focus.md) by adding the projected tick to the
point focus id, with a regression for repeated projections.

Later on 2026-11-19 the velocity lane gained a horizontal note-extent line and a
grab-relative drag, recorded in [T31](T31-velocity-lane-extent-drag.md).

The one-frame red border when switching clips was traced to an unstable lane `Ui` id and
fixed under [T32](T32-piano-roll-lane-id-stability.md).

Switching the arrangement selection now brings the new edit target's start into view;
recorded in [T33](T33-piano-roll-selection-reveals-clip.md).
