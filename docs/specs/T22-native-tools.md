# T22 — Native MIDI/Audio Tools

**Status:** Planning draft — not approved for implementation.
**Priority:** P3.
**Date:** 2026-11-17
**Task:** [T22](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T06, T10, T11, T28; T01 behavior decision.

## Goal

Implement the Clef-owned MIDI/audio processing tools named in the T01 interview that are not
already provided by the T28 rack graph, such as an arpeggiator, with behavior, rack placement,
and UI agreed before development. M/S encode/decode and the utility nodes are delivered by
T28; do not duplicate them.

## Proposed Scope

- Agreed tool set and exact behavior, including timing and parameter ranges.
- Placement: nodes in the T28 FX rack graph (decided 2026-11-18), with the pins and
  instrument-rack versus insert-rack rules agreed here. No second hosting path.
- UI, state, and project persistence.
- Undo/redo and command integration.
- Deterministic tests for MIDI timing and audio processing.

## Out of Scope

A general internal plugin suite; plugin export; the T28 rack infrastructure; unagreed chord
or articulation features.

## Open Questions

- Which tools are required first and what each must do.
- Whether each tool is available in instrument racks, insert racks, or both, and its pins.
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
