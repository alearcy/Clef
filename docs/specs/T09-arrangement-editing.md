# T09 — Arrangement and Clip Editing

**Status:** Planning draft — not approved for implementation.
**Priority:** P1.
**Date:** 2026-11-17
**Task:** [T09](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T06 clip model; T08 undo/redo.

## Goal

Make the arrangement a real editing surface for recorded MIDI: create, move, trim, split,
duplicate, and loop clips on a musical grid, with selection, zoom, keyboard, and context
menus. Every edit goes through the command registry and the T08 history.

## Proposed Scope

- Clip creation and deletion; moving across tracks and time.
- Trimming, splitting, duplicating, and looping clips.
- Snap and grid options, including explicit free-edit behavior.
- Single and multi-selection, marquee/range selection, and keyboard commands.
- Zoom, scroll, and playhead navigation in the arrangement.
- Context menus and tooltips using live command bindings.
- Undo/redo integration for every operation.
- Persistence of clip edits and view state where agreed.

## Out of Scope

Audio clip editing and waveforms (T14); fades and crossfades unless explicitly agreed;
automation lanes; folder tracks; chord/articulation lanes; ARA.

## Open Questions

- Clip versus part terminology in the UI and data model.
- One clip per track or many overlapping clips; how overlap is displayed and played.
- Loop semantics: clip loop count, project loop, and how trimming interacts.
- Trim rules around notes and clip boundaries.
- Snap options and whether snap is per-view or global.
- Editing while the transport runs.
- Whether fades and crossfades are part of T09 or a later task.
- Folder tracks and track grouping in the arrangement.

## UI Proposal Requirement

The arrangement surface requires a written UI proposal derived from the relevant
`docs/UI/` views and user approval before implementation. The proposal must show selection,
snap, zoom, and context-menu states.

## Verification Intent

Deterministic edit operations; undo/redo for each command; save/reopen; timeline scrolling
and zoom; refused operations report a reason; no edits lost when switching selection or
clips.

## Approval Gate

This document is a planning draft. An interview, an approved specification, and an
approved written UI proposal are required before any implementation.
