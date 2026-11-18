# T04 — Multitrack Session and Instrument Engine

**Status:** Implemented and verified automatically/natively. The user confirmed playback on all eight instrument tracks with a physical MIDI keyboard. Specific listening checks remain open. UI remains provisional by explicit agreement; T18 stays separate.
**Priority:** P0.
**Date:** 2026-11-17
**Task:** [T04](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T03 acceptance; T02 realtime and isolation invariants.

## Goal

Turn the single-track T03 application into a multitrack instrument session: an ordered
track list, one isolated plugin worker per track, a bounded realtime mix to a stereo
master, and project schema v2 with a no-loss migration from T03 v1 files.

## Scope

- A track model with stable identity, order, name, instrument identity, and plugin state.
- A worker pool that loads, unloads, and supervises one instrument worker per track.
- A realtime mix path that sums tracks into the master without locks or allocation in the
  audio callback, with per-track delay compensation and miss counters.
- Track management UI: add, remove, reorder, rename, select, and open the instrument
  editor. Timeline, clips, and mixer strips remain out of scope.
- Schema v2 persistence, validation, atomic save, and migration from T03 v1 documents.
- Per-track failure isolation: one failed or missing plugin must not silence or crash the
  other tracks.

## Out of Scope

Audio, bus, or FX track types; timeline and transport; recording; insert effects; the
mixer surface; automation; folder tracks.

## Interview Decisions — 2026-11-17

The user confirmed the following product choices. These decisions refine the draft;
they do not constitute approval of the complete specification or UI proposal.

- Route live MIDI only to the selected track.
- Use eight instrument tracks for the initial validation milestone; determine capacity
  through measurement rather than treating this target as the definitive track limit.
- Load instruments lazily when needed rather than eagerly on project open.
- Quarantine all instances of the same plugin after a crash, across application/project
  reopen, until explicit user reactivation. Missing plugins use a distinct Missing state
  and retain their saved data.
- Load on first selection and retain loaded instruments until project close (or explicit
  removal/replacement). On selection change, release held notes on the old track and let
  its tails finish; route subsequent live MIDI only to the new selection.
- Defer per-track volume, mute, and solo controls to T11 (Mixer).
- Retain a single project file for T04; defer the recorded-audio storage decision to T14.

## Proposed Behavioral Contract

The following details complete the specification for approval. They are proposed
implementation behavior, not additional decisions already confirmed by the user.

### Track identity, selection, and lifecycle

- A project may contain zero or more ordered instrument tracks. Each has a stable,
  non-reused project-local ID, nonblank name, optional plugin identity, and opaque state.
  Reordering never changes IDs, routing, state ownership, or worker ownership.
- Add creates and selects an empty track. The existing searchable instrument picker
  assigns its instrument; replacement requires confirmation before discarding old state.
  Failed replacement retains the previous track data and recoverable worker.
- Remove requires confirmation, releases notes, closes its editor, detaches its audio
  path, and reaps its worker outside the callback. Select the next remaining track, or
  the previous one if removing the last row. An empty project has no selected track.
- Rename and reorder are project edits. Selection and editor visibility are view state;
  they do not alone dirty the project. Editor exposure and live playing continue to mark
  plugin state potentially modified, as in T03. Undo is deferred to T08.
- Open validates and restores metadata/state without starting any instrument worker or
  enabling audio. Highlight the first track without interpreting initial highlighting
  as a user load request. Explicitly selecting that row, opening its editor, or enabling
  audio on it requests its first load. Assigning an instrument to a new track loads it.
- Loading is asynchronous and must not stall healthy loaded tracks or the UI. MIDI
  received while the new selection is loading/unavailable is discarded, never buffered
  for later playback and never redirected to another track. Completion does not enable
  audio if it is stopped. A late completion for a removed/replaced track is discarded.
- Switching tracks releases held notes and sustain on the old track, without All Sound
  Off that would truncate ordinary release/reverb tails. Notes already held on the
  keyboard do not retrigger on the new track. Late note-offs cannot terminate new notes
  on another track; routing changes have an ordered event boundary.
- Reordering, renaming, or selecting an already loaded track does not reload a plugin.
  Loaded healthy tracks remain processed while audio is enabled, including their tails.
- Keep at most one original plugin editor open at a time; changing selection closes the
  old editor without unloading its instrument. New/Open/Close release all session workers.

### Realtime mixing and device behavior

- Keep one audio output device/stereo pair and one physical MIDI input per session.
  Preserve T03 explicit Enable/Disable Audio, supported-device validation, and absence
  of silent fallback routing. Reopening always leaves audio stopped.
- Sum independent stereo instrument paths at unity gain into a stereo master. No track
  or master volume, pan, mute, solo, normalization, or limiter is introduced in T04.
  Preserve floating-point headroom internally; clamp only final device output to
  [-1, 1] and report clipped samples. This output safeguard is not a mixer control.
  Invalid/non-finite worker output is rejected before it can contaminate other paths.
- Each loaded track has its own isolated worker and bounded communication path. A
  blocked worker must not serialize or stall processing of healthy workers. Extend the
  current broker/queue pattern; never call synchronous plugin RPC from the callback.
- Align valid track output on one session sample cursor. Compensate each active path to
  the maximum active plugin latency and include existing transport buffering in reports.
  Delay buffers and topology changes are prepared outside the callback. Latency changes
  invalidate stale generations; any transition silence is bounded and counted, never
  replaced by replayed audio. A quarantined/missing path contributes silence only.
- A missed track deadline contributes silence for that path for that block and increments
  its counter; an isolated late block is not by itself a reason for quarantine. Discard
  stale audio. Other ready paths still reach the master at their correct positions.
- No allocation, locks, waiting, file/control I/O, worker teardown, or unbounded queue
  drain in the device callback. Publish prepared changes at bounded block boundaries;
  reclaim old resources off the callback. Keep per-track misses/faults and master/output
  counters observable; report measured callback size separately from requested size.
- Applying audio/MIDI settings stops audio. A sample-rate change snapshots healthy
  loaded instruments and recreates only those already loaded at the new rate; lazy
  tracks stay unloaded. Abort on snapshot failure without discarding retained state.
  Audio remains stopped until explicit Enable. Device loss stops the shared stream,
  preserves every track/device choice, and does not quarantine plugins.

### Quarantine and recovery

- Quarantine is application-local and persists independently of `.clef` projects. Its
  key is plugin format, canonical bundle path, and plugin entry ID, matching the existing
  catalog identity. Different entries in a multi-plugin bundle and VST3/CLAP variants
  remain distinct. Retain a normalized saved identity when a bundle is missing.
- Unexpected worker exit, a supervised plugin-operation timeout, or invalid plugin IPC
  quarantines that identity. Normal worker shutdown, device errors, and ordinary late
  audio blocks do not. An ordinary reported load error retains a readable unavailable
  state; a missing bundle uses Missing rather than Quarantined.
- Detach/silence all loaded instances of the quarantined identity, block new loads of
  it, and retain all track identities and last recoverable state blobs. Healthy tracks
  with other plugin identities continue. Quarantine is checked before loading, including
  picker assignment; Rescan does not clear it or execute quarantined bundles.
- Persist a bounded record with reason and time atomically. If persistence fails,
  retain the in-memory block and visibly report that it may not survive app restart.
  Invalid quarantine data must not silently be treated as an empty healthy registry.
- Explicit Reactivate shows the affected tracks and warns that unsnapshotted edits in
  terminated instances cannot be restored. Clearing quarantine does not load all
  instances or turn on audio: the selected track may be retried explicitly, with other
  tracks returning to lazy loading. A repeated crash re-enters quarantine.
- Retain the last recoverable state (project-loaded or successfully captured in-session)
  separately from uncertain runtime edits. Save captures all healthy loaded instances;
  missing and never-loaded instances retain their bytes unchanged. After a crash with
  potentially lost edits, Save requires explicit consent to use retained state for the
  named affected tracks. Refusal cancels Save and any pending transition.

### Single-file schema v2 and saving

- Read v1 and v2; write v2. Migrate v1's optional single track to zero/one stable-ID
  tracks, preserving name, full plugin identity, audio settings, and every state byte.
  Open/migration never writes back automatically. The original file remains intact
  until an explicit successful Save; Save As can retain the original v1 file.
- Use a versioned metadata section containing an ordered track array and an ordered
  table of state lengths, followed by raw state blobs in that same order. Each state
  belongs to exactly one track ID; do not serialize bytes as JSON integer arrays.
  Quarantine data, live counters, selection, and editor visibility are not project data.
- Proposed defensive limits: 128 stored instrument tracks, 1 MiB metadata, the existing
  64 MiB per plugin state, and 512 MiB combined state payload. These are parser/resource
  safety ceilings, not a claim that 128 instruments run in realtime. Reject exceeding
  them visibly; validate lengths and checked totals before allocation/read.
- Reject unsupported versions, duplicate/invalid IDs, malformed metadata/identity,
  orphan state, inconsistent lengths, truncation, and trailing data. Invalid Open
  preserves the current session. Keep original plugin binaries/sample libraries external.
- Save stops audio and closes editors before coherent state capture, as T03 does.
  Capture every needed healthy state into a candidate project before publishing it;
  a capture/write failure keeps the existing disk file and dirty flag intact. Never
  clear an unavailable track's blob to make saving succeed.
- Retain atomic same-directory file publication, replacement confirmation, external
  change detection and concurrent-save protection. New/Open/Close retain the shared
  Save/Discard/Cancel transition contract across the whole project.

## Written UI Proposal

See [T04 UI proposal](T04-multitrack-session-ui.md). It extends the current T03 egui
surface using `docs/UI/index.md`, `README.md`, and `foundations.md`. Both this
specification and that proposal require approval before implementation.

## Acceptance Criteria and Evidence Plan

See [implementation verification](#implementation-verification-and-scope-confirmation--2026-11-18) for results and remaining gates. Eight tracks is the initial scale target;
performance feasibility is measured rather than promised for arbitrary plugins.

1. **Track editing:** add/remove/rename/reorder with stable IDs; selection and worker
   completion races cannot misassign state, MIDI, or editors. Validate zero, one, eight,
   and parser-limit sessions; refused edits preserve the previous state.
2. **Lazy lifetime and MIDI:** no workers at Open; first explicit use loads once;
   deselection retains workers and release tails; held notes/sustain and rapid selection
   changes do not stick or leak to the new track. Test a switch during an in-flight load
   and while MIDI is queued. Physical keyboard/listening evidence remains separate.
3. **Multitrack audio:** deterministic two-/three-worker signals sum exactly; unequal
   and changing latency paths align sample-accurately across callback partitions. Eight
   fixture workers run together with recorded configuration, counters, processing/IPC
   timings, and output; record commercial-plugin feasibility separately. No invented
   CPU target or broad compatibility claim.
4. **Failure isolation:** kill/hang one worker while another distinct plugin sounds;
   the healthy path continues. Quarantine all instances of only the failing identity;
   verify missing bundles, late blocks, device loss, and normal close do not trigger it.
   Restart the app and reopen another project to verify persistent blocking; reactivation
   is explicit, lazy, and warns about lost edits. Test registry read/write failures.
5. **State preservation:** v1 migration and v2 round trips retain order/identity/routing
   and byte-exact state, including unloaded, missing, and quarantined tracks. Capture
   failure on the nth track cannot publish a partial save; retained-state fallback
   requires consent. Test malformed lengths/IDs, truncation, future versions, atomic
   write failure, external changes, and competing saves.
6. **Realtime safety:** allocation/locking audit and callback-focused proof for mixing,
   selection, stale generations, worker failure, and topology changes; counters distinguish
   track silence, transition gaps, clipping, and output failures. No synchronous RPC,
   teardown, or unbounded wait on the callback.
7. **UI and native workflow:** compare actual egui screenshots to the approved written
   proposal; inspect focus/accessibility and narrow-window overflow. Validate original
   editors for Pianoverse VST3 and ZebraHZ VST3/CLAP, state save/reopen, quarantine notice,
   and keyboard selection. User listening/physical MIDI confirmation is required for
   selected-track routing, note release/tails, and healthy-track continuation.
8. **Regression gates:** run the repository's locked check/build/test, fmt check and
   warning-free clippy commands; explicitly rebuild native fixtures before ignored
   native tests (see `patches/vst3-host/README.md`). Preserve applicable T02/T03 contracts;
   revise single-track/eager-open assertions only where T04 explicitly supersedes them.

## Source Investigation — 2026-11-17

- Updated the existing code graph with `rtk proxy graphify update .` and queried the
  `src/session.rs`, `src/live.rs`, and `src/project.rs` seeds. Verified the source directly.
- `session.rs` owns one `Instrument`, `Live`, saved state, and async backend; extend
  this ownership to stable-ID track runtimes rather than creating a second session path.
- `live.rs` already separates the callback from synchronous worker exchange with bounded
  queues and generation-tagged output. Extend this for independent track scheduling;
  a serial loop of blocking worker RPCs cannot satisfy failure isolation.
- `graph.rs` owns compensated sums but currently copies the same input to every Input
  node. It needs distinct track inputs to implement a real multitrack sum; do not
  misrepresent the parallel-delay fixture as multiple independent instruments.
- `project.rs` owns bounded v1 decoding and atomic save; extend its format/version
  boundary. `catalog.rs` owns canonicalized plugin identities but no quarantine registry.
- `app.rs` owns shared commands and the T03 surface. Reuse its dispatch/picker/settings
  paths and extract only shared row/control behavior needed by T04.
- Graphify reported partial extraction in 28 vendor C++ files and 52 files without
  extracted symbols; it is a navigation aid, not exhaustive evidence. The code graph
  contains 10,972 nodes; HTML uses the aggregated community view. Planning documents
  are not semantically indexed by the code-only updater.

## Implementation Record — 2026-11-18

- `project.rs`: version-2 ordered tracks, stable project-local IDs, length-table/raw-blob
  persistence, strict validation and v1 migration. Safety ceilings remain 128 tracks,
  1 MiB metadata, 64 MiB per state and 512 MiB total state.
- `session.rs`: stable-ID runtimes, cancellable asynchronous load jobs, activation
  continuation when audio starts during loading, one editor, coherent capture and
  version-specific loss consent. Inactive project-open loads restore without activating
  DSP, retaining T03's byte-exact changed-state restoration proof.
- `multitrack.rs`: independent brokers/queues, one stereo callback, per-path compensation,
  selection-bound MIDI ownership, safe generation changes, final-output clipping protection,
  and per-path diagnostics. The existing graph delay primitive is shared; the old T02
  single-input `Plan` remains available for diagnostics rather than being expanded into
  the future routing task. `Mix` owns the sum of actual independent worker paths.
- MIDI timestamps interpolate from a consistent atomic device-clock anchor. Stream startup
  wall time is not treated as device sample time. Source catch-up subtracts the transport
  lead before deciding a deadline was missed; completed packets are not counted as stale.
- `quarantine.rs` and catalog/session integration: persistent identity-level blocking,
  quarantine-safe scanning, explicit reactivation, retained states and registry failures.
- `app.rs`: functional multitrack commands, list/details, diagnostics and recovery flow.
  Actual egui screenshots/accessibility trees were inspected. The later UI contract marks
  this inline implementation provisional until the shared primitive/gallery batch lands.
- No new DSP/hosting dependency or application-window technology was adopted.

## Remaining Approval and Measurement Boundaries

- The original T04 specification and written UI proposal were approved on 2026-11-17;
  their approval is not pending again.
- The new [UI contract](../UI/contract.md) requires shared primitives, SF Pro/theme tokens,
  gallery cases and committed goldens. It explicitly keeps the T04 surface functional but
  provisional until the first batch lands. [T18 Phase 1](T18-ui-foundation.md) is registered
  separately and still awaits its own approval; T04 cannot claim final visual acceptance.
- Eight commercial instruments have been measured with native output and virtual MIDI,
  including a zero-miss observed interval after the catch-up correction. This is not a
  general performance budget. See [implementation verification](#implementation-verification-and-scope-confirmation--2026-11-18) for the measured configuration.
- The physical keyboard trial confirmed playback on all eight tracks, selected-track-only
  routing, and held-note and sustain release with tails. User confirmation remains open
  for restored sound and uninterrupted audio, including healthy-track continuation
  during another worker's failure.

## Approval Gate

The user explicitly approved this specification and its linked written UI proposal on
2026-11-17 and requested implementation through completion. Implementation is authorized.
Listening and physical-MIDI evidence remain distinct from automated and GUI checks.

## Implementation Verification and Scope Confirmation — 2026-11-18

The user explicitly chose: “Mantieni T04 funzionale e provvisorio; T18 resta separato”.
This retains the functional T04 UI and defers theme/primitive/gallery conformance to T18;
it is not visual acceptance of the unfinished design system. No T18 implementation
was started under T04.

The user confirmed playing each of the eight MIDI tracks with a virtual instrument in
the review project using a physical MIDI keyboard. The user also confirmed that only
the selected track received new notes and that held notes and sustain released while
preserving release tails. The remaining listening checks are listed in
[Remaining Approval and Measurement Boundaries](#remaining-approval-and-measurement-boundaries).

Verification completed: 53 default automated tests passed, 9 native tests passed when
explicitly selected, locked check/build, fmt check and warning-free clippy passed.
Actual GUI/AccessKit inspection covered normal and minimum window sizes, lazy loading,
keyboard selection/scrolling, missing/quarantined plugins, retained-state Save, and
quarantine persistence/reactivation. The virtual-MIDI trial and actual device output are separate from physical keyboard and listening acceptance.

| Evidence | Result |
| --- | --- |
| Model, persistence and lifecycle tests | Stable IDs and lazy Open, bounded v1/v2 decoding, atomic Save, retained-state consent scoped to the current failure, and rejection of obsolete load completions. A second-track capture failure preserved the disk file and the earlier successful capture. |
| Mixer and callback proof | Two/three unequal-latency paths aligned across 1/73/128/511-frame partitions. Healthy paths continued through failure. The counting allocator observed zero allocations and frees on exercised fills. Source inspection found bounded drains and no callback locks, I/O, RPC or worker teardown. |
| [Eight fixture workers](../../reports/T04-native-fixture-2026-11-18.json) | Selected-track MIDI, isolation of one hung worker with seven healthy workers continuing, and audio stop on removal of the owned virtual MIDI source. |
| [Commercial state round trip](../../reports/T04-commercial-roundtrip-2026-11-18.json) | Fresh-worker lazy reopen and original editor lifecycle for Pianoverse VST3 and ZebraHZ VST3/CLAP. |
| [Eight commercial instruments](../../reports/T04-eight-commercial-2026-11-18.json) | One Pianoverse and seven ZebraHZ instances on EVO16 1–2 at 44.1 kHz and actual 256-frame callbacks. With 1,024 transport samples, maximum plugin latency 16 samples and a 500 ms warm-up, every path produced nonzero audio with zero observed misses and no clipping. |
| [Actual latency notifications](../../reports/T04-latency-notifications-2026-11-18.json) | VST3 and CLAP fixtures changed from 0 to 257 samples at 48 kHz/128 frames. Compensation rebuilt, valid impulses continued and neither worker failed. |

TestSynth and both latency fixtures were rebuilt and bundled before the native validation.
The selected checks ran with `rtk cargo test -p clef --locked -- --ignored --test-threads=1 --nocapture`.
The actual GUI confirmed cancellation of retained-state Save and explicit reactivation
after reopening. The fixture quarantine entry was cleared through the UI after the check.

Implementation and automated/native verification are complete for the agreed
functional/provisional scope. Playback with a physical keyboard is confirmed on all
eight tracks, with specific listening checks still open. Performance observations
remain bounded to the recorded runs and presets. Gallery goldens and full UI contract
conformance belong to T18.
