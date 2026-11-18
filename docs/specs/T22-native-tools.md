# T22 — Native MIDI/Audio Tools

**Status:** Planning draft — not approved for implementation.
**Priority:** P3.
**Date:** 2026-11-17
**Task:** [T22](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T06, T10, T11; T01 behavior decision.

## Goal

Implement the Clef-owned MIDI/audio processing tools named in the T01 interview, such as an
arpeggiator and M/S encoder/decoder, with behavior, placement, and UI agreed before
development. Do not infer their design from the legacy DAW.

## Proposed Scope

- Agreed tool set and exact behavior, including timing and parameter ranges.
- Placement model: MIDI processing before an instrument, insert processing, or another
  explicitly designed position.
- UI, state, and project persistence.
- Undo/redo and command integration.
- Deterministic tests for MIDI timing and audio processing.

## Out of Scope

A general internal plugin suite; plugin export; a rack/node infrastructure merely to host
these tools; unagreed chord or articulation features.

## Open Questions

- Which tools are required first and what each must do.
- Where each tool sits in the signal path and how it is shown.
- Parameter model and automation.
- Whether tools are per-track, per-clip, or global.
- MIDI timing behavior and latency.
- UI representation: inline, drawer, or editor window.
- Whether tools can be chained and in which order.

## UI Proposal Requirement

Every tool surface requires a written UI proposal derived from `docs/UI/` and user approval
before implementation.

## Verification Intent

Deterministic MIDI output; audio math for M/S; parameter persistence; undo/redo; no timing
regression to existing playback; documented behavior matches the approved specification.

## Approval Gate

This document is a planning draft. The tool behavior and placement must be agreed in an
interview, followed by an approved specification and UI proposal, before implementation.
