# T15 — Buses, Sends, Groups, and Routing

**Status:** Planning draft — not approved for implementation.
**Priority:** P2.
**Date:** 2026-11-17
**Task:** [T15](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T11 mixer; T12 effects.

## Sequencing

Approved on 2026-11-19: T15 follows T13 and does not wait for T14 (audio tracks). MIDI
instrument tracks can already be grouped and sent to buses, so the routing graph does not
depend on audio recording being implemented.

## Goal

Extend the direct-to-master path into a routing graph: group/bus/FX-return tracks,
pre/post-fader sends, defined solo/mute semantics, delay compensation across the graph,
and rejection of invalid or cyclic routing.

## Proposed Scope

- Group, bus, and FX-return track types as agreed.
- Pre- and post-fader sends with a bounded count.
- Routing surface with explicit source/destination and gain.
- PDC across the full graph.
- Solo/mute semantics across tracks, buses, and returns.
- Cycle and invalid-connection rejection without discarding the last valid routing.
- Persistence and undo integration.

## Out of Scope

A full matrix mixer; sidechain unless agreed; automation of sends/routing; folder tracks
unless agreed; complex surround routing.

## Open Questions

- Which bus, group, and FX-return types are required.
- Send count and pre/post-fader behavior.
- Sidechain routing and its timing; likely a separate decision.
- Solo-in-place versus PFL/AFL and how solo affects returns.
- Routing UI and whether the legacy hierarchy/nodes are relevant.
- Folder tracks and nested groups.
- Automation of routing and send levels; likely later.
- PDC behavior when the graph changes during playback.

## UI Proposal Requirement

The routing surface and any new channel-strip types require a written UI proposal derived
from `docs/UI/` and user approval before implementation.

## Verification Intent

Routing and gain math; PDC across the graph; cycle rejection without losing the last valid
routing; solo/mute truth tables; save/reopen; no stuck notes or audio corruption on graph
changes.

## Approval Gate

This document is a planning draft. An interview, an approved specification, and an
approved written UI proposal are required before any implementation.
