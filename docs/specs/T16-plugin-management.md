# T16 — Plugin Management and Hosting Robustness

**Status:** Planning draft — not approved for implementation.
**Priority:** P2.
**Date:** 2026-11-17
**Task:** [T16](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T03 scanning and catalog; T12 effect hosting.

## Goal

Make plugin discovery and hosting dependable at scale: cached scans, categories and
favorites, missing-plugin resolution, quarantine policy, rescan, clear failure reporting,
and an agreed compatibility target. This task covers hosting quality, not a new format.

## Proposed Scope

- Scan cache with invalidation and a clear rescan path.
- Categories, search, and favorites for instruments and effects.
- Missing-plugin resolution from a project and from the picker.
- Quarantine or blacklist policy for repeatedly failing plugins.
- Plugin details, format, version, path, and failure reason.
- Robust handling of multi-entry bundles and broken bundles.
- An agreed plugin compatibility matrix and evidence standard.
- Performance of scanning and loading, with bounded waits.

## Out of Scope

AU hosting (T21); plugin authoring; internal plugins; automatic crash-tolerant plugin
sandboxing beyond the existing worker isolation.

## Open Questions

- Scan-cache identity and invalidation. Evaluate fingerprints that detect updates to the
  executable inside a macOS bundle. Define a forced rescan path that still respects quarantine.
- Which categories and metadata are reliable across VST3/CLAP.
- Quarantine policy: user-visible, automatic retry, and how to clear it.
- Compatibility target: vendors, plugins, formats, and what counts as acceptable failure.
- Missing-plugin UI and whether state is retained indefinitely.
- Scan UX for large catalogs and slow plugins.
- Per-plugin settings and favorites persistence.

## UI Proposal Requirement

The plugin browser, scanning, quarantine, and missing-plugin surfaces require a written UI
proposal derived from `docs/UI/` and user approval before implementation.

## Verification Intent

Cache invalidation; broken bundles; missing plugins; rescan; failure reporting; multi-entry
bundles; no application crash or indefinite block; compatibility evidence recorded
separately from GUI and listening evidence.

## Approval Gate

This document is a planning draft. An interview, an approved specification, and an
approved written UI proposal are required before any implementation.
