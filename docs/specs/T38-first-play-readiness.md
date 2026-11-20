# T38 — First-Play Readiness: Background Preload and Aligned Start

- **Status** — Done — 2026-11-20. Specification approved by the user on 2026-11-20;
  implementation, automated and native evidence, and the user's listening check complete.
- **Priority** — P1 (user-reported listening defect).
- **Task** — [T38](../../TASKS.md)
- **Dependencies** — T04 (lazy loading, deferred Play), T12 (insert latency restart),
  T15 (routing broker and master delay). Followed by [T39](T39-always-on-audio.md), which starts
  the engine automatically; T38 works with either Enable or automatic start.

## Problem

Opening `validation-output/T12-listening-v2.clef` and pressing Play crackles briefly on the first
Play only; Stop and Play again is clean.

Diagnosis (2026-11-20, temporary ignored session test on the Audient EVO16, 48 kHz / 128 frames,
removed after the measurement):

- No insert reply was late and no track missed a block; plugin cost was 0.2–0.5 ms per block.
- With audio enabled only the selected track (1) had an attached broker. Play lazily loaded
  tracks 2 and 3 (T04 "first explicit use loads once"). Deferred Play waited for the loads, then
  the brokers attached as the transport started. Their insert latency raised the total from 32
  to 289 samples, the engine bumped its epoch and realigned every track's compensation; the
  routing and track readers produced about 2300 transition frames (≈ 48 ms of dry/silent
  substitution) at the start of playback.
- About 1.4 s later the T12 CLAP latency fixture on track 3 requested its deliberate one-time
  restart (after 48 000 processed samples). The restart reported the same latency (257) but still
  forced a structural realignment: about 2000 more transition frames. On the second Play the
  fixture had already restarted, so playback was clean.

## Decisions (interview 2026-11-20)

- Combine both remedies: preload in the background and make Play wait until the graph is aligned.
- Preload every instrument track that has an instrument and is not offline, with or without MIDI
  clips, so it is also ready for live playing.
- A Play pressed while preloading or aligning waits with the existing pending indicator and starts
  as soon as the graph is ready. Stop cancels the wait.
- A track whose instrument fails to load (crash, quarantine, missing file) does not block Play:
  playback starts without it and the message names the track.

## Scope and Acceptance Criteria

### R1 — Background preload when the engine starts

- When the engine starts (Enable today; automatic start after T39), the session requests a load
  for every eligible track: instrument assigned, not offline, not quarantined, plugin path present,
  not already loaded or loading. Loads run on the existing per-track jobs, off the UI and audio
  threads, bounded by the existing job limit; tracks beyond the limit are requested as jobs finish.
- Loaded instruments attach to the engine as they complete, as they do today.
- Ineligible tracks keep today's behavior and messages. Quarantine and offline rules are unchanged.
- This supersedes the T04 rule "no workers at Open" while the engine runs: Open without a running
  engine still starts no worker. T04 and the multitrack-session UI docs are updated accordingly.

### R2 — Play waits for an aligned graph

- Deferred Play starts the transport only when all of these hold: no eligible track is loading,
  every loaded eligible track has an attached broker, and the engine has finished the latency
  realignment those attaches caused (no structural change pending).
- While waiting, the transport shows the existing pending Play state; Stop cancels it. Record
  follows the same rule.
- A failed load ends that track's wait: Play proceeds without it and the message names the track.
- The wait is bounded by the existing load lifecycle limits; no new timeout is introduced.

### R3 — Same-latency restart does not realign the graph

- When an insert or instrument restart completes with an unchanged latency, the stage resumes
  without bumping the engine epoch or rebuilding compensation delays. A changed latency keeps the
  current structural realignment.
- Realtime rules hold: no allocation, lock, wait, formatting, logging, or I/O added to the callback.

### Out of scope

- The T12 fixture's deliberate restart itself (test behavior, unchanged).
- Realignment glitches when latency genuinely changes during playback (separate work if wanted).
- Removing Enable and audio preferences (T39).

## Verification Plan

- Automated: session test — engine start requests loads for every eligible track and skips
  offline, quarantined, and missing-plugin tracks.
- Automated: session test — deferred Play does not start while a track is loading or its latency
  realignment is pending, starts when ready, starts without a failed track and names it, and is
  cancelled by Stop.
- Automated: engine test — a restart reporting unchanged latency leaves the epoch unchanged; a
  changed latency still realigns.
- Native (ignored, real device): open `T12-listening-v2.clef`, start the engine, press Play at
  once; transition frames during the first second of playback stay at the normal transport-start
  value measured on a second Play (512 frames in the 2026-11-20 run).
- Listening (user): first Play after opening T12-listening-v2.clef is clean except the fixture's
  deliberate restart, which no longer realigns the graph.

## Implementation (2026-11-20)

- R1 — `Session::preload_instruments` runs after Enable attaches loaded runtimes; it requests a
  load for every instrument track that is online, unquarantined, has an existing plugin path, and
  is neither loaded nor loading. Refused requests join the Enable message.
- R2 — `Session::play_waiting` (formerly `loading_playback_tracks`) holds deferred Play while any
  online track is loading or `Engine::aligned()` is false. The engine clock keeps an
  `alignment_change` counter, bumped by `Engine::attach`; the device callback publishes the epoch
  and counter it saw when every active reader and the routing delivered packets of that epoch for
  the whole callback (atomics only). `Engine::attach` now publishes the track's insert-chain
  latency with the instrument latency, so the broker's first chain check no longer realigns later.
- R3 — an insert stage whose restart keeps its latency resumes in place (`Processor::process`);
  an instrument restart that keeps its latency skips the latency refresh and epoch bump.

## Verification Evidence

- Automated (2026-11-20): `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D
  warnings`, `cargo test --locked` — 217 passed, 25 ignored. New tests:
  `alignment_waits_for_every_active_track_to_deliver_the_current_epoch` (engine) and
  `engine_start_preloads_every_playable_instrument_and_play_waits_for_loads` (session).
- Native (2026-11-20, ignored `native_t12_first_play_starts_on_an_aligned_graph`, Audient EVO16,
  48 kHz / 128 frames): Open, Enable and Play at once. 0 transition frames in the first 2.5 s of
  the first and of the second Play; latency stable at 289 samples. Before the change the same
  scenario produced about 4350 transition frames on the first Play. The run does not show
  whether the CLAP fixture's one-time restart fell inside the playback window or before it,
  while Play was waiting; R3 has no separate automated test because the restart paths need
  native workers.
- Listening (user, 2026-11-20): first Play right after opening T12-listening-v2.clef and a
  second Play are clean; confirmed by the user.

## Open Questions

None.
