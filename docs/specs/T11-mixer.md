# T11 — Mixer and Master Section

**Status:** Planning draft — not approved for implementation.
**Priority:** P1.
**Date:** 2026-11-17
**Task:** [T11](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T04 multitrack mix path.

## Goal

Give the user a clear balance surface for instrument tracks and the master: gain, pan,
mute, solo, metering, and a defined output path. The mixer is the first surface that makes
gain staging and solo/mute semantics explicit.

## Proposed Scope

- Channel strips for every instrument track and the master.
- Gain, pan, mute, solo, and peak/hold/clip metering.
- Master output level and a clear direct-to-master routing path.
- Meter values driven by the realtime engine, not by UI-side estimation.
- Keyboard and command-registry integration for mute/solo and navigation.
- Persistence of mixer values in the project.

## Out of Scope

Buses, sends, groups, and the routing graph (T15); insert effects (T12); automation;
master limiter or loudness normalization unless explicitly agreed; channel EQ.

## Open Questions

- Pan law and center attenuation.
- Mono versus stereo tracks; how mono sources are represented.
- Meter ballistics, scale, peak/hold, and clip indication.
- Solo modes (PFL, AFL, solo-in-place) and mute/solo interaction.
- Clipping policy at the master; whether headroom is enforced or only indicated.
- Master gain range and default.
- Whether polarity, width, or output channel routing belong in T11.
- Mixer layout and channel count behavior on narrow windows.

## UI Proposal Requirement

The mixer and channel-strip surfaces require a written UI proposal derived from
`docs/UI/views/mixer.md`, `channel-strip.md`, and the relevant components, with user
approval before implementation.

## Verification Intent

Gain and pan math; mute/solo interaction truth table; meter values against deterministic
signals; PDC and routing unaffected; save/reopen; keyboard and command parity.

## Approval Gate

This document is a planning draft. An interview, an approved specification, and an
approved written UI proposal are required before any implementation.
