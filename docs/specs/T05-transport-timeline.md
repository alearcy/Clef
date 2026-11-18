# T05 — Transport, Tempo Map, and Timeline

**Status:** Planning draft — not approved for implementation.
**Priority:** P0.
**Date:** 2026-11-17
**Task:** [T05](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T04.

## Goal

Provide one authoritative musical time base for the application: bars, beats, and ticks
driven by the audio clock, with transport, tempo, timeline, playhead, loop, count-in, and
metronome. T05 is the scheduling foundation for recording, clip playback, and editing.

## Proposed Scope

- A project time model with a tempo and time-signature representation whose complexity is
  agreed in the interview (single tempo versus a map).
- Transport state machine: play, stop, locate, loop; explicit behavior for audio disabled.
- Sample-accurate scheduling API shared by live input, clip playback, and offline render.
- Metronome and count-in with an agreed routing and sound.
- Timeline surface: ruler with bars/beats, track lanes, playhead, locators, zoom/scroll.
- Visual playhead behavior that accounts for output latency.

## Out of Scope

Clips, recording, automation lanes, chord or articulation features, and the full
arrangement editor. T09 owns clip editing; T06 owns recording.

## Open Questions

- Single tempo versus a tempo/time-signature map; how tempo changes are edited.
- Whether the transport moves with audio disabled or requires an explicit audio session.
- Loop, count-in, and metronome defaults; metronome output routing.
- Marker and locator scope; whether markers are in T05.
- Timeline resolution, zoom range, scrolling behavior, and playhead smoothing.
- How live input is timestamped against the transport and output latency.
- Stop/seek semantics for held notes, sustain, and queued events.

## UI Proposal Requirement

The timeline, ruler, transport, and metronome surfaces require a written UI proposal
derived from `docs/UI/` and user approval before implementation.

## Verification Intent

Playhead and event-position math across loop wraps and tempo changes; sample-accurate
scheduling against deterministic fixtures; transport state-machine tests; no stale events
after stop; metronome and count-in timing evidence.

## Approval Gate

This document is a planning draft. An interview, an approved specification, and an
approved written UI proposal are required before any implementation.
