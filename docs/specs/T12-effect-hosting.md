# T12 — Insert Effects Hosting

**Status:** Planning draft — not approved for implementation.
**Priority:** P1.
**Date:** 2026-11-17
**Task:** [T12](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T04 worker pool; T11 mixer output path; T02/T03 hosting invariants.

## Goal

Add external VST3 and CLAP effect hosting to the signal path. Effects are discovered as
effects (not instruments), loaded in isolated workers, arranged in per-track and master
insert chains, and delay-compensated across the chain.

## Proposed Scope

- Effect discovery and catalog filtering for VST3/CLAP effect entries.
- Per-track and master serial insert chains of a bounded length.
- Isolated effect workers with the T02/T03 failure, deadline, and lifecycle rules.
- Bypass, original editor hosting, and complete state persistence.
- Latency compensation across each chain and from tracks to the master.
- Failure isolation: an effect crash must not take down the audio callback or other
  tracks.
- FX surface showing chain order, bypass, editor access, and failure state.
- Save/open round trip of effect identity, order, and state.

## Out of Scope

The legacy node/cable graph; sidechain routing; parameter automation; AU effects (T21);
internal effects; ARA.

## Open Questions

- Maximum inserts per chain and total effect instances.
- Chain reordering rules and whether order changes are undoable immediately.
- Catalog filtering: how effect versus instrument entries are identified reliably.
- Simple serial chain versus node graph; do not inherit the legacy graph by default.
- Plugin parameter automation timing and whether it is a later task.
- Sidechain routing and timing; likely later with T15.
- Latency changes while playback is active; restart behavior and audible handling.
- Effect editor window model and focus behavior.

## UI Proposal Requirement

The insert rack and effect editor surfaces require a written UI proposal derived from
`docs/UI/views/fx-rack.md`, `plugin-window.md`, and the relevant components, with user
approval before implementation.

## Verification Intent

Deterministic effect fixture; chain PDC; bypass; state round trip; effect worker crash
isolation; save/reopen; latency-change transition; T02/T03 regressions still pass.

## Approval Gate

This document is a planning draft. An interview, an approved specification, and an
approved written UI proposal are required before any implementation.
