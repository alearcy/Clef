# T02 Implementation Handoff

## Completed — 2026-11-17

T02 is Done. All eight agreed criteria passed; see the canonical
[T02 verification](T02-engine-validation.md#completion--2026-11-17). The user explicitly
confirmed physical gestures, all three ten-minute scheduled runs and all three
final WAV/live comparisons with completed tails and no problems.

Final root checks pass:22unit tests and2explicit native integrations after default
TestSynth rebuild/bundle; check/build/fmt/Clippy pass. Final exports, recovery,
terminal-group/parent cancellation and both actual format reconfiguration runs
have passing reports. Graph9687nodes/19019edges, code-only, native glue reviewed
directly. Final binary hash eaf857da58df1fe2cbc03cf2381894a4b58d9bfc18991113632f6d179888d5e6.

Three long scheduled runs used one prior binary; the only subsequent product change
classifies terminal-group render cancellation correctly. No live/editor/DSP change
followed the passing hardware matrix. Ten editor cycles, actual preset changes and
fresh-worker restores, and supported original scale changes were verified.
All worker/device sessions were closed normally; no timed run remains active.

Stop T02 work. Do not repeat passed gates, add polish or start another task unless
the user requests it. T01 is linked to the result and remains pending its separate
foundation-adoption decision. Historical checkpoints below are preserved as history.

## Resumed Progress — 2026-11-17

User explicitly resumed T02 to completion. Current full physical **Pianoverse**
run passed software counters and user listening/gestures: report
`reports/T02-pianoverse-in1-qos-midi-5min.json`,3040 MIDI events,~301.82s,
zero missed/stale blocks/errors/overflows/late events. User answered “Tutto
verificato, nessun problema.” Input is MIDIFACE In1, current index2. Added native
pthread QoS for live broker/worker and timing diagnostics;15 root unit tests passed.

**ZebraHZ VST3** original editor showed initialize loaded. Master67.50 was changed
by wheel to20.00 while active, saved in
`validation-output/states/zebrahz-vst3-initialize-master20.state`, fresh worker then
visually showed20.00. CUA completed with “Verificato stato ZebraHZ VST3”. Console
65902 was closed. Its completed five-minute physical test was session74590 with
report/log `reports/T02-zebrahz-vst3-in1-qos-midi-5min.{json,log}`. Completed:14,490,368 measured frames,1834 MIDI events,zero misses/errors/
overflows/late events. The user confirmed all requested controls and no problems. Poll the actual handle before any restart;
do not run heavy builds during a hardware measurement. On completion ask user for
listening/gesture confirmation, then prepare CLAP initialize state and its5min run.

Actual format latency fixtures are being implemented. TestSynth now has optional
`clef-latency-fixture` build feature: factory exposes its single-object class only,
with real0/257 impulse delay, strict lifecycle checks, and an output parameter
notification after48000 realtime samples that reaches the controller and invokes
restartComponent(kLatencyChanged). Default TestSynth behavior is unchanged. New
helper `vendor/rust-vst3-host/test-plugin/src/clef_latency.rs`. Default and feature
builds passed before the latest automatic-notification additions; rebuild both
later. Feature uses separate `target/latency-vst3` to avoid overwriting normal bundle.
Add bundle/provenance/patch documentation and actual fixture tests before claiming.

New workspace crate `fixtures/clap-latency` uses same pinned Clack as the host,
outputs deterministic impulses with real0/257 delay and requests_restart after
48000 realtime samples. In activate, it applies requested latency and invokes
host latency.changed on the main thread; verifies get latency only active. Source
is written but not built; Cargo.lock needs offline resolution of clack-plugin.

Next: compile/fix fixtures; implement coordinated automatic live restart off callback
(current broker still errors on restart), generation invalidation/transition gap
counters, then actual per-format tests. Script `scripts/t02_recovery_probe.py` and
internal `fault-exit`/`fault-hang` hooks were added but not exercised; root check
passes. Need final fmt/clippy/tests. Graph refreshed at task start to9515nodes,
18743edges with SDK services included and samples excluded; AST warns28 SDK syntax
files partially modeled and45 no-symbol files. No semantic docs extraction or viz.

## User Correction and Paused Checkpoint — 2026-11-17

The user corrected the physical connection: SL88 is connected to **MIDIFACE port 1**,
not port 2. Fresh enumeration confirms `MIDIFACE 2x2 Midi In 1` at current MIDI
index **2**; In 2 remains index3. Indices are transient: enumerate before reuse.
This supersedes all earlier In 2 configuration statements below.

The five-minute Pianoverse attempt on In 2 has finished and its process is terminal.
It recorded zero MIDI events and cannot establish physical MIDI or listening
acceptance. The user explicitly withdrew the earlier answer saying they were
playing/hearing the configured input. Final software counters also recorded
29 missed/stale blocks, zero output errors, and128-frame callbacks. The wrong input
does not explain or invalidate those deadline observations; investigate them
separately. Preserve the raw report
`reports/T02-pianoverse-sdk-midi-5min-2026-11-17.json` as failed evidence.

The correction was made while paused. The user subsequently explicitly requested
continuing T02 to completion; that revokes the pause. Use In 1 for physical tests.

## Active Goal and Current Checkpoint — 2026-11-17

User goal: finish all T02 acceptance, and evaluate reverting to the SDK bridge if
errors persist. The user **approved** the concrete SDK bridge proposal in
`reports/T02 VST3 backend reassessment.md`; do not request foundation approval again.

The new `native/vst3_bridge.cpp` and `src/vst3.rs` replace the production VST3
backend. Main/audio handles have separate ownership and bounded atomic parameter
queues, no shared editor/process mutex. SDK 3.8.1 and submodules are pinned in
[`vendor/vst3sdk-provenance.md`](../../vendor/vst3sdk-provenance.md). Command Line Tools build only the required SDK
services through `native/CMakeLists.txt` / `build.rs`. SDK PlugProvider was replaced
by explicit initialization plus SDK ConnectionProxy because the TestSynth lacks
optional IConnectionPoint interfaces; SDK source itself is not patched. The root
rust-vst3-host dependency has been removed; its workspace/fixture remain for tests.

Live CPAL/midir driver and commands now exist in `src/live.rs`: `start DEVICE LEFT
RIGHT`, `midi DEVICE LEFT RIGHT PORT`, `status`, `stop`. Current indices: EVO16=2,
MIDIFACE In 2=3; enumerate before reuse. 48 kHz, requested/observed128 frames,
outputs1–2. Four-block/512-sample IPC lead, bounded preallocated SPSC audio packets,
stale sample positions discarded, callback has no IPC/locks/formatting/allocation.
AudioConnection is moved to the broker while Instrument retains control ownership.
Timeout/framing failures invalidate the audio socket. Explicit recreate is retained.
Dynamic latency currently stops the broker with an error: automatic coordinated
reconfiguration and real per-format fixtures are still required.

State/GUI observations: AA Y7 selected and loaded in original Pianoverse editor,
Width changed by real drag to63.2, played53.7s with zero misses/errors, saved to
`validation-output/states/pianoverse-AA-Y7-width63.state`, destroyed/recreated worker,
then original editor visually showed AA Y7 and Width63.2. This directory is ignored;
keep these commercial snapshots out of vendored/committed assets. The earlier
width65 snapshot came from an AX-only setter and did not preserve that parameter;
do not use it as acceptance. Single-component VST3 restore now skips the redundant
setComponentState call that Pianoverse rejects with code3.

The user replied **ready** for physical MIDI/listening. The current five-minute
Pianoverse run is tool session9012, started using `scripts/t02_live_probe.py` with
AA Y7 width63 state, device2, MIDI3, duration300. Its log/report are
`reports/T02-pianoverse-sdk-midi-5min-2026-11-17.{log,json}`. Revalidate its handle;
do not restart a live test just because it has not printed a final report. A report
is written at completion. User has been told to play now. Ask for observed sound,
dropouts/stuck notes after it finishes; do not infer listening from counters.

Two explicit native tests passed after rebuilding/bundling TestSynth: existing
PDC/WAV fixture plus SDK TestSynth save/recreate/restore/nonzero full WAV. Root
unit suite14passed, two nativeignored. Check/clippy passed before latest minor
cleanup; run final required gates again. RPC regression now covers queued error
reply after peer close and blocked-write/trickle whole deadlines. Nonblocking
bounded polling replaces Darwin SO_RCVTIMEO, which masked rapid failure replies.

Remaining: real VST3/CLAP dynamic latency fixtures and coordinated restart/gap
measurement; crash/hang tests/recovery; MIDI note tracking/overflow cleanup proof;
commercial editor/state/resize matrix for all formats, full 10min scheduled and
5min physical runs per instrument plus user listening/tails. Long scheduled tests
have not run. Zebra initialize preset still needs current GUI confirmation/state.
Pianoverse ten-cycle SDK probe is still pending. New live runner records software
counters; it never asserts human listening. It should preserve latest counters
also on error; this still needs review. Finish Graphify refresh including native
bridge/SDK services and update evidence/spec/TASKS before any final report.

CUA current part completed with summary “Verificato ripristino AA Y7”. No app left
open from that check; its console83472 was closed. If using CUA after compaction,
call cua.rewriteDocumentation first. `/private/tmp/ClefT02.app` is an identical
binary wrapper for CUA identification; recopy current binary only while it is not
running before further GUI checks. No final T02 acceptance is claimed.

## Latest Continuation — 2026-11-17

The first stale-resize issue below is now fixed in the shared backend. `close_editor`
clears pending resize after detach callbacks; see patch 0002 and its regression in
[patch documentation](../../patches/vst3-host/README.md). Regression failed before
and passed after. Clef check/build/test/fmt/Clippy passed; backend default suite:
238 passed, two ignored. No changes to processing ownership or CLAP were made.

ZebraHZ VST3 now completed ten create/destroy cycles with normal worker exit,
963 processing blocks and no detached-editor error. Overall probe still failed:
zero held-note audio and maximum processing RPC 518.25 ms. Report:
`reports/T02-zebrahz-vst3-editor-default-2026-11-17.json`. This run used the initial
plugin state, since `/private/tmp/clef-zebrahz-initial.state` no longer exists.
The state-dependent failed setup attempt is retained separately. Do not treat the
old temporary snapshots or review checkout as available without checking.

Resume with VST3 processing/editor serialization and CLAP full-sequence/lifecycle
diagnostics. Remaining hardware/listening acceptance and implementation below are
still outstanding. The earlier checkpoint below is retained as historical detail.

## Latest User Direction and Authority

Continue T02, using **Rust + egui/eframe** for Clef-owned windows/controls. The prior direct-AppKit draft has now been replaced. General T02 scope and the patched rust-vst3-host backend are already approved; do not ask again. Final engine adoption remains a T01 decision pending complete T02 evidence.

The user confirmed hardware setup in this continuation: **Audient EVO16 outputs 1–2**, **MIDIFACE 2x2 Midi In 2** connected to the SL88, **ZebraHZ initialize** preset. Pianoverse remains **Grand Piano Y7 / AA Y7**. Required rate is 48 kHz, requested buffer 128 frames; output is stereo PCM24 WAV with ten seconds of tail. Do not reinterpret enumeration as actual hardware playback evidence.

Italian conversation; English project artifacts. Prefix every shell command with `rtk` (`rtk proxy` as needed); read `/Users/arcy/.codex/RTK.md`. Read AGENTS.md, TASKS.md and [T02 specification](T02-engine-validation.md). UI authority is `docs/UI/`; legacy C++ references do not select this project's implementation stack. No agents were spawned. This continuation ended with focused verification and evidence recording; no product fixes were made after the newly observed editor failures were identified.

## Approved Dependencies and Earlier Investigation

- VST3: vendored rust-vst3-host 0.11.1, upstream commit `6c64cdffc6740cce0b4c5b6916724697b0ef9ff8`, with the approved stale editor-value/state-restore patch. See [evaluation](../../reports/T02%20rust-vst3-host%20evaluation.md), [patch instructions](../../patches/vst3-host/README.md), and vendored UPSTREAM.md.
- Clack host/extensions: `27ca28391b57a61bad4bb5099288a7bf9247350d`; CPAL 0.18.2, midir 0.11.1, rtrb 0.4.1, hound 3.5.1.
- Window implementation now uses the cached **eframe 0.34.3**, glow renderer, raw-window-handle 0.6.2. No direct Clef objc2/AppKit dependency remains. These do not constitute final T01 foundation adoption.
- Original state-restore bug: 118 Pianoverse editor initialization values (78 differing from restored state) were reapplied in the first processing block. Shared-layer fix clears stale pre-restore values, preserving edits emitted during/after restore. The fixture failed before and passed after. Earlier 238-test evidence must remain distinct from this continuation's single explicitly selected native regression.

Private snapshots still existed when checked:

- `/private/tmp/clef-pianoverse-AA-Y7-original.state`: 901,919 bytes.
- `/private/tmp/clef-zebrahz-initial.state`: 31,369 bytes.
- Review checkout `/private/tmp/clef-rust-vst3-host-review` still existed; all vendored Rust files matched its patched source byte for byte after formatting.

Verify temporary paths before reuse. Do not add commercial state/content to vendor assets.

## Current Implementation

- `src/native.rs`: a small wrapper borrowing eframe's raw NSView handle for plugin ABI attachment only. Window visibility, focus, resizability and size go through egui viewport commands. No application NSWindow/NSView construction or native event pump.
- `src/worker.rs`: one plugin per child; eframe owns its main event loop, using `App::logic` while hidden. The parent window persists hidden, but editor resources are destroyed and recreated on close/open. CLAP show/hide requests are serviced; partial editor-open errors trigger cleanup. User resizing negotiates with the plugin if supported. **Known stale VST3 resize failure remains**, below.
- `src/rpc.rs`: separate control and audio Unix connections in a private 0700 directory; whole-exchange deadlines recalculate remaining time across short reads/writes. Accepted sockets are explicitly blocking on macOS (otherwise inherited nonblocking mode caused EAGAIN). Audio packets capped at 128 KiB; state/control packets bounded separately. Owned-child kill/reap remains outside audio callbacks.
- `Request::SuspendAudio` is exchanged on the audio connection before lifecycle control; the worker stops CLAP processing on its processing thread before main-thread deactivation. Disconnect teardown shuts both connections, joins readers, then detaches/stops. Full hostile crash/hang recovery has not been verified.
- `src/clap_host.rs`: unused input storage removed; inactive parameter-flush requests serviced separately from on-main-thread callbacks, active process exchanges parameter changes. **Inactive latency querying and realtime diagnostics remain**, below.
- `src/graph.rs`: now wired into main and compiled. Existing reconverging 257-sample fixture, cycle/channel validation and block-partition tests pass. No format-specific dynamic-latency fixture exists yet.
- `src/render.rs`: shared fixed sequence + graph render driver; offline plugin mode, declared-latency trimming, exact range plus ten-second tail, abort on reconfiguration, atomic no-overwrite publish only after successful processing/stop/WAV finalization.
- `src/main.rs`: usable partial CLI, no longer an unconditional scaffold error:
  - `rtk proxy target/debug/clef devices`
  - `rtk proxy target/debug/clef plugin PLUGIN [STATE]`: console commands `open`, `close`, `save PATH`, `restore PATH`, `recreate`, `render PATH`, `quit`.
  - `rtk proxy target/debug/clef render PLUGIN WAV [STATE]`.
  - `@fixture` is an internal deterministic backend; its render uses the 257-sample reconverging plan.
- `scripts/t02_worker_probe.py`: reproducible worker-only concurrent editor diagnostic, no audio device. Requires Unix sockets and WindowServer; saves failure evidence too, refuses report overwrite.
- `tests/t02_native.rs`: explicit ignored integration test launching the actual fixture process, checking compensated MIDI offset, PCM24/stereo/rate/frame count and refusal to overwrite.
- **No live CPAL/MIDI driver**, bounded realtime handoff, live start/stop controls or recovery broker exists yet.

## Verification Results — 2026-11-16

Passed on final source state:

- `rtk cargo check -p clef --locked`
- `rtk cargo build -p clef --locked`
- `rtk cargo test -p clef --locked`: **10 pass**, one integration test ignored by default.
- `rtk cargo fmt --all --check`
- `rtk cargo clippy -p clef --all-targets --locked -- -D warnings`
- Outside sandbox: `rtk cargo test -p clef --locked --test t02_native -- --ignored --nocapture`: **one pass**. Proves actual isolated eframe fixture → fixed sequence → PDC → exact WAV; no live audio claim.
- Explicit TestSynth state regression: build `rtk cargo build -p vst3-host-testplug --locked --offline`, bundle using `rtk proxy env CARGO_TARGET_DIR=/Users/arcy/dev/clef/target PROFILE=debug bash vendor/rust-vst3-host/scripts/bundle-test-plugin.sh`, then run the exact ignored regression from patches README with `--offline`: **one pass**, 238 filtered out.
- Graphify refreshed before analysis and after source changes: 3,543 nodes / 7,447 edges. Command: `rtk proxy graphify extract /Users/arcy/dev/clef --code-only`. Cargo.toml has no extractor symbols; do not treat that warning as a missing Rust module.

Outside-sandbox execution is needed for Unix socket listeners/WindowServer and actual hardware enumeration. There was no auto-review rejection. No Git repository exists at this project root, so no commit/diff baseline is available.

### Offline Samples and GUI

[Offline measurements](../../reports/T02-offline-probe.json): **all three commercial cases** produced readable stereo 48 kHz/PCM24 WAVs with exactly **2,323,200 frames** and nonzero samples. Paths/hashes/peaks are in the JSON. Exports remain under `/private/tmp`, not the repository. Pianoverse peak PCM24 2,484,530; ZebraHZ VST3 3,465,532; ZebraHZ CLAP 3,336,095. Both Zebra renders first exceed 32 PCM24 units at output frame 28,808. No listening or tail acceptance occurred.

ZebraHZ VST3's original editor was visually observed rendered inside the eframe window. CUA could not identify the unbundled binary, so an identical binary was copied to temporary `/private/tmp/ClefT02.app` (`dev.clef.t02`) solely for inspection. Its console and worker were closed after inspection. If resuming CUA after context compaction, use `cua.rewriteDocumentation()` as required by that tool. The CUA portion received its completion summary. No process should be assumed running.

## Newly Reproduced Issues — Resume Here

1. **VST3 pending resize after editor close.** [ZebraHZ VST3 diagnostic](../../reports/T02-zebrahz-vst3-editor-probe.json) exits code 1 after two completed cycles, stderr `Plugin editor is not open`. `MainPlugin::service` takes and applies editor resize requests regardless of whether an editor is attached; vendored `resize_editor` errors when `plugin_view` is absent. Leading host-side cause is stale resize servicing after close. Verify ownership/lifecycle, discard requests for detached views, and add regression coverage before rerunning ten actual create/destroy cycles. Do not call this a confirmed plugin crash or replace lifecycle testing with hide/show.
2. **VST3 editor work still stalls processing.** [Pianoverse diagnostic](../../reports/T02-pianoverse-vst3-editor-probe.json): ten cycles and nonzero audio, but maximum process exchange **977.38 ms**, correlated with first editor open **985.85 ms**. 32 exchanges exceeded 2.667 ms. Shared `Arc<Mutex<Plugin>>` structurally serializes controller/editor and processor. Two sockets alone are insufficient. Fix ownership/preparation coherently in the shared layer; do not silently precreate/cache the editor or adopt plugin-specific workarounds as an acceptance substitute. This is not a CPAL underrun measurement.
3. **CLAP lifecycle and short realtime probe.** [ZebraHZ CLAP diagnostic](../../reports/T02-zebrahz-clap-editor-probe.json): all ten cycles, normal exit, 1,067 blocks and max process RPC **1.124 ms**, but held-note peak remained zero. Full offline sequence is nonzero, so do not claim all CLAP processing is silent. Use the complete agreed scheduled sequence to distinguish MIDI/transport/arp/preset timing from editor interference. Current diagnostic sends one held note at the first block, not the complete sequence. Plugin warns about latency queries while inactive; `MainPlugin::load/control → update_latency → ClapMain::latency` currently does this. Offline also emits a main-thread warning around render/startup; inspect exact callback requirements and thread-check results rather than assuming it is a host violation or harmless.

## Remaining T02 Implementation / Acceptance

After resolving the issues above:

- Implement bounded preallocated parent audio handoff: callback only copies frames, emits silence and updates atomics, with no IPC, locks, allocation/destruction or formatting. Count misses/errors/recovery gaps and reject stale generations.
- Open explicit EVO16 outputs 1–2 at 48 kHz/requested 128 frames; record actual callback sizes. MIDIFACE input label confirmed above; preserve MIDI channel, clock mapping/late policy and overflow panic cleanup. Enumeration found the real hardware outside sandbox; no need to infer it missing from the old empty system_profiler output.
- Add actual VST3 and CLAP reported-latency/restart fixtures; main-thread reconfigure/audio-thread stop ordering and observed transition gaps. Graph fixture alone is insufficient.
- Verify worker termination/unresponsive fixtures: responsive supervisor, silence, failure within two seconds, explicit restart from last successfully saved state, unsaved edits lost. Do not induce commercial plugin crashes deliberately.
- Complete state-edit/save/destroy/new-worker/restore checks including observable parameter and preset, full editor resize behavior, scheduled/live control commands and cancellation/failure evidence.
- Run approved ten-minute repeated sequence and five-minute SL88 tests **for each mandatory instrument**, with user listening and counters; listen to offline tails and compare musical behavior. No hardware/listening evidence currently exists.

Keep TASKS.md concise and put detailed evidence in the specification/reports. T02 remains **in progress**, and T01 foundation selection remains pending. Do not mark any acceptance criterion complete from a build, a short worker diagnostic or file-format checks alone.

Final rebuilt/bundled TestSynth verification on 2026-11-17: both explicitly selected
`state_editor_queue_tests` native regressions passed (238 filtered out).

## Current CLAP GUI Check

ZebraHZ CLAP initially failed a window-size update. Source/official CLAP protocol
confirmed plugin-originated resize requests already change the GUI and do not
require a set_size echo. Worker now only updates the host area for those requests,
skips set_size if the current GUI already matches a host size, and clears pending
GUI signals on destroy. Actual corrected full-sequence ten-cycle report passed:
`reports/T02-clap-resize-fixed-sequence.json`,14400 blocks,nonzero0.39769 peak,
max RPC4.28ms. This is not a live-device underrun metric.

Current wrapped CLAP console is session31358. Original editor visually confirmed
initialize loaded and Master67.50; wheel changed it to20.00 while MIDI/audio active.
Stop/save/recreate/open was requested for
`validation-output/states/zebrahz-clap-initialize-master20.state`. Need visual
confirmation of Master20 after recreation, then CUA completion summary and close
console before the5min physical CLAP runner. App binding is clapApp. No test is
currently running on the hardware except this short setup console;31358 must be
closed before the dedicated run. Keep user informed when physical measure starts.
