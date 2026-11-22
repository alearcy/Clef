# T43 — Instruments with Several Audio Buses

- **Status** — Todo — 2026-11-22: specification approved by the user; implementation follows
  T41.
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
