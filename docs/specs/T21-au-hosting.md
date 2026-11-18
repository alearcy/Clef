# T21 — AU Hosting

**Status:** Planning draft — not approved for implementation.
**Priority:** P3.
**Date:** 2026-11-17
**Task:** [T21](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T12 effect hosting; a compatibility and validation decision.

## Goal

Add macOS Audio Unit hosting for instruments and effects once the VST3/CLAP foundation is
stable: discovery, isolated loading, original editors, state, delay compensation, and
defined failure behavior.

## Proposed Scope

- AU discovery and catalog entries with stable identity.
- Isolated AU workers using the established process and realtime rules.
- Instrument and effect hosting, editors, and state persistence.
- PDC and routing integration.
- Validation and failure policy for incompatible or crashing AUs.
- Compatibility evidence on a bounded plugin matrix.

## Out of Scope

AUv3 extensions on iOS; AU plugin authoring; cross-platform AU equivalents; ARA.

## Open Questions

- AUv2 versus AUv3 scope and whether both are required.
- Validation approach and whether to use Apple's validation tooling.
- Compatibility matrix, vendors, and acceptable failure behavior.
- Sandbox/entitlement implications for out-of-process AU hosting.
- Editor embedding differences from VST3/CLAP.
- Whether AU hosting changes the plugin catalog or picker model.

## UI Proposal Requirement

Any new plugin-picker or hosting surface requires a written UI proposal derived from
`docs/UI/` and user approval before implementation.

## Verification Intent

Fixture AU instrument and effect; state/editor round trip; PDC; failure isolation;
compatibility evidence; save/reopen; no regression to VST3/CLAP paths.

## Approval Gate

This document is a planning draft. A product decision to support AU, an interview, an
approved specification, and an approved written UI proposal are required before
implementation.
