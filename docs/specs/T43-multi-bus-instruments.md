# T43 — Instruments with Several Audio Buses

- **Status** — Done — 2026-11-22: implemented, verified automatically and confirmed by the user
  in the app; closed.
- **Priority** — P1, right after T41 (user decision).
- **Task** — [T43](../../TASKS.md)
- **Dependencies** — Follows [T41](T41-session-window-layout.md) round 4, where the cause was
  found. Extra outputs and instrument sidechain routing belong to
  [T28](T28-fx-rack.md); plugin robustness in general to [T16](T16-plugin-management.md).

## Problem (2026-11-22)

Kontakt 7, Opus, SINE Player and Falcon (VST3) never load. New Tracks and Choose Instrument both
create the track without an instrument and show nothing. Zebra2 and ZebraHZ load. A native test
loading SINE Player reported "unsupported VST3 stereo audio bus layout". The VST3 bridge
(`native/vst3_bridge.cpp`, `stereo_buses`) accepts an instrument only when it declares exactly
one output bus, stereo, and no input bus. The CLAP host (`crates/plugin-host/src/clap_host.rs`)
applies the same rule to audio ports ("unsupported CLAP audio port layout"). Instruments with
multiple outputs or a sidechain input are therefore refused.

## Requirements

R1. **Main output only (user).** An instrument with more than one output bus (VST3) or port
    (CLAP) loads. Clef plays its main output: the VST3 output bus 0 of type main, or the CLAP
    output port flagged main (port 0 when none is flagged). Every other output stays deactivated
    and is not heard. Routing extra outputs to the mixer is later work (T28).
R2. **Inputs deactivated (user).** An instrument that declares audio input buses or ports
    (sidechain, external input) loads with them deactivated and receives silence. Sidechain into
    instruments is later work (T28).
R3. **Channel layout (user).** Clef asks for stereo on the main output. If the instrument keeps a
    mono main output, its signal is copied to left and right. A main output with more than two
    channels (surround) is refused with a clear reason.
R4. **Both formats (user).** R1–R3 apply to VST3 and CLAP alike. The effect rules (stereo main
    input, auxiliary inputs) are unchanged.
R5. **Visible failure (user: modal).** When an instrument cannot load, from New Tracks, Choose
    Instrument or project open, a modal notice names the instrument and the reason, and stays
    until the user closes it. Several failures from one action (New Tracks with several tracks)
    share one notice. The track's inspector, under INSTRUMENT, keeps "Could not load <name>:
    <reason>" until another instrument is chosen. The track keeps no instrument, as today.
R6. **Realtime rules unchanged.** Bus activation and buffer preparation happen at load time on
    the worker. The process call only adds pointers to preallocated silent and scratch buffers
    for inactive buses, with no allocation or lock (AGENTS.md, native/AGENTS.md).

## Scope

In scope: R1–R6. The bridge (`native/vst3_bridge.cpp`), the CLAP host, the notice and inspector
text, a VST3 and a CLAP multi-output fixture with a sidechain input, tests, and documentation of
the notice and inspector in `docs/UI/`.

Out of scope: routing extra outputs or sidechain inputs (T28); surround; scanning changes; AU (T21).

## Acceptance criteria

1. The VST3 and CLAP fixtures (four stereo outputs, one sidechain input) load as instruments.
   Notes played into them are heard on the main output only. This is covered by automated tests.
2. A mono-main fixture variant, or a unit test of the bridge's channel mapping, shows the signal
   copied to both channels.
3. A surround-main layout is refused, and its reason appears in the modal notice and the
   inspector.
4. An ignored native test loads Kontakt 7, Opus, SINE Player and Falcon from the user's
   installation, skipping any that are not installed. The user confirms by listening that each
   one plays from New Tracks and from Choose Instrument.
5. A failing load from New Tracks with three tracks shows one modal notice.
6. `cargo fmt`, `clippy -D warnings`, `cargo test` pass; the TestSynth native regressions still
   pass.

## Open questions

None at drafting time.

## Progress

- 2026-11-22 — Cause found during T41 round 4. Interview with the user. Specification drafted
  and approved.
- 2026-11-22 — Implemented after committing T41 round 4 (`0c46c5e`); automated verification
  passed; awaiting the user's listening check.

## Implementation notes

- VST3 (`native/vst3_bridge.cpp`): `instrument_buses` picks the main output (first bus of type
  main, else 0), asks stereo on it and keeps the other buses' own arrangements, then accepts the
  layout the plug-in keeps (mono main is copied to both sides; more than two channels is refused
  with the count). Up to 128 output and 32 input buses of at most 32 channels; Kontakt 7 declares
  59 output buses. Inactive outputs point at one preallocated scratch block and every input
  except an effect's main one at the silent block; `ProcessData` lists every bus. The effect
  rules moved unchanged into `effect_buses`; the input role is the loaded role (`effect_role`).
- CLAP (`crates/plugin-host/src/clap_host.rs`): `instrument_main_output` picks the port flagged
  main (else 0) and checks mono or stereo and the channel limits; `ClapAudio` keeps a buffer per
  channel of every port, gives instrument inputs constant silence and returns the main port
  (mono duplicated).
- Failures (`crates/session`): a failed load records `LoadFailure { sequence, track, instrument,
  reason }` in the snapshot (latest 32) and the track's `TrackView::load_error`, cleared when
  another load starts. The UI gathers new sequences into one `NoticeBox` and shows the error in
  the inspector.
- Fixtures: `clef-multi-bus-fixture` and `clef-mono-main-fixture` (VST3, patch
  [0004](../../patches/vst3-host/0004-clef-multi-bus-fixture.patch)) and `multi-bus-fixture` and
  `mono-main-fixture` (CLAP, `fixtures/clap-latency`): four outputs, auxiliary outputs written at
  0.9, a sidechain that must arrive silent. Bundled by `scripts/t43_bundle_multi_bus_fixtures.py`.
  The scanner is unchanged: the VST3 fixtures have no subcategories and a sidechain, so its
  legacy heuristic does not list them, and the tests load them by path and class ID.

## Verification

Automated (2026-11-22): `cargo fmt --all --check` and `cargo clippy --all-targets --locked --
-D warnings` are clean. `cargo test --locked` reports 291 passed and 30 ignored. New tests:
- `t43_instruments_play_their_main_output_in_mono_or_stereo_only` (CLAP layout: main selection,
  surround and oversized ports refused);
- `t43_load_failures_share_one_notice` (one notice for three failures, OK dismisses, inspector
  line).

Native, ignored by default, all passed:
- `native_t43_multi_bus_fixtures_play_their_main_output` (VST3 and CLAP, multi-bus and mono main:
  the note is heard on the main output only, at 0.25 on both sides; the sidechain arrives
  silent);
- `native_t43_installed_multi_bus_instruments_load` (Kontakt 7, Opus, SINE Player and Falcon load
  through New Tracks with audio running);
- `native_t43_failed_loads_are_reported_per_track`.

The existing TestSynth, T08, T10, T12 and insert native regressions passed. One exception:
`clef-engine` `a_restart_request_is_served_off_the_processing_thread` fails the same way on the
committed baseline without these changes (three runs out of three). It is not caused by T43.

Not covered: the VST3 surround refusal has no fixture (the CLAP rule is unit-tested; the VST3
branch was reviewed in code), and nothing yet checks by ear that the commercial instruments play.

Listening: the user loaded the previously refused instruments through New Tracks in the running
app and confirmed they work (2026-11-22).
