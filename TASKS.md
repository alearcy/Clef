# Tasks

Each task has a separate specification in `docs/specs/`. Requirements, decisions, progress, and verification evidence belong in that specification or its linked supporting documents. This index lists open work only: when a task is completed, its row is removed and the specification becomes the record of that work.

The post-T03 order was approved on 2026-11-17. Its tasks are `Todo`: their
specifications are planning drafts until the corresponding interview and specification
approval are complete. See the [post-T03 backlog proposal](docs/planning/post-T03-backlog.md)
for the rationale, priorities, and open product decisions.

On 2026-11-20 the user reordered the open work: T15 moves ahead of T13 and becomes P1
because it is the next task after T11/T12, while T13 is deferred to the end, after T23 and
before T29.

Rows follow the dependency chain: a task is placed after the work it extends or depends on,
even when its priority is lower; the note column records priority. The numeric ID is a stable
identifier and is not renumbered when a task is inserted. T26–T29 were added on 2026-11-18 from
decisions already taken in the external Composer DAW project; each specification cites the
reference and remains a planning draft.

## Open work

| Task | Status | Specification | Next step / dependency |
| --- | --- | --- | --- |
| T36 — Audio, CPU, and Memory Audit Fixes | In Progress | [T36 specification](docs/specs/T36-audit-fixes.md) | P1. All fixes and automated evidence landed; GUI (P4, S2) and listening (E1–E3) checks pending. |
| T15 — Buses, Sends, Groups, and Routing | In Progress | [T15 specification](docs/specs/T15-routing.md) | P1. Implementation and automated evidence landed; native bus-chain PDC and manual GUI/listening passes pending. |
| T39 — Always-On Audio and Global Audio Preferences | Todo | [T39 specification](docs/specs/T39-always-on-audio.md) | P1. Spec approved 2026-11-20; two open questions to answer before implementation. |
| T14 — Audio Tracks and Recording | Todo | [T14 specification](docs/specs/T14-audio-recording.md) | P1. After T05 and T11; media-storage decision required. |
| T27 — Audio Comping and Take Lanes | Todo | [T27 specification](docs/specs/T27-audio-comping.md) | P2. After T14 and T26; T14 media-storage decision required. |
| T28 — FX Rack and Virtual Cable Graph | Todo | [T28 specification](docs/specs/T28-fx-rack.md) | P2. After T12 and T15; audio and MIDI nodes connected by virtual cables. |
| T16 — Plugin Management and Hosting Robustness | Todo | [T16 specification](docs/specs/T16-plugin-management.md) | P2. After T12; compatibility target required. |
| T17 — Device Resilience and Preferences | Todo | [T17 specification](docs/specs/T17-device-resilience.md) | P2. After T04; extends T03 device handling. |
| T18 — egui Design System, Commands, and Accessibility | Todo | [T18 specification](docs/specs/T18-ui-foundation.md) | P2. UI docs normalized to Rust/egui on 2026-11-17; Phase 1 (8 primitives) proposed, awaiting approval. |
| T19 — Performance, Scale, and Realtime Safety | Todo | [T19 specification](docs/specs/T19-performance.md) | P2. After T04/T11/T12/T14; measurable targets must be agreed first. |
| T20 — Automated App Regression Harness | Todo | [T20 specification](docs/specs/T20-regression-harness.md) | P2. Build from T04 onward; keep evidence categories separate. |
| T21 — AU Hosting | Todo | [T21 specification](docs/specs/T21-au-hosting.md) | P3. After T12; needs AU validation decision. |
| T22 — Native MIDI/Audio Tools | Todo | [T22 specification](docs/specs/T22-native-tools.md) | P3. After T06/T10/T11 and T28; rack placement, behavior to agree. |
| T23 — Packaging and Cross-Platform Groundwork | Todo | [T23 specification](docs/specs/T23-packaging.md) | P3. After a stable feature set; distribution and platform scope to agree. |
| T13 — Export and Bounce | Todo | [T13 specification](docs/specs/T13-export.md) | P1, deferred to the end on 2026-11-20; after T23 and before T29, which needs its render path. |
| T29 — ARA 2 Hosting and Audio Part Editing | Todo | [T29 specification](docs/specs/T29-ara2-hosting.md) | P3. After T14, T13, and T16; ARA host probe gates implementation. |
