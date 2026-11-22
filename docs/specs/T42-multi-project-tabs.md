# T42 — Multi-Project Window with Tabs (Exploratory)

- **Status** — Draft — 2026-11-21. Exploratory: no interview yet.
- **Priority** — P3 (future exploration, user request 2026-11-21).
- **Task** — [T42](../../TASKS.md)
- **Dependencies** — After [T41](T41-session-window-layout.md), which keeps the project title in
  place until this is decided.

## Goal

Explore whether one Clef window should hold several projects as tabs, and where the project
title and its unsaved-changes state would live if it did.

## Questions to explore

1. Can several projects be open at once, and does only the front tab own the audio engine?
2. Where do the title and the unsaved-changes state go: in the tab, the window frame, or both?
3. How do Close, Save, and quit behave with several unsaved tabs?
4. Do plugin editors and side windows belong to a tab?

## Decisions

None yet. Interview the user before any specification work.
