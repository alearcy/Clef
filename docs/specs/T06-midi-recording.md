# T06 — MIDI Recording and Clip Playback

**Status:** Planning draft — not approved for implementation.
**Priority:** P0.
**Date:** 2026-11-17
**Task:** [T06](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T05; T03 MIDI input and latency handling.

## Goal

Deliver the first complete composition loop: arm an instrument track, record live MIDI
with latency compensation, create a playable clip, and hear it scheduled sample-accurately
through the track's plugin worker. Persist clips, notes, and transport state.

## Proposed Scope

- Record-arm and input selection for instrument tracks, reusing the T03 MIDI device model.
- Count-in and metronome integration from T05.
- Timestamped MIDI capture with output-latency compensation and explicit overflow policy.
- A clip model holding notes and timing; create, reuse, and display clips minimally in the
  arrangement.
- Playback scheduling through T05, including loop wrap and all-notes-off behavior.
- Project persistence for clips, notes, transport, and tempo state.
- Stop/record transitions that cannot leave stuck notes.

## Out of Scope

Piano-roll editing (T10); arrangement editing beyond the minimal clip view (T09); audio
recording (T14); comping, punch, and take lanes; automation.

## Open Questions

- Replace, overdub, and loop recording modes; whether all three are in T06.
- One clip per take versus merged recording into an existing clip.
- Count-in default, metronome sound, and whether count-in is mandatory.
- Quantize while recording; if included, whether it is destructive or a display property.
- How plugin latency affects recorded timestamps and clip starts.
- MIDI channel handling, note chase, sustain, pitch bend, and modulation on record/stop.
- Clip boundary rules on stop and loop wrap.
- Interaction with T03's explicit audio enable/disable contract.

## UI Proposal Requirement

The record controls, minimal clip view, and recording feedback require a written UI
proposal derived from `docs/UI/` and user approval before implementation.

## Verification Intent

Deterministic fake-MIDI capture with timestamp checks; latency compensation; loop record
and loop wrap; save/reopen; no stuck notes; playback event positions match the recording;
T02/T03 MIDI regressions still pass.

## Approval Gate

This document is a planning draft. An interview, an approved specification, and an
approved written UI proposal are required before any implementation.
