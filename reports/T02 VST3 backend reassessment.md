# T02 — VST3 Backend Reassessment

Date: 2026-11-17. Status: user approved the SDK bridge amendment on 2026-11-17; implementation in progress.

## Finding

The original editor lifecycle error is fixed, but the current backend still fails
the concurrent processing gate. The complete shared 16-bar sequence produces audio
with ZebraHZ VST3 (peak 0.41475648), completes ten editor create/destroy cycles and
14,400 processing blocks, but the maximum processing RPC is 514.69 ms, with 148
exchanges above the 2.667 ms block duration. See
[T02-zebrahz-vst3-sequence-2026-11-17.json](T02-zebrahz-vst3-sequence-2026-11-17.json).
These are worker RPC measurements, not hardware underrun or listening evidence.
The earlier single-note silence does not reproduce with the complete sequence.

`src/worker.rs` shares one `Arc<Mutex<Plugin>>` between the editor and processing
threads. `Plugin` owns a single `Box<dyn PluginInternal>`; the in-process
`PluginImpl` combines controller, view, processor, lifecycle, processing buffers,
MIDI mapping, state and host callbacks behind mutable access. `service_host_requests`
also performs processing suspension and reactivation from the control thread.
Two sockets, `try_lock`, caching the editor or increasing queues do not supply
independent processor ownership or satisfy the agreed ten live editor cycles.

## Options and Recommendation

1. Refactor the vendored backend to split main-thread controller/editor ownership
   from audio-thread processing state. This preserves existing host services, but
   reaches the internal trait, public facade, process/bus/state/restart paths and
   playback/helper consumers. It would turn Clef's small correctness patch set
   into ownership of a substantial upstream architectural fork.
2. Return to the originally proposed narrow SDK-backed bridge. Use C++ internally
   and an `extern "C"` ABI for Rust; VST3 is a C++ interface SDK, not a standalone
   C hosting library. Reuse the pinned SDK module loader, host classes, event and
   parameter containers, connection utilities and provider where they fit.
   Separate the controller/editor handle from the processing handle explicitly.
   This adds a native build and ABI lifetime obligations, but gives Clef direct
   ownership of the boundary currently blocking the acceptance gate.

**Recommend option 2 for the next T02 validation attempt**, conditional on the
same instrument/editor/state/restart tests. This is an architectural recommendation,
not runtime proof that SDK code removes all plugin-internal contention.

## Concrete Proposed Amendment

- Retain Rust scheduling, graph/PDC, CPAL, MIDI, offline WAV publication and process
  isolation. Retain Clack for CLAP. Replace only the VST3 backend inside workers.
- Pin VST3 SDK 3.8.1 at `3cdf9ca5d1f5b1b21e0a86832aa4abe55607bd96`, public SDK
  `586dc5e6c8012c3e4b01c79389375cbe96bdb1da`, interfaces
  `4f547e8e102b47de4a8b8aaf343c73b700786372`; record all required submodule pins
  and licenses before adding source. Do not use the older SDK bundled in the
  unrelated local JUCE checkout as a substitute.
- Keep the eframe parent window and event loop. Native code only attaches/removes
  the original plugin view to the borrowed NSView; no native app window or event loop.
- Main-thread handle owns module/controller/view/state and control callbacks.
  Audio handle owns process buffers/context/events and invokes `setProcessing` and
  `process` on the processing thread. Activate/deactivate/reconfigure only after
  explicit processing suspension; destroying either handle while in use is forbidden.
- Bounded parameter queues connect editor edits to the processor; report overflow.
  Host callbacks only enqueue requests. Latency/I/O restart is coordinated by the
  supervisor, with a recorded gap and panic cleanup, never automatic mutation of
  running processing state from the editor loop.
- Use caller-owned buffers, bounded events/state, validated sizes and explicit error
  returns. Catch C++ exceptions at ABI entry points. Native module references must
  outlive every processor/controller/view reference.
- First prove deterministic fixture processing and concurrent editor cycles for
  ZebraHZ and Pianoverse; preserve state restore and original-editor tests. Retire
  the production dependency on rust-vst3-host only after equivalent evidence.
- No change to T02 acceptance: full live/hardware durations, per-format dynamic
  latency fixtures, recovery, tails and listening remain mandatory.

## Primary References Checked

- [Pinned SDK license](https://github.com/steinbergmedia/vst3sdk/blob/3cdf9ca5d1f5b1b21e0a86832aa4abe55607bd96/LICENSE.txt): MIT.
- [Pinned PlugProvider](https://github.com/steinbergmedia/vst3_public_sdk/blob/586dc5e6c8012c3e4b01c79389375cbe96bdb1da/source/vst/hosting/plugprovider.cpp): component/controller creation, initialization and connection ownership; does not provide Clef's scheduling or isolation.
- [Pinned processing interface](https://github.com/steinbergmedia/vst3_pluginterfaces/blob/4f547e8e102b47de4a8b8aaf343c73b700786372/vst/ivstaudioprocessor.h): processing/lifecycle contract remains the bridge's responsibility.
