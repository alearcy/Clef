---
name: Clef Multitrack Session
kind: view
status: app
impl: app
source: [crates/ui/src/lib.rs, crates/session/src/lib.rs, crates/engine/src/multitrack.rs, crates/model/src/project.rs, crates/plugin-host/src/quarantine.rs]
components: [text-button, menu-button, text-field, list-row, selection-edge, notice-box]
snapshot: pending
---

> **Implementation status (2026-11-17):** inline in `crates/ui/src/lib.rs` with stock egui widgets and a
> partial palette; no primitive is `gallery` and no renderer snapshot exists. This surface is
> provisional: the prose below is the approved design, not what the code already achieves. See
> the [UI contract](../contract.md).

# Clef Multitrack Session

## Purpose

The current Clef workspace extends T03 to ordered instrument tracks. The
[T04 specification](../../specs/T04-multitrack-session.md) and
[written UI proposal](../../specs/T04-multitrack-session-ui.md) were approved on
2026-11-17. This is a Rust/egui translation of the repository UI library.

## Anatomy and Metrics

Keep T03's 1024 × 640 initial content area, 760 × 480 minimum, project command row,
project identity, unsaved indicator and explicit Enable/Disable Audio control.
The workspace uses two top-aligned vertical regions: a 320-point preferred track
list (240 at narrow widths) and selected-track details filling the remainder.
Use 12-point padding/gaps and the existing Composer Dark roles.

The track list has a count, Add Instrument Track, and independent vertical scroll.
Each 44-point row shows a 13-point name and 11-point plugin/format/status caption,
each independently truncated with the full accessible label and tooltip retained.
The selected row has the standard accent selection edge over panel; hover uses
field. Keyboard focus has its own inset accent edge, distinct from selection.

The details region contains inline name editing, Rename, Move Up/Down, Remove,
instrument identity, Choose/Replace, Open/Close Editor, Retry/Reactivate, and the
shared MIDI/output summary. Controls wrap; the details scroll independently.
Bottom diagnostics/status scroll within the remaining window height. There are
no timeline, mixer strips, faders, mute/solo, pan or artificial CPU readouts.

## States

Empty, Not loaded, Loading, Ready, Missing, Unavailable and Quarantined are text
states, not color-only badges. Disabled text uses the disabled role. An empty
session offers Add Instrument Track. Unavailable data is never replaced by a
fictional value. Diagnostics expose track latency, processed/missed/stale blocks,
transition frames, MIDI input/drop counts and maximum RPC/worker time, plus the
actual device callback size and final-output clipping count.

## Behavior

Click or keyboard-select a track to load it on first use. Project Open restores
only metadata/state and initially highlights the first track; audio then starts by
itself with the effective audio configuration (T39: project override, else Clef's
global preferences, else the macOS default output). Clicking that initially highlighted
row explicitly requests loading. Open Editor can also request the first load. Starting
the audio engine loads every online, unquarantined instrument track in the background (T38), and a
Play pressed meanwhile shows the pending state until those loads finish and the engine's latency
alignment reaches every attached track; a failed load does not hold Play.
Loading and activation run in a track job; late completions are checked against
project generation, stable track ID and request version.

Loaded instruments stay loaded on deselection, while subsequent MIDI routes only
to the new selection. Selection releases held notes/sustain on the old path and
preserves release tails. At most one original editor is open; changing selection
closes the previous editor without unloading it. No input is replayed after loading.

Names commit through Rename/Enter, selection change, Add or Save; Escape cancels
staged name editing. Reorder does not change track identity or worker ownership.
Remove and Replace explicitly confirm potential state loss. New/Open/Close retain
T03's Save/Discard/Cancel protection. Save stops audio/closes editors for capture.

A worker fault quarantines all instances of its plugin identity; other identities
continue. Missing remains distinct and preserves saved bytes. A once-on-open
quarantine notice lists plugin, track and reason. Continue, Return, Escape or an
outside click dismisses it; Show Quarantined Plugins opens the shared manager.
Session-time faults use persistent row/status feedback without a disruptive modal.

Reactivate names the affected tracks and warns about lost unsnapshotted edits;
clearing the block loads nothing and does not restart audio. Retained-state Save
requires explicit confirmation naming the affected tracks. Consent is tied to
specific failure versions, so it cannot authorize a subsequent failure silently.

## T06 arm and arrangement

Each instrument row has a Record Arm target. Its accessible name includes the track name and its
current action. The target fills when armed and squares while a take records. The armed track receives
live MIDI independently of selection. After disarm, live input resumes on the selected track. At most
one instrument is armed. The target is guarded throughout count-in and capture.

The recording preview and committed MIDI clips share the timeline transform with the ruler and
playhead. Committed clips draw read-only note marks from their active sections below the label.
The danger recording preview draws note attacks as they arrive, extends held notes with the
playhead, and fixes their length on release. In loop recording, the latest completed
pass containing notes stays dimmed underneath the current pass and shares its time and pitch
coordinates in Merge, Replace, and New take. A committed clip shows its type and take count where space allows. Its accessible
description includes the track, musical range, and take count.
Take lanes and comp editing are specified in [T26](../../specs/T26-midi-comping.md). See
[T06](../../specs/T06-midi-recording.md) for states and playback rules.

## T26 take lanes and comping

A track with two or more takes shows the `T n` chip beside its arm target; the chip is hidden
below two takes and toggles the track's lanes. Lanes sit directly under the track at their saved
heights (40 points by default, 24–320). Their headers carry the take name and a `Plays …` summary,
with a `▾` menu for Rename…, Edit in editor, Use whole take, Delete take, Flatten comp, and Take to
new track. Lane bands use the accent role at 8 %, with the comp sections at 24 %, dashed cuts, and
the take's note marks drawn low to high at full strength on the comp and dimmed elsewhere.

A Select drag on a lane comps that take over the dragged range in every touched clip of the track,
as one `Comp take` step. A cut drag waits for the press before it moves, snaps to the grid, stops
one grid step before its neighbours, and uses Option for a free one-tick move. Double-clicking a
clip's lane section opens that take in the editor. The selected-track inspector adds the track's
`Record mode` menu; the transport keeps the global mode chip. All lane view state is saved with the
project and never enters the edit history.

## T09 arrangement editing

The provisional Rust timeline adds clip selection, a marquee, a Snap control,
and clip context menus. Click or Command-click to select clips. Drag the body
to move the selection in time or between tracks. The upper right-edge target
extends the clip by whole cycles, using its visible length on the first loop.
The lower target changes the clip length using the arrangement snap setting; on a
repeated clip it rounds up to whole cycles, so no material sits past the repetition.
Repetition boundaries are drawn inside the clip. The moved clip is drawn in front of lower
clips in the same track, and pointer hit-testing uses the same stacking order.

Hover and selection expose the two edge targets. Keyboard focus uses a separate
outline. A drag shows the proposed bounds and musical position. The grid line
shows the resolved snap target and the readout identifies free movement.
An invalid edit reports its reason in the session status. Pending moves and
resizes keep their preview until the session responds. Snap, scroll position,
and zoom are saved with the project.

Right-click or use Shift-F10 in the focused arrangement lane to open its menu.
The menu reads the live bindings for Split at Playhead, Duplicate, and Delete.
An empty lane opens Create Clip. Tab reaches visible clips and Enter selects the
focused clip. Arrangement shortcuts are scoped to arrangement focus. The
Commands window lists the bindings for moving in time or between tracks,
resizing, editing the repeat extent, and selecting all clips. The split
confirmation offers Split Notes and Cancel. Escape cancels the pending split
or an active drag.

See [T09](../../specs/T09-arrangement-editing.md) for scope and verification.

## Commands and Accessibility

The project command row exposes Undo, Redo and the revision slider through the
[Edit History](edit-history.md) menu and window.

Project/track/recovery controls use the shared command definitions, enablement,
dispatch, command search and session shortcut reassignment. Parameterized track
selection/name actions use stable IDs. Up/Down in a focused row navigates tracks;
text fields retain their own editing/navigation. Names, lifecycle states, selected
state and recovery actions are exposed through AccessKit. Keyboard and pointer
focus cues are separate. UI instructions remain English.

## Verification

Actual macOS egui screenshots and accessibility trees were inspected during T04,
including long names, Missing, quarantine while a healthy path remains active,
retained-state cancellation/confirmation, app-restart quarantine notice and explicit
reactivation. There is no committed renderer snapshot yet; screenshots in the task
conversation are observations of this Rust surface, not legacy kit snapshots.
The [T04 verification](../../specs/T04-multitrack-session.md#implementation-verification-and-scope-confirmation--2026-11-18) records final
checks, native measurements and outstanding listening/physical-MIDI evidence.

## Layout Tree (Provisional Composition)

```text
surface multitrack-session (1024 x 640, min 760 x 480)
  column gap=workspacePadding pad=workspacePadding
    row wrap hug: project commands
    row hug: project identity + unsaved status
    row wrap hug: audio command + audio state
    row fill top-aligned
      column fixed(320 -> 240 at narrow width)
        row wrap hug: track count + add command
        scroll fill: track list (rows fixed(44))
      column fill
        scroll fill wrap: selected-track details + routing summary
    scroll hug: diagnostics + actual device configuration + session status
```

The 320/240-point column widths and 44-point rows are the explicit T04 proposal
choices. They remain inline implementation values until the theme/primitive migration;
this document does not claim that they already exist as Rust theme tokens.

## Primitive Mapping and Acceptance Gate

| Primitive | Role | Commands/state | Current evidence |
| --- | --- | --- | --- |
| text-button | Project, track, audio and recovery actions | Shared registry; enabled/disabled | Provisional app widgets |
| text-field | Rename and project path | Enter/escape/staged text | Provisional app widgets |
| list-row | Track identity/plugin/status | Stable-ID selection; long/missing/quarantined | App::track_row; observed AX/screen |
| selection-edge | Selection distinct from focus | Selected and keyboard-focused | Inline app painter |
| notice-box | Once-on-open quarantine information | Continue / Show Quarantined Plugins | Observed app modal |
| window-frame | Clef settings/pickers/recovery dialogs | Close, keyboard and modal focus | Provisional egui containers |
| caption | Identity, status and diagnostics | Heading/body/secondary/unavailable | Provisional styles |
| menu-button | Inherited Audio/MIDI setting selectors | Device/rate/buffer selection | Existing T03 raw controls |

- [x] Functional layout/commands and AccessKit names observed.
- [x] Long-name truncation and actual minimum-width resizing exercised.
- [ ] Theme/font/token implementation matches the contract.
- [ ] Required primitives have exact Rust contracts, gallery states and goldens.
- [ ] Surface is recomposed without raw egui controls or inline painting.
- [ ] 1024/760 state snapshots are committed and reviewed as visual acceptance.

The unchecked items are the explicit [UI contract](../contract.md) / T18 dependency,
not a green check inferred from `cargo test` or an observed functional screenshot.

## Provisional UI Scope Confirmed — 2026-11-18

The user explicitly kept T04 functional/provisional and T18 separate. The requested
physical-keyboard/listening trial remains pending. Functional verification does not
turn any planned primitive into `gallery` or close the unchecked design-system gates above. The final
keyboard path scrolls the selected row into view, including row eight and narrow layouts.

The selected-track details and transport draw from the same workspace state. Session waits
retain the appearance of otherwise available buttons. Longer work still displays the
working indicator. See [T25](../../specs/T25-ui-flicker.md).
