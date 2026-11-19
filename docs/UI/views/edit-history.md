---
name: Edit History
kind: view
status: app
impl: app
components: [menus, caption-and-heading]
snapshot: snapshots/egui/t08-history-ready-1024.png
---

# Edit History

The project command row places **Edit** after **Save As**. The menu projects
**Undo**, **Redo**, and **Edit History…** from Clef's command registry. Undo
and Redo add the available edit name and display the live shortcut.

## Layout and interaction

```text
project command row wrap
  Edit → Undo, Redo, Edit History…
non-modal Edit History window
  revision slider + Revision label
  status or replay error, when present
  scroll column: oldest retained state through newest edit
```

The window initially opens below the right side of the project commands and
can be moved. The 264-point slider leaves room to distinguish individual
revisions in a full 100-entry history. The list scrolls as its height grows.
The current revision is marked with **Current**. Dragging previews the target
edit, releasing applies it, and Escape cancels the preview. With slider focus,
the arrow keys move one revision at a time.

When history is empty, show **No edits in this session.** During playback,
count-in, recording, instrument loading or a pending transition, the slider
exposes its disabled state. If replay fails, keep the current revision and show
the session's error in the window. The revision slider exposes a name and the
current numeric value to accessibility. Command shortcuts leave text field
undo and redo available while a text field has focus.

The egui menu and window composition are provisional under [T08](../../specs/T08-undo-redo.md).
The gallery captures [ready at 1024 points](../snapshots/egui/t08-history-ready-1024.png)
and [760 points](../snapshots/egui/t08-history-ready-760.png), plus empty
and locked states at both widths.
