# T37 — T36 Known-Limit Follow-ups

- **Status** — Done — 2026-11-20. Specification approved by the user on 2026-11-20;
  implementation, automated evidence, and the user's manual GUI and listening checks complete.
- **Priority** — P1.
- **Task** — [T37](../../TASKS.md)
- **Dependencies** — T36 (late-reply protocol P1, off-thread insert reconfigure P2, send drag S2,
  view-only changes S8).

## Goal

Resolve four known limits recorded in [T36](T36-audit-fixes.md#known-limits-and-remaining-evidence).
Decisions were taken with the user on 2026-11-20.

## Scope and Acceptance Criteria

### L1 — Often-late insert: visible warning, no quarantine

Decision: quarantine protects the project from broken or corrupt plugins; a plugin that is late
because it is CPU-heavy (oversampling, high quality modes) is the user's to manage. Clef keeps
the T36 behavior (a late block outputs the delay-aligned dry signal; the plugin keeps running)
and makes the condition visible.

- Each running insert counts processed and late blocks over consecutive windows of about 2 s of
  processed audio (whole blocks, derived from the sample rate).
- At the end of a window the late share is published. If it is above 10 %, the insert is
  **Late**; the state clears at the end of the first window at or below 10 %.
- Late is never a failure: no quarantine registry entry, no bypass, no change to processing,
  undo history, or dirty state. Failure rules from T36 (broken connection, invalid reply, 1 s
  stall) are unchanged and take precedence over Late.
- Counting uses atomics on the existing shared effect state; the processing path does no
  allocation, locking, formatting, or I/O. The session reads the published share when it
  refreshes insert views (no new polling cadence).
- UI (proposal, needs approval): the slot keeps its Active look (`peacock` fill, `text` label,
  live power) and adds a 1.5-point `danger` edge; the label stays `text`, which distinguishes it
  from Failed (`danger` label, `field` fill, disabled power). The tooltip and accessible name
  add "Late: N % of blocks in the last 2 s. Reduce the plugin's load (for example oversampling)."
  No new token, icon, or layout. Documented in [slot button](../UI/components/slot-button.md)
  with a gallery state and egui snapshot.
- Proof: automated engine test — a fixture forced late on more than 10 % of a window reports
  Late, then clears after a clean window; a single late block does not. Session test — Late
  reaches `InsertView` without quarantine. Gallery snapshot of the Late slot.

### L2 — Instrument reconfigure off the broker thread

Decision: handle an instrument restart request like an insert restart (T36 P2).

- When an instrument reply requests a restart or reports a new latency, the track broker (and
  the live broker) moves the instrument's audio connection to a short-lived helper thread that
  runs Suspend, Stop, and Start under the existing 1 s lifecycle limits.
- While the helper runs, the broker keeps serving its queue: the instrument contributes silence,
  the track's insert chain keeps processing (tails continue), meters and diagnostics stay live.
- When the helper returns the connection and latency, the broker publishes the latency and
  resumes exactly as the current synchronous path does after `reconfigure` (latency report,
  clock latency refresh and epoch bump on tracks; plan reset, panic, and resync on live).
- A helper failure fails the instrument as the synchronous path does today. A second restart
  request while one runs is ignored until it finishes.
- Proof: automated engine tests with `FixtureRestart` for the track broker and the live broker:
  the broker produces blocks while the restart runs and the new latency is published after.

### L3 — Fader and pan release always commits

Decision: mixer control drags behave like send drags (T36 S2).

- A release always commits the last dragged value. If another action is in flight, the commit
  waits and is sent as soon as the session is idle; the strip shows the pending value meanwhile.
- A commit whose epoch is stale (the project was replaced, or history moved the mix) is
  rejected by the session as today, and the UI drops it with the preview.
- Escape, focus loss, or a dialog still cancel the drag.
- Proof: automated UI test — drag, start another action, release: one commit is sent once the
  session is idle, and the value is not reverted.

### L4 — View changes saved silently when leaving the project

Decision: when leaving a project (app close, New, Open), if the project has a file, has no
unsaved content changes, and only the arrangement view (scroll, zoom, snap) changed since the
last save, Clef saves the project file without prompting.

- View-only changes still never start an autosave and never mark the project dirty (T36 S8).
- An untitled project is not saved; its view is lost as before.
- If content is dirty, the existing Save/Discard/Cancel dialog applies unchanged; Discard
  discards the view too.
- A silent save failure does not block leaving the project; the view change is lost and the
  error is logged off the UI thread.
- Proof: automated session/UI tests — scroll then close writes the view to the file without a
  dialog; untitled project does not write; dirty project still prompts.

## Out of Scope

- Removing an existing quarantine or a quarantine management UI (T16).
- Changing the late threshold or window at runtime.

## Decisions During Implementation

- L1: only a block whose reply is late (`process_effect` returns no reply) counts as late; a
  block skipped because the chain deadline already passed, a bypassed block, and a block during
  a restart are not counted. The share is published per mille in atomics owned by
  `clef_engine::inserts::Effect` (`LateBlocks`); `Effect::late_percent` exposes it and
  `InsertView::late` carries it to the UI. Missing, Failed and Bypassed take precedence over
  Late, so a bypassed effect never shows a stale warning.
- L2: `clef_engine::restart::InstrumentAudio` owns the instrument connection for the track
  and live brokers. A restart moves it to a `clef-instrument-reconfigure` helper; if the
  helper cannot start, the restart runs on the broker so the connection is never lost.
  Stopping a broker waits for a running helper before returning the connection. The track
  broker pushes silent instrument blocks through bypass, trim, inserts and delay meanwhile;
  the live broker waits (its callback is already silent while `reconfiguring` is set).
- L3: a fader or knob drag is no longer cancelled by another action, whether started from the
  UI (`App::send` no longer cancels mixer gestures) or already in flight. A released drag is
  deferred like a send commit (`MixerUi::deferred`) and sent once the session is idle; a drag
  released while the strip is disabled commits its last previewed value. Mute and solo
  commands are not deferred.
- L4: the session records the arrangement view last read from or written to the file
  (`saved_view`) and exposes `view_unsaved`. New and Open write the view first; app close
  sends `Action::SaveViewBeforeClose` only when `view_unsaved` and the project is clean. The
  write uses the in-memory project, which equals the file apart from the view while the
  project is clean, with the same lock and external-change check as Save. A failure is
  printed by the session thread and never fails the action.

## Open Questions

None.

## Verification Evidence

### Automated (2026-11-20)

- L1: `inserts::tests::often_late_blocks_mark_the_effect_late_until_a_clean_window` (one late
  block and exactly 10 % are not Late; 25 % is; the state holds until a clean window clears
  it). A session-level test was not added: it needs a running effect worker; the mapping is
  covered by the gallery state.
- L2: `restart::tests::an_instrument_restart_runs_on_a_helper_while_the_broker_keeps_going`
  (fixture worker, ignored by default): `begin_restart` returns in under 50 ms, the helper
  holds the connection while the caller keeps polling, the new latency is returned and the
  connection processes again. Broker-level tests need an audio device and were not added.
- L3: `mixer::tests::a_fader_drag_survives_another_action_and_its_release_commits_when_idle`.
- L4: `tests::a_view_only_change_is_saved_silently_when_leaving_a_clean_saved_project`
  (session: untitled writes nothing, close and New write the view, a dirty project writes
  nothing) and `tests::closing_a_clean_project_with_only_view_changes_saves_without_a_prompt`
  (UI: no dialog, close waits for the session).
- Workspace: `cargo test --locked` 215 passed, 24 ignored; `cargo test -p clef-engine --lib --
  --ignored` 7 passed with the built worker; `cargo clippy --all-targets --locked -- -D
  warnings` and `cargo fmt --all --check` clean. Graphify code graph refreshed.

### GUI

- Gallery: `t12-late` at [1024](../UI/snapshots/egui/t12-late-1024.png) and
  [760](../UI/snapshots/egui/t12-late-760.png) points.

### Manual Checks (passed, reported by the user on 2026-11-20)

- GUI: drag a fader, press Play during the drag, release: the value stays and is undoable.
- GUI: scroll/zoom a saved project, quit: no prompt; reopen shows the view.
- Listening: an instrument that changes latency keeps its insert tails during the restart.
- Listening: a CPU-heavy insert shows Late and clears after lowering its load.
