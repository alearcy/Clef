# T26 — MIDI Comping and Take Lanes

**Status:** Done — 2026-11-19. Approved and implemented on 2026-11-19; the user accepted the
delivered GUI behavior and requested closure on the same day.
**Priority:** P2.
**Date:** 2026-11-18 (interview 2026-11-19).
**Task:** [T26](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T06 MIDI capture; T08 undo/redo; T09 clip editing; T10 piano roll.

## Reference decisions

The product behavior was decided and verified in the external Composer DAW project. It is not
part of this repository and is never an API:

- `~/Documents/ChatGPT/DAW/Specs/D66-MIDI-Takes.md` — DONE 2026-11-09, with its approved
  mockup and implementation evidence, including the later additions for take-lane height,
  take names, editing a take, and dragging a comp cut.
- [Track lanes](../UI/views/track-lanes.md), [arrangement](../UI/views/arrangement.md),
  [track header](../UI/views/track-header.md), and [piano roll](../UI/views/piano-roll.md)
  are the Clef UI references.

Clef inherits the behavior, not the C++ implementation. Tracktion `MidiClip` takes,
`MidiCompManager`, and the Composer storage format are historical references: Clef owns its
take model and comp resolution in Rust. Where this specification and the reference disagree,
this approved Clef specification wins.

## Goal

Let a composer decide what a MIDI recording does to existing material, keep alternative
performances as takes, and assemble a comp by selecting ranges, without ever losing a take.

## Interview decisions — 2026-11-19

- No punch in T26. The recorded range is the actual capture start through Stop, or the loop
  range in loop recording. Replace removes only material inside that range.
- The comp is virtual. Takes remain the only stored source; the comp is resolved from the
  ordered half-open sections with the attack rule. There is no second rendered sequence to
  keep in sync. Editors and playback both use the same resolution.
- A Select drag on a take lane is one `Comp take` undo step across every clip of the track it
  touches.
- The global record mode stays on the transport chip. The per-track mode is a row in the
  selected-track inspector: `Global`, `Merge`, `Replace`, `New take`.
- Take to new track copies the take only; the new track has no instrument.
- Projects move to schema version 3 now. T27 adds audio takes under the same version with
  defaulted fields; T26 does not bump again.
- No command gets a default keystroke.
- Take lanes, their visibility, heights, and names are view state: saved with the project,
  never an undo step.
- A take lane is track-wide: lane n is the n-th take index of every clip of the track. Take
  actions on a lane therefore act on every clip of that track holding that index, in one undo
  step. Double-clicking one clip on a lane edits that clip's take; the Edit Take command edits
  the lane's take across the track's clips.

## Approved behavior

### Record mode

- Global mode: Merge by default, saved with the project, undoable. Per-track mode: Global by
  default, saved with the track, undoable. A track left on Global follows the global mode;
  otherwise its own mode wins. Both are refused while a take is in progress. The recording
  target freezes the effective mode at Record.
- Loop: every pass follows the mode. Merge sums the passes, Replace keeps the last non-empty
  pass, New take creates one take per non-empty pass.
- Replace removes notes whose attack lies in the recorded range and controller events inside
  it; clips stay. A pass that captured nothing removes nothing.
- New take without loop: the recording becomes a new take of the clip under it. A recording
  that touches several clips first joins them into one, their content and gaps becoming take 1.
  The clip grows to cover the recorded range; older takes are empty in the grown span. On empty
  space a new clip with one take is created. Loop: one take per non-empty pass, in one clip
  covering the loop range.
- A newly recorded take becomes active over its recorded range; the rest of the comp is
  unchanged.
- In a clip that already has takes, Merge adds each new event to the take active at its attack
  and Replace removes from the takes active in the range.
- T06 owns capture and the first clip, take, and active-section model. T26 adds track
  overrides, take editing, comp editing, and lane display.

### Takes and comp

- A clip with two or more takes plays its comp: a sequence of half-open sections, each a tick
  range assigned to one take. A note plays when its attack lies in a section of its own take,
  with its full length, and may cross a section boundary. Nothing is truncated. A controller
  event plays when it lies in a section of its own take.
- The comp is virtual: playback and editors resolve the active events from the takes and
  sections. A note edit is applied to its source take. A note moved so its attack leaves its
  take's section starts or stops playing without moving take.
- Take lanes: a track-header chip `T n` (n = the most takes in any clip of the track; hidden
  below 2) shows or hides one lane per take index under the track. Lane n shows take n of every
  clip that has it, the active sections at full strength and the rest dimmed. Visibility, lane
  heights, and lane names are view state saved with the project.
- **Comp by selection**: a Select drag on a take lane, snapped to the arrangement grid, makes
  that take active over the dragged range in every touched clip of the track. One undo step,
  `Comp take`. A range that touches no clip is unchanged and records no step.
- **Use whole take**: a click on a lane's header, or the command, makes that take active over
  the whole of every clip of the track that has the lane. One undo step.
- **Delete take**: removes that index from every clip of the track. Its sections pass to the
  previous take index (the next index when deleting index 1). Later lane names and heights
  shift down with their indices. When one take remains, the clip becomes a normal clip and the
  lane goes. One undo step.
- **Flatten comp**: the comp becomes the clip's only sequence in every clip of the track that
  has takes; the other takes are removed. One undo step.
- **Take to new track**: copies the lane's take of every clip of the track, as normal clips at
  the same positions, to one new instrument track below the owner (no instrument copied). The
  lane name becomes the new track name. One undo step.
- **Move comp cut**: a cut between two sections can be dragged on the lanes. It stops at the
  neighbouring cut so each narrowed section keeps at least one grid step. Option frees it from
  the grid with a one-tick minimum. One undo step, `Move comp cut`. A cut is grabbed only on a
  take lane, within 4 points of the edge, with a horizontal-resize cursor and an accent edge on
  both lanes involved. Escape cancels.
- A clip that repeats follows the same comp on every repetition. A comp edit also updates the
  repeat source, and the visible sections are regenerated from it. Take deletion and flatten
  update both the materialized clip and its repeat source.
- A take can be renamed. The name belongs to the lane index across the track and defaults to
  `Take n`. A blank name restores the default.
- Each lane has its own height, 40 points by default, from 24 to 320 points in 4-point steps.
- Lane names, heights, and visibility are never an undo step. Undoing a take deletion restores
  the takes but not a shifted lane name or height; this is the accepted consequence of view
  state staying out of history.

### Comp editing

- The piano roll and inline editing show the comp. Moving, resizing, deleting, or changing a
  note edits it in its source take. A note moved so its attack leaves its take's section moves
  into the take active at its new attack, as T10 already does. Drawn and pasted notes go into
  the take active at their attack. Controller lanes follow the same rule per event.
- Double-clicking a clip's section on a take lane, or **Edit Take**, opens that take in the
  piano roll: every note and controller event of the take is listed, the events the comp does
  not play are dimmed, and edits write into the take without changing the comp. A 3-point
  accent strip under the ruler marks the ranges where the comp plays the take. Each edit is its
  usual one undo step.
- The Edit Take command with a focused lane opens the lane's take across every clip of the
  track in the multi-clip editor. Double-clicking one clip opens only that clip. Selecting a
  clip in the arrangement, closing the roll, or deleting or flattening the take returns the
  editor to the comp.
- Clip operations keep takes: move, copy/duplicate, resize, note-driven growth, split (every
  take and the comp are split), glue/join (takes kept per index, comp sections kept), delete.
  Split and duplicate allocate new take identities for the copied clips while preserving lane
  indices, names, and comp. Clip automation stays on the clip, not on the takes.

### Commands

All commands use the existing command registry, Commands search, and assignable keystrokes,
with live bindings in menus and tooltips. Until T18 lands, this is the existing application
command path with the same identifiers and availability rules.

- Record mode: Merge / Replace / New take (global).
- Track record mode: Global / Merge / Replace / New take (selected track).
- Show/hide take lanes (selected track).
- Use whole take, Delete take, Flatten comp, Take to new track (focused take lane).
- Rename Take, Edit Take (focused take lane; Edit Take also available with a selected clip with
  takes).

No command gets a default keystroke.

## Model and persistence

- Schema version 3. `Project` owns the global record mode. `Track` owns its mode choice and its
  take-lane view state (visibility plus per-index height and optional name).
- A v2 project migrates on open: its global mode is taken from the v2 MIDI arrangement, tracks
  default to Global with hidden lanes, and the file is written as v3 on the next save. A v1
  project migrates through the existing path and then to v3. Older projects open unchanged in
  behavior: Merge global, no lanes, no comp.
- `TrackMode` is `Global`, `Merge`, `Replace`, or `New take`. The effective mode is the track's
  mode, or the project's global mode when the track is on Global.
- Lane heights are validated to 24–320 points; names are non-empty after trimming and bounded
  like other project text. Lane counts are bounded by the existing take and project limits.
- A rendered comp note carries the identity of its source take and source event through the
  existing `EventId`, so an edit is redirected to the take without a stored copy. The comp is
  re-resolved in the same transaction as the take edit; playback adopts the new arrangement.
- Save stays atomic. An older project round-trips without data loss.

## Scheduling and realtime

- Comp resolution happens off the device callback when a take, section, mode, or clip changes.
  The callback consumes a bounded, immutable playback plan and performs no allocation, lock,
  formatting, logging, or I/O.
- Comp changes during playback are applied at a buffer boundary through the existing
  `set_arrangement` path with the existing note-chase and all-notes-off guarantees, so no note
  is left stuck and no half-updated plan is heard.
- The number of takes, sections, and notes is bounded by the existing clip limits; exceeding
  them fails the edit with a reason rather than degrading the callback.

## UI proposal

Derived from [track lanes](../UI/views/track-lanes.md),
[arrangement](../UI/views/arrangement.md),
[track header](../UI/views/track-header.md), and
[piano roll](../UI/views/piano-roll.md). The reference mockup is evidence of intent, not a
pixel target.

```text
surface Multitrack session — take lanes
  track row: name / instrument · arm · T n
  take lane (one per take index, only while T n is on and the track has two or more takes)
    header: Take n (or its name) · Plays … · ▾
    band: take region dimmed; comp sections at full strength; dashed comp cuts
    seam: 4-point grab under the lane for its height

surface Transport — record mode
  REC Merge ▾   (global mode, existing chip and menu)

surface Selected-track inspector — recording
  Record mode  [ Global ▾ ]   (disabled during a take)

surface Piano roll — Edit Take
  row: Take n · name        Back to Comp
  ruler strip: accent ranges where the comp plays this take
  notes and controllers: comp events normal; others dimmed
```

| Element | Role | States |
| --- | --- | --- |
| `T n` chip | Show or hide the track's take lanes; n is the highest take count | Hidden below 2 takes, off, on, hover, focus, disabled during a take |
| Take lane header | Name the take, show what it plays, open its menu | Default name, renamed, playing, not in comp, hover, focus |
| Lane menu | Rename…, Edit in editor, Use whole take, Delete take, Flatten comp, Take to new track | Enabled per lane and take-count availability |
| Lane band | Show the take region and its comp sections | Inactive, active, selected, comp drag preview |
| Comp selection | Make a take active over a grid range | Idle, dragging, snapped, committed |
| Comp cut | Adjust the boundary between two sections | Rest, hover, dragging snapped, dragging free with Option |
| Lane seam | Resize one lane | Rest, hover, dragging with preview |
| `Record mode` inspector row | Override the global mode for one track | Global, Merge, Replace, New take, disabled during a take |
| Edit Take banner | Identify the edited take and return to the comp | Comp view, take view |

- The `T n` chip sits in the track row beside the record-arm target, using the existing chip
  pattern. It reads the highest take count of the track and is hidden below 2. Toggling it
  saves immediately, marks the project dirty, and adds no undo step.
- Lanes sit directly under their track, inset under the header stripe. The default height is
  40 points; the lane's header shows the name and a short `Plays …` summary. A lane whose take
  never plays reads `Not in comp`.
- The lane paints each clip's take region at 8 % of the accent colour; the sections where the
  comp plays that take use 24 %, with a 1-point accent edge. Comp cuts are dashed 1-point
  accent lines. The lane also draws that take's note marks at their attack and duration, mapped low
  to high across the lane: full-strength where the comp plays the take and dimmed elsewhere.
- Comp by selection uses the shared range overlay and an accent readout chip with the snapped
  musical position. Option is not a bypass in this gesture; the drag is always grid-snapped.
- Hovering a cut within 4 points shows the horizontal-resize cursor and a 2-point accent edge
  on both lanes involved. Dragging previews the moved bands live and shows a `Cut bar.beat.tick`
  chip. Option frees the move from the grid; the one-tick minimum still applies. Escape
  cancels, and the release commits one `Move comp cut` step.
- The lane seam is a 4-point grab under the lane. Dragging previews the height, snaps to
  4-point steps, and saves on release. The minimum is 24 points and the maximum 320.
- Double-clicking a lane name edits it inline; Enter commits, Escape cancels, and a blank name
  restores `Take n`. A single click on the lane header outside its name applies Use whole take;
  the name has its own hit area, so the rename gesture never changes the comp.
  Double-clicking a clip section on a lane opens that take in the editor. The lane menu is a
  three-dot vertical button drawn with the accent palette, not a font glyph; it and the transport
  record-mode button share a filled `field` hover with no border. The lane menu carries the same
  take actions as the commands.
- The inspector's `Record mode` row is a labelled menu. It shows the track's choice, lists
  Global plus the three modes with the current choice ticked, and explains that Global follows
  the transport mode. It is unavailable during count-in, recording, pending transport
  transitions, or instrument loads. The transport chip remains the global mode.
- In Edit Take, the piano roll keeps its existing layout and adds the take banner, the comp
  strip, and dimmed out-of-comp events. The existing multi-clip lanes identify each clip. Edit
  commands write into the take. `Back to Comp`, Esc from the banner, or selecting a clip
  returns to the comp.
- All controls expose names and states to accessibility: `Take lanes, n takes`, `Take n, plays
  <range>`, `Record mode, <choice>`, `Editing take n`. The T chip, lane headers, menus, and
  banner are reachable by keyboard and the commands remain in Commands search. There is no
  desktop-style Tab order through lane names in this task; T18 owns the general focus model.
- View the surface at 760 and 1024 points: lane headers keep the name, `Plays` summary
  ellipsises first, and the menu stays reachable.

### Differences from the Composer reference

- No punch. The recorded range is capture start to Stop.
- Lane heights are set by the seam only; Clef has no Track Height command set to extend.
- Track colours do not exist yet, so lane bands use the accent role.
- Commands live in the existing Clef Commands window, not a native menu bar.
- Tab-order traversal across track and lane name fields is deferred to T18.
- Undoing a take deletion does not restore shifted lane names or heights, because view state
  is outside history.

## Interaction with neighbouring tasks

- T06 provides capture, the global mode, and the initial clip, take, and active-section model.
  T26 extends these without changing T06's playback guarantees.
- T09's split, join, duplicate, and move preserve takes and comp sections.
- T10 shows the comp and redirects note edits to source takes, including the dimmed Edit Take
  mode.
- Audio takes and audio comping are T27; this task must not add a second take model.
- T18 takes over the shared command registry and the full focus model; T26 uses the current
  command path.

## Out of scope

Audio takes and comping (T27); automatic alignment of takes; named whole-track playlists;
retrospective Capture MIDI; cross-track comping; articulation maps, automation lanes, and
chord/scale lanes; ARA (T29); punch; a new recording engine; desktop-style Tab order through
lane names.

## Resolved open questions

| Question | Decision |
| --- | --- |
| Punch and the recorded range | No punch. Actual capture start to Stop, or the loop range. |
| Schema layout | Version 3: project global mode, track mode and take-lane view state; migration from v2; shared with T27. |
| Comp representation | Virtual comp resolved from takes and sections; source identity through `EventId`; no rendered copy. |
| Lane heights | 40 default, 24–320 in 4-point steps, per lane, view state. |
| Take identity across clip operations | Split and duplicate allocate new identities for copied clips while preserving index order, names, and comp. |
| Undo for multi-clip comp drag | One `Comp take` step across every touched clip. |
| Empty recording | No take and no undo step; existing material stays intact. |
| Command names | As listed above; no default keystrokes. |
| Loop-clip comping | The comp edit updates the repeat source and regenerates the visible sections. |
| Track-wide lane actions | Lane actions apply to every clip of the track holding that take index, in one step. |

## Verification intent

- Deterministic comp logic at both 44.1 and 48 kHz: the attack rule, controller placement,
  section boundaries, cut clamping with grid and free modes, loop-clip regeneration, multi-clip
  comp ranges, delete fallback, flatten, and take-to-track copying.
- Record modes with and without loop: Merge sums, Replace removes only inside the recorded
  range, New take joins clips and creates one take per pass; track override precedence.
- One undo step per action; redo; no undo step for lane visibility, height, or name; mode
  changes undoable.
- Schema v3 round-trip; v2 migration keeps takes, comp, and modes; atomic save; older project
  opens unchanged.
- No stuck notes across comp changes while the transport runs; bounded callback work.
- Command availability and registry entries; no default keystrokes.
- egui checks for the T chip, lane rows and heights, comp drag and cut drag, rename, inspector
  row, Edit Take banner and dimming, and layout at 760 and 1024 points.
- Real-component snapshots compared with the approved boards, recorded as separate GUI
  evidence from automated tests and listening.

## Acceptance criteria

1. Global Merge with a track on Global behaves as T06, in and out of loop; a track override
   wins and returning to Global restores the global behavior.
2. Replace removes only inside the recorded range; a loop recording keeps the last non-empty
   pass; an empty pass changes nothing.
3. New take over a clip adds a take active over the range; over several clips joins them; on
   empty space creates a clip with one take; in loop creates one take per non-empty pass.
4. `T n` shows and hides lanes; a comp drag makes that take active over the dragged range in
   every touched clip, and boundary notes follow their attack with their full length.
5. Use whole take, Delete take, Flatten comp, and Take to new track work from the lane and the
   commands, each as one undo step, with the specified fallbacks.
6. A comp cut drags between its neighbours with the grid and with Option free, one undo step;
   Escape cancels.
7. Editing a comp note changes its source take; recomping keeps the edit; a drawn note lands in
   the active take. Edit Take lists all events of the take, dims out-of-comp events, and edits
   only that take.
8. Split, resize, duplicate, move, and join keep takes and comp sections.
9. Undo/redo per step; lane view state survives save and reopen and is not undoable; mode
   changes are undoable.
10. A v2 project opens with Global/Merge and no lanes, and saves as v3 without loss.
11. No stuck notes across comp changes while the transport runs.
12. Commands appear in Commands search with availability rules and no default keystrokes.
13. The surface works at 760 and 1024 points; automated proof stays separate from GUI,
    listening, and physical-keyboard evidence.

## Implementation plan

1. Model: schema v3 fields and migration; `TrackMode`; take-lane view state; comp operations
   on `Arrangement` (range, whole take, delete, flatten, cut move, loop-source regeneration).
2. Session: effective mode at Record; mode actions with history; track view-state action;
   comp-edit action with atomic validation and one history entry; take-to-new-track; tests.
3. Commands: registry entries, availability, dispatch, no default shortcuts.
4. Piano roll: take view, dimmed events, comp strip, Edit Take/Back to Comp, edits targeting
   the take; tests.
5. Arrangement and inspector: T chip, lane rows, band painting, comp and cut gestures, seam
   resize, rename, record-mode row; egui tests and layout checks.
6. Documentation: update the UI view files and index entries; TASKS status; verification
   evidence recorded here.

## Implementation — 2026-11-19

- Schema v3 in `crates/model/src/project.rs`: project global mode, per-track mode, per-track
  take-lane view state, v2/v1 migration to v3, and validation of lane heights and names. The
  legacy `Arrangement::mode` stays readable for v2 files but is no longer written.
- `crates/model/src/midi/comp.rs` owns the virtual comp edit: track-wide use-whole-take, range
  selection, delete with the previous/next fallback, flatten, cut move, loop-source regeneration,
  and copying a take lane to standalone clips. Take IDs stay stable per clip; copied clips get new
  identities.
- `crates/session/src/lib.rs` captures the effective mode at Record, adds
  `SetTrackRecordMode`, `TrackView`, `CompEdit`, and `TakeToNewTrack`, records one history entry
  for each comp and mode action, keeps lane view state out of history, and shifts or clears lane
  state when a take is deleted or flattened. Comp edits are accepted while the transport plays
  and are refused during capture, count-in, pending transitions, or instrument loading.
- Commands add the global and track record modes, Show/Hide Take Lanes, Use Whole Take, Delete
  Take, Flatten Comp, Take to New Track, Rename Take, and Edit Take. None has a default shortcut.
- The arrangement adds the `T n` chip, lane rows with names and `Plays …` summaries, lane
  painting, comp-by-selection, cut dragging, seam resizing, inline rename, the lane menu, and
  double-click Edit Take. The inspector adds the track `Record mode` row; the transport chip
  keeps the global mode.
- The piano roll adds the Edit Take banner, the comp strip, dimmed out-of-comp events, and
  take-targeted edits. Take events are listed through `roll_take_events`; the comp is unchanged
  by an edit.

## Verification — 2026-11-19

Automated:

- `rtk cargo check --locked`, `rtk cargo build --locked`, `rtk cargo test --locked`,
  `rtk cargo fmt --all --check`, and
  `rtk cargo clippy --all-targets --locked -- -D warnings` all pass.
- The locked suite reports 153 passed and 12 explicitly selected native cases skipped.
- Model tests cover the attack rule, controller placement, multi-clip comp selection, delete
  fallback, flatten, cut clamping, take-to-new-track copying, loop-source regeneration, v2
  migration with default lanes, and lane height/name validation.
- Session tests cover track-mode precedence and undo/redo, one-step comp actions, take-to-track
  undo, comping while the transport plays, lane view state saved but absent from history, and
  save/reopen.
- The egui tests exercise a take-lane comp drag, a cut drag, the take note marks and their
  active/dimmed states, double-click opening the take, the take-lane menu, the take-view event
  listing, the T chip and both lane headers through AccessKit at 760 and 1024 points, command
  availability with no default shortcuts, and the global borderless button hover.

Not verified: live GUI gestures in the running app (double-click timing, seam hover cursor,
scroll behavior), listening, physical-keyboard timing, and real-component snapshots. Those stay
separate user evidence and are not claimed here.

## Completion — 2026-11-19

The user reviewed the delivered take lanes, take notes, three-dot menus, uniform borderless button
hover, and double-click Edit Take, then accepted the result and asked to close the task. T26 is
removed from the open task index; this specification is its completion record.

Final state: schema v3 with v2 migration, virtual comp and take editing, track record-mode
overrides, commands without default shortcuts, lane view state outside history, Take to New Track,
and the piano-roll Edit Take mode. The locked suite reports 153 passed and 12 explicitly selected
native cases skipped; check, build, format, and Clippy pass. GUI evidence is the user's accepted
session; listening and physical-keyboard timing were not separately recorded because T26 added no
new audio path.

## Approval gate

Awaiting user approval of this specification and the written UI proposal. Implementation starts
after that approval. The reference decisions remain the product baseline; this document records
the Clef-specific decisions from the 2026-11-19 interview.
