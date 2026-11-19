---
name: Startup
kind: view
status: app
impl: gallery
reference-source: [Source/StartupChooser.h, Source/MainComponent.cpp (StartupLoadingView)]
components: [caption-and-heading, list-row, text-button, project-dialog]
reference: composer::style::paintStartupBackdrop, paintStartupPanel
snapshot: snapshots/startup-chooser.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Startup (D97, D98)

## Clef Startup and Recovery — T07

The chooser opens before the workspace. The centered panel uses the Composer Dark surface and
divider, a 4-point radius and 22 × 20 points of padding. Its maximum width is 760 points.

```text
startup (1024 × 640, minimum 760 × 480)
  centered panel
    column
      heading: Clef
      caption: Recent Projects
      row: scrolling recent rows + selected project details (wrap)
      row wrap: Recovery…, Remove, Locate…, Open, Open…, New Project
      status and error feedback when needed

recovery
  project dialog
    heading and optional recovery list
    selected project, checkpoint time and original path
    recovery instructions or error
    row wrap: Later, Discard…, Recover
```

At 760 points, selected details move below the list. The panel scrolls if needed at the
minimum height. Paths wrap in the details area. Each row line truncates independently.
The row tooltip and accessible name retain the full path and status. Up/Down select a recent
project and Return opens the selected readable project. Tab moves between actions.

A missing project remains selectable and exposes Locate. An incompatible project explains
the failure and disables Open. A read-only project can open and routes Save to Save As.

The recovery dialog opens when recoverable data exists, with a list when multiple sessions
are available. Fallback displays the usable checkpoint time and the latest checkpoint's
error. If all checkpoints fail, Recover is disabled. Discard requires confirmation.
Later returns to the chooser and keeps the Recovery action visible. Escape postpones
recovery or cancels discard confirmation.

| Primitive | Role and commands |
| --- | --- |
| [Caption and heading](../components/caption-and-heading.md) | Titles, paths, dates, preparation, empty and error states. |
| [List row](../components/list-row.md) | Select recent projects or recovery sessions. Activate a readable recent item with Return or a double-click. |
| [Text button](../components/text-button.md) | Bind Open, Locate, Remove, New Project and recovery actions to shared commands. |
| [Project dialog](../components/project-dialog.md) | Recovery chooser and destructive discard confirmation. |

The [egui gallery](../snapshots/egui/) covers loading, empty, recent, missing, incompatible,
read-only, available recovery, fallback, unavailable recovery, discard confirmation and Later
at 1024 and 760 points. See [recent projects](../snapshots/egui/t07-startup-recent-1024.png),
[narrow recovery](../snapshots/egui/t07-recovery-ready-760.png) and the
[T07 acceptance evidence](../../specs/T07-autosave-recovery.md).

## Legacy Startup Reference

- **Backdrop and panel** (approved 2026-11-11, board "Startup · framed panel and background texture" of
  [Settings and Startup Loading](https://claude.ai/artifact/Hq7LePESbpfweCnZLE7aFC)): both screens paint
  `paintStartupBackdrop`, `canvas` with a damask of one symmetric line ornament (outlines only, half-drop,
  60 × 79 tile) in `text` at 2 %, so faint it reads as a surface; the content sits in
  `paintStartupPanel`, `panel` with a 1-pixel `divider` edge and 4-pixel corners, no shadow, padded 22 × 20.
  This panel is the app's one bounded container, an explicit exception to "no decorative cards"; never
  reuse either on the workspace.
- **Chooser**: a 760-pixel centred column, "Composer DAW" 20 bold; "Recent sessions"
  (440 wide, caption 13) of 44-pixel rows (name 14, path 12 `secondary`, date 12 `secondary`; the
  selected row `field` with the selection edge, its date replaced by a `peacock` **Open ↩** and a
  close glyph; missing files `disabled` "(Not found)"); right of it the selected session's details
  and plugins; bottom right `field` buttons Audio/MIDI settings…, Open…, New session with live
  bindings.
- **Loading**: a 440-pixel column, "Opening last session" 13, the name 20 bold, the path
  13, a `field` track 4 high with an indeterminate `accent` sweep (a 30 % highlight with soft ends crosses
  it every 1.6 s, eased at both ends, only the bar repainting at 60 Hz; it pauses while one plugin
  instantiates on the message thread), active plugins at 14 in 26-pixel rows (place at 12
  `secondary`), at most twelve then "… and N more".
