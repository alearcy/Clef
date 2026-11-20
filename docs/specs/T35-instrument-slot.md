# T35 — Instrument Slot, Bypass, and Offline Tracks

- **Status** — In Progress. Requirements agreed on 2026-11-20; implementation and automated
  verification complete. Device/template verification and listening remain.
- **Priority** — P1.
- **Task** — [T35](../../TASKS.md)
- **Roadmap** — Post-T03 backlog.
- **Dependencies** — T11 mixer, T12 insert effects, and the existing isolated instrument worker path.

## Behavior

- Every instrument track shows one fixed **Plugin** slot as the first row of its Inserts section.
  The slot is not an insert effect, is not reorderable or removable, and always precedes the
  effect chain. Master has no Plugin slot. A later audio-track type has no instrument slot.
- The slot shows the instrument name, truncated in the column; the full name and format remain
  available through the tooltip and accessible name. Activating it opens the original
  instrument editor through the existing hosted-editor path.
- If the track has no instrument, the slot shows **No instrument** and activating it opens the
  instrument picker. Missing, quarantined, failed, loading, and ready states use the same slot
  state vocabulary as inserts.
- The slot power control is a normal plugin bypass: the instrument worker stays online and
  continues to process MIDI and state, but its stereo output is replaced with silence before
  Gain and the effect chain. Downstream effects receive silence; their tails may continue.
  Bypass is persisted and undoable.
- The slot menu and the command registry provide Bypass/Enable and Take Offline/Bring Online.
  The same menu offers Open Instrument Editor and Replace Instrument while the track is online.
- **Offline** is a persisted per-track state separate from bypass. While offline:
  - the instrument worker is released after its current state is captured into the project;
  - the instrument is not loaded on project open, track selection, Enable Audio, play, or record
    preparation;
  - the track produces no instrument audio; its effect chains are not newly activated;
  - the slot uses the field surface with a disabled name and power control, and the track status
    reads **Offline**;
  - the saved plugin identity and state are retained for a later Bring Online.
- Bring Online clears the flag and loads the instrument from the saved state on the next explicit
  use. The load is asynchronous like an initial instrument load.
- Bypass and Offline are separate: an offline track has no worker to bypass; a bypassed online
  track still has a worker and editor.
- The mixer reserves the Plugin row plus up to two effect rows and the **Add effect…** row, for a
  maximum of four rows. All instrument strips and Master reserve the same total height so Pan,
  fader, meter and name stay aligned. Longer effect chains scroll inside the effects area; the
  Plugin slot stays visible.
- Existing projects open online and not bypassed. Projects with the flags absent decode with both
  clear.
- Add, remove, move and bypass of effects remain unchanged and remain undoable. Bypass and Offline
  changes are one history step each.
- Accessible names identify the channel, the **Plugin** role, the instrument name and the state.
  The Plugin/power controls have keyboard activation; the command registry exposes the state
  commands for the focused channel.

## Out of Scope

Audio-track input slots, instrument replacement undo semantics beyond the existing replacement
flow, per-instrument gain/trim, sidechain routing, AU hosting, and a general plugin manager.

## Acceptance Criteria

- On instrument tracks the first Inserts row is the Plugin slot; no effect can be inserted,
  dropped, or moved before it. Master has no Plugin slot. An empty track opens the instrument
  picker from its slot.
- Bypass silences the instrument output before Gain and effects while the instrument editor and
  worker remain active. Undo and redo restore the flag. The state survives save and reopen.
- Offline releases the instrument worker and preserves its state; opening the project, selecting
  the track, enabling audio, playback and record preparation do not load it. Bring Online loads
  the saved state. Both directions survive undo/redo where history is available.
- Existing projects without the flags open online and unbypassed. Save and reopen round-trips
  both flags and the instrument state.
- The command registry and context menu expose both states; keyboard focus on the Plugin slot
  targets them. Accessible names and long-name tooltips are present.
- Compare egui renders at 1024 and 760 points for online, bypassed, offline, empty, long-chain
  and Master states. The Plugin row stays visible while effect chains scroll.
- Run the T02/T03 hosting regressions and T12 insert effects regressions after the worker
  lifecycle changes.

## Decision Record

- 2026-11-20 — The user asked for the first slot of an instrument track to be the virtual
  instrument and chose a fixed slot with a power control.
- 2026-11-20 — The user chose normal plugin bypass for power (output muted, worker online) and a
  separate context-menu/command Offline state that releases the worker, for templates where
  instrument tracks are brought online gradually.
- 2026-11-20 — The user chose a four-row capacity: Plugin, up to two effects, and Add effect.

## Implementation and Verification (2026-11-20)

### Model and persistence

- `Track` carries `instrument_bypassed` and `instrument_offline`, both omitted when false and
  defaulted when absent. Existing version 1, 2 and 3 projects therefore open online and
  unbypassed. The flags round-trip through project save and reopen; the project state sizes and
  identity checks are unchanged.

### Engine and worker lifecycle

- `TrackStats` carries the bypass flag. The instrument broker still runs the worker and its MIDI
  and state path, then applies a 5 ms gain ramp to silence before Gain, trim and insert
  processing. Bypass does not stop the worker or close its editor. The ramp prevents the
  click that an immediate sample fill produced.
- Offline is enforced in the session before loading: project open, track selection, Enable Audio,
  playback preparation, editor open, replace and restart all skip or refuse an offline track.
  Taking a track offline closes its editor, captures the current instrument state, removes the
  worker and detaches its broker. The effect chain is published as empty while offline; the
  insert workers are retained for a later rebuild. Bringing the track online clears the flag and
  loads the saved state on explicit use.
- `Edit::InstrumentBypass` and `Edit::InstrumentOffline` are one history step each. History
  re-release and reload paths use the existing worker release rules; offline transitions also
  reset the arm state.
- Offline tracks report **Offline** in the track status and are skipped by playback loading, so a
  template with many offline instruments does not load them all at once.

### UI

- The fixed Plugin row is rendered before the effect scroll area on instrument tracks. Master
  has no Plugin row: its effects and Add effect begin directly under the Inserts caption, with
  the alignment space after them. It uses the slot primitive with role `Plugin`, the `Empty`
  and `Offline` states, the vector power glyph and the same channel naming as effects.
- Activating an online Plugin slot opens the instrument editor; an empty slot opens the
  instrument picker. The context menu and the `InstrumentBypass` / `InstrumentOffline` commands
  expose both states. The registry commands use the focused Plugin slot or the selected mixer
  channel.
- The insert area reserves the Plugin row, up to two effect rows and Add effect (four rows at
  capacity). The Plugin row stays visible while effects scroll; all strips keep their lower
  controls aligned.
- The T11/T12 gallery fixtures now include instrument names; the T12 gallery adds
  `vi-bypassed`, `vi-offline` and `vi-empty` states. Ten T12 states were rendered at 1024×768
  and 760×768, and the T11 mixer states were regenerated at 768.

### Evidence

- Locked workspace suite: **177 passed, 0 failed, 20 ignored**; build, formatter and Clippy pass.
- `project::tests::instrument_flags_round_trip_and_older_projects_default_online` covers the
  flags and old metadata.
- `instruments::tests::bypass_and_offline_flags_are_undoable_and_offline_blocks_loading` covers
  bypass/offline undo, offline redo, rejected loads and the Offline track status.
- The engine instrument path applies a 5 ms bypass ramp; the insert path crossfades wet/dry
  over the same window and does not bump the compensation epoch for a bypass-only edit.
  `stop_fades_out_and_keeps_the_requested_stop_frame` covers the transport fade and the closed
  stopped gate; live audition reopens it explicitly.
- `instruments::tests::offline_releases_the_instrument_worker_and_keeps_its_state` loads
  TestSynth through a worker, takes it offline, verifies the nonempty preserved state and the
  released runtime, then exercises history in both directions.
- `editor_lifecycle` verifies that an unavailable editor reply from a plugin leaves the
  worker and its audio connection alive, and that the bundled TestSynth VST3 editor can be
  opened, closed and reopened while processing continues.
- Two VST3 host defects were found and fixed while testing Glass Strings:
  - The RPC layer killed the child on an editor reply error. That turned "VST3 editor
    unavailable" into an exited worker and then a quarantine. Plugin reply errors are now
    returned without terminating the worker.
  - The `MessageProxy` marked itself connected only after `source->connect(this)` returned.
    JUCE plugins send their processor pointer synchronously from inside `connect`; that
    handshake message was dropped, `createView` returned null, and the editor never opened.
    The proxy now marks the connection before calling the source and forwards main-thread
    notifications synchronously.
  Glass Strings VST3 now opens its editor, processes audio and no longer quarantines.
  - The parent advanced its editor-close generation only in the generic `call` path, but
    `Instrument::editor` sends the request through `call_reply` to preserve a live worker on
    reply errors. The worker's close notification therefore never matched, `runtime.editor`
    stayed true, and activating the Plugin slot was treated as "already open" until another
    track selection closed editors. The open path now advances the generation on success,
    drains stale notifications before opening, and the session consumes a pending native close
    before deciding that an open request is redundant. `editor_close_notification_matches_the_latest_open`
    reproduces the mismatch against a fake worker; it fails without the generation update.
- Quarantine reasons now carry the broker/worker detail instead of a generic text, so a future
  fault identifies the failing block or connection.
- UI tests cover the Plugin-slot accessible label, focused-channel command availability, effect
  insertion gaps, and mixer fader/name alignment at both review widths.
- Existing T02, T03 and T12 native regressions pass: 4/4, 1/1 effect integration, 2/2 engine
  insert tests and 1/1 session insert test. `editor_lifecycle` passes 2/2 ignored editor tests.
- Gallery: [online](../UI/snapshots/egui/t12-populated-1024.png),
  [VI bypassed](../UI/snapshots/egui/t12-vi-bypassed-1024.png),
  [VI offline](../UI/snapshots/egui/t12-vi-offline-1024.png),
  [empty VI](../UI/snapshots/egui/t12-vi-empty-1024.png), and the T11 mixer states.

### Device/template setup (prepared 2026-11-20)

`validation-output/T35-template.clef` is a local, git-ignored project for the device/GUI pass.
It contains three offline instrument tracks playing the same sustained chord: **Glass Offline
A** (Glass Strings), **TestSynth Offline** (bundled TestSynth), and **Glass Offline B** (Glass
Strings). All three carry `instrument_offline`, so opening the project must not start a worker
or a load job. The Glass Strings states are copied from a saved project so bringing a track
online exercises the preserved-state path. The project was read and validated through
`Project::read`/`validate_storage`; the session reports all three tracks as Offline without
loading. A local session probe then brought Glass Offline A online (Ready), saved the project
with A online and the other two offline, and read back the nonempty preserved state.

### Completion (2026-11-20)

The task is complete. The user device/GUI pass confirmed the all-offline template, bringing
tracks online one at a time, audible VI bypass with the worker still online, save and reopen
with the offline flags and preserved states, and a click-free Stop. The Stop crackle was
traced to the stopped output gate reopening after the transport fade and unmuting the decaying
instrument tail; the gate now stays closed until Play or live audition, and the retest after
relaunching `target/debug/clef` passed.

Automated evidence at completion: **177 passed, 0 failed, 20 ignored**; build, formatter and
Clippy pass. The template project remains at `validation-output/T35-template.clef`.
