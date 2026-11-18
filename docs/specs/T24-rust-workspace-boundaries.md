# T24 — Rust Workspace Boundaries

**Status:** Done. Approved and verified on 2026-11-18.  
**Task:** [T24](../../TASKS.md)

## Goal

Organize Clef's Rust code by ownership and use a short, English `AGENTS.md` for each
crate. Preserve the existing application, project format, native bridge, and
validation workflows through the refactor.

## Proposed workspace

| Package | Owns | Local guidance |
| --- | --- | --- |
| `clef` (root binary) | App, worker, scanner and diagnostic entry points and their wiring | Root instructions and existing CLI/build contracts |
| `clef-model` | Tracks, project persistence, stable plugin identity and schema migration | Data compatibility and dependency boundaries |
| `clef-engine` | Scheduling, mix, routing, latency compensation and callback-facing state | Bounded realtime work, ownership and resource retirement |
| `clef-audio-io` | CPAL and MIDI enumeration, stream setup and device callbacks | Timestamps, preallocated handoffs and device changes |
| `clef-plugin-host` | Worker dispatch, IPC, scanning, CLAP and the SDK-backed VST3 integration | Isolation, timeouts, plugin lifecycle and ABI ownership |
| `clef-session` | Coordination of model, engine and host, including lazy loading, save and recovery | Session transitions, cancellation, quarantine and state consistency |
| `clef-ui` | Clef-owned egui application views and interaction | The existing UI contract, including dense views, repaint, cache and GPU profiling |

Keep CLAP and VST3 in separate modules inside the plugin host. Preserve the existing
Steinberg SDK bridge and its local native guidance. The plugin host may use eframe
for original editor windows in the worker. The model and engine have no egui
dependency. Keep peak generation and AU hosting within their future task scopes.

## Extraction order and dependency constraints

1. Capture a baseline with the current binary, tests and fixture build instructions.
2. Extract model and persisted plugin identity. Preserve identity fields and bytes on
   disk. The catalog supplies model values without introducing a model-to-host edge.
3. Extract host/IPC and native build ownership. Pass the executable path from the
   composition root. Keep the binary name, child flags and test discovery stable.
   Move the catalog's repaint notification to its UI caller.
4. Separate device setup from the engine's callback-facing logic. Establish one
   dependency direction without making the mixer responsible for CPAL discovery or
   device configuration. Share existing frame and event types through the crate
   that owns their contract and avoid a general utility crate.
5. Extract session and UI. The UI sends existing commands and reads session views.
   Maintain the current direction of dependencies and keep the root binary thin.
6. Move area-specific instructions to the owning crate. Give each rule one home and
   use short links to the existing UI contract or task specs for deeper detail.

Keep intermediate states buildable. Stop and revisit the boundary if an extraction
would create dependency cycles or require behavior changes.

## Acceptance

- Cargo enforces the ownership boundaries and the root binary retains its existing
  package name, entry points, bundle output and worker/scanner launch behavior.
- Version 1 and version 2 projects read under the same rules. Round trips preserve
  identifiers, settings, track order and opaque plugin state. Existing save failure
  and recovery tests continue to pass.
- Realtime callback bounds, counting-allocator proof, deterministic mix/PDC and
  worker failure isolation continue to pass. VST3 still links the native SDK bridge.
- Run locked check/build, formatting, tests and Clippy across the Clef packages.
  Rebuild and bundle TestSynth and the latency fixtures before the selected native
  regressions. Verify app launch and original editor attachment through eframe.
  Record automated, GUI and any hardware evidence separately in this spec.
- The existing T04 physical-keyboard/listening acceptance remains tracked in T04.
  This task introduces no UI redesign, dependency upgrade, routing feature or project
  format change.

## Implementation and verification

- The six libraries are Cargo default workspace members alongside the root
  binary. Each has a short English `AGENTS.md`. The root keeps cross-cutting rules.
  Current UI documentation and planned T18 paths reflect the new workspace.
- The executable passes its path to session and scanner launches. The plugin host
  owns the SDK build, both format adapters and eframe worker editors. The engine
  consumes host IPC types and has no direct eframe dependency. CPAL and MIDI input
  setup live in audio I/O. The root owns CLI dispatch and console orchestration.
- Locked check, build, tests, formatting check, and Clippy with warnings denied
  passed across the default workspace members. The baseline and final suites both
  record 53 ordinary passes and nine tests ignored by default. Cargo metadata
  confirms the internal dependency directions and SDK bridge path.
- After rebuilding and bundling TestSynth and both latency fixtures, the complete
  explicit native run passed nine tests. It covered the scanner, worker startup,
  original editor attachment, state round trips, eight tracks, fault isolation,
  and actual VST3/CLAP latency notifications. An initial invocation stopped at
  the two T02 tests. Both passed in a focused repeat and in the complete rerun.
  The latency-notification run reported missed and stale blocks while meeting its
  compensation and worker-lifecycle assertions.
- The rebuilt bundle matched the debug executable byte for byte. The actual
  application opened its untitled window and exposed the expected controls through
  accessibility. There was no new manual listening or physical-keyboard trial.
- Graphify was refreshed before and after the extraction. Its code graph retains
  the existing partial SDK parsing and Objective-C++ extraction limitations.
