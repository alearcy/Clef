---
name: List row
kind: component
status: app
impl: gallery
reference-source: [Source/Browser.cpp, Source/PluginList.cpp, Source/AudioFileList.cpp, Source/PresetList.cpp, Source/PlaylistPanel.cpp, Source/PluginPicker.h, Source/StartupChooser.h, Source/Commands.h (CommandPalette), Source/SettingsPanel.h]
tokens: [panel, field, accent, peacock, text, secondary, disabled]
used-in: [side-drawers, startup, command-palette, settings, callout, multitrack-session]
snapshot: snapshots/egui/t18-list-row-1024.png
api: ui::primitives::list_row::ListRow
evidence: [t18-list-row, startup and arrangement surface tests]
---

> **Migrated to Rust/egui (T18, 2026-11-21).** The Clef section at the end of this file is the
> implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

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

## Clef Startup Variant — T07

`primitives::project::row` renders startup and recovery lists at 44 points. The top line is
the project name. The second shows its path, timestamp or unavailable status. Both truncate
independently. The accessible name and tooltip retain the full identity and status.
Idle rows use `panel`, hover and selected rows use `field`, and focus or selection adds
an `accent` outline. Unavailable rows use `disabled` text and remain selectable for repair.
See [recent](../snapshots/egui/t07-startup-recent-1024.png),
[missing](../snapshots/egui/t07-startup-missing-760.png) and
[recovery](../snapshots/egui/t07-recovery-ready-760.png).

## Clef Rust/egui — T18

**API**

```rust
impl<'a> ListRow<'a> {
    pub fn new(name: &'a str) -> Self;           // 22 high by default
    pub fn detail(self, detail: &'a str) -> Self; // second line in SECONDARY
    pub fn accessible(self, name: &'a str) -> Self;
    pub fn height(self, height: f32) -> Self;     // 22, 26 or 44
    pub fn width(self, width: f32) -> Self;
    pub fn selected(self, selected: bool) -> Self;
    pub fn enabled(self, enabled: bool) -> Self;
    pub fn available(self, available: bool) -> Self; // false: DISABLED text, still selectable
    pub fn header(self) -> Self;                  // selection keeps PANEL, edge only
    pub fn show(self, ui: &mut egui::Ui) -> egui::Response;
}
```

| State | Fill | Edge | Text |
| --- | --- | --- | --- |
| rest | `PANEL` | — | `TEXT` / `SECONDARY` |
| hover | `FIELD` | — | unchanged |
| selected | `FIELD` (header: `PANEL`) | [selection edge](selection-edge.md) | unchanged |
| keyboard focus | unchanged | inset 1-point `ACCENT` | unchanged |
| unavailable / disabled | unchanged | — | `DISABLED` |

Each line truncates to one row; the accessible name and tooltip keep the full text. Used by the
arrangement track headers (44, header), startup and recovery rows (44), the instrument and effect
pickers and the file list (22).

## Clef Rust/egui — T18 Phase 2 addition

`ListRow::trailing(&str)` paints right-aligned `BODY` text in `secondary` (`disabled` when the
row is unavailable) on the first line and narrows the name to fit; an empty string paints
nothing. The [command palette](../views/command-palette.md) uses it for live shortcuts.

## Clef T41 header mode

`.header()` rows keep `panel` when selected and paint no selection edge: the track header that
owns the row draws the edge around its whole bounds.
