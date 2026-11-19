# T09 — Arrangement and Clip Editing

**Status:** Done — 2026-11-19.
**Priority:** P1.
**Date:** 2026-11-17
**Task:** [T09](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T06 clip model; T08 undo/redo.

## Goal

Make the arrangement a real editing surface for recorded MIDI: create, move, trim, split,
duplicate, and loop clips on a musical grid, with selection, zoom, keyboard, and context
menus. Every edit goes through the command registry and the T08 history.

## Scope

- Clip creation and deletion; moving across tracks and time.
- Trimming, splitting, duplicating, and looping clips.
- Snap and grid options, including explicit free-edit behavior.
- Single and multi-selection, marquee/range selection, and keyboard commands.
- Zoom, scroll, and playhead navigation in the arrangement.
- Context menus and tooltips using live command bindings.
- Undo/redo integration for every operation.
- Persistence of clip edits and arrangement view state per project.

## Out of Scope

Audio clip editing and waveforms (T14); fades and crossfades;
automation lanes; folder tracks; chord/articulation lanes; ARA.

## Interview Decisions — 2026-11-19

- Use `Clip` in Clef. Allow edits only while the transport is stopped.
- During recording, use the T06 Merge, Replace, and New take modes. When
  moving clips creates a same-track overlap, the frontmost clip wins in the
  covered interval.
- If a cut would split one or more MIDI notes, ask whether to split those notes
  or cancel the operation.
- Extend from the upper part of the right edge to loop the clip. Extend from
  the lower part of that edge to lengthen it. Use Duplicate or Command-D to
  create an independent copy.
- Offer arrangement snap choices `Bar`, `Beat`, `1/8`, `1/16`, and `Off`.
  Default to `Beat`. Option temporarily bypasses snap during a gesture. Save
  snap, horizontal zoom, and horizontal scroll position per project.

## Boundary Rules

- A move places the moved clip in front of existing clips at the destination.
  A group move preserves the order within the group. Save that order and leave
  the lower clips intact, so moving or deleting the front clip exposes them.
  Draw overlapping clips back to front and hit-test in reverse order.
- In an overlap, play only the front clip. End a sounding note from the lower
  clip when it becomes covered. On re-exposure, use the existing MIDI chase
  rules to resume material that is still sounding and restore controller state.
  Apply T06 recording as before on tracks without overlapping material. In a
  layered recording range, apply the chosen mode to the exposed clip at each
  position and preserve the hidden clips and takes.
- Lower-right extension adds space, preserving existing notes, takes, and
  repetitions. For inward resize, propose retaining hidden MIDI so the lower
  edge can reveal it again. Notes crossing the audible end stop there.
- The first upper-right extension takes its cycle length from the current
  visible clip, including its selected take sections. Upper-right drag and
  keyboard loop commands use whole cycle boundaries, independently of the
  arrangement grid and Option bypass. Existing repeats keep their period and
  phase. Ordinary lower-edge resizing keeps its snap and free-edit rules.
  A clip cycle is independent of the transport loop. Duplicate creates an
  independent clip with the same source content and repeat settings.
- On a split, check all takes and the effective note positions in repetitions.
  When any note crosses the split point, confirm once for the full selection.
  On confirmation, end the left note at the split and create a right note
  beginning there with the same pitch, channel, and attack velocity. Preserve
  the existing end and release in the right clip. Cancel leaves every clip
  unchanged. Split repeated clips at their audible position and preserve the
  repeat phase on each side.

## Interaction

- Click a clip to select it. Command-click toggles it. Drag empty lane space to
  select clips intersecting the marquee. Escape cancels an active gesture first
  and otherwise clears clip selection when the arrangement has focus.
- Create an empty MIDI clip by double-clicking empty lane space or invoking
  `Create Clip` from the lane menu. Start at the chosen position with a default
  length of one local bar. Create at the playhead when invoked by keyboard.
- Drag the body of a selected clip to move the selection by one time and track
  delta, preserving relative positions. Drag the lower-right handle to resize
  a clip or the upper-right handle to change its repeated extent.
  Show the proposed result before release. A cancelled, refused, or unchanged
  gesture leaves the project and history untouched.
- The clip menu targets the current selection when opened on one of its clips.
  Opening it on another clip selects that clip. Offer `Split at Playhead`,
  `Duplicate`, and `Delete`. Duplicate proposes an adjacent
  copy, preserving relative offsets for multiple selected clips. A split acts
  on selected clips whose interiors intersect the playhead.
- Use the shared command registry for edit commands and live bindings. Bind
  Duplicate to Command-D and propose the Delete key for deleting selected clips.
  Provide keyboard selection, move, resize, and repeat commands.
  Scope edit shortcuts to arrangement focus and preserve text-field editing keys.

## Provisional UI Proposal

Extend the existing [multitrack session](../UI/views/multitrack-session.md), using
the MIDI section of the [arrangement reference](../UI/views/arrangement.md)
and the [dense editing surface contract](../UI/contract.md#dense-editing-surfaces).
Use one egui viewport transform for ruler, grid, clips, hit targets, playhead,
and previews. Preserve the existing track headers, recording previews and
transport controls. This composition is provisional pending T18.

```text
multitrack session
  arrangement controls: Snap value
  existing timeline: locator band + bar ruler
  vertical scroll: track headers + matching MIDI lanes
    lane: grid + stacked clips with note marks and repeat boundaries + playhead
    overlays: marquee, edit preview, focus
    context menu: commands for a clip selection or empty lane
  existing horizontal scroll and zoom controls
  existing session status: refused edit reason
  modal on a cut through notes: Split Notes / Cancel
```

| State | Proposed presentation |
| --- | --- |
| Idle, hover, focus | Existing clip fill and note marks. Hover reveals two right-edge targets with separate labels, `Loop Clip` above and `Resize Clip` below. Keyboard focus has its own cue and accessible clip name, track, range and selection state. |
| Single or multiple selected clips | Outline each selected clip, with visible handles on eligible edges. Draw the marquee with the shared range overlay. |
| Overlap or repeat | Draw the audible clip in front in the shared lane. Use the clip's own ink to mark repeat boundaries within its outline. |
| Snap active or temporarily bypassed | Keep the selected subdivision visible in the Snap control. Highlight the resolved grid line while an edit snaps. Indicate free movement during bypass. |
| Dragging or refused | Draw the pending clip bounds and start/end position in the same viewport. Give a specific reason when the destination is invalid. Restore the committed geometry on cancellation or refusal. |
| Context menu | Use existing menu primitives and live bindings. Disabled actions identify the relevant unavailable operation. |
| Cut through notes | Use the existing confirmation-dialog pattern. Name the pending action and offer `Split Notes` and `Cancel`. Escape cancels the whole operation. |
| Zoomed or scrolled | Keep ruler, playhead, grid, and clip hit targets aligned. Keep scroll and zoom controls available. Zoom around the visible playhead or the viewport center when the playhead is outside it. |

Reuse the timeline, menu, dialog and selection primitives. Extend the timeline primitive
with clip hit targets and use the shared range overlay and theme roles for
selection. Check the provisional composition at 760 and 1024 points, including
active snap, free edit, multiple selection, overlap, repeat versus resize,
the split confirmation, refusal, zoom, and an open menu.

## Acceptance

- Create, move within and across tracks, trim, split, duplicate, repeat and
  delete clips. Check single and group operations and each agreed MIDI boundary
  rule. Preserve take content and active-section mapping through permitted edits.
- Verify adjacent boundaries, project-start and project-end bounds, and moves
  between tracks. Check snap across meter changes, temporary bypass, and
  cancellation. Report rejections without partial group edits.
- Move one clip over another and verify priority, a note crossing into cover,
  chase after exposure, and intact lower material after moving the upper clip
  away. Check all three recording modes on an existing layered destination.
- Compare loop extension, ordinary extension, and duplication. Cut through
  active and inactive notes, including in a repetition. Verify both dialog
  outcomes and verify that a cut through an empty interval needs no prompt.
- Each committed operation produces one T08 entry. Undo and redo restore the
  exact arrangement and playback. Failed and unchanged edits keep history and
  the saved-state indicator intact.
- Save and reopen edited MIDI, including projects written before T09. Check
  view state according to the agreed persistence policy.
- Check pointer and keyboard interaction, menu enablement and shortcut labels
  after reassignment, independent selection and focus, and aligned drawing and
  hit-testing for partially visible clips. Capture the provisional UI states and
  verify playback and edit workflows in the app.

## Approval Gate

The user approved the specification, proposed boundary rules, and written
provisional UI on 2026-11-19.

## Implementation

The arrangement stores clip precedence, trims and repeats in the optional MIDI
metadata of the existing project format. Clip edits are validated as a group
before publishing the playback plan and recording one T08 entry. Snap and
viewport preferences are persisted separately from clip history. Playback
projects foreground intervals before handing plans to the isolated instrument
workers. Lower clips retain their content, and resumed foreground intervals
chase active notes and controllers.

## Verification

- On 2026-11-19, `cargo check --locked`, `cargo build --locked`, `cargo test
  --locked`, `cargo fmt --all --check`, and `cargo clippy --all-targets --locked
  -- -D warnings` passed. The default test run reported 103 passes and 11
  ignored tests.
- Model and engine tests cover group move rollback, stacking, overlap
  playback boundaries and controller chase, each recording mode over an
  existing overlap, non-destructive trim, repetition, split confirmation,
  inactive takes, repeat phase, and persistence.
- Session tests cover one history entry for a cross-track edit, stale edit
  rejection, undo and redo, and reopening project view preferences. egui
  input tests cover both right-edge targets, Option bypass, and cross-track
  drag through the shared viewport.
- The native egui gallery rendered nine states at 760 and 1024 points.
  Inspect the [context menu](../UI/snapshots/egui/t09-arrangement-menu-760.png),
  [loop extent](../UI/snapshots/egui/t09-arrangement-repeat-760.png),
  [free preview](../UI/snapshots/egui/t09-arrangement-free-preview-1024.png),
  [refusal](../UI/snapshots/egui/t09-arrangement-refused-760.png), and
  [split confirmation](../UI/snapshots/egui/t09-arrangement-split-confirm-1024.png).
  Regenerate these states with the `t09_gallery` example in `clef-ui`.
- The explicitly selected native session test
  `native_t06_records_routes_reopens_and_plays_an_instrument_clip` passed with
  a rebuilt TestSynth, CoreAudio, and a temporary CoreMIDI source. After
  moving a captured clip to the other instrument track and extending its
  loop, the test located into the repeated segment and observed nonzero
  output from the destination worker. Undo restored the original
  arrangement. This measured worker output; no acoustic measurement was
  performed.

## Completion — 2026-11-19

The approved arrangement commands, provisional UI, project persistence, and
playback checks are complete. T10 can use the resulting clip editing model.

## Loop Length Correction — 2026-11-19

The first Loop gesture uses the visible clip length after an ordinary resize
or trim. The repeated material fills the added interval, and undo restores
the previous source. The upper handle and the keyboard loop commands resolve
to cycle boundaries. Arrangement snap and Option still control ordinary
resizing.

The reported resize, trim, and loop sequence failed the regression test
before the fix. The model regression, pointer tests with snap and Option,
and 104 default tests passed after the fix. The native TestSynth playback
test passed separately. The loop gallery was reviewed at 760 and 1024 points.
Build, check, formatting, and Clippy passed.

The user accepted the correction and requested closure on 2026-11-19.

## Loop Tail Correction — 2026-11-19

T10 found that a repeated clip could be lengthened past its repetition, leaving played
material outside the repeat that the piano roll neither shows nor edits. The user decided
that a repeated clip has no tail: the lower edge rounds up to whole cycles and repeats the
source into the new interval. A clip stored with a tail by an earlier build still plays as
before, and splitting inside that tail turns the remainder into an ordinary clip instead of
an empty cycle. Snap, Option bypass, and the keyboard resize commands keep their rules for
clips that are not repeated. The model regression
`a_looped_clip_lengthened_from_its_lower_edge_adds_whole_cycles` and
`splitting_a_legacy_repeat_tail_keeps_the_tail_editable` cover the new behavior.

## Split State Correction — 2026-11-19

T10 found two defects in the split and repetition path.

- `split_boundary` treated any zero-valued controller as neutral, so a clip whose last
  CC7 or CC11 value was 0 lost that state at the split, and switch controllers such as the
  sustain pedal were never carried. The split now carries the last value of every
  controller below CC120, including switch controllers and real zero values. Channel-mode
  commands (CC120–127) stay out of the carried state, and pitch bend at its centre value
  is still dropped as neutral.
- Extending a repetition whose start sits mid-cycle mapped only events beginning at or
  after the phase, so a note that crossed the seam was missing from the first partial
  cycle. Crossing notes are now clipped at the seam instead of vanishing. The no-tail
  decision above keeps the model from producing that phase in normal editing; the fix
  covers cyclic data written by earlier builds.

The regressions `a_split_carries_switch_and_zero_valued_controllers` and
`extending_a_cycle_from_a_mid_cycle_phase_keeps_a_crossing_note` cover both cases. The
model suite, the workspace gates, and the native delivery test passed afterwards.
