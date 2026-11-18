# T07 — Autosave, Crash Recovery, and Recent Projects

**Status:** Planning draft — not approved for implementation.
**Priority:** P1.
**Date:** 2026-11-17
**Task:** [T07](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T04 schema v2; T06 recorded content.

## Goal

Protect user work after recording becomes possible: crash-safe periodic autosave, recovery
on relaunch, bounded backups, and a recent-projects/startup surface. A failed write must
never damage the last valid project file.

## Proposed Scope

- Periodic autosave to a crash-safe location, using the existing atomic publish rules.
- Recovery prompt on next launch with explicit recover/discard choices.
- Bounded backup retention and cleanup.
- Recent-projects/startup surface with clear missing-file and incompatible-file states.
- An explicit plugin-state capture policy for autosave.
- Defined interaction with T03's Save / Discard / Cancel contract.

## Out of Scope

Cloud sync, version-control integration, full project history, and automatic conflict
resolution across machines.

## Open Questions

- Autosave interval, location, and retention count.
- Whether plugin state is captured on every autosave or only when it changed; how the cost
  is bounded for large plugin states.
- Recovery UX and what happens when the autosave is invalid or from an incompatible version.
- Autosave during playback or recording; whether it pauses the transport.
- Whether autosave is always on or a preference.
- How recent projects handle moved, missing, or read-only files.
- Interaction with T08 undo history: is history restored after recovery?

## UI Proposal Requirement

The recovery dialog and startup/recent-projects surface require a written UI proposal
derived from `docs/UI/` and user approval before implementation.

## Verification Intent

Simulated crash and recovery; atomic writes under injected failure; backup rotation; a
failed autosave leaves the last good file intact; recent-list behavior; no data loss when
the application is terminated during autosave.

## Approval Gate

This document is a planning draft. An interview, an approved specification, and an
approved written UI proposal are required before any implementation.
