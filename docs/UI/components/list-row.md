---
name: List row
kind: component
status: app
impl: planned
reference-source: [Source/Browser.cpp, Source/PluginList.cpp, Source/AudioFileList.cpp, Source/PresetList.cpp, Source/PlaylistPanel.cpp, Source/PluginPicker.h, Source/StartupChooser.h, Source/Commands.h (CommandPalette), Source/SettingsPanel.h]
tokens: [panel, field, accent, peacock, text, secondary, disabled]
used-in: [side-drawers, startup, command-palette, settings, callout, multitrack-session]
snapshot: snapshots/list-row.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# List row

## Purpose
One entry in a vertical list: browser items, presets, plugins, playlist entries, recent sessions,
command results, settings categories.

## Anatomy
Name in `text`, optional `secondary` caption (maker · format, kind, path, length) right-aligned or on
a second line; optional leading colour bar or glyph. Category rows are bold with ▾/▸ and a count;
group headings are 11-px uppercase `secondary`.

## Metrics
22 (browser, picker), 26 (playlist, settings sidebar, parts column), 44 (session chooser, two lines).

## States
hover lifts to `field`; selected: `field` (or `panel`) fill plus the [selection edge](selection-edge.md);
"current" (playing playlist entry) an `accent` outline; picker selection may be `peacock`;
unavailable (missing file) `disabled` with "(Not found)"; favourite ★ in `solo`.

## Behaviour
Arrow keys move the selection, Return opens, type-to-search where a search field exists. Drag
sources show a 2-pixel `accent` bar where they would land.

## Do / Don't

## UI10.7
The Browser's Plugins segment and the plugin picker are one component, `PluginList`: 22-pixel rows, the accent selection edge, field hover, maker or category rows with ▾/▸ and counts, an optional leading action row (accent +), one row per plugin with its formats in the caption. The other lists (presets, playlist, chooser, command palette, settings sidebar) keep their own painters with the same heights and cues; a new list reuses `PluginList`'s pattern or one of them, never a new look.

The Browser's Audio segment (`AudioFileList`, D05) follows `PluginList`'s pattern: 22-pixel rows, `field` hover, the accent selection edge (with a `field` fill), bold folder rows with ▾/▸ and counts, length and format as the caption.

## Clef Rust/egui Translation — T04

`crates/ui/src/lib.rs::App::track_row` translates the shared two-line list-row pattern for the
[multitrack session](../views/multitrack-session.md). It wraps an egui button with
AccessKit selected-state information, a 44-point target, a 13-point name and an
11-point caption. Each line truncates separately; the tooltip/accessibility label
retains full identity and lifecycle state. Panel, field hover, disabled text and
accent selection/focus use foundation roles. Keyboard focus has an inset edge
shown only after keyboard input, independently of the selected-row edge.
