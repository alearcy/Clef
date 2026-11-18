# T20 — Automated App Regression Harness

**Status:** Planning draft — not approved for implementation.
**Priority:** P2.
**Date:** 2026-11-17
**Task:** [T20](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T04 onward; grows with each feature.

## Goal

Make application-level behavior reproducible to test: deterministic fixtures and
integration tests for multitrack mixing, recording timing, undo, routing, and project
migration, plus a GUI smoke path where practical. Reduce manual fixture setup and keep
automated, GUI, listening, and hardware evidence separate.

## Proposed Scope

- Deterministic fixtures for multitrack, recording, editing, routing, and migration.
- Integration tests that exercise the real worker and project paths without requiring
  commercial plugins.
- A reproducible build/bundle script for fixture plugins.
- A GUI smoke path or documented alternative where WindowServer automation is impractical.
- Test-data and report conventions compatible with T02/T03 evidence.
- Clear separation between automated, GUI, listening, and physical-hardware evidence.

## Out of Scope

Replacing listening or hardware acceptance; claiming CI where no runner exists; broad
plugin compatibility testing; fuzzing every parser unless separately agreed.

## Open Questions

- Which user workflows must be automated first.
- Headless versus WindowServer-dependent tests and how they are marked.
- Which fixture plugins are needed and how they are versioned.
- CI availability and whether the repository will have a stable runner.
- Ownership and runtime budget for the suite.
- How reports record failures without overwriting prior evidence.

## Verification Intent

The harness catches known regressions with deterministic results; fixtures build
reproducibly; test categories remain distinguishable; a failing test produces actionable
evidence rather than a manual reproducer.

## Approval Gate

This document is a planning draft. An interview and an approved specification are required
before implementation.
