# T08 — Undo/Redo and Edit History

**Status:** Done — 2026-11-19.
**Priority:** P1.
**Date:** 2026-11-17
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T04 schema v2; T06 clip/note model. Must land before T09 and T10.

## Goal

Provide command-based undo and redo for project edits, with predictable granularity, a
bounded memory budget, and integration with the shared command registry. This is required
before arrangement and piano-roll editing become safe.

## Out of Scope

Full project version history, branching, collaboration, and file-level versioning. Audio
waveform editing undo is out of scope until T14 defines that editing model.

## Agreed Behavior — 2026-11-19

### First undoable edits

- Cover track creation, removal, reordering and committed renames, completed MIDI
  recordings, tempo and meter edits, and committed loop range changes. Restore a
  removed track at its original position with its ID, instrument and MIDI material.
- Use one entry per visible edit. A completed recording from Record through Stop
  is one entry, including multiple loop passes. Preview updates, canceled gestures,
  empty recordings and unchanged values leave history alone. A rename committed
  implicitly by Save or track selection is still a rename.
- Include assigning and replacing a track's instrument. Create the entry only
  after the replacement has loaded and the project has adopted it. Preserve the
  previous instrument identity and state before releasing its worker. A failed
  replacement leaves the current instrument and journal intact. Undo and redo
  restore the appropriate instrument identity and state on that track.
- Include changes within VST3 and CLAP editors where the host can capture a
  coherent opaque state before and after editing. A reliable per-parameter
  history requires gesture and replay support from the host integration. The
  initial supported unit is one editor visit when the worker can be snapshotted
  without interrupting audio. In T08, open and close the editor with audio disabled
  for this capture. Preserve the existing unsaved-state protection where snapshots
  are unavailable.
- Exclude audio/MIDI device settings, record arming, transport navigation and
  transport preferences from this first slice. Later clip, note, mixer and routing
  actions can add their own entries when those workflows land.

### History lifetime and runtime

- Keep a linear, session-local history. Save and Save As retain undo and redo and
  mark the saved revision. Returning to it clears the edit-related dirty state.
  Unsaved changes outside the journal still count toward the project's dirty state.
  New, Open and Recovery start a new journal on success. Failed transitions leave
  the current journal intact. Autosave checkpoints the current project revision.
- Enable both actions only when transport is stopped, including when audio is
  disabled. Disable them during playback, count-in, recording, pending transport
  transitions and instrument loads. A successful new undoable edit after undo
  drops the redo branch. Unrelated non-journaled changes leave compatible entries
  usable. Changes that invalidate an entry must prevent replay of the stale edit.
- Bound retained history to 100 entries and 64 MiB of retained edit data. Evict
  oldest entries first. An edit that cannot be kept within the byte budget remains
  applied, clears history and reports why. Keep the resulting MIDI when an oversized
  recording completes.
- Keep worker lifecycle operations on the existing session/worker path. Before
  removing a track with unsaved instrument changes, preserve its latest state.
  If that state cannot be captured, report the failure before removing the track.
  Undo can restore retained state for an unavailable plugin without starting it
  or bypassing quarantine. Unrelated live instrument state must survive undo/redo.
  When a replay step would overwrite instrument state that changed outside the
  journal, preserve the state and reject the jump. Close the affected editor before
  replaying steps that would replace its worker.

### Provisional Edit menu and History window

The current session has a project command row and Commands / Keyboard Shortcuts.
Use an approved provisional egui Edit popup in that row under the
[UI contract](../UI/contract.md). Follow the
[Menus](../UI/components/menus.md) command projection and menu states.

```text
surface multitrack-session
  row wrap: project commands + Edit
    popup anchored to Edit
      menu row: Undo [edit name]      live shortcut
      menu row: Redo [edit name]      live shortcut
      menu row: Edit History…
  window: Edit History
    slider: revision from oldest retained state to newest state
    status: current edit or previewed edit
    scroll column: history rows in chronological order
```

| Element | Role | States |
| --- | --- | --- |
| Text button | Open Edit in the existing project command row | Rest, hover, keyboard focus, open |
| Menu and command rows | Show Undo, Redo and Edit History from the shared registry | Enabled, highlighted, focused, disabled |
| egui window frame | Open Edit History as a non-modal Clef window | Closed, open, focused |
| Discrete slider | Choose a retained revision | Empty, current, dragging, focused, disabled |
| List rows and caption | Identify retained revisions, with the current revision marked and the preview labeled | Empty, current, available, preview, replay error |

Default shortcuts are Command-Z and Command-Shift-Z on macOS, using the existing
platform command modifier and rebinding flow. A focused text field owns its local
undo/redo shortcuts. Menu labels show the available edit name or the base command
name when the corresponding stack is empty. The rows expose their labels and
disabled state to accessibility, support keyboard navigation and close on Escape.
Keep the menu within the viewport and preserve the command row's wrapping at the
760-point minimum. A native menu bar is outside this proposal.

The History slider previews the target revision while dragged. Releasing it
applies the required undo or redo steps in order. Escape cancels a preview.
Keyboard users can move one revision at a time with the arrow keys. Expose the
current revision and the slider value to accessibility. During replay, disable
the slider until the session has acknowledged the target. If a step fails, keep
the current revision and show the error. Keep the playhead position independent of the
history slider. Use the same stopped-transport eligibility as Undo and Redo.
Fit the viewport at 760 points and scroll the revision list when needed.

### Interview decisions — 2026-11-19

- The user approved the proposed history lifetime, transport gating and limits,
  and the provisional Edit menu with Undo and Redo.
- The user requested instrument changes in this release, including edits inside
  plugin editors where feasible.
- The user requested an Edit History window with a slider for navigating retained
  revisions and approved navigation on release.
- The requested Command-Shift-P searchable command palette is already specified
  in [T18](T18-ui-foundation.md) and the
  [Command Palette UI reference](../UI/views/command-palette.md). T08 registers
  its commands in the existing registry.

## Verification — 2026-11-19

- `cargo check --locked`, `cargo build --locked`, `cargo fmt --all --check` and
  `cargo clippy --all-targets --locked -- -D warnings` passed.
  `cargo test --locked` passed with 93 tests and 11 ignored by default.
- Session tests covered multi-step seeks, saved-revision and Save As behavior,
  branch invalidation, stale requests, failed Open, stopped-transport gating,
  MIDI and plugin state restoration on track removal, and both eviction limits.
  The history jump preserved the current revision when a worker-state conflict
  prevented replay.
- The explicitly selected TestSynth and CoreMIDI regression verified undo and
  redo of a single recording entry containing two loop passes. It also checked
  routing to the armed track and autosave recovery. The fixture was rebuilt
  from the vendored source before running the test.
- The explicitly selected T08 native worker test verified instrument
  assignment and replacement, undo and redo of a captured editor state, and
  preservation of the previous instrument and history after a failed load.
  The editor-state change used the deterministic worker fixture. The existing
  commercial VST3/CLAP editor and save/reopen smoke test also passed.
- The egui test exercised preview, release, Escape, keyboard step and
  accessibility bounds. The shortcut test verified that unavailable Redo
  cannot trigger Undo. See [Edit History](../UI/views/edit-history.md) for the
  native gallery renders at 1024 and 760 points.

Individual parameter gestures and live-editor snapshots are outside the
supported editor transaction. The commercial smoke test covered editor
lifecycle and state round-tripping. Direct parameter gestures in commercial
editors were not manually exercised as part of the development verification.

## Completion — 2026-11-19

The user tried the rebuilt macOS app, confirmed the delivered T08 workflow
worked, and requested closure. T08 is closed and has been removed from the
open task index.
