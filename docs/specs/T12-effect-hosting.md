# T12 — Insert Effects Hosting

- **Status** — In Progress. Specification and written UI proposal approved on 2026-11-19;
  implementation and automated verification complete. Listening validation is pending.
- **Priority** — P1.
- **Task** — [T12](../../TASKS.md)
- **Roadmap** — [Post-T03 backlog](../planning/post-T03-backlog.md)
- **Dependencies** — T04 multitrack worker isolation, T11 mixer, and T02/T03 hosting contracts.

## Behavior

- Discover VST3 and CLAP audio effects by their declared capabilities. The insert picker
  shows effect entries with a compatible stereo processing path.
- Each instrument track and Master owns one ordered serial insert chain. The chain has
  no fixed slot count. Add, remove, reorder, and bypass are undoable project edits.
- Reuse isolated plugin workers and original editor hosting. Persist plugin identity,
  order, bypass, and opaque state. Preserve unavailable entries and their saved data so
  the user can restore them later. Existing projects open with empty insert chains.
- Keep existing project and plugin-state size checks and bound worker handoffs, buffer
  memory, and prepared compensation. If a chain change cannot be prepared within
  supported resources, explain the failure and retain the previous chain. Prepare and
  reclaim resources off the device callback.

## Signal Path and Failure Behavior

- Instrument track → Gain → inserts → Width → Pan → fader → mute/solo gate → Master sum.
- Master sum → inserts → Pan → fader → meter → device output safeguard.
- Include each chain’s effective processing delay in path alignment. Bypassed, missing,
  or failed slots pass the aligned dry signal onward. A late or crashed worker cannot
  stall the device callback or healthy tracks. Discard results that arrive after their
  deadline or belong to a retired chain.
- Prepare topology and latency updates outside the device callback and publish them at a
  block boundary. When a plugin changes its reported latency during playback, keep
  transport running and measure the transition. Count transition frames and retain the
  existing T04 protections against stale output.

## Written UI Proposal

```text
Mixer
  Instrument bank (horizontal scroll)    Master (docked)
    Instrument strip                       Master strip
      Gain
      Inserts                               Inserts
        Effect slot + Power                  Effect slot + Power
        Insertion line (on hover)            Insertion line (on hover)
        Effect slot + Power                  Effect slot + Power
        …                                    …
        Add audio effect…                    Add audio effect…
      Pan / Width                           Pan
      Fader / meter                         Fader / meter
```

- T35 adds a fixed Plugin row before the effect chain on instrument tracks. It is the track's
  source, not an effect insert, and is specified separately; the T12 effect chain starts after
  it.
- Use the mixer’s [slot button](../UI/components/slot-button.md) and
  [power button](../UI/components/power-button.md) primitives. **Add audio effect…** at the
  end appends an insert and is also available on an empty chain. Scroll long chains
  vertically in the insert area while keeping the lower mixer controls reachable and
  Master docked.
- On an instrument strip or Master, hovering over the gap between two populated insert
  slots reveals a thick horizontal accent line marking that insertion point. Clicking the
  gap opens the effect picker for that position. Selecting a plugin inserts a new slot
  between the two existing slots, preserving their relative order and settings. Dismissing
  the picker leaves the chain unchanged. The preceding slot's **Insert Effect After…**
  menu action provides a keyboard-accessible path to the same picker and position.
- Activate the slot name to open the [original plugin editor](../UI/views/plugin-window.md).
  Use the existing editor window path, including attach-before-reveal and focus behavior.
  Move a slot by dragging it or by choosing **Move Up** or **Move Down** in its menu. The
  menu also provides **Remove**. Route interactions through the shared command registry.
- Show empty, active, bypassed, missing, and failed states. A failed slot exposes the cause
  and preserves its last saved settings. Provide accessible names that identify the
  channel and plugin, plus keyboard paths for the picker, editor, bypass, move, and
  remove actions. Show long names in full through accessible text and tooltips.
- Follow the [mixer reference](../UI/views/mixer.md) and
  [UI change protocol](../UI/contract.md#change-protocol). Review empty, populated,
  bypassed, missing, failed, hovered insertion gap, and long-chain renders at 1024 and 760 points.

## Out of Scope

The T28 node/cable graph, sidechain routing, parameter automation, AU (T21), internal
effects, and ARA (T29).

## Acceptance Criteria

- Verify VST3 and CLAP effect discovery, instrument filtering, serial processing, and track
  and Master insert placement using deterministic fixtures. Exercise a chain with more
  than eight effects, including scrolling, reordering, and save/reopen.
- On both an instrument strip and Master, verify that hovering between two active slots
  reveals the insertion line and clicking opens the effect picker for that gap. With
  `A → B` loaded, choosing `C` yields `A → C → B` without replacing or changing either
  existing effect. Verify picker cancellation, the keyboard path, and undo/redo for
  insertion at that position.
- Verify alignment across tracks with different chain latencies and through a delayed
  Master chain. Exercise bypass, missing plugins, worker misses and crashes, and reported
  latency changes during playback. Record transition frames and check for stale output,
  callback allocation or blocking, and disruption to healthy tracks.
- Verify undo/redo of add, remove, reorder, and bypass. Round-trip identity, order, bypass,
  and changed plugin state. Check that a rejected resource change leaves the prior chain
  usable and that older projects open with empty insert chains.
- Verify keyboard access and plugin editor open/reopen. Compare real egui renders with
  the approved UI proposal and run the existing T02/T03 hosting regressions.
- Listen with at least one VST3 effect and one CLAP effect, including at least one effect
  with nonzero reported latency. Confirm audible processing, bypass, and restored sound
  after reopening. Record listening separately from automated and GUI results. The exact
  plugins remain open until validation.

## Decision and Approval Record

- 2026-11-19 — The user approved the interview baseline and asked to remove the proposed
  eight-insert limit. The specification uses resource-based constraints instead of a
  fixed slot count.
- 2026-11-19 — The user requested direct insertion between populated slots from the mixer
  gap, with a thick hover line and the effect picker opening on click.
- 2026-11-19 — The user approved the updated specification and UI proposal and authorized
  implementation.
- 2026-11-20 — The user reported that FabFilter Pro-Q 4 VST3 showed no analyzer while its
  CLAP build did. The bridge now relays off-thread `IConnectionPoint` messages from the
  worker audio thread to the controller instead of dropping them.

## Implementation and Verification (2026-11-19, updated 2026-11-20)

### Model and persistence

- `Project::Insert` stores a stable slot identity, `Plugin` identity, bypass, and opaque
  state. Track chains and `master_inserts` are persisted in project version 3; state bytes
  follow track state bytes in chain order. State size remains bounded per slot and across
  the project.
- Version 1 and 2 projects decode with empty insert chains, so older projects open
  unchanged. Version 3 keeps the same metadata shape for projects that only use T11
  fields.
- Insert identities share the project identity space with tracks; duplicate or
  out-of-range identities are rejected. A missing plugin path is preserved rather than
  discarded so the slot can be restored or removed later.

### Hosting and discovery

- The VST3 scanner reports effect classes separately from instruments and checks a stereo
  main input/output path in the disposable scanner process. CLAP discovery requires the
  `audio-effect` feature and a compatible stereo main input; instruments remain filtered
  out of the insert picker.
- Effect workers use the existing isolated worker path with an explicit `effect-worker`
  role. VST3 and CLAP processing accepts one stereo input block and produces one stereo
  output block; auxiliary CLAP/VST3 inputs are fed silence and a stereo main path is
  required.
- Quarantine entries record whether the plugin was hosted as an effect, so a blocked
  effect stays visible in the picker as unavailable instead of reappearing as an
  instrument.
- VST3 editor feedback is not lost when a plugin sends it from its audio thread. The
  bridge's `MessageProxy` queues bounded `IConnectionPoint` notifications in both
  directions and delivers them from the worker's main-thread poll; the SDK proxy dropped
  those off-thread messages. This restores analyzer and meter feedback from effects such
  as FabFilter Pro-Q 4 VST3, whose CLAP build already updates internally.

### Engine

- `Engine::inserts` builds a per-target `Chain` on the worker or broker thread. A track
  chain runs after instrument processing and Gain; the Master chain runs on a dedicated
  bounded broker between the stereo sum and the Master fader. No chain allocation,
  locking, or plugin RPC runs on the device callback.
- Each slot keeps its own dry delay. Bypassed, failed, missing, and late slots pass the
  aligned dry signal; a late or crashed worker cannot stall later stages or healthy
  tracks. A latency change restarts the slot, bumps the compensation generation, counts
  transition frames, and discards stale output.
- A bypass toggle crossfades wet and dry over 5 ms instead of stepping between them. A
  bypass-only chain replacement keeps the same stages and dry delays: `Engine::set_insert_chain`
  does not bump the compensation epoch when the effect sequence is unchanged, and
  `Processor::set_chain` retargets the crossfade without rebuilding the delays. Stopping the
  transport fades the final five milliseconds before the transport commit, so an abrupt cut
  does not click; the stop frame and record end are captured at the request, not after the fade.
  The output gate then stays closed while stopped: reopening it after the fade would unmute the
  instrument release tail a few milliseconds later, producing a dip and return. Live audition
  explicitly reopens the gate through `Engine::audition`; Play reopens it immediately.
- Chain latency is included in the track's reported latency and the maximum
  compensation delay. Master latency is added to the published transport latency. The
  prepared chain is rejected if total compensation exceeds the bounded capacity, and the
  previous chain remains published.
- The Master broker has a fixed dry buffer sized from the maximum compensation and the
  device lead, a bounded packet ring, and a deadline per block. A missed Master packet
  falls back to the aligned dry signal.

### Session and UI

- Session reconciliation prepares all affected chains before publishing a project edit;
  a preparation failure returns the cause and leaves the prior chain and project state
  intact. Add, remove, move, bypass, and insertion-position edits share one history edit
  and preserve slot state.
- The insert picker lists only `catalog.effects`, supports search, Rescan, Cancel, and
  opens for the visible gap position. Add, open editor, bypass, move, remove, and
  insert-after are shared commands. Dragging reorders slots.
- The mixer shows an `Inserts` section on every instrument strip and Master with 20-point
  slot rows, the 5-point insertion gap, the vector power glyph, and the `Add effect…`
  row. Long names truncate with full accessible text and tooltips. The T35 Plugin row and
  up to two effect rows plus Add reserve the same height in every strip so Pan, fader, and
  name stay aligned; longer chains scroll inside the section.
- The UI documentation and index now describe the T12 states and link the gallery
  renders.

### Evidence

Rebuild the deterministic fixtures without installing plugins:

```sh
rtk cargo build -p vst3-host-testplug --features clef-insert-fixture --target-dir target/t12-vst3-fixture --locked
rtk cargo build -p clef-clap-latency-fixture --features effect-fixture --target-dir target/t12-clap-fixture --locked
rtk proxy python3 scripts/t12_bundle_insert_fixtures.py
```

The script writes `ClefInsertVST3.vst3` and `ClefInsertCLAP.clap` under
`validation-output/fixtures/` and verifies the copied binaries byte for byte. The bundles
stay local and are not installed as system plugins.

- Automated, locked workspace suite: **177 passed, 0 failed, 20 ignored**; build,
  `cargo fmt --all --check`, and `cargo clippy --all-targets --locked -- -D warnings`
  passed.
- Native integration evidence, run explicitly with the bundled local fixtures:
  - `--test insert_effects` — VST3 and CLAP effect discovery, instrument filtering,
    stereo processing, latency restart, and state/latency restore: **1 passed**.
  - `clef-engine` serial chain and bypass test: **1 passed**; it also asserts that a
    bypass-only replacement keeps the same stage topology.
  - `clef-engine` bypass crossfade test: **1 passed**; the largest sample step stayed below
    0.09 while the wet/dry amplitude changed over 5 ms. The same test fails with a one-sample
    ramp, so it detects the old click.
  - `clef-engine` bypass/crash dry-path test with a delayed slot: **1 passed**.
  - `clef-engine` transport stop declick test: **1 passed**; the gain falls monotonically over
    240 samples, the stop frame is preserved, the stopped gate remains closed after the fade,
    and `resume_gate` reopens it only for live audition.
  - `clef-session` nine-insert track chain plus Master, insertion undo/redo, move
    undo/redo, bypass undo/redo, remove undo/redo, save and reopen: **1 passed**.
  - Existing T02 and T03 native hosting regressions: **4 passed**.
- GUI evidence: seven effect states plus the T35 Plugin states rendered at 1024×768 and
  760×768 by the `t12_gallery` example. Renders reviewed for empty, populated, hovered insertion gap, bypassed,
  missing, failed, and long-chain states at both widths. Snapshots are linked from the
  [mixer view](../UI/views/mixer.md#clef-t12).
- Realtime evidence: the callback proof test covers zero allocations in the Master
  handoff and drops late wet blocks; the engine fault test covers a crashed slot with a
  delayed neighbour remaining aligned. The T02/T03 callback and regression suites still
  pass.
- VST3 editor feedback: a manual native probe loaded the installed FabFilter Pro-Q 4
  VST3 as an effect, opened its editor, and processed audio. The bridge relayed 609
  connection-point messages to the controller; before the change the SDK proxy discarded
  them and the analyzer stayed blank while audio passed. The probe was temporary and is
  not part of the committed suite.

### Listening setup (prepared 2026-11-20)

`validation-output/T12-listening-v2.clef` is a local, git-ignored project for the
device/listening pass. It uses fresh copies of the two fixture bundles under
`validation-output/fixtures/listen/`, so a stale quarantine entry for the earlier fixture path
cannot block it. It contains three online TestSynth tracks playing the same sustained chord:

- **VST3 Pro-Q** with an active FabFilter Pro-Q 4 VST3 insert.
- **CLAP Pro-Q** with an active FabFilter Pro-Q 4 CLAP insert.
- **CLAP 257** with the bundled Clef CLAP Insert Fixture restored to 257 samples of latency.
- **Master** with the bundled Clef VST3 Insert Fixture restored to 257 samples of latency.

The zero-latency and 257-sample tracks share the same notes, so a latency-compensation fault is
audible as flam or comb filtering rather than as a level change. The fixture insert states are
the four-byte `257` parameter value; an empty state would select their default zero latency.
The project was read and validated through `Project::read`/`validate_storage` and opened by
the session with all three tracks online and the Master insert present. A local probe then loaded
all four insert workers through the project: Pro-Q 4 VST3 and CLAP loaded at zero latency, while
the CLAP fixture and the Master VST3 fixture both reported exactly 257 samples. A six-second
muted playback probe with a clean quarantine registry reported all three tracks Ready, all four
inserts without error, and no new quarantine entry. An earlier fixture path had acquired a
`worker exchange deadline exceeded` entry at 09:31; that stale record blocks the old path but
does not reproduce with the fresh copies. The fixture states also pass the existing
`insert_effects` integration test.

### Completion (2026-11-20)

The task is complete. The user listening pass with `validation-output/T12-listening-v2.clef`
confirmed audible VST3 and CLAP processing, bypass, the Pro-Q 4 VST3 analyzer, latency
alignment with two different insert latencies through a delayed Master chain, and restored
sound and settings after reopening the project. The remaining Stop crackle was traced to the
stopped output gate reopening after the transport fade and unmuting the decaying instrument
tail; the gate now stays closed until Play or live audition. The retest after relaunching
`target/debug/clef` passed.

Automated evidence at completion: **177 passed, 0 failed, 20 ignored**; build, `cargo fmt
--all --check`, and `cargo clippy --all-targets --locked -- -D warnings` passed. The native
insert, editor and engine regressions listed above also pass.
