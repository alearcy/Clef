# Tasks

Each task has a separate specification in `docs/specs/`. Requirements, decisions, progress, and verification evidence belong in that specification or its linked supporting documents. This index lists open work only: when a task is completed, its row is removed and the specification becomes the record of that work.

The post-T03 order was approved on 2026-11-17. Its tasks are `Todo`: their
specifications are planning drafts until the corresponding interview and specification
approval are complete. See the [post-T03 backlog proposal](docs/planning/post-T03-backlog.md)
for the rationale, priorities, and open product decisions.

Rows follow the dependency chain: a task is placed after the work it extends or depends on,
even when its priority is lower; the note column records priority. The numeric ID is a stable
identifier and is not renumbered when a task is inserted. T26–T29 were added on 2026-11-18 from
decisions already taken in the external Composer DAW project; each specification cites the
reference and remains a planning draft.

## Todo

| Task | Status | Specification | Next step / dependency |
| --- | --- | --- | --- |
| T06 — MIDI Recording and Clip Playback | Todo | [T06 specification](docs/specs/T06-midi-recording.md) | P0. After T05. |
| T07 — Autosave, Crash Recovery, and Recent Projects | Todo | [T07 specification](docs/specs/T07-autosave-recovery.md) | P1. After T06; protect recorded work. |
| T08 — Undo/Redo and Edit History | Todo | [T08 specification](docs/specs/T08-undo-redo.md) | P1. After T06; before arrangement and piano-roll editing. |
| T09 — Arrangement and Clip Editing | Todo | [T09 specification](docs/specs/T09-arrangement-editing.md) | P1. After T06 and T08. |
| T10 — Piano Roll and MIDI Editing | Todo | [T10 specification](docs/specs/T10-piano-roll.md) | P1. After T06 and T08. |
| T26 — MIDI Comping and Take Lanes | Todo | [T26 specification](docs/specs/T26-midi-comping.md) | P2. After T10; extends the MIDI take and comp model shared with T27. |
| T11 — Mixer and Master Section | Todo | [T11 specification](docs/specs/T11-mixer.md) | P1. After T04. |
| T12 — Insert Effects Hosting | Todo | [T12 specification](docs/specs/T12-effect-hosting.md) | P1. After T04 and T11. |
| T13 — Export and Bounce | Todo | [T13 specification](docs/specs/T13-export.md) | P1. After T11; include T12 effects if present. |
| T14 — Audio Tracks and Recording | Todo | [T14 specification](docs/specs/T14-audio-recording.md) | P1. After T05 and T11; media-storage decision required. |
| T27 — Audio Comping and Take Lanes | Todo | [T27 specification](docs/specs/T27-audio-comping.md) | P2. After T14 and T26; T14 media-storage decision required. |
| T15 — Buses, Sends, Groups, and Routing | Todo | [T15 specification](docs/specs/T15-routing.md) | P2. After T11 and T12; not required for first mixes. |
| T28 — FX Rack and Virtual Cable Graph | Todo | [T28 specification](docs/specs/T28-fx-rack.md) | P2. After T12 and T15; audio and MIDI nodes connected by virtual cables. |
| T16 — Plugin Management and Hosting Robustness | Todo | [T16 specification](docs/specs/T16-plugin-management.md) | P2. After T12; compatibility target required. |
| T17 — Device Resilience and Preferences | Todo | [T17 specification](docs/specs/T17-device-resilience.md) | P2. After T04; extends T03 device handling. |
| T18 — egui Design System, Commands, and Accessibility | Todo | [T18 specification](docs/specs/T18-ui-foundation.md) | P2. UI docs normalized to Rust/egui on 2026-11-17; Phase 1 (8 primitives) proposed, awaiting approval. |
| T19 — Performance, Scale, and Realtime Safety | Todo | [T19 specification](docs/specs/T19-performance.md) | P2. After T04/T11/T12/T14; measurable targets must be agreed first. |
| T20 — Automated App Regression Harness | Todo | [T20 specification](docs/specs/T20-regression-harness.md) | P2. Build from T04 onward; keep evidence categories separate. |
| T21 — AU Hosting | Todo | [T21 specification](docs/specs/T21-au-hosting.md) | P3. After T12; needs AU validation decision. |
| T29 — ARA 2 Hosting and Audio Part Editing | Todo | [T29 specification](docs/specs/T29-ara2-hosting.md) | P3. After T14 and T16; ARA host probe gates implementation. |
| T22 — Native MIDI/Audio Tools | Todo | [T22 specification](docs/specs/T22-native-tools.md) | P3. After T06/T10/T11 and T28; rack placement, behavior to agree. |
| T23 — Packaging and Cross-Platform Groundwork | Todo | [T23 specification](docs/specs/T23-packaging.md) | P3. After a stable feature set; distribution and platform scope to agree. |
