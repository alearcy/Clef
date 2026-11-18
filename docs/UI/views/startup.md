---
name: Startup
kind: view
status: app
impl: planned
reference-source: [Source/StartupChooser.h, Source/MainComponent.cpp (StartupLoadingView)]
components: [list-row, text-button]
reference: composer::style::paintStartupBackdrop, paintStartupPanel
snapshot: snapshots/startup-chooser.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Startup (D97, D98)

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
