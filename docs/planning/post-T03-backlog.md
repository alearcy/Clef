# Post-T03 Backlog Proposal

**Status:** Backlog order approved 2026-11-17. The T04–T23 entries were registered in
[TASKS.md](../../TASKS.md); completed rows are removed from that index once their
specification records the work. Each remaining task stays a planning draft until its own
interview and specification approval.
**Date:** 2026-11-17
**Basis:** [T01](../specs/T01-engine-foundation.md), [T02](../specs/T02-engine-validation.md), the [T03 specification](../specs/T03-single-instrument-project.md), `AGENTS.md`, and the [UI library](../UI/README.md). Product direction comes from the T01 interview: composition with virtual instruments first, then audio recording and mixing, with robust VST3/CLAP hosting.

This document records the approved order and rationale for the work that follows T03.
It does not approve implementation scope. Each task below still requires its own interview,
English specification, and — where it changes the UI — an approved written UI proposal.
The per-task planning drafts live in `docs/specs/` and are linked from `TASKS.md`.

T03 is still in progress. Its specification does not yet record final acceptance
evidence, so T03 must be accepted and its specification completed before T04 starts.
The proposed sequence assumes T03's agreed behavior (one instrument track, one project
file, explicit audio enable) remains intact.

## Priority model

- **P0 — Composition core.** Without it there is no usable composition workflow. Do next.
- **P1 — Daily-use DAW.** Required before the application is credible for regular work.
- **P2 — Hardening and scale.** Quality, compatibility, performance, and maintainability
  work that should land before larger releases and broader plugin use.
- **P3 — Conditional expansion.** Valuable, but it needs a new product decision or a
  stable VST3/CLAP foundation first.

Priority is about product dependency and risk, not about development effort. All
estimates are deliberately omitted until each task is specified.

## At a glance

| ID | Priority | Proposed task | Depends on |
| --- | --- | --- | --- |
| T04 | P0 | Multitrack session and instrument engine | T03 accepted |
| T05 | P0 | Transport, tempo map, and timeline | T04 |
| T06 | P0 | MIDI recording and clip playback | T05 |
| T07 | P1 | Autosave, crash recovery, and recent projects | T04, T06 |
| T08 | P1 | Undo/redo and edit history | T04, T06 |
| T09 | P1 | Arrangement and clip editing | T06, T08 |
| T10 | P1 | Piano roll and MIDI editing | T06, T08 |
| T11 | P1 | Mixer and master section | T04 |
| T12 | P1 | Insert effects hosting | T04, T11 |
| T13 | P1 | Export and bounce | T11, T12 |
| T15 | P2 | Buses, sends, groups, and routing | T11, T12 |
| T14 | P1 | Audio tracks and recording | T05, T11; media decision |
| T16 | P2 | Plugin management and hosting robustness | T03, T12 |
| T17 | P2 | Device resilience and preferences | T03, T04 |
| T18 | P2 | egui design system, commands, and accessibility | ongoing UI work |
| T19 | P2 | Performance, scale, and realtime safety | T04, T11, T12, T14 |
| T20 | P2 | Automated app regression harness | T04 onward |
| T21 | P3 | AU hosting | T12 |
| T22 | P3 | Native MIDI/audio tools | T06, T10, T11, T28 |
| T23 | P3 | Packaging and cross-platform groundwork | stable feature set |

T26–T29 were added on 2026-11-18; see [Additions — 2026-11-18](#additions--2026-11-18).

## Milestone 1 — Composition core (P0)

The goal is the smallest end-to-end workflow that deserves the name DAW: several
instruments, a musical timeline, and recorded MIDI that plays back sample-accurately.

### T04 — Multitrack session and instrument engine

**Outcome:** the project holds an ordered list of instrument tracks. Each track owns an
isolated instrument worker. A bounded realtime audio path mixes all tracks plus a master.
Tracks can be added, removed, reordered, renamed, and selected. Project schema v2 stores
the whole session and migrates T03 v1 single-track files without data loss.

**Why first:** every later feature — timeline, recording, mixer, inserts — assumes more
than one track and a project document that represents them. Changing the schema after
recording and routing exist is far more expensive.

**Scope guardrails:** instrument tracks only; no audio, bus, or FX track types yet; no
timeline or clips; no insert effects. Keep the existing single-track flows working during
migration.

**Interview questions:** maximum simultaneous instruments and CPU budget; eager versus
lazy loading when a project opens; behavior for a missing or failed track plugin (retain
identity, silent track, explicit retry); whether per-track gain is in T04 or T11; schema
v2 layout and state-blob layout for multiple plugins.

**Verification:** deterministic two- and three-worker mix; per-track delay compensation;
one failed worker does not silence or crash the others; v1-to-v2 round trip; T02/T03
regressions still pass.

### T05 — Transport, tempo map, and timeline

**Outcome:** a musical timeline measured in bars, beats, and ticks, driven by the audio
clock. Play, stop, loop, count-in, and metronome. A tempo and time-signature model (scope
agreed in the interview). A ruler, track lanes, playhead, locators, and zoom/scroll
behavior. The transport is the shared clock for live input, clip playback, and recording.

**Why now:** T06 and all arranging/editing depend on one authoritative time base. A
transport bolted on later produces timing drift between tracks and between live and
offline rendering.

**Scope guardrails:** no clips or recording yet; no automation lanes; no chord or
articulation features. Decide explicitly whether the transport can move while audio is
disabled, rather than inheriting T03's "audio stopped on open" state by accident.

**Interview questions:** single tempo versus tempo/time-signature map; loop and count-in
defaults; metronome routing and sound; whether the playhead moves with audio disabled;
visual latency compensation; locator and marker scope; timeline units and zoom range.

**Verification:** playhead and event-position math across loop wraps and tempo changes;
sample-accurate scheduling; transport state machine; metronome/count-in timing; no stale
events after stop.

### T06 — MIDI recording and clip playback

**Outcome:** arm an instrument track, record live MIDI with latency compensation, create
or reuse a clip, and play it back through the track's worker. Project persistence includes
clips, notes, and transport/tempo state. The arrangement shows the recorded clip and the
playhead.

**Why now:** this is the first user-visible composition result. It validates T04 and T05
together and gives the first honest answer to "can I write music with this?".

**Scope guardrails:** MIDI only; one recording input selection inherited from T03; no
audio recording, comping, punch, or take lanes yet; no piano roll (a minimal clip view is
enough until T10).

**Interview questions:** replace/overdub/loop recording modes; count-in and metronome
defaults; quantize while recording; one clip per take versus merged clips; note chase and
all-notes-off behavior; how plugin latency affects recorded timestamps; MIDI channel
handling; what happens on stop during a held note.

**Verification:** deterministic fake-MIDI capture with timestamp checks; latency
compensation; loop record and loop wrap; save/reopen; no stuck notes; playback matches
recorded event positions.

## Milestone 2 — Editing safely (P1)

Once recording exists, losing or corrupting work becomes unacceptable. This milestone
protects the work before adding more editing surfaces.

### T07 — Autosave, crash recovery, and recent projects

**Outcome:** crash-safe periodic autosave, recovery on next launch, bounded backup
retention, and a recent-projects/startup surface. The last valid project file is never
replaced by a failed write. Plugin-state capture policy is explicit.

**Why now:** the moment T06 records user material, an application crash can destroy a
session. Autosave is cheaper and more valuable before arrangement and piano-roll editing.

**Interview questions:** interval and location; whether plugin state is captured on every
autosave (potentially expensive) or only project structure; backup count; recovery UX;
autosave during playback/recording; how autosave interacts with the explicit Save/Discard/
Cancel contract from T03.

**Verification:** simulated crash and recovery; atomic writes under failure; backup
rotation; failed autosave does not damage the last good file; startup recent list.

### T08 — Undo/redo and edit history

**Outcome:** a command-based undo/redo history for project structure, clips, notes, mixer
values, and routing, with coalescing for continuous gestures and a bounded memory budget.
Menu entries and keyboard bindings follow the shared command registry.

**Why now:** arrangement and piano-roll editing are unsafe without undo. Retrofitting undo
after those editors exist multiplies the work and the bugs.

**Interview questions:** whether plugin parameter edits are undoable, and at what memory
cost; history granularity and coalescing; whether history survives save/open; undo during
recording or playback; memory limit and user-visible history depth.

**Verification:** per-command undo/redo; redo invalidation; gesture coalescing; memory
bound; save/open semantics; no stale plugin state after undo.

### T09 — Arrangement and clip editing

**Outcome:** create, delete, move, trim, split, duplicate, and loop clips; snap and grid;
selection and multi-selection; zoom and scroll; context menus and keyboard commands. The
arrangement layout follows the relevant UI-library views and has an approved proposal
before implementation.

**Why now:** T06 can record one clip; composition needs to shape many clips in time.

**Interview questions:** clip versus part terminology; one versus many clips per track;
loop semantics; trim rules around notes; fades/crossfades (likely later); snap options;
editing while the transport runs; whether folder tracks are in scope.

**Verification:** deterministic edit operations; undo/redo integration; save/reopen;
timeline scrolling and zoom; refused operations report a reason.

### T10 — Piano roll and MIDI editing

**Outcome:** edit the notes of a clip: add, delete, move, resize, velocity, multi-select,
copy/paste, quantize/transpose (scope agreed), snap, zoom, and playhead following. Pitch
and controller lanes follow the agreed scope; the surface is specified and approved
before implementation.

**Why now:** recorded MIDI is rarely right first time, and MIDI writing is the product's
first priority. This is the core writing surface.

**Interview questions:** velocity and CC lanes; quantize/humanize/transpose subset; scale
or chord assistance (not assumed); step input; audition on note edit; multiple open parts;
note chase; how the roll shares edit commands and undo with T09.

**Verification:** note-edit commands; undo/redo; rounding and snap; save/reopen; keyboard
and pointer parity; no edits lost when switching clips.

## Milestone 3 — Mixing and production (T11–T14 P1, T15 P2)

The app can compose. This milestone makes the result mixable and exportable.

### T11 — Mixer and master section

**Outcome:** channel strips for every track plus master, with gain, pan, mute, solo, and
peak/hold/clip metering. A clear master output path. The mixer view follows the UI library
and gets an approved proposal.

**Why now:** multiple instruments need balance before inserts and export. It also exposes
gain-staging and solo/mute semantics early.

**Interview questions:** pan law; mono versus stereo tracks; meter ballistics and scale;
solo modes (PFL/AFL); clipping policy; master gain/limiter (not assumed); whether channel
width or polarity is included.

**Verification:** gain and pan math; mute/solo interaction; meter values against
deterministic signals; PDC and routing unaffected; save/reopen.

### T12 — Insert effects hosting

**Outcome:** per-track and master insert chains of VST3/CLAP effects, discovered and
loaded through the existing isolated-worker path, with bypass, original editor, state
persistence, failure isolation, and latency compensation across the chain.

**Why now:** external effects hosting is a T01 priority, and mixing without inserts is
incomplete. It also stresses the hosting architecture beyond instruments.

**Interview questions:** maximum inserts per chain; serial order and reordering; catalog
filtering for effects versus instruments; whether the FX rack is a simple chain or the
legacy node graph (do not assume the graph); plugin parameter automation later; sidechain
later; latency changes while playing.

**Verification:** deterministic effect fixture; chain PDC; bypass; state round trip; effect
worker crash isolation; save/reopen; T02/T03 regressions.

### T13 — Export and bounce

**Outcome:** an export surface for the session or a range, using the existing offline
render path: WAV format/depth/rate, ten-second (or agreed) tail, latency trim, progress,
cancel, and overwrite protection. Stems are included only if agreed.

**Why now:** the T02 render path exists but is not reachable from the product. Composition
has no deliverable without export.

**Interview questions:** WAV only or additional formats; stems; sample-rate conversion;
normalization/limiting (not assumed); range selection; tail policy; filename templates;
what happens with missing plugins or offline-rejected plugins.

**Verification:** exact frame counts; PDC; cancellation leaves no partial file; overwrite
refusal; empty session; readability of output.

### T14 — Audio tracks and recording

**Outcome:** an audio track type, input selection and monitoring, recording to disk,
waveform display, missing-media handling, and project references to media. Takes and
comping are not part of the initial scope; they require a separate decision.

**Why now:** T01 names audio recording after virtual instruments. It also forces the most
important unresolved storage decision (see below).

**Critical decision:** T03 deliberately uses one project file. Audio recording may require
a project folder/package or a sidecar media directory. This decision changes the project
schema, save/open, backup, and missing-file behavior, so it must be agreed before T14 is
specified — ideally before T04 freezes schema v2.

**Interview questions:** media storage model; recording format/depth; punch and count-in;
input monitoring through the plugin chain; comping/takes; latency compensation; missing
media relink; disk streaming; maximum recording length; whether audio clips share the T09
clip grammar.

**Verification:** deterministic capture; media references; missing/relinked files; save/
reopen; no partial recordings published as complete; monitoring and PDC.

### T15 — Buses, sends, groups, and routing

**Outcome:** group/bus/FX-return tracks, pre/post-fader sends, a routing surface, PDC and
solo/mute semantics across the graph, and cycle rejection.

**Why now:** direct-to-master is enough for the first mixes; buses and sends are the next
step, not a prerequisite for composition. MIDI instrument tracks can already be grouped and
sent, so routing follows export (T13) and does not wait for audio tracks (T14).

**Interview questions:** bus types and counts; send count; sidechain; solo-in-place versus
PFL/AFL; routing UI; folder tracks; automation of sends and routing (later).

**Verification:** routing and gain math; PDC across the graph; cycle rejection without
losing the last valid routing; solo/mute semantics; save/reopen.

## Milestone 4 — Hardening and scale (P2)

These tasks should interleave with Milestones 1–3 where they protect the work, rather than
waiting for the end.

### T16 — Plugin management and hosting robustness

Scan cache and invalidation, categories/favorites/search, missing-plugin resolution,
quarantine policy, rescan, plugin details, multi-entry bundles, and failure reporting.
Define the compatibility target and evidence standard. AU is not part of this task.

### T17 — Device resilience and preferences

Hotplug and device-loss handling, default devices, sample-rate/buffer preferences, MIDI
channel filters, clear reconfiguration warnings, and persisted preferences. Extend, rather
than replace, T03's explicit enable/disable contract.

### T18 — egui design system, commands, and accessibility

Extract design tokens and reusable widgets from `app.rs` into modules that all new
surfaces share; implement menus, focus/keyboard navigation, accessible names, contrast,
scaling, command palette, and shortcut editing; update `docs/UI` in the same changes.
Formalize this before the mixer/piano roll grow, even if the audit task lands later.

### T19 — Performance, scale, and realtime safety

Agree measurable targets first (track/plugin counts, sample rates, buffer sizes, CPU,
load times, acceptable xruns). Then profile the worker pool, IPC, mixing, scheduling, and
UI; add stress fixtures and long-run evidence. T01 explicitly did not agree performance
thresholds, so none may be invented here.

### T20 — Automated app regression harness

Deterministic fixtures and integration tests for multitrack mixing, recording timing, undo,
routing, and project migration; a GUI smoke path where feasible; scripts that build/bundle
fixtures reproducibly. Keep automated, GUI, listening, and hardware evidence separate, as
T02 and T03 already do.

## Milestone 5 — Conditional expansion (P3)

### T21 — AU hosting

Audio Unit instruments and effects on macOS with isolation, scanning, editors, state, and
PDC. Requires a compatibility and validation decision. VST3/CLAP must be stable first.

### T22 — Native MIDI/audio tools

Arpeggiator, M/S encode/decode, and similar tools named in T01. Their exact behavior,
placement (insert versus MIDI processing), UI, persistence, and automation require their
own specification. Do not assume the legacy rack design.

### T23 — Packaging and cross-platform groundwork

Signed/notarized macOS app bundle, update and crash-reporting policy, platform abstraction
review, and a Windows/Linux feasibility decision. Cross-platform work is groundwork only
until the product scope is agreed.

## Recommended sequence

```
T03 accepted
   |
   v
  T04
   |---> T05 ---> T06 ---> T07
   |               |------> T08 ---> T09
   |                              \-> T10
   |---> T11 ---> T12 ---> T13 ---> T15
   \---> T14 (after T05 + T11 and the media decision)

T16, T17, T18, T19, T20: interleave where they protect the current milestone.
T21 after T12; T22 after T10; T23 last.
```

**Next three:** T04, T05, T06. They form one vertical slice: several instruments, a real
clock, and recorded MIDI. Do not start T07/T08 before T06 unless T04/T05 expose a problem
that makes them necessary.

After T06, run T07 and T08 before T09/T10, then start T11/T12 in parallel with the editors
if capacity allows.

T26 follows T10, T27 follows T14, T28 follows T15, and T29 follows T21; see
[Additions — 2026-11-18](#additions--2026-11-18) for the dependencies.

## Decisions required before the next specifications

These are product decisions, not implementation details. Each must be settled in the
relevant interview; several affect the project schema and should be decided before T04
freezes it.

1. **Media storage:** single project file versus project package/folder for recorded audio.
   This is the highest-risk unresolved decision because T03 chose a single file.
2. **Session limits and loading:** maximum simultaneous instruments, CPU budget, eager or
   lazy loading, and behavior for missing or failed track plugins.
3. **Time model:** single tempo versus a tempo/time-signature map; timeline resolution;
   whether the transport can run with audio disabled.
4. **Recording semantics:** replace/overdub/loop, count-in, metronome, quantize-on-record,
   take policy, and MIDI channel handling. The take and comp decisions are now T26/T27.
5. **Undo scope:** project/editing only, or plugin parameter edits too; memory budget and
   history persistence.
6. **Effect rack shape:** simple serial chains or the legacy node graph; maximum inserts;
   sidechain timing. **Decided 2026-11-18:** T28 implements the node/cable graph; macros,
   modulation, and the browser are a later phase.
7. **Plugin compatibility target:** which formats, vendors, and plugins must be validated
   for each milestone; what counts as acceptable failure.
8. **Performance targets:** track/plugin counts, buffer sizes, CPU, and load times, with a
   measurement method agreed before T19.
9. **First-release accessibility and packaging level:** screen-reader support, keyboard-
   only workflows, signing/notarization, and update policy.

## Risks and technical debt to carry forward

- **Worker-pool realtime cost.** T02/T03 validate one isolated plugin worker. Mixing many
  workers in one callback needs bounded IPC, scheduling, and PDC evidence; CPU and latency
  overhead is unmeasured at scale.
- **Schema evolution.** T03's v1 document is single-track and stores one plugin-state blob.
  v2 migration, validation, and atomic save must be designed before features accumulate.
- **Plugin-state cost.** Autosave and undo over opaque plugin state can be expensive.
  Decide capture policy and memory limits early, not after the features exist.
- **Single audio device and sample rate.** T03 opens one device and restarts on rate
  changes. Multi-track sessions multiply the cost of reconfiguration; device loss behavior
  must remain explicit and safe.
- **UI growth.** `app.rs` already carries the whole T03 surface. New screens need shared
  widgets, tokens, commands, and accessibility before they multiply one-off controls.
- **Test realism.** Many native tests are ignored by default and need rebuilt fixtures.
  T20 should reduce manual setup, not add another layer of unverified claims.
- **Compatibility claims.** The T02 matrix covers three instruments on one host. It does
  not establish broad plugin compatibility, effect hosting, or multitrack performance.

## Deliberately not assumed

The following appear in the legacy UI library or in general DAW expectations but were not
agreed for Clef. They are not part of this backlog until the user requests them and a
specification defines their behavior: automation lanes and parameter automation, chord/
scale lanes and chord tools, articulation maps, video/film sync, notation, MIDI 2.0/
MPE, control surfaces, sidechain, freeze/bounce-in-place, drum replacement, internal
instrument/effect plugins, cloud collaboration, and a full mixer graph beyond the
proposed T15.

**Update 2026-11-18:** ARA 2, MIDI and audio comping, and the rack node graph are no longer
excluded. The user requested them, and they are registered as T26–T29 with the Composer DAW
decisions cited in their specifications. The rest of this list still stands.

## Additions — 2026-11-18

The user asked to add ARA 2, MIDI and audio comping, and the FX rack node graph. The product
decisions already exist in the external Composer DAW project (C++/JUCE/Tracktion). Each Clef
specification cites the reference and inherits its behavior, not its code. These are planning
drafts: the Clef implementation interview and the written UI proposal still gate each task.

| ID | Priority | Task | Depends on | Row placement |
| --- | --- | --- | --- | --- |
| T26 | P2 | MIDI comping and take lanes | T06, T08, T09, T10 | after T10 |
| T27 | P2 | Audio comping and take lanes | T14, T26, T08, T09 | after T14 |
| T28 | P2 | FX rack and virtual cable graph | T12, T15, T08, T04 | after T15 |
| T29 | P3 | ARA 2 hosting and audio part editing | T14, T13, T16, T08; probe first | after T21 |

Why this order:

- Comping follows the recorder and editor it extends. MIDI comes first, after the piano roll
  (T10); audio follows the media decision in T14 and the shared take-lane model in T26.
- The FX rack follows T12's inserts and T15's routing and PDC, because a rack is a container
  inside the track chain, not a second mixer.
- ARA is last: it extends audio clips and needs T13's render path, T16's catalog and
  quarantine, and a new ARA host in Clef's VST3 worker. The Composer DAW could reuse
  Tracktion's host; Clef must pass its own probe before implementation.
- Rows follow the dependency chain. Priority is in the table above, not the row position.

These decisions close the effect-rack-shape question: T28 implements the node graph, with
macros, modulation, and the rack browser as a later phase after a separate approval.

## Next actions

1. Close T03: record final acceptance evidence in
   [the T03 specification](../specs/T03-single-instrument-project.md) and update `TASKS.md`.
2. Decide the media-storage question before T04 freezes project schema v2.
3. Interview and approve the [T04 specification](../specs/T04-multitrack-session.md).
4. Only after T04's scope is approved, refine the T05 and T06 drafts; do not treat the
   planning drafts as approved requirements.
