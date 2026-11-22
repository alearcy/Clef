# vst3-host State-Restore Patch

Base: `HelgeSverre/rust-vst3-host` commit `6c64cdffc6740cce0b4c5b6916724697b0ef9ff8` (crate 0.11.1, upstream MIT license).

Patch: [0001-clear-stale-editor-values-on-state-restore.patch](0001-clear-stale-editor-values-on-state-restore.patch)
SHA-256: `d04efb84497445d0ac4c3b3b631cbf24f2e784d3b2b7e8f28f498199344418d0`.

## Purpose and Scope

Discard editor-to-processor parameter values queued before a state restore. Otherwise the next audio block can reapply values from initialization or an earlier edit and overwrite the restored processor state. The fix is in the shared in-process implementation, which the isolated helper also uses. It runs after processing is suspended and before the new component state is applied, preserving new callbacks emitted during or after restoration. There is no plugin-name check, automatic editor opening, parameter replay, or extra dependency.

The regression uses the bundled TestSynth, queues an old channel-level edit, restores an audible snapshot, and checks actual rendered output. It failed with `peak=0` before the fix and passes after it. It also verifies that a new editor edit after restoration still affects the processor. The explicit test fails if its fixture bundle is missing; it does not silently skip.

This is a local patch artifact, not an upstream commit or a Clef dependency adoption. No issue, pull request, or external message was sent. If the library is selected, pin both the source revision and this patch until an equivalent upstream fix is verified. Reassess the patch when updating the dependency; do not apply it blindly to newer releases. Removing the patch returns to the known stale-edit bug at this base revision.

## Apply and Verify

Run from a clean checkout at the base revision:

```sh
rtk git checkout 6c64cdffc6740cce0b4c5b6916724697b0ef9ff8
rtk git apply --check /Users/arcy/dev/clef/patches/vst3-host/0001-clear-stale-editor-values-on-state-restore.patch
rtk git apply /Users/arcy/dev/clef/patches/vst3-host/0001-clear-stale-editor-values-on-state-restore.patch
rtk cargo build -p vst3-host -p vst3-host-testplug --no-default-features --features vst3-host/process-isolation --locked
rtk proxy env PROFILE=debug bash scripts/bundle-test-plugin.sh
rtk cargo test -p vst3-host --lib --no-default-features --features process-isolation --locked
rtk cargo test -p vst3-host --lib --no-default-features --features process-isolation --locked internal::plugin_impl::state_editor_queue_tests::restored_state_supersedes_queued_editor_values -- --ignored --exact --nocapture
rtk cargo clippy -p vst3-host --lib --tests --no-default-features --features process-isolation --locked -- -D warnings
```

The local checks used cached dependencies with `--offline` after the initial dependency download. The commands above permit dependency resolution for a fresh checkout. No audio device is opened by these selected tests.

Observed results: 238 existing unit tests passed; the new fixture regression passed when explicitly selected (ignored in the default suite); Clippy passed. Application to the pinned original and reverse applicability to the tested source were checked. Applying the artifact to a fresh copy reproduced the tested source byte for byte.

Selected-preset runtime evidence and limits are in the [T02 evaluation](../../reports/T02%20rust-vst3-host%20evaluation.md). GUI, listening, hardware, and full T02 acceptance are separate gates.

## Editor-Close Patch — 2026-11-17

Apply [0002-discard-editor-resize-on-close.patch](0002-discard-editor-resize-on-close.patch)
after patch 0001. SHA-256: `0ed3a1412e2beb65dce0537750417fd34274abeea3c9b1c5baf9517b2a2cdf98`.

`PluginImpl::close_editor` now clears the pending resize slot after `removed()` and
`setFrame(nullptr)`, including requests emitted by detach callbacks and repeated closes.
This prevents a queued request from reaching a detached or subsequent editor. Direct
`resize_editor` calls on an absent view still return an error; real resize errors remain visible.
The patch changes no processing synchronization or editor creation policy.

The explicitly selected TestSynth regression
`internal::plugin_impl::state_editor_queue_tests::closing_editor_discards_pending_resize_requests`
failed before the fix (`Some((640, 480))` instead of `None`) and passed afterward.
It also checks that a later request is still delivered. Build and bundle TestSynth as
above before selecting this ignored regression. Default backend unit checks: 238 passed,
two native regressions ignored. Clef checks/build/tests/fmt/Clippy passed (10 tests,
one native integration ignored).

ZebraHZ VST3 completed ten actual editor create/destroy cycles and exited normally;
see [runtime report](../../reports/T02-zebrahz-vst3-editor-default-2026-11-17.json).
The overall probe **failed** its nonzero-audio assertion, with a maximum processing RPC
of 518.25 ms. The old temporary preset snapshot was unavailable, so this used the
plugin's initial state, with no preset/listening acceptance. This provides editor
lifecycle evidence only. VST3 controller/processor serialization remains unresolved.

Final explicit native verification after rebuilding/bundling TestSynth selected both
`state_editor_queue_tests` regressions: two passed, 238 filtered out. Reverse patch applicability against the vendored source was checked.

## Clef Latency Fixture Variant — 2026-11-17

[Patch0003](0003-clef-latency-fixture.patch) changes only the upstream test-plugin
crate. SHA256: `dc7a51fb953142c9ab5c7ab5b7e6039b4103f6440f78e12b80333e911f282fa1`.
The opt-in `clef-latency-fixture` feature exposes one strict-lifecycle impulse
instrument whose processing delay changes from0to257samples via a real controller
restart notification. Default TestSynth classes/processing remain available without
the feature. This fixture does not select a production hosting backend.

Build the two fixture variants separately from the Clef workspace:

```sh
rtk cargo build -p vst3-host-testplug --locked --offline
rtk proxy env CARGO_TARGET_DIR=/Users/arcy/dev/clef/target PROFILE=debug bash vendor/rust-vst3-host/scripts/bundle-test-plugin.sh
rtk cargo build -p vst3-host-testplug --features clef-latency-fixture --target-dir target/latency-vst3 --locked --offline
rtk cargo build -p clef-clap-latency-fixture --locked --offline
rtk proxy python3 scripts/t02_bundle_latency_fixtures.py
```

The last script bundles local fixtures under ignored `validation-output/fixtures/`
on macOS and checks copied binaries byte for byte. It changes no installed plugin.
The new CLAP fixture is Clef-owned Rust using the host's existing pinned Clack
revision; it implements real latency/restart notifications and the same impulse
behavior. Run `scripts/t02_latency_probe.py` once per bundled format after building
Clef. Reports and limits are linked from the [T02 verification](../../docs/specs/T02-engine-validation.md#completion--2026-11-17).

## Clef Multi-Bus Fixture Variants — 2026-11-22

[Patch0004](0004-clef-multi-bus-fixture.patch) changes only the upstream test-plugin crate,
on top of Patch0003. SHA256: `fdff9a7de1ca53af0105691cee247d3edb0a461e0851b5c769b485e226452a69`.
For [T43](../../docs/specs/T43-multi-bus-instruments.md), the opt-in
`clef-multi-bus-fixture` feature turns the Clef latency fixture into an instrument with four
stereo output buses (bus 0 main, the others auxiliary and written at 0.9) and one stereo
auxiliary sidechain input that must arrive silent; `clef-mono-main-fixture` adds a mono main
output that refuses stereo. Default TestSynth classes and the existing fixture variants are
unchanged. Build and bundle them with the commands in `scripts/t43_bundle_multi_bus_fixtures.py`;
the script writes ignored bundles under `validation-output/fixtures/` and installs nothing.
Reverse patch applicability against the vendored source was checked.
