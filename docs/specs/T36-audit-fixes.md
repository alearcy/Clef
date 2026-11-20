# T36 — Audio, CPU, and Memory Audit Fixes

Status: In Progress (started 2026-11-20)

## Goal

Fix every defect found by the 2026-11-20 code audit of the realtime engine, plugin hosting,
session, and UI. The user authorized fixing all findings in one task on 2026-11-20 and left the
implementation choices to the agent; each choice below is recorded as a decision.

## Scope and Acceptance Criteria

Each item lists the defect, the fix decision, and the required proof. "Automated" means a
focused unit or integration test; items without a feasible automated test record why.

### Engine (realtime path)

- **E1 — Bus sum repeats across blocks in one callback.** `Mix::fill` cleared the per-bus block
  accumulator once per device callback, so with callbacks longer than `BLOCK` (128) samples,
  samples at the same in-block offset added up (an impulse at frame 37 was heard at 37, 165, 293).
  Decision: clear each bus sample slot at the first write for its block offset in every block.
  Proof: automated, callback > `BLOCK` with a bus-routed impulse is heard exactly once.
- **E2 — Routing packet header set once per callback.** `RoutingInput::begin` ran only at the
  callback start, so later blocks in the same callback carried a stale `at`/epoch/transport.
  Decision: begin a routing packet at every block boundary inside the sample loop.
  Proof: automated, two blocks in one callback produce two packets with consecutive `at`.
- **E3 — Bus Gain (trim) ignored without inserts.** Bus trim was applied only by the routing
  worker. Decision: apply bus trim in the callback's direct bus path and in the dry fallback
  path; the routing worker keeps applying it on the processed path.
  Proof: automated, bus trim changes the bus level with no inserts.
- **E4 — Broker polling at realtime priority.** Track brokers, the live broker, and the routing
  broker slept 100 µs in a loop (~10k wakeups/s each). Decision: wait about half a block
  between queue checks when the queue is full or empty, and longer when the routing worker has
  no chains. The lead (≥ 4 blocks) absorbs the added wake-up delay.
  Proof: automated tests unchanged; wake interval recorded in code.
- **E5 — Routing worker priority.** The routing thread ran at normal priority while feeding the
  device. Decision: request audio priority as the track brokers do.
- **E6 — A clip edit retriggers notes on every track.** `set_arrangement` replaced every track's
  plan and bumped one global revision. Decision: reuse the previous `Arc<Plan>` for tracks whose
  clips and timeline map are unchanged; brokers replace their plan only when the `Arc` differs.
  Proof: automated, editing one track leaves other tracks' plan `Arc` identical.

### Plugin hosting

- **P1 — One late insert block quarantines a healthy plugin.** A process timeout marked the
  audio connection failed, closed the socket, and the session quarantined the plugin.
  Decision: a late reply yields the delay-aligned dry signal for that block; the connection
  stays open and the late reply is discarded when it arrives (request sequence numbers). Only a
  broken connection, an invalid reply, or a sustained stall (no reply for 1 s) is a failure.
  Proof: automated where the fixture allows; otherwise protocol-level tests.
  Implemented: `Request::Process` and `Reply` carry a `sequence` that the worker echoes.
  `AudioConnection` keeps the outstanding requests in order, buffers a partially received reply
  frame and unsent request bytes across calls, and validates every reply against the expected
  sequence. A discarded late reply still hands its restart request, latency, and plugin error to
  the next delivered reply. `process_effect` returns `Ok(None)` for a late block. At most two
  blocks may be outstanding; further blocks are skipped as dry without being sent. The
  connection fails on a broken socket, an invalid or out-of-sequence reply, or no reply to the
  oldest outstanding request within the stall limit (1 s realtime, 60 s offline). Instrument
  `AudioConnection::process` keeps its signature and already waits up to the stall limit, so an
  instrument block that is late but under 1 s is not a failure; it needs no late outcome.
- **P2 — Insert reconfigure blocks the routing worker for up to ~3 s.** Decision: bound the
  reconfigure on the processing thread to the existing 1 s lifecycle limits, and output dry
  while it runs; record residual blocking as a known limit if a full off-thread restart needs
  T16.
  Implemented off the processing thread: a reply that requests a restart or reports a new
  latency outputs dry and starts a short-lived helper thread that holds the effect's audio
  connection for Suspend, Stop, and Start (1 s limit each). The stage outputs its delay-aligned
  dry signal while the helper runs; when it publishes the new latency, `Processor::process`
  returns `true` so the caller performs the structural restart. A helper failure fails the
  effect as before. Known limits: `Effect::deactivate` and `Effect::capture` on the session thread
  wait for a running helper (at most about 3 s); instrument reconfiguration in the track and live
  brokers stays synchronous and is outside this item.
- **P3 — Capturing state from an active insert always fails.** `Effect::capture` called
  `worker.save()` which rejects while audio is attached. Decision: send `Save` over the control
  connection while the effect is stopped.
  Implemented: `ControlConnection::save` sends `Save` with the 10 s limit and enforces
  `MAX_STATE`; `Effect::capture` runs it inside `with_stopped`, so the stage is dry only while
  the plugin is briefly stopped.
- **P4 — VST3 editor edits overflow while the engine is stopped.** `performEdit` filled a queue
  drained only by `process`. Decision: drain pending edits into the component on the main
  thread when audio processing is not active, so edits are applied and saved and never
  overflow while stopped.
  Implemented in `native/vst3_bridge.cpp`: `performEdit` never fails for capacity. While the
  component is inactive, or while the audio queue is full or has a backlog, edits go to a
  main-thread backlog coalesced to the latest value per parameter; every backlog edit is newer
  than every queued edit, so order per parameter is kept. The main-thread poll moves the
  backlog into the audio queue while the component is active, and `activate` forwards it so the
  first blocks receive it. While inactive the audio thread never drains the queue, so the main
  thread merges leftovers into the backlog. Before `getState`, `save` applies the backlog with the
  SDK's validated parameter flush (setupProcessing, setActive(true), setProcessing(true), a
  zero-sample process with no audio buses, then setProcessing(false) and setActive(false)) on
  the main thread while no audio thread processes, and forwards output parameter changes to the
  controller. Restore clears the backlog. Only processor-to-editor feedback can still report an
  overflow, which cannot happen while stopped.
- **P5 — Control connection desynchronized after a timeout.** A timed-out control exchange
  left an unread reply. Decision: a control timeout terminates the worker (its state is then
  handled as an exited worker), so a stale reply can never be read as a later answer.
  Implemented: any I/O, timeout, or decode failure of a control exchange shuts down the control
  socket and kills the worker through a child handle shared by the instrument and its control
  connections (reaped under its lock, so no PID reuse). Waiting for another caller's exchange
  is not an exchange, so a lock-wait timeout returns an error without killing.
- **P6 — Deadline overshoot.** The poll timeout rounded up to whole milliseconds plus one.
  Decision: use sub-millisecond waits for audio exchanges.
  Implemented: the audio connection waits with `kevent` and a `timespec` timeout on macOS
  (`ppoll` on Linux) and re-checks its deadline after every wake. Control exchanges keep the
  millisecond poll.
- **P7 — UI waits up to 60 s on a hung worker.** Decision: keep long limits only for requests
  that legitimately take long (load, editor open, state restore) and shorten the rest; record
  the limits in the specification.
  Limits: worker startup 60 s, `Restore` 60 s, `Editor { open: true }` 60 s, `Save` 10 s (large
  plugin state), every other control request (`Start`, `Stop`, editor close, `Quit`, fixture
  controls) 5 s. Unchanged: insert lifecycle steps inside `with_stopped` 1 s each; audio stall
  limit 1 s realtime and 60 s offline.

### Session and UI

- **S1 — UI redraws ~33 times/s while idle.** Per-block counters in track diagnostics and the
  engine summary changed every poll. Decision: refresh volatile diagnostics at most once per
  second.
  Proof: automated, two polls inside one second with an unchanged project produce no update.
  Implementation: broker diagnostics and the engine summary are re-read when due; a newly
  attached track is shown at once and enabling audio resets the cache.
- **S2 — Send-level drag can lose its commit.** `PreviewLevel` waited for an acknowledgement
  and disabled the strip during the drag. Decision: send previews without acknowledgement,
  like fader previews.
  Implementation: a preview reaches only the published mix; the project, dirty state and
  history are unchanged. `SendEdit::CancelPreview` ends a cancelled drag. Any send edit,
  project change, history seek or stale epoch clears the preview. The UI shows its local drag
  value. A release always commits: while another action is in flight the commit waits and is
  sent when the session is idle. A release without a commit event, for example when the strip
  was disabled mid-drag, commits the last previewed level. Escape, focus loss or a dialog
  cancels the drag. The strip still disables while another action is in flight, so the visible
  design is unchanged.
- **S3 — Live recording rebuilds the whole preview each tick.** Decision: build the preview
  incrementally from new input only.
  Implementation: `clef_engine::recording::LivePreview` processes each captured packet once,
  using the same pass rules as `note_preview`. It rebuilds when input cannot be appended:
  after a re-sort, for a packet in a pass already left, or for one before the window. Each
  change still copies the visible marks of the current pass into the view.
- **S4 — Undo publishes to the engine before inserts reconcile.** Decision: reconcile first,
  publish only after success.
  Implementation: new timing is validated first, insert chains are reconciled, then timing and
  arrangement are published. If publishing fails, the previous timing and insert chains are
  restored (best effort).
- **S5 — Plugin paths are checked on every refresh and UI frame.** Decision: cache existence,
  re-check on the existing 2 s device rescan, and expose it in the view for the UI.
  Implementation: paths are checked once when first seen and re-checked every 2 s on the
  device-rescan cadence. Deviation: the re-check also runs while audio is off, unlike the
  device rescan, so missing plugins are still reported. `TrackView::missing` feeds the
  instrument slot; `InsertView::missing` uses the same cache.
- **S6 — File picker lists its folder every frame.** Decision: cache the listing per folder and
  refresh on navigation or explicit refresh.
  Implementation: navigating to a folder or reopening the picker reads the folder again.
  Reopening is the explicit refresh; no Refresh button was added, so the visible design is
  unchanged.
- **S7 — Instrument state copied per autosave and undo.** Decision: store instrument state as
  shared immutable bytes (`Arc<[u8]>`) like insert state, without changing the file format.
  Implementation: autosave snapshots, history entries and loads share the bytes. History
  memory accounting counts the shared length.
- **S8 — Arrangement scroll/zoom triggers full autosaves.** Decision: view-only changes do not
  mark the project dirty for autosave; they are still saved with the next real save.
  Implementation: `ArrangementView` (snap, scroll offset and zoom) updates the project
  without marking it dirty. It is written by the next Save or by an autosave caused by a real
  change, consistent with T07, which checkpoints dirty content. Consequence: closing after
  only view changes does not prompt, and those view changes are not kept. Take-lane view
  state keeps the T26 rule and still marks the project dirty.
- **S9 — Clip edits store whole tracks in history.** Decision: store only the clips that changed.
  Implementation: each track change stores the differing run of its clip list and the
  lengths of the unchanged prefix and suffix. Arrangement edits, comp edits and Record MIDI
  use this. Undo and redo check the list length and the stored run before applying.

## Verification Evidence

### Engine (2026-11-20)

- E1: `bus_sum_restarts_every_block_when_a_callback_spans_several_blocks` (callbacks of 400,
  256 and 73 frames; impulse heard once, no callback allocation). Before the fix the same
  setup produced the impulse at frames 37, 165 and 293. The accumulator is now one frame per
  bus, cleared before each sample.
- E2: `every_block_of_a_long_callback_gets_its_own_routing_packet` (one 384-frame callback
  yields packets at 0, 128, 256; the impulse appears only in the first packet).
- E3: `bus_gain_applies_without_bus_inserts`. Additionally, bus Gain/fader changes on the
  callback path are now ramped per bus id (5 ms, same as channels) instead of stepping.
- E4/E5: track, live and routing brokers wait `idle_wait` (half a block, ~1.3 ms at 48 kHz)
  instead of 100 µs; the routing worker waits 8× that while it has no chains, and now requests
  audio thread priority. Wake-ups drop from ~10,000/s to ~750/s per broker.
- E6: `republishing_an_arrangement_keeps_the_plans_of_unchanged_tracks`; brokers call
  `replace_plan` only when their plan `Arc` changed. Cost: the engine keeps one copy of the
  published clips to detect unchanged tracks.
- `cargo test -p clef-engine`: 52 passed, 3 ignored (native fixtures);
  `cargo clippy -p clef-engine --all-targets -- -D warnings`: clean.

### Plugin hosting (2026-11-20)

- P1: protocol tests in `crates/plugin-host/src/rpc.rs`:
  `late_effect_block_is_dry_and_its_reply_resumes_mid_frame_then_is_discarded` (a late reply
  split mid-frame returns `None` in under 30 ms, then the next block receives its own reply;
  the discarded reply's restart request is carried), and
  `outstanding_blocks_are_bounded_and_only_a_sustained_stall_fails` (six late blocks send only
  two requests; the connection fails only after the stall limit). Also
  `out_of_sequence_audio_reply_fails_the_connection`. Native fixture test (ignored, run):
  `a_late_block_outputs_dry_and_keeps_the_effect_running` (30 ms worker delay with a 2 ms
  budget gives the dry block, the next block is wet, no effect error, worker alive).
- P2: native fixture test (ignored, run): `a_restart_request_is_served_off_the_processing_thread`
  (every block returns in under 50 ms and stays dry until the helper finishes, then
  `process` requests the structural restart and the effect processes again without an error).
- P3: native fixture test (ignored, run): `capture_saves_state_while_the_effect_stays_attached`
  (an active insert captures its edited state and keeps processing).
- P4: no automated test: no available fixture calls `performEdit` without user input. Evidence:
  the bridge builds with `-Wall -Wextra` (no new warnings); `tests/insert_effects.rs`
  (VST3 and CLAP insert fixtures, ignored, run) and the native session tests below pass through
  the new save path. GUI evidence (edit an open VST3 editor while stopped, then save and
  reopen) is pending.
- P5: `control_timeout_terminates_the_worker_so_a_stale_reply_is_never_read` (the child is
  reaped right after the timeout; a later call fails instead of reading the stale reply).
- P6: `audio_waits_do_not_round_up_to_whole_milliseconds` (fastest of ten 300 µs waits measured
  339 µs; the previous poll waited at least 1 ms).
- P7: `only_load_editor_open_and_restore_keep_long_control_limits`.
- `cargo test -p clef-plugin-host`: 21 passed (the existing quarantine test
  `quarantine_survives_reopen_and_keeps_other_entries_and_formats_independent` failed in 2 of
  about 15 runs with "registry busy" and passed on rerun. The likely cause is a test that spawns
  a child process (`sleep`) while the lock file is open, so the child briefly holds the lock.
  The existing editor test already did this, and the new P5 test spawns one too; the quarantine
  code is unchanged). `cargo test -p clef-engine`: 53 passed, 6 ignored; all 6 ignored insert tests
  passed with `--ignored`. Native session tests passed with `--ignored`:
  `native_t08_replacement_restores_an_isolated_workers_captured_state`,
  `offline_releases_the_instrument_worker_and_keeps_its_state`, and
  `nine_inserts_and_master_survive_ordered_edits_undo_and_reopen`; and
  `tests/insert_effects.rs`. `cargo clippy --all-targets -- -D warnings`: clean.

### Session and UI (2026-11-20)

- S1: `volatile_diagnostics_refresh_at_most_once_per_second` (a changing engine-like counter
  is republished only after 1 s) and `idle_polls_inside_one_second_publish_no_new_view`.
  The existing `unchanged_idle_snapshots_do_not_repaint_but_commands_still_complete` passes.
- S2: `send_level_previews_are_transient_and_the_commit_is_one_edit` (session: previews leave
  the project, dirty state and history alone, and the commit is one undo step; cancel and New
  clear the preview) and `send_drags_preview_without_busy_state_and_the_release_always_commits`
  (UI: drags never set busy; a release during another action commits once the session is idle).
  GUI evidence pending.
- S3: `live_preview_matches_a_full_rebuild_for_every_poll` (looped and linear windows,
  irregular polls, controllers, packets before the window, and a late packet after a wrap).
- S4: no automated test. Without audio, reconciling inserts cannot fail, and with the
  fixtures it does not fail. Evidence: code order, and the native history tests listed below.
- Native session tests: all 10 ignored session tests passed with `--ignored`
  (`--test-threads=1`, 47 s), including `native_t06_records_routes_reopens_and_plays_an_instrument_clip`
  (live preview while recording), `native_t08_replacement_restores_an_isolated_workers_captured_state`,
  `nine_inserts_and_master_survive_ordered_edits_undo_and_reopen` and the commercial-instrument
  state tests (shared instrument state).
- S5: `plugin_paths_are_cached_and_rechecked_on_the_rescan_cadence` (a bundle created after the
  first check stays Missing until the next rescan).
- S6: `file_picker_lists_a_folder_once_until_navigation`.
- S7: `shared_instrument_state_keeps_the_container_byte_identical` (exact container bytes,
  shared bytes after clone, and the same state payload when re-encoding local
  `validation-output/*.clef` projects).
- S8: `arrangement_scroll_and_zoom_never_start_an_autosave` (no checkpoint while only the view
  changed; the next real autosave contains the view). The existing arrangement view
  save/reopen test passes.
- S9: `a_one_note_clip_edit_stores_only_the_changed_clip_and_round_trips` (a one-note edit in
  a 16-clip track costs two clips, under a quarter of the track; undo and redo restore
  the exact lists; a stale redo is rejected).
- Workspace: `cargo test --locked` passed (session 38 passed, 10 ignored; UI 55; model 44;
  engine 53, 6 ignored). `cargo clippy --all-targets --locked -- -D warnings` and
  `cargo fmt --all --check` are clean.

### Integration (2026-11-20)

- The quarantine "registry busy" flake was fixed: `update_kind` now retries the registry lock
  for up to 0.5 s (a child spawned by another thread briefly shares the lock descriptor between
  fork and exec). `cargo test -p clef-plugin-host` then passed 15 of 15 consecutive runs.
- Whole workspace after all three parts: `cargo test --locked` 211 passed, 23 ignored;
  `cargo clippy --all-targets --locked -- -D warnings` and `cargo fmt --all --check` clean.
  Graphify code graph refreshed.

### Known Limits and Remaining Evidence

- GUI pending: P4 (edit an open VST3 editor while stopped, save, reopen) and S2 (send drag
  during another action).
- Listening pending: bus routing with a device buffer above 128 frames (E1/E2) and bus Gain
  without inserts (E3).
- A plugin that is often late alternates wet and dry blocks instead of being quarantined (P1);
  T37 keeps this by decision and adds a Late warning. See [T37](T37-audit-limits.md).
- Instrument reconfigure in the track and live brokers stays synchronous (outside P2); moved
  off the broker thread by [T37](T37-audit-limits.md).
- A fader drag was cancelled, not committed, when another action started during the drag;
  [T37](T37-audit-limits.md) makes the release always commit.
- With S8, closing after only scrolling or zooming did not prompt, so that view change was lost;
  [T37](T37-audit-limits.md) saves it silently when leaving a clean, saved project.

## Open Questions

None. Behavior-affecting decisions above were delegated by the user on 2026-11-20.
