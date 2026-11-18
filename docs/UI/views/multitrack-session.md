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
only metadata/state, initially highlights the first track and leaves every worker
unloaded and audio stopped. Clicking that initially highlighted row explicitly
requests loading. Open Editor and Enable Audio can also request the first load.
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
clearing the block loads nothing and does not enable audio. Retained-state Save
requires explicit confirmation naming the affected tracks. Consent is tied to
specific failure versions, so it cannot authorize a subsequent failure silently.

## Commands and Accessibility

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
