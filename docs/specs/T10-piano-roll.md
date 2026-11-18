# T10 — Piano Roll and MIDI Editing

**Status:** Planning draft — not approved for implementation.
**Priority:** P1.
**Date:** 2026-11-17
**Task:** [T10](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T06 note/clip model; T08 undo/redo.

## Goal

Provide the core MIDI writing surface: edit the notes of a clip with pointer and keyboard,
hear the result through the track instrument, and undo every change. MIDI writing is the
product's first priority, so this surface needs an approved design before implementation.

## Proposed Scope

- Add, delete, move, resize, and velocity-edit notes.
- Single and multi-selection, marquee selection, copy/paste, and duplicate.
- Quantize and transpose subset agreed in the interview.
- Snap, zoom, scroll, playhead following, and note audition on the track instrument.
- Pitch grid and controller lanes at the scope agreed in the interview.
- Keyboard parity for the main editing commands.
- Undo/redo for every note operation.
- Persistence of note edits and view state where agreed.

## Out of Scope

Chord/scale lanes and chord tools; articulation maps; notation; ARA; audio editing; step
input unless explicitly agreed; automation lanes.

## Open Questions

- Velocity and CC lanes: which lanes, how many, and whether they are in T10.
- Quantize, humanize, and transpose subset; grid and strength options.
- Scale or chord assistance: explicitly not assumed.
- Step input, note audition, and multiple open clips/parts.
- Note chase and audition behavior during playback.
- How the roll shares commands and undo scope with the arrangement (T09).
- Pitch range, zoom limits, and keyboard navigation.

## UI Proposal Requirement

The piano-roll surface requires a written UI proposal derived from
`docs/UI/views/piano-roll.md`, the note component, and related views, with user approval
before implementation. The proposal must show selection, editing, velocity, snap, and
controller-lane states if included.

## Verification Intent

Note-edit commands; undo/redo; rounding and snap; save/reopen; keyboard and pointer parity;
no edits lost when switching clips; deterministic note timing after edits.

## Approval Gate

This document is a planning draft. An interview, an approved specification, and an
approved written UI proposal are required before any implementation.
