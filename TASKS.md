# Tasks

Each task has a separate specification in `docs/specs/`. Requirements, decisions, progress, and verification evidence belong in that specification or its linked supporting documents. Keep Done entries brief.

The post-T03 order was approved on 2026-11-17. Its tasks are `Planned`: their
specifications are planning drafts until the corresponding interview and specification
approval are complete. See the [post-T03 backlog proposal](docs/planning/post-T03-backlog.md)
for the rationale, priorities, and open product decisions.

## Active and completed

| Task | Status | Specification | Next step / dependency |
| --- | --- | --- | --- |
| T01 — Evaluate the DAW Engine Foundation | Done | [T01 specification](docs/specs/T01-engine-foundation.md) | Completed 2026-11-17; Clef-owned foundations adopted. |
| T02 — Validate the MIDI to Plugin to Audio and Offline Render Path | Done | [T02 specification](docs/specs/T02-engine-validation.md) | Completed 2026-11-17; agreed acceptance matrix passed. |
| T03 — Single-Instrument Project | Done | [T03 specification](docs/specs/T03-single-instrument-project.md) | Completed 2026-11-17; project save/reopen/live MIDI workflow accepted. |
| T24 — Rust Workspace Boundaries | Done | [T24 specification](docs/specs/T24-rust-workspace-boundaries.md) | Completed 2026-11-18. |

## Planned — post-T03 backlog

| Task | Status | Specification | Next step / dependency |
| --- | --- | --- | --- |
| T04 — Multitrack Session and Instrument Engine | In progress | [T04 specification](docs/specs/T04-multitrack-session.md) | Eight-track physical MIDI, selected-track routing, and release tails confirmed; remaining listening checks in the specification. |
| T05 — Transport, Tempo Map, and Timeline | Planned | [T05 specification](docs/specs/T05-transport-timeline.md) | P0. After T04. |
| T06 — MIDI Recording and Clip Playback | Planned | [T06 specification](docs/specs/T06-midi-recording.md) | P0. After T05. |
| T07 — Autosave, Crash Recovery, and Recent Projects | Planned | [T07 specification](docs/specs/T07-autosave-recovery.md) | P1. After T06; protect recorded work. |
| T08 — Undo/Redo and Edit History | Planned | [T08 specification](docs/specs/T08-undo-redo.md) | P1. After T06; before arrangement and piano-roll editing. |
| T09 — Arrangement and Clip Editing | Planned | [T09 specification](docs/specs/T09-arrangement-editing.md) | P1. After T06 and T08. |
| T10 — Piano Roll and MIDI Editing | Planned | [T10 specification](docs/specs/T10-piano-roll.md) | P1. After T06 and T08. |
| T11 — Mixer and Master Section | Planned | [T11 specification](docs/specs/T11-mixer.md) | P1. After T04. |
| T12 — Insert Effects Hosting | Planned | [T12 specification](docs/specs/T12-effect-hosting.md) | P1. After T04 and T11. |
| T13 — Export and Bounce | Planned | [T13 specification](docs/specs/T13-export.md) | P1. After T11; include T12 effects if present. |
| T14 — Audio Tracks and Recording | Planned | [T14 specification](docs/specs/T14-audio-recording.md) | P1. After T05 and T11; media-storage decision required. |
| T15 — Buses, Sends, Groups, and Routing | Planned | [T15 specification](docs/specs/T15-routing.md) | P2. After T11 and T12; not required for first mixes. |
| T16 — Plugin Management and Hosting Robustness | Planned | [T16 specification](docs/specs/T16-plugin-management.md) | P2. After T12; compatibility target required. |
| T17 — Device Resilience and Preferences | Planned | [T17 specification](docs/specs/T17-device-resilience.md) | P2. After T04; extends T03 device handling. |
| T18 — egui Design System, Commands, and Accessibility | Planned | [T18 specification](docs/specs/T18-ui-foundation.md) | P2. UI docs normalized to Rust/egui on 2026-11-17; Phase 1 (8 primitives) proposed, awaiting approval. |
| T19 — Performance, Scale, and Realtime Safety | Planned | [T19 specification](docs/specs/T19-performance.md) | P2. After T04/T11/T12/T14; measurable targets must be agreed first. |
| T20 — Automated App Regression Harness | Planned | [T20 specification](docs/specs/T20-regression-harness.md) | P2. Build from T04 onward; keep evidence categories separate. |
| T21 — AU Hosting | Planned | [T21 specification](docs/specs/T21-au-hosting.md) | P3. After T12; needs AU validation decision. |
| T22 — Native MIDI/Audio Tools | Planned | [T22 specification](docs/specs/T22-native-tools.md) | P3. After T06/T10/T11; behavior and placement to agree. |
| T23 — Packaging and Cross-Platform Groundwork | Planned | [T23 specification](docs/specs/T23-packaging.md) | P3. After a stable feature set; distribution and platform scope to agree. |
