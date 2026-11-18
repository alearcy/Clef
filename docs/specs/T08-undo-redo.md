# T08 — Undo/Redo and Edit History

**Status:** Planning draft — not approved for implementation.
**Priority:** P1.
**Date:** 2026-11-17
**Task:** [T08](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T04 schema v2; T06 clip/note model. Must land before T09 and T10.

## Goal

Provide command-based undo and redo for project edits, with predictable granularity, a
bounded memory budget, and integration with the shared command registry. This is required
before arrangement and piano-roll editing become safe.

## Proposed Scope

- A command/journal model covering project structure, clips, notes, mixer values, and
  routing changes.
- Undo and redo with coalescing for continuous gestures such as drags and fades.
- Bounded history memory and a defined eviction policy.
- Menu entries, keyboard shortcuts, and enabled-state integration.
- Defined behavior across save, open, project replacement, autosave, and plugin failure.
- Defined behavior for plugin parameter edits if they are included at all.

## Out of Scope

Full project version history, branching, collaboration, and file-level versioning. Audio
waveform editing undo is out of scope until T14 defines that editing model.

## Open Questions

- Whether plugin parameter edits are undoable; if so, whether by snapshot or by parameter
  events, and what the memory cost is.
- History granularity: per command, per gesture, or per user-visible operation.
- Whether history survives save/open or is discarded on project replacement.
- Undo during playback or recording: allowed, disabled, or stop-first.
- Memory limit, history depth, and whether the user can see or clear history.
- How undo interacts with failed plugin workers and unsaved plugin edits.

## UI Proposal Requirement

Undo/redo menu entries and any history surface require a written UI proposal derived from
`docs/UI/` and user approval before implementation.

## Verification Intent

Per-command undo/redo; redo invalidation after a new edit; gesture coalescing; memory-bound
eviction; save/open semantics; no stale plugin state after undo; interaction with worker
failure and recovery.

## Approval Gate

This document is a planning draft. An interview, an approved specification, and an
approved written UI proposal are required before any implementation.
