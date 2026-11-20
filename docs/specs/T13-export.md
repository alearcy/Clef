# T13 — Export and Bounce

**Status:** Planning draft — not approved for implementation.
**Priority:** P1.
**Date:** 2026-11-17
**Task:** [T13](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T11 mixer; include T12 effects if present.

## Sequencing

Sequencing decision 2026-11-20: T13 keeps priority P1 but is deferred to the end of the open
work, after T23 and before T29, which needs T13's offline render path. T15 now comes first.
The offline render path stays available to automated tests in the meantime; this changes the
order, not the scope.

## Goal

Expose the existing offline render path as a product export: render the session or a range
to a WAV file with latency compensation, a defined tail, progress, cancellation, and
overwrite protection. This is the deliverable path for a composition.

## Proposed Scope

- Export session or selected range to stereo WAV at agreed rate and bit depth.
- Compensated leading latency and a defined tail policy.
- Progress reporting, cancellation, and cleanup of partial outputs.
- Refusal to overwrite without explicit confirmation.
- Clear failure reporting for missing plugins, offline-rejected plugins, and write errors.
- Agreement on whether effects are included and how offline processing is verified.

## Out of Scope

Stems unless agreed; additional formats unless agreed; normalization or limiting unless
agreed; cloud export; batch export; loudness standards.

## Open Questions

- WAV only or additional formats; sample rates and bit depths.
- Stems and per-track/per-bus export; if included, naming and folder layout.
- Sample-rate conversion policy and dithering.
- Normalization and limiting: explicitly not assumed.
- Range selection behavior and default export length.
- Tail length and whether it is automatic or fixed; how tails are verified.
- Filename templates and default directory.
- Handling of missing or failed plugins during offline render.

## UI Proposal Requirement

The export surface requires a written UI proposal derived from `docs/UI/` and user approval
before implementation.

## Verification Intent

Exact frame counts; PDC alignment; cancellation leaves no partial file; overwrite refusal;
empty session; readability of the output; failure reporting for missing plugins; T02 render
regressions still pass.

## Approval Gate

This document is a planning draft. An interview, an approved specification, and an
approved written UI proposal are required before any implementation.
