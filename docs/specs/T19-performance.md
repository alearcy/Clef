# T19 — Performance, Scale, and Realtime Safety

**Status:** Planning draft — not approved for implementation.
**Priority:** P2.
**Date:** 2026-11-17
**Task:** [T19](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T04, T11, T12, T14; performance targets must be agreed first.

## Goal

Establish measured performance and realtime-safety limits for multitrack work: agreed
track/plugin counts, CPU and latency budgets, load times, and stress evidence. T01
explicitly did not agree numerical thresholds, so this task starts by agreeing them.

## Proposed Scope

- Agreed performance targets and the hardware/configuration they apply to.
- Profiling of the worker pool, IPC, realtime mixing, scheduling, plugin loading, and UI.
  For [dense editing surfaces](../UI/contract.md#dense-editing-surfaces), measure frame time during
  playback, scroll, and zoom. Isolate geometry generation, upload, and rendering costs before
  proposing GPU-side caching or a renderer change.
- Stress fixtures for many tracks, many plugin instances, long sessions, and device churn.
- CPU/disk meters only if agreed and based on real measurements.
- Verification of the device callback contract in [AGENTS.md](../../AGENTS.md#realtime-and-plugin-isolation),
  including allocation and final memory release on the callback path.
- Long-run stability evidence and documentation of observed limits.

## Out of Scope

Claims of broad hardware compatibility; a general benchmarking product feature; cloud
rendering; optimizations without evidence.

## Open Questions

- Track and plugin counts for the first target.
- Sample rates, buffer sizes, and acceptable xrun/miss thresholds.
- CPU measurement method and how it is displayed.
- Load-time and project-open targets.
- Which hardware configurations define the evidence.
- Memory limits for plugin state, undo, and sample data.
- Whether performance budgets are per release or per task.

## UI Proposal Requirement

Any new performance or diagnostics surface requires a written UI proposal derived from
`docs/UI/` and user approval before implementation.

## Verification Intent

Stress scenarios with recorded counters; profiling evidence; long-run stability; callback
audit; measured behavior compared with the agreed targets; no performance claim without
the corresponding evidence category.

## Approval Gate

This document is a planning draft. The performance targets, an interview, an approved
specification, and an approved written UI proposal for any new surface are required before
implementation.
