# T17 — Device Resilience and Preferences

**Status:** Planning draft — not approved for implementation.
**Priority:** P2.
**Date:** 2026-11-17
**Task:** [T17](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T04; T03 audio/MIDI device selection and explicit enable/disable contract.

> **2026-11-20:** [T39](T39-always-on-audio.md) removes Enable/Disable Audio and implements global
> audio preferences, the per-project override, first-launch defaults, and missing-device fallback.
> T17 keeps hotplug, reconnection, and the remaining scope below.

## Goal

Make audio and MIDI device handling resilient across real sessions: hotplug and device
loss, default devices, sample-rate/buffer preferences, MIDI channel filtering, clear
warnings, and persisted preferences. Extend T03's explicit control instead of silently
falling back.

## Proposed Scope

- Device hotplug and loss detection with explicit stop and reconfiguration.
- Default audio output and MIDI input preferences.
- Sample-rate and buffer preferences with supported-configuration validation.
- MIDI input filters or channel selection if agreed.
- Clear warnings for unavailable devices and unsupported configurations.
- Persisted preferences separate from the project where appropriate.
- No stuck notes or stale packets across device changes.

## Out of Scope

Aggregate devices unless agreed; multiple simultaneous audio devices unless agreed;
control surfaces; per-track device routing; cross-platform audio backends.

## Open Questions

- Fallback policy when a saved device is missing: block, silent, or suggest a substitute.
- Whether preferences are global or per project for each device field.
- Default device behavior on first launch.
- Aggregate devices and multi-device output.
- MIDI channel filters and multi-client MIDI.
- Sample-rate mismatch behavior across tracks and plugins.
- Which device settings belong in the project and which in preferences.
- Control-surface support timing.

## UI Proposal Requirement

Device and preference surfaces require a written UI proposal derived from
`docs/UI/views/settings.md` and the relevant components, with user approval before
implementation.

## Verification Intent

Device removal and reconnect; invalid configuration rejection; no stuck notes; preferences
persist and restore; no silent rerouting; actual active configuration remains visible.

## Approval Gate

This document is a planning draft. An interview, an approved specification, and an
approved written UI proposal are required before any implementation.
