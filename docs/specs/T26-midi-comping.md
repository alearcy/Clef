# T26 — MIDI Comping and Take Lanes

**Status:** Planning draft — not approved for implementation. Product behavior is inherited
from the Composer DAW; the Clef implementation interview and approval are still required.
**Priority:** P2.
**Date:** 2026-11-18
**Task:** [T26](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T06 MIDI capture; T08 undo/redo; T09 clip editing; T10 piano roll.

## Reference decisions

The product decisions were taken and verified in the external Composer DAW project. It is not
part of this repository and is never an API:

- `~/Documents/ChatGPT/DAW/Specs/D66-MIDI-Takes.md` — DONE 2026-11-09, with its approved
  mockup and implementation evidence. It also contains the later additions for take-lane
  height, take names, editing a take, and dragging a comp cut.
- [Track lanes](../UI/views/track-lanes.md), [arrangement](../UI/views/arrangement.md), and
  [piano roll](../UI/views/piano-roll.md) are the Clef UI references.

Clef inherits the behavior, not the C++ implementation. Tracktion `MidiClip` takes,
`MidiCompManager`, and the Composer storage format are historical references: Clef owns its
take model and comp resolution in Rust. If this draft and the reference disagree after the
Clef interview, the approved Clef specification wins.

## Goal

Let a composer decide what a MIDI recording does to existing material, keep alternative
performances as takes, and assemble a comp by selecting ranges, without ever losing a take.

## Inherited behavior

### Record mode

- Three modes: **Merge** (the current overdub), **Replace**, **New take**. A global mode and a
  per-track mode exist; a track left on **Global** follows the global mode, otherwise its own
  mode wins. Both are saved with the project, undoable, and cannot change during a take.
- Loop: every pass follows the mode. Merge sums the passes, Replace keeps the last pass, New
  take creates one take per pass.
- Replace removes notes whose attack lies in the recorded range and controller events inside
  it; the clips themselves stay. A pass that captured nothing removes nothing.
- New take without loop: the recording becomes a new take of the clip under it. A take that
  touches several clips first joins them into one, their content and gaps becoming take 1.
  The clip grows to cover the recorded range; older takes are empty in the grown span. On
  empty space a new clip with one take is created. Loop: one take per non-empty pass, in one
  clip covering the loop range.
- A newly recorded take becomes active over its recorded range; the rest of the comp is
  unchanged.
- If T06 does not deliver all three modes, T26 adds the missing modes. The effective-mode
  model, storage, and undo rules are defined here.

### Takes and comp

- A clip with two or more takes plays its comp: a sequence of sections, each a beat range
  assigned to one take. A note plays when its attack lies in a section of its own take, with
  its full length; nothing is truncated. Controller events play when they lie in their take's
  section.
- Take lanes: a track-header chip “T n” (n = most takes in any clip of the track; hidden below
  2) shows or hides one lane per take index under the track. Lane n shows take n of every clip,
  the active sections at full strength and the rest dimmed. Visibility, lane height, and lane
  names are view state saved with the project, never an undo step.
- **Comp by selection**: a Select drag on a take lane, snapped to the grid, makes that take
  active over the dragged range inside the clip. One undo step, “Comp take”.
- **Use whole take**: a click on a lane's header, or the command, makes that take active over
  the whole clip.
- **Delete take**: removes the take; its sections pass to the previous take (the next one when
  deleting take 1). When one take remains the clip becomes a normal clip and its lanes go.
- **Flatten comp**: the comp becomes the clip's only sequence; the other takes are removed.
- **Take to new track**: copies the take, as a normal clip at the same position, to a new
  instrument track below the owner (no instrument copied).
- A cut between two sections can be dragged on the lanes; it stops at the neighbouring cut so
  the narrowed section keeps at least one grid step. Alt frees it from the grid with a one-tick
  minimum. One undo step, “Move comp cut”.
- A take can be renamed and each lane has its own height, set and persisted as view state.

### Comp editing

- The piano roll, inline editing, and the multi-clip editor show the comp. Moving, resizing,
  deleting, or changing a note edits it in its source take; a note moved so its attack leaves
  its take's section moves into the take active at its new attack. Drawn and pasted notes go
  into the take active at their attack. Controller lanes follow the same rule per event.
- Double-clicking a clip section on a take lane, or **Edit Take**, opens that take in the
  editor: every note is listed, notes the comp does not play are dimmed, and edits write into
  the take without changing the comp.
- Clip operations keep takes: move, copy/duplicate, resize, note-driven growth, split (every
  take and the comp are split), glue/join (takes kept per index, comp sections kept), delete.
  Part/clip automation stays on the clip, not on the takes.

### Commands

All commands go through the shared command registry, Actions list, Commands search, and
assignable keystrokes, with live bindings in menus and tooltips:

- Record mode: Merge / Replace / New take (global).
- Track record mode: Global / Merge / Replace / New take.
- Show/hide take lanes.
- Use whole take, Delete take, Flatten comp, Take to new track.
- Rename Take, Edit Take, and Track Height commands targeting the focused lane.

No command gets a default keystroke unless the interview agrees one.

## Clef requirements

### Model and persistence

- The project document owns the global record mode, each track's mode, each clip's take
  sequences, its comp sections, and the lane view state (visibility, height, name).
- A rendered comp note carries the identity of its source take and source note, so an edit is
  redirected to the take and the comp can be re-rendered in the same transaction.
- Persist through the project schema with a migration. Projects without takes open as
  Global/Merge with no lanes and no comp, unchanged on save. Coordinate a single schema bump
  with T27; do not bump once per task.
- Save stays atomic. An older project round-trips without data loss.

### Scheduling and realtime

- Comp resolution happens off the device callback when a take, section, mode, or clip changes.
  The callback consumes a bounded, immutable playback plan and performs no allocation, lock,
  formatting, logging, or I/O.
- Comp changes during playback are applied at a buffer boundary with the existing note-chase
  and all-notes-off guarantees, so no note is left stuck and no half-updated plan is heard.
- The number of takes, sections, and notes is bounded by the existing clip limits; exceeding
  them fails the edit with a reason rather than degrading the callback.

### Undo and commands

- Each recording, take creation, comp selection, cut move, rename, delete, flatten, or
  take-to-track action is one undo step with a descriptive name. Redo invalidation follows T08.
- Lane visibility and other pure view state are saved but not undoable.
- Commands are registered with the shared registry as T18 defines it; until T18 lands, use the
  existing application command path with the same identifiers and availability rules.

### Interaction with neighbouring tasks

- T06 provides capture and the initial clip model; T26 extends both without changing T06's
  playback guarantees.
- T09's split, join, duplicate, and move must preserve takes and comp sections.
- T10 shows the comp and redirects note edits to source takes, including the dimmed
  Edit Take mode.
- Audio takes and audio comping are T27; this task must not add a second take model.

## Out of scope

Audio takes and comping (T27); automatic alignment of takes; named whole-track playlists;
retrospective Capture MIDI (not a Clef task); cross-track comping; articulation maps,
automation lanes, and chord/scale lanes; ARA (T29); a new recording engine.

## Open questions

- Whether all three record modes belong to T06 or T26, and how a mode change is refused during
  a take.
- Punch: whether it exists when T26 starts, and the recorded-range definition when it is absent.
- Exact schema layout for takes, sections, and note identity; one bump shared with T27.
- How the comp is represented for playback and for the piano roll (rendered note list versus
  per-take scheduling), and how much is cached.
- Lane height limits and whether they follow T09's track-height presets.
- How take identity survives split/join and clip copies.
- Undo granularity for a multi-clip comp drag and for a recording that joins clips.
- Whether a recording that captures nothing creates a take or an undo step.
- Command names and categories, if they differ from the reference.

## UI proposal requirement

Take lanes, the record-mode surfaces, the comp gesture and cut dragging, lane height and
names, and the take-editing state require a written UI proposal derived from
[track lanes](../UI/views/track-lanes.md), [arrangement](../UI/views/arrangement.md), and
[piano roll](../UI/views/piano-roll.md), with user approval before implementation. The
reference mockup is evidence of intent, not a pixel target.

## Verification intent

- Deterministic comp logic: the attack rule, controller placement, section boundaries, and
  cut dragging with grid and free modes.
- Record modes with and without loop and punch: Merge sums, Replace removes only inside the
  recorded range, New take joins clips and creates one take per pass.
- One undo step per action; redo; no undo step for view state.
- Save/reopen keeps takes, comp, modes, lane names, heights, and visibility; an older project
  opens unchanged.
- No stuck notes across comp changes while the transport runs.
- Command availability and registry entries.
- Real-component snapshots compared with the approved boards.
- Automated proof stays separate from GUI, listening, and physical-keyboard timing evidence.

## Approval gate

This document is a planning draft. The product decisions are the referenced Composer DAW
specification; a Clef implementation interview for the open questions, an approved
specification, and an approved written UI proposal are still required before implementation.
