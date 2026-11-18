# T23 — Packaging and Cross-Platform Groundwork

**Status:** Planning draft — not approved for implementation.
**Priority:** P3.
**Date:** 2026-11-17
**Task:** [T23](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** A stable feature set; distribution and platform scope decision.

## Goal

Prepare the application for real distribution and future platforms: a signed/notarized
macOS bundle, an update and crash-reporting policy, a review of platform-specific
abstractions, and a Windows/Linux feasibility decision. Cross-platform work is groundwork,
not a promised port.

## Proposed Scope

- Reproducible macOS application bundle and deployment of native/VST3 resources.
- Signing, notarization, and clean-machine launch verification.
- Update policy and crash-reporting or diagnostics policy.
- Review of platform-specific code paths and a portability assessment.
- Windows/Linux feasibility report with concrete blockers.
- User-facing installation/onboarding requirements where applicable.

## Out of Scope

Shipping on another platform without a product decision; telemetry without consent; an
auto-update service that changes behavior without approval; App Store distribution unless
agreed.

## Open Questions

- Distribution channel and signing identity.
- Update mechanism: manual, in-app, or none for the first release.
- Crash reporting: local-only logs, user-submitted bundles, or a service.
- Telemetry and privacy policy.
- Windows/Linux priority and which platform is next.
- Minimum macOS version and hardware requirements.
- How plugin paths and bundles are packaged without redistributing commercial content.

## UI Proposal Requirement

Onboarding, update, and crash-reporting surfaces require a written UI proposal derived from
`docs/UI/` and user approval before implementation.

## Verification Intent

Clean-machine launch; notarization success; bundle contents and plugin discovery; update
and crash-reporting policy documented and tested; portability report with cited blockers.

## Approval Gate

This document is a planning draft. Distribution, platform scope, an interview, an approved
specification, and an approved written UI proposal for any new surface are required before
implementation.
