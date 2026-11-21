---
name: Command palette
kind: view
status: app
impl: app
reference-source: [Source/Commands.h (CommandPalette)]
components: [text-field, list-row]
snapshot: snapshots/egui/t18-command-palette-1024.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Command palette

App-local, Spotlight-style search over the whole command registry (⌘⇧P on macOS, Ctrl-Shift-P
elsewhere, plus the Commands… button). A search field over result [rows](../components/list-row.md)
with each command's live shortcut right-aligned in `secondary`. The owner restores the originating
focus and context before invoking, so context-aware commands act where the user was.

A command may offer results of its own for the typed query (D61.3: one per automation parameter). An offer marked
as replacing its command stands in for the command's own row (D87: "Edit with Melodyne", one per scanned ARA
plugin, instead of "Edit with ARA Plugin").

## Clef Rust/egui — T18 Phase 2

`crates/ui/src/palette.rs` (view) over `crates/ui/src/commands.rs` (registry).

- **Open:** ⇧⌘P (Ctrl-Shift-P elsewhere; remappable, command `window.palette`), the menu bar's
  **Commands…** button, or Window › Command Palette…. While it is open no other shortcut fires.
- **Layout:** a modal [window frame](../components/window-frame.md), 560 wide (narrower windows
  keep a 16-point margin), centred horizontally with its top at one sixth of the window height.
  A [text field](../components/text-field.md) ("Search commands", focused) over a scrolling list
  of [list rows](../components/list-row.md) with the live shortcut right-aligned in `secondary`
  (`ListRow::trailing`). The highlighted row uses the list row's selected state.
- **Rows:** every registry command in registry order, with its live name (Undo and Redo name the
  edit). Case-insensitive substring match on the name; no fuzzy matching or recents in this phase.
  Unavailable commands stay listed, dimmed; ↑/↓ skip them and wrap.
- **Run:** Return or a click runs the highlighted command; Escape or a click outside closes. The
  palette first gives keyboard focus back to the widget that had it, so arrangement and
  piano-roll commands act where the user was; a piano-roll command is queued for the focused lane
  and runs on the next frame.
- **Remapping** stays in Commands / Keyboard Shortcuts (Window menu, ⌘K): each row has Set and
  Clear; Set waits for the next key (Escape cancels; keys without modifiers are allowed). A
  shortcut already used in an overlapping context asks "Shortcut already in use — Replace /
  Cancel". Restore Defaults clears every change. Changes are saved in the global preferences
  (`shortcuts.json` beside `audio.json`) and apply to every project.

Evidence: gallery `t18-command-palette`; `palette::tests::filters_skips_unavailable_rows_runs_with_return_and_closes_with_escape`;
`tests::palette_shortcut_opens_it_and_a_clashing_assignment_asks_first`;
`tests::unmodified_shortcuts_wait_while_text_is_typed`; `commands::tests::*`;
`shortcuts::tests::saved_shortcuts_round_trip_and_bad_files_fall_back` (session).
