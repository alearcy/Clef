# T02 — End-to-End Engine Validation

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

**Status:** Done — agreed acceptance matrix passed on 2026-11-17.
**Current evidence:** See [Completion](#completion--2026-11-17). Earlier checkpoints document the validation history.
**Date:** 2026-11-16
**Task:** [T02](../../TASKS.md)
**Foundation evaluation:** [T01](T01-engine-foundation.md)

## User-Requested Outcome

Validate the complete path:

**MIDI → external instrument with native editor and persisted state → latency-compensated routing → audio output and offline rendering.**

The purpose is to test whether a Clef-owned engine built using focused libraries can support the critical workflow. The prototype should provide evidence for T01 rather than assume that an architecture or library list is already proven.

## Established Direction and Research Input

The user has established VST3 and CLAP as joint first priorities, without a strict order between them, with AU later. Rust is preferred, not mandatory; a limited native bridge is acceptable where it provides a concrete advantage. These choices do not need to be asked again.

The [extended T01 evaluation](../../reports/Engine%20ownership%20and%20library%20selection.md) supplies conditional library choices and compares Clef-owned responsibilities against Maolan reuse. Clack and either a Rust VST3 backend or SDK-backed bridge require real host services; they are not turnkey hosting. CPAL output and supporting libraries remain provisional. Partial VST3 source/build/runtime checks and a local correctness patch are now recorded in the candidate evaluation; the full prototype and T02 acceptance remain outstanding.

## Confirmed Test Inputs — 2026-11-16

- User-selected instruments: Pianoverse and ZebraHZ.
- MIDI sources: both a scheduled test sequence and a physical keyboard.
- Audio interface: Audient EVO. Exact EVO variant and output-channel labels will be recorded when the device is enumerated; do not guess them.
- MIDI interface: MIDIFACE, with a Studiologic SL88 keyboard. Exact MIDIFACE variant and input-port labels will be recorded during device setup.
- Initial live configuration: 48,000 Hz, requested buffer size 128 frames. Record the actual device configuration and callback sizes; do not silently substitute a different sample rate or buffer setting.
- Offline output: stereo WAV, 48,000 Hz, 24-bit PCM, including the sound tail. This does not prescribe the plugin processing sample representation.
- Plugin crash isolation is required: external plugins run in separate processes, outside the application/audio-device process.
- Local metadata inspection found Pianoverse VST3 1.0.11, ZebraHZ VST3 2.9.4, and ZebraHZ CLAP 2.9.4 in the standard system plugin directories. Versions are from bundle metadata, not runtime validation. No Pianoverse CLAP bundle was found in the inspected system/user VST3 and CLAP directories.
- Execution host reports macOS 26.6.2 on arm64. Selected VST3 loading/processing and assisted Pianoverse editor/content/state checks now have partial runtime evidence in the candidate report. CLAP, audio/MIDI connectivity, full GUI acceptance, and hardware behavior remain unverified.
- `rtk proxy system_profiler SPAudioDataType` returned an empty device list in this execution context; it did not confirm the interface model or connection state.

## Approved Validation Plan — 2026-11-16

The user approved this proposal on 2026-11-16 while requesting evaluation of `rust-vst3-host` as a VST3 alternative. Scope, process isolation, terminal-based control, test durations/tolerances, and the ten-second tail policy are approved. The user subsequently approved rust-vst3-host 0.11.1 at the reviewed pin plus the verified local patch in place of the standalone SDK bridge. This is provisional adoption for T02, not final T01 engine adoption. Component choices derive from the [T01 extended evaluation](../../reports/Engine%20ownership%20and%20library%20selection.md); that research is not local runtime evidence.

### Provisional Foundations

- Validate a Clef-owned Rust processing core, with CPAL for audio output, midir for physical MIDI, Clack for CLAP hosting, and rust-vst3-host 0.11.1 at commit `6c64cdffc6740cce0b4c5b6916724697b0ef9ff8` plus the verified local state-restore patch for VST3. Use bounded inter-thread handoff where needed and hound for the bounded WAV artifact. Pin exact versions/revisions and check their integration constraints before implementation; report any material deviation for agreement.
- The approved VST3 library owns module/controller/editor/state/host services in its helper. Clef owns the audio device, bounded handoff, scheduling and routing. Its source is vendored to apply the verified patch reproducibly without relying on an unpublished remote fork; preserve the pin, license, patch and provenance record.
- Each loaded plugin instance runs in its own child process. Its native editor and main-thread plugin callbacks live in that process; the supervisor owns the audio device, routing, process lifetime, and failure reporting. CLAP and VST3 follow the same isolation requirement.
- Prepare buffers and IPC storage outside the audio callback. Audio processing must not wait indefinitely for a worker, perform control I/O, or reclaim plugin objects on the audio thread. Bound IPC payloads, validate sizes/offsets, and account for transport buffering in path latency. A missed deadline produces silence for the affected path and an observable counter; offline processing fails rather than silently skipping missing blocks.
- Share processing/event/routing semantics between live and offline drivers, while using each plugin format's required processing mode and lifecycle. Plugin processing uses floating-point audio; 24-bit quantization belongs to WAV output.

### Test Matrix and Control Surface

- Mandatory instrument cases: Pianoverse VST3 1.0.11, ZebraHZ VST3 2.9.4, and ZebraHZ CLAP 2.9.4, subject to confirming the actual loaded binary versions and native compatibility.
- Run each instrument separately, with one selected stereo output and one MIDIFACE input; accept channel-voice input on all MIDI channels and preserve channel identity. Exercise notes, velocity, sustain pedal, pitch bend, and modulation. This does not establish MPE or MIDI 2.0 coverage.
- Use a command-line validation runner plus original plugin editors hosted through egui/eframe windows. The user reaffirmed egui as the UI foundation and rejected direct native macOS application UI construction; do not use the current AppKit window draft as the implementation direction. Provide commands to select device/port/plugin, start/stop a fixed sequence or live MIDI, open/close the editor, save/recreate/restore state, render, and explicitly restart a failed worker. No complete DAW surface is proposed for this task; egui/eframe supplies the host window around the original editor. A later product UI task will require its own approved proposal.
- Load explicit plugin paths in workers; a general plugin browser/scanner is outside this gate. Missing content, authorization dialogs, unsupported architecture, unavailable hardware, and load failures must be reported as blockers, not counted as passes.

### Approved Acceptance Criteria

1. **Scheduled MIDI and transport:** use a fixed 16-bar sequence at 100 BPM in 4/4, with a playback start at bar 3, sample-offset events across block boundaries, repeated notes, chords, sustain, pitch bend, and modulation. Automated checks establish exact event offsets and ordering using a deterministic test processor. Stop, worker replacement, and input overflow must not leave held notes or sustain when playback resumes. Plugin-dependent sound termination is checked manually.
2. **Native editors and state:** for each mandatory instrument, open/close/reopen its original editor ten times during playback and exercise resizing if supported. Change a preset and an observable parameter, save state, destroy the worker, create a fresh worker, and restore. Confirm preset/parameter restoration and audible behavior; do not require bit-identical output from nondeterministic instruments. No crash, stuck editor, or application hang is acceptable.
3. **Routing and PDC:** split a deterministic impulse source into two reconverging stereo paths with unequal declared latency (0 and 257 samples). After compensation, the two arrivals must coincide at the same sample; compare live-core and offline output within an absolute tolerance of 1e-6 for this fixture. Include IPC buffering in cumulative latency. Reject cycles and incompatible channel connections without replacing the last valid routing plan. The test processor is a fixture, not a new product effect.
4. **Reported latency and restart:** exercise format-specific plugin-reported latency and restart notifications with a controllable test plugin for each format, including a latency change while playback is active. Reconfigure off the audio callback and resume without stale delay alignment or stuck notes. Record the transition gap; this first gate does not promise a gapless latency change. Real instrument tests alone cannot establish this behavior if they report no latency or never change it.
5. **Live output and hardware MIDI:** test at 48 kHz with a requested 128-frame buffer through Audient EVO. For each instrument, run the scheduled sequence repeatedly for ten minutes and play the SL88 via MIDIFACE for five minutes, including chords and sustain. After loading/warm-up, require zero detected worker deadline misses or output errors, no stuck notes, and no audible dropout reported by the user. Record actual callback sizes and all observable counters. Software timestamps do not establish physical key-to-speaker latency; this gate has no numerical hardware latency claim.
6. **Crash/hang isolation:** deliberately terminate a worker and separately simulate an unresponsive worker. The supervisor must remain responsive and replace the affected output with silence, report the failure within two seconds, and never block the audio callback waiting for recovery. Restart is explicit and uses the last successfully saved state; report unsaved plugin edits as lost. Do not automatically reload a repeatedly failing plugin. Use controlled fixtures rather than intentionally destabilizing commercial plugins.
7. **Offline export:** render the scheduled range to stereo WAV at 48 kHz/24-bit PCM, trim compensated leading latency, and append a proposed fixed ten-second tail. Listen to the ending for each selected preset; if the tail is still audible at the boundary, record failure and agree on a longer tail before acceptance. This is a bounded prototype tail policy, not automatic silence detection. Verify format, exact expected frame count, successful finalization, and file readability. Reject clipping rather than normalize silently; report cancellation/write failures and never publish a partial file as a successful render or overwrite an existing file without explicit choice.
8. **Live/offline consistency:** deterministic fixtures use identical event positions and compensated alignment with the tolerance above before integer WAV conversion. For actual instruments, compare presets, note timing, pitch, duration, and audible tails; do not assert numerical equivalence unless the instrument is demonstrated deterministic. Record any plugin that rejects offline mode as an unresolved failure, not an automatic real-time-render fallback.

### Remaining Setup and VST3 Decision

- Approved VST3 amendment: user-suggested [rust-vst3-host](https://github.com/HelgeSverre/rust-vst3-host) at commit `6c64cdffc6740cce0b4c5b6916724697b0ef9ff8` plus the [verified local state-restore patch](../../patches/vst3-host/README.md), replacing the standalone SDK bridge for T02. Fresh-worker AA Y7 realtime restoration now passes the no-editor smoke check. Preparation/runtime deadline separation remains an implementation requirement. Preserve the approved callback, isolation, recovery, and export requirements when assessing reuse. See the [candidate evaluation](../../reports/T02%20rust-vst3-host%20evaluation.md).
- Pianoverse test content is Grand Piano Y7 / AA Y7, selected explicitly by the user. Confirm a ZebraHZ preset, the EVO output pair, and MIDIFACE input port during setup. These labels need not be invented to approve the test design, but must be recorded before running hardware acceptance.
- If the approved component pins cannot support the proposed isolated native-editor path on this host, document the concrete blocker and revisit the provisional foundation rather than silently replacing it.

## Boundaries

- No complete arrangement editor, mixer, internal plugin suite, arpeggiator or M/S implementation is implied.
- Use fitting existing components before implementing their low-level responsibilities.
- Process isolation is user-approved. The particular IPC design, provisional dependencies, and final engine foundation are not yet adopted.
- Provisional component agreement is required before implementation; final T01 adoption can follow the prototype results, avoiding a circular dependency.
- Follow the repository UI guidance if the agreed prototype introduces UI.
- Generate and consult Graphify for the introduced application source, and refresh it after development changes.

## Evidence and Completion

### Planning Progress — 2026-11-16

The user confirmed Pianoverse and ZebraHZ, both scheduled and physical MIDI, Audient EVO, MIDIFACE with Studiologic SL88, an initial 128-frame buffer at 48 kHz, stereo WAV 24-bit export with tails, and separate-process plugin isolation. The user approved the architecture and acceptance proposal while requesting consideration of rust-vst3-host. Initial source/build/runtime evidence is recorded in the linked candidate evaluation; The fresh-worker Pianoverse silence was traced to stale initialization/editor parameter values being reapplied after state restoration. A local shared-layer patch and a regression test fix that cause: AA Y7 and ZebraHZ now produce sound after fresh-worker restoration without opening an editor. The fixture regression failed before and passed after the patch; 238 existing unit tests and Clippy passed. This resolves the investigated correctness gate, not full hosting acceptance. Slow preparation and active-playback deadlines must still be separated. Local read-only plugin bundle metadata and host platform checks were performed through `rtk proxy python3`; they do not establish runtime compatibility. The user approved the patched VST3 backend. A pinned, patched source copy and the Rust workspace are being introduced; application and worker implementation is in progress. Codebase graph generation now applies to the introduced source.

General and patched-backend approval are recorded above. Verify implementation against the approved acceptance criteria. Record revisions, commands, configuration, observed results, failures and limits. Keep automated tests, GUI checks, listening and physical-hardware measurements separate.

T02 is complete only when the agreed criteria have evidence and its findings are linked from T01. A pass applies to the tested configuration; it does not establish compatibility with untested plugins, formats, workloads or hardware.

## Implementation Checkpoint — 2026-11-16

The AppKit window draft has been replaced by eframe 0.34.3 / egui. Clef no longer directly depends on objc2/AppKit; `src/native.rs` borrows eframe's raw parent view only for the plugin ABI. The worker runs plugin main-thread services through `eframe::App::logic`, including while its window is hidden. Opening/closing recreates/destroys plugin editor resources; the eframe parent window remains alive. No custom DAW controls were added. ZebraHZ VST3's original editor was visually observed in this window through computer-use tools using an identical binary copied into a temporary `.app` wrapper. This is a single visual smoke check, not full editor acceptance.

Control/state/editor IPC and processing IPC now use separate Unix connections in a private directory. Accepted sockets are explicitly returned to blocking mode on macOS. Whole-exchange deadlines are recomputed across short reads/writes; audio packets have a smaller allocation cap. Lifecycle requests first stop CLAP processing on its processing thread before main-thread deactivation. The supervisor owns and kills/reaps only its child. Actual crash/hang/restart acceptance is still pending.

The CLI now provides `devices`, `plugin PLUGIN [STATE]`, and `render PLUGIN WAV [STATE]`. The plugin console supports open/close, save/restore, recreate and render. Files are published without implicit overwrite. Offline rendering uses the existing sequence and graph, trims declared plugin/graph latency, appends ten seconds and verifies the final frame count. Live start/stop, bounded CPAL handoff and physical MIDI processing are **not implemented**.

### Confirmed Hardware Selection

Enumeration through CPAL/midir outside the restricted sandbox found **Audient EVO16**, with 24 F32 output channels and 48 kHz support; advertised buffer range 15–4096 frames. The user selected **outputs 1–2**. Input **MIDIFACE 2x2 Midi In 2** and ZebraHZ **initialize** were confirmed. These are configuration choices and enumeration evidence only: no audio stream or MIDI input connection was opened, no actual callback sizes or physical keyboard activity were measured.

### Automated Checks

- `rtk cargo check -p clef --locked`, `rtk cargo build -p clef --locked`, `rtk cargo test -p clef --locked`, `rtk cargo fmt --all --check`, and `rtk cargo clippy -p clef --all-targets --locked -- -D warnings`: pass. Root tests: **10 pass**, one native integration test ignored by default.
- Explicit native test: `rtk cargo test -p clef --locked --test t02_native -- --ignored --nocapture`: pass outside the sandbox. It launches the actual isolated eframe fixture, checks aligned event sample 37 after 257-sample graph compensation, stereo 48 kHz/PCM24, exact 2,323,200 frames, and refusal to overwrite an existing artifact. This does not establish live callback or dynamic plugin-latency transitions.
- Built `vst3-host-testplug`, bundled it with `CARGO_TARGET_DIR=/Users/arcy/dev/clef/target PROFILE=debug`, then explicitly ran `internal::plugin_impl::state_editor_queue_tests::restored_state_supersedes_queued_editor_values -- --ignored --exact --nocapture`: **one pass**, 238 filtered out. No new claim that all 238 were rerun. Vendored Rust files still match the previously patched review checkout byte for byte.
- Graphify structural extraction refreshed before analysis and after source changes; main graph remains in `graphify-out/`.

### Offline and Concurrent-Editor Diagnostics

[Offline artifact measurements](../../reports/T02-offline-probe.json) record complete renders for Pianoverse VST3 AA Y7, ZebraHZ VST3 initialize, and ZebraHZ CLAP initialize: each is stereo 48 kHz/PCM24, **2,323,200 frames**, readable and nonzero. Exports are private temporary files, not repository assets. These are format/sample checks, not listening, preset/parameter restoration acceptance, tail acceptance, or realtime/offline equivalence.

Reproducible worker-only editor diagnostic: `rtk proxy python3 scripts/t02_worker_probe.py PLUGIN REPORT [--state STATE]`. It performs concurrent 128-frame processing with a held note and ten editor create/destroy cycles. Reports use create-new semantics. It opens no audio device. The test is intentionally narrower than the agreed full-sequence/live acceptance.

- [Pianoverse](../../reports/T02-pianoverse-vst3-editor-probe.json): ten cycles, 1,117 processing blocks, nonzero peak 0.11813932. Maximum processing RPC was **977.38 ms**, with 32 exchanges exceeding the 2.667 ms duration of 128 frames. This is an RPC diagnostic, not a CPAL underrun count. The VST3 shared controller/processor mutex still blocks processing during editor work; two connections alone do not solve it.
- [ZebraHZ VST3](../../reports/T02-zebrahz-vst3-editor-probe.json): **failed** after two completed cycles; worker exited with code 1 and stderr `Plugin editor is not open`. Source inspection found `MainPlugin::service` unconditionally applying pending resize requests even after editor close, while the vendored resize method rejects an absent view. Treat stale resize servicing as the leading host-side cause; do not describe this as a confirmed plugin crash. No fix or successful rerun is recorded at this checkpoint.
- [ZebraHZ CLAP](../../reports/T02-zebrahz-clap-editor-probe.json): ten cycles and normal worker exit, 1,067 blocks, maximum processing RPC 1.124 ms, but the held-note probe's nonzero assertion failed. The full offline sequence for this format produces nonzero output, so this does not establish that CLAP hosting is universally silent. Realtime event/transport/preset behavior needs investigation using the complete scheduled sequence. Plugin logs also warn that latency is queried while inactive, and the offline run emits a main-thread warning; inspect the host lifecycle and distinguish plugin expectations from actual CLAP ABI requirements before changing it.

### Remaining Work

Prioritize the stale VST3 editor resize failure, the measured VST3 processing/editor serialization, and CLAP lifecycle/realtime diagnostics. Then finish bounded live audio/MIDI handoff and commands, actual per-format latency/restart fixtures, crash/hang recovery tests, state-edit/fresh-worker restoration checks, and the approved hardware/listening durations. No acceptance criterion has been weakened. T02 remains in progress and final T01 foundation adoption remains pending.

The exact continuation state, commands and priorities are in [T02 handoff](T02-handoff.md).

## Continuation — 2026-11-17: VST3 Editor-Close Resize

The shared VST3 backend retained a pending editor resize after detaching its view.
`MainPlugin::service` could consequently pass it to `resize_editor`, which correctly
rejects an absent view. The backend now clears its resize slot after detach callbacks,
including on repeated closes. No worker-side error suppression, editor caching, or
plugin-specific workaround was added. Patch and provenance are recorded in
[patch documentation](../../patches/vst3-host/README.md).

The TestSynth regression failed before the fix with a retained `(640, 480)` request
and passed after; it also verifies delivery of a subsequent request. Root check,
build, test (10 passed, one ignored), fmt, and Clippy passed. Backend default unit
suite: 238 passed, two ignored. The fixture was explicitly rebuilt/bundled before
final native regression verification. Graphify was refreshed before analysis and
after the change; code-only extraction does not cover documentation semantics.

The first runtime attempt could not bind local sockets inside the sandbox. An
outside-sandbox attempt found the previous temporary state snapshot missing;
[its failure report](../../reports/T02-zebrahz-vst3-editor-probe-2026-11-17.json)
records zero editor cycles and supplies no compatibility evidence.

A new run using the plugin's initial state completed all ten actual create/destroy
cycles, 963 concurrent processing blocks, and normal worker exit, without the old
`Plugin editor is not open` error. However, the
[overall probe failed](../../reports/T02-zebrahz-vst3-editor-default-2026-11-17.json)
because its single held note produced zero samples. Maximum processing RPC remained
518.25 ms. This is partial editor lifecycle evidence, not a successful audio,
preset, live playback, GUI usability, or hardware acceptance result. The previous
snapshot's preset cannot be claimed for this run. Existing earlier reports remain
historical observations rather than current blockers already fixed by this patch.

Next work remains the shared VST3 controller/processor serialization and the CLAP
lifecycle/full-sequence diagnostics, followed by bounded live audio/MIDI and the
remaining approved acceptance matrix. T02 remains in progress.

Final rebuilt/bundled TestSynth verification on 2026-11-17: both explicitly selected
`state_editor_queue_tests` native regressions passed (238 filtered out).

## Approved SDK Bridge Amendment — 2026-11-17

The user approved replacing the VST3 backend with the narrow SDK-backed C ABI bridge
proposed in [backend reassessment](../../reports/T02%20VST3%20backend%20reassessment.md),
after full-sequence concurrent editor diagnostics still measured 514.69 ms stalls.
The SDK pins, separate main/audio ownership, eframe embedding, exception and size
boundaries, restart coordination and validation gates in that proposal are agreed.
Rust scheduling, CPAL/midir, Clack, graph/PDC, process isolation and every T02
acceptance criterion remain in scope. This supersedes the provisional selection of
rust-vst3-host for production VST3 hosting; retain its deterministic fixture and
historical evidence while validating the replacement.

The live driver is being added with a four-block (512-sample) transport lead,
preallocated output packets and a lock-free SPSC handoff. The device callback only
copies frames and updates atomic counters; stale packets are discarded against the
output sample cursor. MIDI timestamps use a monotonic host clock, late events clamp
to the next unrendered block and are counted; overflow triggers sustain/note cleanup.
Each live session owns new queues, preventing packets from earlier sessions from
reaching a replacement worker. IPC, graph processing, allocation, lifecycle and
logging run outside the device callback. These are implementation invariants, not
yet evidence that live/hardware acceptance passes.

The first SDK bridge diagnostic completed 14,400 ZebraHZ VST3 blocks and ten editor
cycles with nonzero audio and a maximum RPC duration of 62.92 ms, down from 514.69 ms
in the earlier backend diagnostic. These are RPC measurements. Short live smoke runs
on EVO16 1–2 at 48 kHz/128 frames played 269,568 VST3 frames and 226,944 CLAP frames
without missed blocks or output errors. The final listening evidence is recorded below.
[SDK worker sequence](../../reports/T02-zebrahz-sdk-sequence-2026-11-17.json) ·
[VST3 smoke log](../../reports/T02-zebrahz-sdk-live-editors-2026-11-17.log) ·
[CLAP smoke log](../../reports/T02-clap-live-smoke-2026-11-17.log).

The SDK fixture caught an activation-order issue. The bridge queries VST3 latency after
`setupProcessing` and before `setActive(true)`. For Pianoverse's combined component
and controller, the bridge restores the object once, since a separate
`setComponentState` returned code 3. The build uses the pinned, unmodified SDK hosting
sources and Command Line Tools.

## Physical Pianoverse Acceptance Evidence — 2026-11-17

The correct input is MIDIFACE 2x2 Midi In 1 (current index2). After assigning
user-interactive QoS to the broker/worker audio threads, the full measured physical
run covered14,487,552 frames (~301.82s), received3,040 MIDI events, and recorded
zero missed/stale blocks, output errors, MIDI overflows or late events. Actual
callbacks were128 frames at48kHz through EVO16 outputs1–2, with512 samples of
transport lead. Worker peak0.37907824 confirms nonzero processed samples, not a
physical latency measurement. [Report](../../reports/T02-pianoverse-in1-qos-midi-5min.json).
The user explicitly confirmed every requested gesture (chords, repeated notes,
sustain, pitch bend and modulation), with no audible dropout or stuck notes.
This satisfies the Pianoverse physical five-minute portion for that configuration;
the ten-minute scheduled run, editor cycle matrix, export/listening and remaining
instruments still require evidence.

ZebraHZ VST3 GUI state check: original editor showed initialize as loaded. Master
was changed from67.50 to20.00 using the plugin's mouse-wheel handling while active,
saved to `validation-output/states/zebrahz-vst3-initialize-master20.state`, then a
fresh worker restored and visually showed Master20.00. Host GUI drag gestures did
not move this proprietary knob; wheel handling did. No preset file was overwritten.
The completed physical run covered14,490,368 measured frames (~301.88s) and1834 MIDI
events, with zero missed/stale blocks, output errors, overflows or late events.
The user confirmed all requested controls and no audible dropout or stuck notes.
[Report](../../reports/T02-zebrahz-vst3-in1-qos-midi-5min.json).

ZebraHZ CLAP: plugin-originated resize requests were incorrectly echoed into
`set_size`, which ZebraHZ rejected. Such requests already reflect a resized plugin
GUI; the host now accepts its client area without that echo, and avoids redundant
host-originated size writes. A full scheduled worker diagnostic completed ten
editor create/destroy cycles with nonzero audio (max RPC4.28ms, no device opened).
[Report](../../reports/T02-clap-resize-fixed-sequence.json). The real editor then
restored initialize and Master20.00 after a fresh worker; computer-use visually
confirmed20.00. Its five-minute physical test on In1 passed:14,488,320measured frames and1224MIDI
events, zero misses/errors/overflows/late events. The user confirmed every requested
gesture and no audible problems. [Report](../../reports/T02-zebrahz-clap-in1-qos-midi-5min.json).
Latency reconfiguration now has actual per-format fixture and live-device evidence
in [Completion](#completion--2026-11-17).

## Final Acceptance Continuation — 2026-11-17

All three physical five-minute tests have separate software reports and explicit
user confirmation. Actual VST3 and CLAP latency/restart fixtures, bounded failure
recovery, saved-state restoration with unsaved-edit loss, export cancellation and
write-finalization failure checks have passed. Default TestSynth was explicitly
rebuilt/bundled before the two native integration tests passed. Root checks now
include22unit tests; final native tests remain explicitly selected, not default.

Pianoverse editor destruction reproduced a CALayer delegate callback after its
owner lifetime. A dedicated disposable plugin ABI surface now clears discarded
layer delegates/animations before destruction; ten actual editor cycles during
live playback then passed. Flush-only, hide-only, autorelease-pool-only and
surface-only attempts did not resolve it and are not retained as fixes.

Original ZebraHZ scale menus were exercised100%->80%->100% in VST3 and CLAP,
with correct host geometry after fixing the VST3 plugin-request path. canResize
restricts user dragging; it does not prohibit plugin-originated scale requests,
which require onSize acknowledgement. See the separate
[GUI observations](../../reports/T02-editor-resize-observations.md).

Earlier scheduled attempts are retained as failed evidence. One Zebra run included
opening another DAW; the user reported clicks and then confirmed a quiet environment
for repetition. Subsequent bundled-worker runs still showed late processing after
editor hide; the CLI physical runs had passed. App Nap is a leading explanation,
not a result inferred solely from timing. NSProcessInfo latencyCritical/user-initiated
activity is now scoped to realtime activation and released at stop/deactivation;
the OS assertion was observed. Three600s bundled-worker runs are executing with
that activity. Device128-frame callbacks and512-sample transport buffering remain
unchanged. Thread Mach constraints and socket-readiness waiting run outside the
callback. The callback retains its copies/atomics-only handoff.

Final exports, tail/live comparison, scheduled listening confirmations and complete
acceptance consolidation remain open. Do not mark T02 Done before those criteria
have evidence. T01 receives the results; final foundation adoption is a separate
user decision.

## Completion — 2026-11-17

All eight agreed acceptance criteria passed. The user
explicitly confirmed all three physical tests, all three completed scheduled runs,
and final commercial WAV consistency/tail endings with no problems. Actual format
latency/restart fixtures, native state/editor/resize checks, deterministic routing/
PDC, crash/hang/saved-state recovery, export failure/cancellation and final automated
checks passed. Earlier incomplete/failed checkpoints above remain historical and
are not acceptance evidence for later versions.

### Final verification evidence

- **Hardware and listening:** EVO16 outputs 1–2 at 48 kHz, actual 128-frame callbacks
  and 512 samples of transport buffering, with SL88 on MIDIFACE Midi In 1. Each format
  case ran separately. Physical runs lasted about 302 seconds (3,040 / 1,834 / 1,224 MIDI
  events); scheduled runs lasted about 602 seconds with ten editor cycles each. All six
  runs reported zero missed/stale blocks, output errors, MIDI overflows and late events.
  [Pianoverse physical](../../reports/T02-pianoverse-in1-qos-midi-5min.json) /
  [scheduled](../../reports/T02-pianoverse-scheduled-10min-activity.json),
  [ZebraHZ VST3 physical](../../reports/T02-zebrahz-vst3-in1-qos-midi-5min.json) /
  [scheduled](../../reports/T02-zebrahz-vst3-scheduled-10min-activity.json),
  [ZebraHZ CLAP physical](../../reports/T02-zebrahz-clap-in1-qos-midi-5min.json) /
  [scheduled](../../reports/T02-zebrahz-clap-scheduled-10min-activity.json).
- **Scheduling, routing, and editors:** the fixed 16-bar, 100 BPM sequence starts at bar 3.
  Event ordering and stop/overflow cleanup were exercised. PDC aligned the 0/257-sample
  branches across 128/73-frame partitions, and the isolated offline fixture aligned at sample
  37 after trim. Fresh-worker state restoration and scale-menu resize passed.
  [GUI and state observations](../../reports/T02-editor-resize-observations.md).
- **Format changes:** VST3 and CLAP fixtures passed actual 0→257-sample notifications and
  exact realtime-core/offline comparison. Both live paths reported 769 cumulative samples,
  one restart and three discarded old packets. Transition silence was 1,280 VST3 frames
  and 512 CLAP frames, with zero misses/errors after transition and warm-up.
  [VST3 fixture](../../reports/T02-vst3-format-latency.json) /
  [live](../../reports/T02-vst3-reconfiguration-activity.json),
  [CLAP fixture](../../reports/T02-clap-format-latency.json) /
  [live](../../reports/T02-clap-reconfiguration-activity.json).
- **Recovery and export:** controlled exit and hang were reported in 0.0273 s and 1.0142 s.
  Explicit restart restored the saved state and discarded unsaved edits.
  Three final 48 kHz stereo PCM24 exports had 2,323,200 frames each, nonzero unclipped
  output and the agreed ten-second tail. The user accepted the live/export comparison.
  Parent-only and terminal-group cancellation left no final or partial file.
  [Recovery](../../reports/T02-live-recovery-final.json) ·
  [Export](../../reports/T02-offline-final.json) ·
  [Cancellation](../../reports/T02-export-cancellation-terminal-fixed.json).
- **Automated:** locked check/build, formatting and warning-free Clippy passed. All 22
  default tests and both explicitly selected native tests passed after rebuilding TestSynth.

The three final scheduled runs used binary hash `0e88cc5e…ec728ac`. The subsequent
`eaf857da…88d5e6` build added terminal cancellation classification only and supplied the
final native, export, recovery and format-reconfiguration evidence. Interrupted and failed
runs remain in the raw reports.

T02 is complete for the tested configuration. Results are linked from T01; its
final foundation-adoption decision remains separate. No broader compatibility,
physical-latency or multitrack performance claim is made.
