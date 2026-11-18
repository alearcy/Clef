---
name: Command palette
kind: view
status: app
impl: planned
reference-source: [Source/Commands.h (CommandPalette)]
components: [text-field, list-row]
snapshot: snapshots/command-palette.png
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
