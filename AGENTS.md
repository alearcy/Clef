# Repository Guidelines

## Project Direction & Language

Build a personal, custom DAW using **Rust + egui**. Define product behavior with the user; do not assume a Pro Tools clone or inherit another DAW’s feature scope.

Communicate with the user in **Italian**, including interviews. Write documentation, specifications, skills, code comments, interface text, and other project artifacts in **English**.

## Spec-Driven Workflow

Record all tasks, their status, and links to specifications in `TASKS.md`. Keep `AGENTS.md` limited to repository guidelines; do not put task descriptions or a backlog here.

Give every task its own English Markdown specification in `docs/specs/`, linked from `TASKS.md`, including planning drafts. Keep requirements, scope, acceptance criteria, open questions, decisions, progress details, and verification evidence in that specification or linked supporting documents, not inline in `TASKS.md`.

Keep `TASKS.md` a concise index of open work only: task ID and title, `Todo` or `In Progress` status, specification link, and at most a short dependency or next-step note. Do not keep a Done list or completed tasks in the index; when a task is completed, remove its row. The task specification in `docs/specs/` is the completion record, so keep the completion date, history, and verification evidence there.

Before each development task, interview the user in Italian about goals, workflows, constraints, edge cases, and acceptance criteria. Do not replace unresolved product decisions with assumptions.

Write the agreed requirements into an English task specification before implementation. Obtain agreement on the specification, then implement and verify against it. Keep specifications current and record unresolved questions explicitly.

Agree with the user on DSP, plugin hosting, and engine foundations before adopting them.

## Realtime and Plugin Isolation

The device callback and its callees must do bounded work without allocation, final release of heap
memory, locks, waits, text formatting, logging, or I/O. Prepare resources and reclaim retired
state off the callback. Keep plugin processing and lifecycle operations behind isolated workers.
Use bounded handoffs that let the callback continue when a worker is late or unavailable.

## Project Structure & UI Authority

The `clef` binary entry points are in `src/`. Libraries in `crates/` have local `AGENTS.md` files. Read the ones that own the behavior being changed. Focused unit tests live beside the code. The root `Cargo.toml`, `Cargo.lock`, and `rust-toolchain.toml` define the workspace. Reviewed third-party source is in `vendor/`, with provenance and local patches recorded separately.

Clef hosts VST3 through its Steinberg SDK bridge. Read [native/AGENTS.md](native/AGENTS.md) for bridge and platform ABI changes, including changes to their Rust callers.

Build Clef-owned windows and controls with **egui/eframe**. Do not construct application windows or controls directly with AppKit, `NSWindow`, or other native macOS UI libraries. Host original plugin editors through the egui windowing path; isolate any unavoidable plugin-ABI embedding glue from the application UI. Native plugin editors do not authorize a native macOS application UI.

For UI tasks, start from `docs/UI/index.md`. It indexes every component and view; use it to locate the few files a change touches and do not read the whole `docs/UI/` tree. Then read `docs/UI/README.md` for the binding rules, `docs/UI/foundations.md` for the tokens, and `docs/UI/contract.md` for the primitive and surface contract, the gallery gate, and the rules for agents. Follow their design, behavior, accessibility, and proposal approval rules. Update UI documentation, and its index entry, with implementation.

Use `docs/UI/` as the design authority for Clef. Do not use the Impeccable skill for Clef UI work.

These references contain legacy C++/JUCE/Tracktion paths and symbols. That code is not in this repository and is reference-only: translate the design into Rust + egui, never treat its paths, symbols, or classes as APIs, and never add JUCE, Tracktion, or native AppKit UI code. Resolve conflicts with the user.

## Mandatory Graphify Workflow

Use Graphify for every development task once source code exists. At task start, generate the graph if missing; otherwise update it before analysis and development. Consult it to trace affected modules and callers, then verify relevant source directly. Refresh it after task changes.

Follow the installed Graphify skill; keep outputs in `graphify-out/`. Until source exists, report that codebase graph generation is pending.

## Commands, Testing & Contributions

Always prefix shell commands with `rtk`; use `rtk proxy <command>` where needed. Read `/Users/arcy/.codex/RTK.md` first.

Use `rtk cargo check --locked`, `rtk cargo build --locked`, `rtk cargo test --locked`, `rtk cargo fmt --all --check`, and `rtk cargo clippy --all-targets --locked -- -D warnings`. Cargo's default members cover the Clef packages. Build the vendored TestSynth explicitly before selecting its ignored native regression; see `patches/vst3-host/README.md`. Add focused behavioral tests with implementation; distinguish automated, GUI, listening, and hardware evidence. Use descriptive names and consistent Rust conventions. No Git history is available to infer commit style: use concise imperative subjects. Pull requests should explain purpose, verification, relevant specifications, and UI screenshots where applicable.
