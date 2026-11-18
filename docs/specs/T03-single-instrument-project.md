# T03 — Single-Instrument Project

**Status:** Done — approved workflow implemented and verified 2026-11-17.
**Date:** 2026-11-17
**Task:** [T03](../../TASKS.md)

## Goal

Turn the T02 validation path into a usable macOS Rust/egui application: create a
project, add one instrument track, play it from a physical MIDI input, save it as
one file, and reopen it with its instrument state and routing restored.

## Confirmed Requirements

The user confirmed the following in the T03 interview:

- One instrument track, with live playing only. No MIDI recording, clips or piano roll.
- One project file rather than a project directory.
- Save the track name, plugin state, MIDI input and audio output selection.
- Discover installed VST3 and CLAP plugins through scanning and present a selectable list.
- Select audio and MIDI devices in the interface, including audio sample rate and
  buffer size among supported configurations.
- Open projects with audio stopped; require an explicit command to enable audio.
- Protect unsaved changes with Save / Discard / Cancel before closing or replacing a project.

## Proposed Behavior

The details below are part of the specification proposal, not additional interview decisions.

### Project Lifecycle and Persistence

- New creates an unsaved empty project. Add Instrument Track creates its single
  track; creating a second track is unavailable in T03.
- New, Open, Save, Save As and Close share command handling and keyboard access.
- The project stores a versioned document, track name, plugin identity including
  format, complete host-supported plugin state, MIDI input, audio device/output
  pair, sample rate and requested buffer configuration in one file.
- Plugin libraries and external sample libraries remain external. A single project
  file does not imply packaging commercial plugin content or installed binaries.
- Save captures a consistent plugin state and publishes the completed file
  atomically. Capture/write failures preserve the previous file and unsaved state.
  Save As requires confirmation before replacing an existing file.
- Validate the document before replacing the current project. Invalid or unsupported
  documents report an actionable error without discarding the current project.
- Save / Discard / Cancel protects modified projects on New, Open and Close.
  Failed or cancelled saving does not continue the destructive transition.
- Plugin edits must be covered by unsaved-change protection; do not rely solely on
  track/routing changes to detect a modified project. Confirm the backend's change
  notifications and use a conservative policy where notification is incomplete.

### Plugin Discovery and Hosting

- Scan standard macOS VST3/CLAP locations and identify instrument entries, including
  multiple entries in one bundle. Display name and format; provide Rescan.
- Run plugin inspection outside the application process, with bounded waiting.
  Report failures and continue scanning other candidates; a broken plugin must not
  crash or indefinitely block the application. Reuse existing isolation where it fits.
- Selecting an instrument loads it in an isolated worker. Its original editor opens
  through the established eframe embedding path and retains T02 resize behavior.
- Missing plugins retain their saved identity and state in the project. Show the
  reason audio cannot start rather than silently substituting another instrument.
- Worker failure stops audio, reports the failure and offers an explicit restart
  from the last saved state, with a warning that unsaved plugin edits are lost.
- Replacing an instrument is an explicit operation with confirmation of state loss.

### Live Audio and MIDI

- Select a physical MIDI input and a stereo output pair on an audio output device.
  Preserve the T02 MIDI gesture coverage: notes, sustain, pitch bend and modulation.
- Audio settings expose supported sample rates and buffer configurations; reject
  unsupported combinations with an explanation. Display the actual active
  configuration so a request is not mistaken for a guaranteed device setting.
- Enable Audio and Disable Audio provide explicit live-session control. No timeline
  Play/Record controls or scheduled T02 demonstration sequence belong in this UI.
- Opening a project never starts audio automatically. Missing or disconnected
  devices leave audio stopped and preserve saved choices for explicit reassignment.
- Stop and clean up notes before device, plugin or routing changes. Reconfiguration
  leaves audio stopped, ready for explicit activation. Prevent stuck notes and stale
  packets crossing session boundaries.
- Keep the T02 callback, process isolation, deadline, state and failure invariants;
  UI, scanning and persistence must not enter the audio callback.

## Written UI Proposal — Awaiting Approval

- **Main window:** project name and modified indicator, project commands, Audio/MIDI
  Settings, and Enable Audio / Disable Audio with a readable status. The empty
  project offers Add Instrument Track. Its one track presents an editable name,
  instrument selector with format, MIDI input, stereo output pair and Open Editor.
- **Instrument picker:** searchable scanned instrument list with name and format,
  loading/scanning feedback, Rescan and readable scan failures. No full media browser.
- **Settings:** focused Audio and MIDI pages, with device selection, output pair,
  supported sample rate/buffer controls and actual active configuration. Do not expose
  unrelated legacy settings categories.
- **Feedback:** distinguish empty, loading, ready, audio active, unavailable plugin,
  unavailable device, worker failure and save failure. Errors state the next action.
- **Visual authority:** translate [foundations](../UI/foundations.md) and relevant
  [settings](../UI/views/settings.md), [plugin window](../UI/views/plugin-window.md),
  menu, button, text-field, list-row and notice components into egui. Preserve the
  Composer Dark palette, spacing-based separation, readable text and focus states.
  Do not inherit the legacy DAW's complete window layout or feature scope.
- Clef-owned windows and controls use egui/eframe. Original plugin editors retain
  their appearance. Every action uses shared command dispatch, discoverable shortcuts
  and context-aware availability in accordance with the UI library.
- Controls have accessible names, keyboard navigation, visible focus and labels
  independent of color. Compare the implemented UI with this proposal before visual acceptance.

## Acceptance and Verification

1. Through the UI, create an empty project, add one track, select a scanned VST3 or
   CLAP instrument, open its editor and play through selected MIDI/audio devices.
2. Rename the track, edit the plugin, save one project file, quit and reopen it.
   Verify name, instrument state and routing restoration; audio remains stopped.
   Exercise Pianoverse VST3, ZebraHZ VST3 and ZebraHZ CLAP separately.
3. Exercise supported audio configurations and reject unsupported requests; observe
   actual configuration and verify explicit stop/restart across configuration changes.
4. Exercise all unsaved-change choices, cancelled/failed saves, malformed documents,
   missing plugins/devices and device disconnection. Preserve the last valid file,
   recoverable state and user choices; never silently route elsewhere.
5. Exercise failed/hung plugin scanning and worker failure without losing the main
   application; restart from saved state with the documented unsaved-edit warning.
6. Add focused automated tests for document validation/round trips, atomic save
   failure, lifecycle decisions, scanning failures and session transitions. Reuse
   deterministic T02 fixtures and keep its regressions passing.
7. Run the repository's required check/build/test/fmt/Clippy commands. Update Graphify
   before implementation analysis and after changes, trace affected callers and
   verify source directly. Update UI documentation with implementation.
8. Record automated, GUI, user listening and physical MIDI evidence separately.
   The user confirms the complete save/reopen/play workflow and no audible dropout
   or stuck notes in the exercised configuration. This does not establish broad
   plugin compatibility or multitrack performance.

## Boundaries and Approval Gates

- No recording, timeline, clips, piano roll, multitrack operation, effects chain,
  mixer, export UI, AU hosting, autosave or content bundling in T03.
- Reuse the T02 implementation; do not select a replacement DSP or hosting stack.
  T01's final foundation adoption remains pending and must be resolved before
  implementation; completion of T02 alone is not that approval.
- The project container encoding, extension and bounded validation rules will be
  documented after inspecting existing persistence facilities. No new dependency
  or storage framework is selected by this draft.
- Specification and written UI proposal require user approval before implementation,
  per AGENTS.md and docs/UI/README.md. No application code has changed in this task.

## Approval — 2026-11-17

The user approved this specification, its written UI proposal and adoption of the
T02 foundations, and instructed implementation. T01 is now Done. Earlier approval
gate wording above records the draft state and no longer blocks implementation.

## Implementation Record — 2026-11-17

- `src/app.rs`: eframe workspace, staged settings, plugin/file pickers, shared commands,
  session shortcuts, accessibility and Save/Discard/Cancel decisions.
- `src/session.rs`: asynchronous owner of project, isolated instrument and live session;
  explicit stop/reconfigure/restart, stable device/port identity and failure monitoring.
- `src/project.rs`: `.clef` version 1, eight-byte `CLEFPRJ1` magic, little-endian u32
  metadata/state lengths, UTF-8 JSON metadata followed by raw state bytes. Metadata is
  capped at 64 KiB; state at 64 MiB; unknown fields, versions, invalid shapes, truncation
  and trailing data are rejected. Writes are staged in a private same-directory file,
  synced and atomically published. Existing-file changes are checked before ordinary Save.
- `src/catalog.rs`: standard-location discovery and isolated, bounded per-bundle scanning;
  preserve stable VST3 class IDs / CLAP IDs and multiple instruments in one bundle.
  Legacy VST3 factories without subcategories are classified by component buses in the
  scanner process. Ordinary support files are ignored. Shutdown cancels/kills/reaps scans.
- Existing T02 loading/worker/live paths now carry selected plugin ID and sample rate;
  requested device buffer sizes scale the preallocated transport lead. The 128-frame
  processing quantum and callback copy/atomic-only boundary remain. T02 CLI defaults
  stay at 48 kHz/128 frames. Actual callback size is reported separately.
- Enabled the installed eframe default-font and AccessKit features. No new library was added.
- `native/app_lifecycle_mac.mm` adds the missing cancellable termination hook to the
  existing winit delegate class. It creates no application UI. An initial delegate-proxy
  experiment conflicted with winit's delegate type check and was removed; it is not the fix.
- Save deliberately stops audio and closes the editor before capturing coherent state,
  reusing T02's lifecycle. Editor use/live playing conservatively dirties the document;
  Save restores a clean stopped state. Staged track-name edits are included in Save.
- Local GUI bundle: build with the required Cargo command, then run
  `rtk proxy python3 scripts/bundle-clef.py`. Launch `target/Clef.app` or run
  `rtk proxy target/debug/clef app`. The script verifies the copied executable.

See the [UI translation](../UI/views/single-instrument-project.md) and
[Completion](#completion--2026-11-17) for current verification.

## Completion — 2026-11-17

The approved single-track GUI workflow, installed-plugin discovery, single-file
project persistence, selectable device/rate/buffer setup, safe transitions and
saved-state recovery are implemented. The user explicitly confirmed
ZebraHZ VST3, ZebraHZ CLAP and Pianoverse MIDI/listening checks, including correct
AA Y7 restoration in the final GUI at 44.1 kHz / 256-frame actual callbacks.
Automated evidence includes 38 unit tests and six explicitly selected native tests.
The last GUI Save left the project clean with audio stopped.

- **Automated and native:** locked check/build, formatting and warning-free Clippy passed.
  The six native tests covered T02 render/state regressions, VST3 bundle entry selection
  and sample-rate behavior, three commercial state round trips, and virtual MIDI removal.
  Default tests covered bounded decoding, atomic saves, concurrent access, safe GUI
  transitions, and scanner timeout/cancellation.
- **GUI and recovery:** AccessKit and the actual app showed device selection, scanning,
  Save As/Open, staged-name persistence, explicit audio enable and Command+Q cancellation.
  A controlled worker hang left the UI responsive and required confirmation before restoring
  saved state. Removing a temporary CoreMIDI source stopped audio and preserved the selection.
  [Device report](../../reports/T03-device-disconnect.json) ·
  [Project artifacts](../../reports/T03-project-container-observations.json).
- **Commercial checks:** ZebraHZ VST3 and CLAP passed physical MIDI/listening at
  48 kHz/128 frames, and Pianoverse passed the final restored-piano check at 44.1 kHz/256
  frames, all on EVO16 1–2 and MIDIFACE In 1. Fresh worker recaptures after real Save/Open
  were byte-identical for all three cases. ZebraHZ CLAP emitted a preset-name diagnostic
  without a failed request. [State round-trip log](../../reports/T03-commercial-state-roundtrip.log).

This closes T03 for the exercised macOS configuration. No recording, piano roll,
multitrack implementation or broader compatibility is implied. Native tests retain
platform/device/fixture prerequisites; they are not default-test coverage.
