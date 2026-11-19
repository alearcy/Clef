# T07 — Autosave, Crash Recovery, and Recent Projects

**Status:** Done — 2026-11-19.
**Priority:** P1.
**Date:** 2026-11-17
**Last updated:** 2026-11-19
**Task:** [T07](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T04 schema v2; T06 recorded content.

## Goal

Protect user work after recording becomes possible: crash-safe periodic autosave, recovery
on relaunch, bounded backups, and a recent-projects/startup surface. A failed write must
never damage the last valid project file.

## Scope

- Periodic autosave to a crash-safe location, using the existing atomic publish rules.
- Recovery prompt on next launch with explicit recover/discard choices.
- Bounded backup retention and cleanup.
- Recent-projects/startup surface with clear missing-file and incompatible-file states.
- An explicit plugin-state capture policy for autosave.
- Defined interaction with T03's Save / Discard / Cancel contract.

## Out of Scope

Cloud sync, version-control integration, full project history, and automatic conflict
resolution across machines.

## Agreed Behavior

- Autosave is always on. Checkpoint dirty project content every 60 seconds and request a
  checkpoint as soon as a MIDI take is finalized. Saved and untitled projects are eligible.
  Keep the three newest valid recovery generations per project in Clef's Application Support
  directory. Use the project's atomic publication mechanism and rotate only after a new
  generation has been published. The explicit project file is written only by Save or Save As.
- Autosave includes committed project edits and finalized MIDI takes. During playback or an
  active take it may persist the last committed content. It leaves transport and editors alone
  and uses the newest retained plugin state already available to the session. Changes made
  after that state was acquired and the in-progress take may be lost in a crash. Explicit Save
  keeps its coherent plugin capture behavior. Autosave work stays off the device callback and
  the UI thread.
- On startup, report available recovery before ordinary project selection. Recover opens an
  unsaved copy and routes Save to Save As. Retain its recovery data until a successful explicit
  save or deliberate discard. Keep the original project available. When several sessions have
  recovery data, show each separately and act only on the selected session. Later postpones the prompt
  and keeps the recovery data accessible on the chooser. If a generation fails validation,
  offer the newest valid fallback and show its timestamp. If no valid version remains, explain
  the failure, allow opening another project, and require an explicit discard to remove the data.
- Keep the ten most recently opened or explicitly saved project paths across launches. A missing
  path can be relinked to a verified project or removed. An incompatible file shows the reason
  and keeps Open disabled. A readable, read-only file can open and routes Save to Save As.
- A successful explicit Save or Save As retires the open session's recovery generations. In
  the existing Save / Discard / Cancel flow, Discard retires them when the transition succeeds.
  Cancel and failed saves or opens preserve the generations. T08 undo history starts empty
  after recovery.

## Startup and Recovery UI Proposal

This proposal uses the [Startup](../UI/views/startup.md) view, Composer Dark
[foundations](../UI/foundations.md), and the [surface contract](../UI/contract.md).
Use the centered startup panel and shared spacing, typography and focus roles. Keep the
selected name, path, last-opened date and actionable status together in the details area.

```text
surface startup (review at 1024 and 760 points)
  centered startup panel
    column
      heading("Clef")
      row fill
        column scroll: heading("Recent Projects") + list rows
        column wrap: selected project name, path, date and status
      row wrap: Recovery… (when pending), Remove, Locate…, Open, Open…, New Project

surface recovery (modal over startup)
  column
    heading("Recover Unsaved Work?")
    list rows (when multiple): project and checkpoint time, newest first
    caption: selected project, latest usable checkpoint time and optional original path
    body wrap: Save As behavior and any recovery limits or read error
    row wrap: Later, Discard…, Recover
```

| Primitive | Role and states |
| --- | --- |
| [Caption and heading](../UI/components/caption-and-heading.md) | Title, project details, empty recent list and readable project or recovery status. |
| [List row](../UI/components/list-row.md) | Recent entry with normal, hover, focused, selected and unavailable states. Use a textual status as well as color. |
| [Text button](../UI/components/text-button.md) | Bind project, recent and recovery actions through the shared command path. Use one primary action per surface. |
| Modal container | Use the same container for recovery and discard confirmation. Give the dialog a keyboard focus boundary and return focus to the chooser on dismissal. Document and verify the primitive as part of implementation. |

At 760 points, stack selected details below the scrolling rows and wrap footer actions.
Long paths wrap in details and remain available in full through the accessible row name.
Arrow keys select recent rows, Return opens the selected readable project, and Escape in
the recovery dialog acts as Later. Unavailable entries remain selectable for status and
repair actions. Hide inapplicable repair actions. A recovery with no readable checkpoint
keeps Recover disabled and shows the reason. Discard requires a separate confirmation.
This proposal leaves the workspace and plugin editor layouts outside T07.

## Acceptance Checks

1. After a take is finalized, forcibly terminate Clef after its checkpoint and recover the take
   from named and untitled projects. Verify the original file is unchanged before an explicit
   save and that the recovered copy follows Save As.
2. Inject a checkpoint write failure and terminate during publication. Verify the last valid
   checkpoint remains usable, normal Save is intact, and successful checkpoints rotate to three.
   Verify fallback to an older valid generation and handling when all generations are unusable.
3. Verify playback, recording and an open plugin editor continue across a checkpoint. Verify
   recovery of retained plugin state and the documented boundary for unfinalized takes.
4. Exercise Save, Discard, Cancel, failed Save, Later and recovery relaunch. Verify that recovery
   remains available until a successful explicit save or discard.
5. Verify recent ordering, deduplication and the ten-entry cap across relaunch. Exercise missing,
   relinked, removed, incompatible and read-only entries.
6. Verify keyboard navigation, accessible action names and layout at 1024 and 760 points, with
   linked gallery and surface evidence under the UI contract.

## Decisions and Implementation

On 2026-11-19 the user approved the 60-second and take-finalization triggers, retention,
use of retained plugin state, and the proposed startup, recovery and recent-project flow.

Recovery data lives in `~/Library/Application Support/Clef/Recovery/`. Checkpoints reuse the
existing `.clef` project format and atomic publication mechanism. Each open session owns its
journal through a file lock. Concurrent sessions for the same project keep separate recovery
copies so one session cannot retire the other's work. A worker serializes one checkpoint at a
time from committed session data. If a take completes during a write, its checkpoint becomes
eligible as soon as the writer is free. A process terminated before publication leaves the
previous generation available. Startup reconciles an interrupted rotation when there are
extra copies. Journal retirement happens after successful Save or on confirmed discard.

Recent projects use an atomic local index of canonical paths, with inspection on launch.
Startup, recovery, and discard use the [Clef startup view](../UI/views/startup.md) and the
[project dialog](../UI/components/project-dialog.md). Recovery leaves the original available
and suggests a separate file name in Save As.

## Verification — 2026-11-19

- The recovery tests terminated an isolated child process while it was writing an unpublished
  checkpoint. Sessions with and without an original file recovered the finalized MIDI; the
  original remained unchanged. Tests also exercised an injected publication failure, rotation,
  interrupted rotation, fallback, unusable copies and exclusion of a live journal.
- Session and UI tests exercised the 60-second schedule, immediate take trigger, retained plugin
  state, failed open and Save, explicit Save As, discard before close, Cancel, Later and read-only
  Save As. The recent-path test verified ordering, deduplication, the ten-entry limit, restart,
  and missing, relinked, removed, incompatible and read-only entries.
- The explicitly selected `native_t06_records_routes_reopens_and_plays_an_instrument_clip`
  check passed with the rebuilt TestSynth VST3, CoreAudio and a virtual CoreMIDI source.
  With the original TestSynth editor open during an active take, a periodic checkpoint was
  queued and recording continued. The next checkpoint included the finalized takes and the
  editor and audio remained active. This verified native operation through the session, with
  virtual MIDI input and a single test plugin.
- The startup gallery produced 22 [renders](../UI/snapshots/egui/) at 1024 × 640 and 760 × 640.
  The UI tests checked AccessKit names, disabled controls, bounds at both widths, and arrow and
  Return operation. The [startup view](../UI/views/startup.md) lists the rendered states.
- `cargo check`, `cargo build`, `cargo test`, formatting and Clippy passed with the locked
  offline dependencies. The default suite passed 86 tests and left 10 native checks ignored.
  The selected CoreMIDI/CoreAudio check was run separately.
