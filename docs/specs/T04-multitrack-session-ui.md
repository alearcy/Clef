# T04 — Multitrack Session UI Proposal

**Status:** Behavior/layout approved 2026-11-17; functional implementation remains provisional under the later UI contract.
**Date:** 2026-11-17
**Specification:** [T04](T04-multitrack-session.md)

## Authority and Purpose

Extend the current [T03 egui view](../UI/views/single-instrument-project.md) so the
composer can select and play one instrument while other loaded instruments keep
their tails. Use the [UI index](../UI/index.md), [binding rules](../UI/README.md),
and [foundations](../UI/foundations.md). The user explicitly directed this work
to the repository UI references rather than the Impeccable skill.

Relevant library patterns: [track header](../UI/views/track-header.md),
[list row](../UI/components/list-row.md),
[selection edge](../UI/components/selection-edge.md),
[slot button](../UI/components/slot-button.md),
[notice box](../UI/components/notice-box.md), and
[quarantine notice](../UI/views/quarantine-notice.md).
Legacy controls outside T04's agreed scope do not become requirements.

## Layout

Keep the eframe main window at T03's initial 1024 × 640 and minimum 760 × 480.
Project commands and project/unsaved identity stay at the top, followed by the
shared Enable/Disable Audio action. The workspace becomes two adjacent regions:

- **Track list, left:** 320-point preferred width, shrinking to 240 at the minimum
  window size. A heading with track count and Add Instrument Track precedes a
  vertically scrolling list. Each 44-point two-line row shows the track name,
  plugin/format or “No instrument”, and a readable lifecycle status. Fixed column
  placement keeps the list scannable. Rows keep the panel surface, with the standard
  accent selection edge; no track-color background wash.
- **Selected track details, right:** the selected track name with inline Rename,
  instrument slot, Choose/Replace Instrument, Open/Close Editor, Move Up/Down,
  Remove Track, and recovery actions when applicable. Show the shared MIDI input
  and stereo destination as a routing summary, with Audio/MIDI Settings available.
  No mixer strip, fader, mute, solo, meter bank, or timeline placeholder.
- **Status, below:** shared device configuration, actual callback sizes and latency,
  concise session message, and a bounded disclosure for per-track counters/errors.
  Unavailable measurements show a dash; no fabricated CPU percentage.

Use 12-point workspace spacing, 24-point control height, 13-point body text,
11-point secondary text and 16-point headings from the library. Separate regions
with spacing and background roles. Controls wrap in the details region; names
ellipsize with full tooltips. The track list and long diagnostics scroll separately
so project/audio actions remain reachable. Preserve Composer Dark tokens.

## Track Interaction

Click a row to select it and request lazy loading if needed. Clicking the initially
highlighted row after Open also counts as explicit use. Up/Down in the focused
track list selects adjacent rows; selection is exposed independently of focus.
Double-click a name or invoke Rename to edit; Enter commits, Escape cancels.

Move Up/Down provides keyboard-accessible ordering without requiring drag-and-drop.
At the first/last row, the impossible action is disabled. Removing a track opens
a confirmation naming the track and loss of unsaved state; cancel changes nothing.
Replacing a plugin uses the existing picker and explicit state-loss confirmation.

Selection immediately changes the live MIDI destination. Show “Loading” on a new
selection; new input is discarded until ready. Returning to an already loaded track
is immediate. Changing selection closes the previous original editor, retaining its
worker and release tails. An asynchronous load result is tied to track ID and plugin
identity, never to the row's current index.

## States and Feedback

Rows show text states: Empty, Not loaded, Loading, Ready, Missing, Unavailable,
or Quarantined. Audio enabled/stopped is a shared session state, not an extra track
monitor toggle. Loading or failure on one track does not disable unrelated tracks.
Block conflicting project/save/settings actions during a coherent transition;
keep Disable Audio and safe cancellation available.

Missing retains name, plugin identity and saved state, with Retry after installation.
Quarantined displays the recorded reason and “Reactivate Plugin…”; its confirmation
lists all affected tracks and explains that unsnapshotted edits were lost. Reactivation
does not automatically load every instance or enable audio. The picker marks blocked
identities and prevents choosing them until explicitly reactivated.

On Open with quarantined identities, adapt the existing once-on-open notice pattern:
plugin, affected track, reason; Continue dismisses without recovery, and
“Show Quarantined Plugins…” opens a compact egui list with the same Reactivate command.
During a session, show a persistent status message and row state without interrupting
healthy playing with a modal notice. Quarantine registry failures remain visible.

Save normally stops audio/closes editors before capturing every healthy loaded state.
If crashed instances have uncertain edits, an explicit confirmation names the affected
tracks: “Save Using Retained State” or Cancel. No automatic fallback. Existing
New/Open/Close Save/Discard/Cancel and overwrite/external-change safeguards remain.

An empty session shows Add Instrument Track in the workspace. A project with only
unavailable tracks still supports rename, reorder, remove and state-preserving save.

## Commands and Accessibility

Extend the existing shared command definitions, enablement, dispatch, command search,
and shortcut reassignment for Select Track, Add/Remove, Rename, Move Up/Down,
Choose/Replace, Open/Close Editor, Retry, Show Quarantined Plugins and Reactivate.
Use stable IDs in parameterized actions. Buttons, row/context actions, and shortcuts
must reach the same implementation. Disable controls with a readable reason.

Keep AccessKit semantics, accessible names including track identity/state, visible
keyboard focus, meaningful tab order, and non-color error/selection feedback.
Do not intercept list navigation while editing text. Plugin editors remain original
plugin-owned content inside the existing eframe windowing path.

## Verification and Documentation

Before claiming UI acceptance, capture actual egui states at initial and minimum
window sizes: empty, eight tracks, long names, loading, missing, quarantine notice,
reactivation and retained-state save. Compare them with this proposal and inspect
the accessibility tree/keyboard actions. Listening and physical MIDI acceptance are
recorded separately in T04.

With implementation, add the current Clef multitrack view to `docs/UI/views/`, link
it in both UI indexes, and mark the T03-only behavior as superseded where appropriate.
Until then this document is a proposal, not a description of implemented behavior.

## Current Contract Gate — 2026-11-18

The [UI contract](../UI/contract.md) now requires the T18 primitive/gallery batch
before final visual acceptance. The functional inline egui surface and observations
above do not constitute that acceptance. No primitive or surface is claimed to have
a committed egui golden yet. See the [T04 verification](T04-multitrack-session.md#implementation-verification-and-scope-confirmation--2026-11-18).

## Functional Verification and Visual Boundary — 2026-11-18

The user explicitly retained a functional, provisional T04 UI and kept T18 separate.
Actual egui/AccessKit checks verified the functional states, normal/minimum widths,
and keyboard selection with scroll-to-selected behavior. The physical-keyboard/listening
trial has been requested and is still pending. This does not accept the missing shared primitive
contracts, SF Pro/theme migration, gallery cases or committed visual goldens.
See the [T04 verification](T04-multitrack-session.md#implementation-verification-and-scope-confirmation--2026-11-18) for the evidence and limits.
