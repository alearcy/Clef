# T01 — DAW Engine Foundation Evaluation

**Status:** Done — foundations adopted 2026-11-17.
**Date:** 2026-11-16
**Task:** [T01](../../TASKS.md)

## Goal

Choose an evidence-backed engine foundation for a personal, custom DAW written in Rust with an egui interface. Compare [maolan-engine](https://github.com/maolan/maolan) and [SoundCraft](https://github.com/storytold/soundcraft), including suitable external libraries that can fill their gaps. Do not inherit another DAW's feature scope.

**Scope extension (2026-11-16):** The user approved deeper evaluation of individual low-level libraries and a Clef-owned engine versus Maolan reuse. The [extended evaluation](../../reports/Engine%20ownership%20and%20library%20selection.md) addresses this alternative and supersedes the initial validation ranking. [T02](T02-engine-validation.md) is planned to provide end-to-end evidence; its implementation requires the remaining interview and specification agreement.

## Agreed Interview Requirements

- Support composition with virtual instruments, audio recording, and mixing. Virtual instruments have first priority.
- Robust hosting of external instruments and effects is the priority. Developing an internal plugin suite or exporting Clef's tools as plugins is outside the current scope.
- Native tools such as arpeggiators and M/S encoders/decoders belong to MIDI/audio processing and routing workflows. Do not assume they must be inserted into conventional plugin insert/send slots. Their exact behavior, placement, ordering, and interface require later specifications.
- Target macOS initially while preserving a credible path to future cross-platform builds.
- Prioritize VST3 and CLAP equally, without a strict order between them; AU follows later. Identify platform restrictions and distinguish formats available now from those needing integration or development.
- Rust is preferred rather than mandatory for every component. A limited C++/Objective-C bridge is acceptable where existing native services provide a concrete advantage; assess its ownership, threading, deployment and licensing cost.
- Give greatest attention to latency, plugin hosting, rendering, and routing.
- Open-source licensing is acceptable. Explain the actual licenses and their implications for integration and distribution; do not assume every license combination is compatible.
- Prefer existing, proven components over writing replacements. Custom development is acceptable where justified.
- Consider external libraries alongside either engine to address missing capabilities before recommending implementation from scratch.

## Evaluation Scope

For each candidate, inspect current primary sources and relevant implementation paths. Record source links, inspected revisions when available, and the inspection date.

1. **Maturity and maintenance:** release history, activity, tests, documented usage, known limitations, and evidence of stability. Activity alone does not establish reliability.
2. **Real-time safety and latency:** audio callback ownership, allocation and blocking risks, UI/engine communication, buffer and device latency handling, plugin latency reporting, and delay compensation across routed paths.
3. **Plugin hosting:** instrument and effect support, AU/VST3/CLAP coverage, MIDI and event timing, native editor integration with egui on macOS, state persistence, scanning, and failure handling.
4. **Rendering:** real-time and offline processing, export support, handling of plugin latency and tails, and constraints imposed by plugins.
5. **Routing:** instruments, audio tracks, buses, sends, sidechains, graph ordering, channel layouts, cycle handling, and interactions with delay compensation. These are comparison dimensions, not an approved application feature backlog.
6. **DSP and platform foundations:** audio and MIDI I/O, sample-rate handling, existing DSP dependencies, macOS support, portability constraints, and Rust/egui integration boundaries.
7. **Licensing and integration cost:** engine and relevant dependency licenses, reusable components, architectural constraints, and the maintenance cost of proposed integrations.

For material gaps, assess fitting external components using the same evidence standard. Keep this search focused on gaps in the priority areas; do not produce an unrelated library catalog.

## Evidence and Deliverable

Produce an English evaluation containing:

- A side-by-side comparison tied to the interview requirements.
- A clear distinction between implemented capabilities supported by source inspection, upstream claims, runtime observations, and unresolved gaps.
- An integration outline for each viable option: responsibilities supplied by the engine, external libraries, and custom code.
- Material risks, licensing constraints, unknowns, and any focused experiments needed before adoption.
- A justified recommendation, which may conclude that neither candidate is ready for adoption.

Source inspection and upstream tests do not establish local runtime performance. Report automated, GUI, listening, and physical-hardware evidence separately. Do not claim measured latency or plugin compatibility without the corresponding checks.

## Acceptance Criteria

- Both named candidates are evaluated against the priority workflow and critical engine responsibilities.
- Material conclusions cite current primary evidence and disclose uncertainty.
- Missing functionality has a concrete disposition: existing external component, justified custom work, or unresolved adoption risk.
- The recommendation explains the tradeoff between reuse, integration effort, reliability evidence, and future portability.
- The user reviews the results and explicitly agrees on the DSP, plugin-hosting, and engine foundations before adoption.
- The specification, evaluation, and final decision are linked from `TASKS.md`. T01 remains incomplete until the foundation decision is agreed.

## Boundaries and Open Questions

- This task evaluates foundations; it does not implement the application, adopt dependencies, or select the complete product feature set.
- Plugin-authoring frameworks are not solutions to external hosting gaps. The engine evaluation must distinguish external plugin lifecycle/editor/state support from the ability to integrate native MIDI/audio routing tools; those tools are not authorized for implementation by this evaluation.
- No fixed latency target, plugin test matrix, workload size, or hardware acceptance threshold has been agreed. Do not invent these values. If experiments become necessary for the decision, agree on their scope and thresholds first.
- VST3 and CLAP are joint first priorities and AU is later. Actual plugins, coverage within T02 and first-release completeness remain undecided; do not ask the user to choose the format priority again.
- egui remains the UI direction. Rust is preferred, with a limited native bridge acceptable for a concrete advantage. Engine, DSP, and hosting choices remain open.
- Codebase graph generation is pending because application source code does not yet exist.

## Approval and Decision

- Interview completed: 2026-11-16.
- Specification approved: 2026-11-16, following the user's “si procedi”.
- Scope clarified by the user: 2026-11-16. Prioritize robust external plugin hosting; native arpeggiator and M/S tools are processing/routing tools, with no internal plugin suite required for now.
- Evaluation: [DAW engine foundation comparison](../../reports/DAW%20engine%20foundation%20comparison.md), 2026-11-16.
- Extended investigation approved: 2026-11-16, following the user's request to deepen individual-library choices versus Maolan; VST3 and CLAP first, AU later, Rust preferred but not mandatory.
- Extended evaluation: [Engine ownership and library selection](../../reports/Engine%20ownership%20and%20library%20selection.md), 2026-11-16. Pinned helper inspection changes the earlier macOS-hosting assessment; truce-rack 1.1.5 is rejected as the initial hosting shortcut.
- Foundation decision: pending user review and explicit agreement. Current recommendation is a conditional Clef-owned validation path, with Maolan as the reuse baseline, not final adoption. No prototype or dependency adoption has occurred.

## Evaluation Record — 2026-11-16

- Initial source review: Maolan engine `7e7694a915fe7f4f6b927363af68af924a2547bb` (0.4.22), its application `1ec0ea0c4072d15d05ce972b12016eb0e954a9ed`, and SoundCraft `648ed9db9a0e98cc55643babffac381521a556c8` (workspace 0.20.1). Covered latency/PDC, hosting, rendering, routing, macOS portability, licensing, integration, and focused external components.
- Extended review: pinned library sources and Maolan helper `19da7755824aca87f43f9a3d32e040bcb7316e5d` (0.1.3). Native macOS VST3/CLAP editors are missing in the inspected helper; truce-rack 1.1.5 has critical host omissions. See the linked extended evaluation for evidence and ownership tradeoffs.
- The extended evaluation supersedes the initial Maolan-first validation recommendation. Conditionally validate a Clef-owned core using Clack, a deliberately selected VST3 backend, CPAL output, and only required support libraries; compare total ownership against Maolan reuse. This is a research recommendation, not dependency or foundation adoption.
- Evidence: read-only primary-source and pinned-code inspection. Configured upstream tests were inspected, not run locally. Document links and requirement coverage were checked locally.
- Limits: no local build, automated runtime tests, GUI/editor checks, listening checks, or hardware measurements. No dependencies were added. T02 remains subject to its interview and specification agreement; its results will inform the final T01 decision.

## Additional T02 Candidate Evidence — 2026-11-16

The user approved the general T02 plan and requested evaluation of `rust-vst3-host` as a VST3 alternative. The [candidate report](../../reports/T02%20rust-vst3-host%20evaluation.md) records pinned source review, 238 passing unit tests, isolated TestSynth/ZebraHZ/Pianoverse smoke checks, assisted Pianoverse AA Y7 editor loading, and a verified local fix for stale editor values corrupting fresh-worker realtime restoration. The patched VST3 backend remains a proposed T02 amendment awaiting agreement. This is additional evidence, not final foundation adoption or completed T02 acceptance.

## T02 Validation Evidence — 2026-11-17

The user approved T02 and then the SDK bridge amendment after measured contention
in the Rust VST3 backend. Implementation now exists; earlier no-prototype/no-source
statements describe the evaluation date only. Current evidence, failed attempts,
and remaining acceptance gates are consolidated in the
[T02 verification](T02-engine-validation.md#completion--2026-11-17). Physical MIDI,
state restoration, actual format-latency fixtures and controlled failure recovery
have evidence; scheduled/editor and final export/listening gates remain open.
Final foundation adoption still requires the user's explicit agreement and is not
implied by approval to implement and complete T02.

## Completed T02 Evidence — 2026-11-17

T02 completed the agreed acceptance matrix, including three ten-minute scheduled
runs, three five-minute physical MIDI runs, original editor/state/resize checks,
format-specific changing latency, deterministic PDC, explicit crash/hang recovery
and final offline exports accepted by the user. The
[T02 verification](T02-engine-validation.md#completion--2026-11-17) records configuration,
raw reports, failures, final checks, native-bridge ownership and limits. This is
positive end-to-end evidence for the Clef-owned Rust/egui path in that configuration.
T01 remains pending the user's separate final foundation decision; no additional
implementation scope or general compatibility is inferred.

## Final Foundation Decision — 2026-11-17

The user explicitly approved final adoption alongside the T03 specification.
Adopt the T02-validated Clef-owned Rust core, egui/eframe application UI, CPAL audio,
midir MIDI, Clack CLAP hosting and narrow SDK-backed VST3 bridge, with isolated
plugin workers and the established routing/PDC and realtime safety boundaries.
This supersedes pending-decision statements in historical entries above.
Compatibility and performance evidence remains limited to the T02 tested matrix.
