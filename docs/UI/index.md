---
name: Clef UI index
kind: index
version: 2.0.0
updated: 2026-11-22
---

# Clef UI index

The navigation map for `docs/UI/`, the UI library of Clef (Rust + egui). Read this file first, then
only the files the change actually touches. Do not read the whole tree.

- Binding rules, authority order, statuses and file template: [README](README.md).
- How a design becomes code, and the primitive/surface contract: [contract](contract.md).
- Colour roles, typography, metrics, depth, separation, states, accessibility, pointer and
  commands: [foundations](foundations.md).
- Legacy C++ illustrations: `snapshots/`. Renders of the real Clef UI: `snapshots/egui/` (T18).

## How to use this index

1. Find the surface in **Views**, or the control in **Components**.
2. Read that file, plus [foundations](foundations.md) for the tokens it names.
3. Read the components a view is made of, and the views a component is used in: a change to a
   shared component must be checked against every surface that lists it.
4. Read [README](README.md) and [contract](contract.md) for the binding rules before implementing
   anything.
5. Update the component or view file, and this index when a file is added, renamed or removed, in
   the same change as the code.

## Reading recipes

| Task | Read |
| --- | --- |
| Any UI change | [README](README.md), [foundations](foundations.md), [contract](contract.md), then the affected view and the components it lists |
| Range gestures, button state, or plugin editor presentation | [Interaction frame regressions](contract.md#interaction-frame-regressions) and the affected view |
| Timeline, arrangement, or piano-roll rendering | [Dense editing surfaces](contract.md#dense-editing-surfaces) and the affected view |
| Restyle or change a shared control | Its component file and every view in its **Used in** column |
| Add a new surface | [contract](contract.md#surface-contract), [window](views/window.md), the closest existing view, and the components that view lists |
| Add a new component | [contract](contract.md#primitive-contract), [foundations](foundations.md), the nearest existing component, and every view that would use it |
| Rust/egui implementation work | [multitrack-session](views/multitrack-session.md), [contract](contract.md), then the primitives it uses; [single-instrument-project](views/single-instrument-project.md) records the T03 predecessor |
| Startup, install or quarantine behaviour | [startup](views/startup.md), [quarantine-notice](views/quarantine-notice.md), [notice-box](components/notice-box.md) |

## Implementation state

`status` in the tables below is **design ownership**, not availability: `shared` should become a
primitive in `crates/ui/src`, `app` is built for one surface, `mixed` is a primitive with surface glue.
The `impl` field in each file says what exists: `planned` (no Clef code; a legacy C++ reference may),
`app` (inline in `crates/ui/src/lib.rs`, provisional) or `gallery` (in `crates/ui/src/primitives/`, with a golden and
tests).

The startup chooser and recovery dialog have egui gallery cases. Their supporting files
document the Clef variants alongside the legacy references.

T18 Phase 1 (2026-11-21) made eleven components `gallery`: text button, tool button, chip, menu
button, text field, caption and heading, list row, selection edge, scroll and zoom bars, notice box
and window frame. Their gallery is `cargo run -p clef-ui --example t18_gallery -- docs/UI/snapshots/egui
<1024|760>`, written as `snapshots/egui/t18-<component>-<width>.png`.

T18 Phase 2 (2026-11-21) made menus (with the menu bar), the segmented control and the new value
slider `gallery`, and added the command palette (`t18-command-palette`) over the command registry
in `crates/ui/src/commands.rs`.

T18 Phase 3 (2026-11-21) made fifteen more components `gallery`: icons, checkbox, tabs, glyph
toggle, track-state button, search field, segment display, range overlay, and the mixer controls
(fader, knob, width knob, level meter, mute and solo, power button, slot button). The gallery also
renders at `pixels_per_point = 2.0`: pass `2` after the width to write `…-1024@2x.png` (T18, T09, T10
and T34 stages). [Foundations](foundations.md#contrast) holds the WCAG AA contrast table, and
`gallery::tests::every_gallery_control_has_an_accessible_name` walks the accesskit tree of every
gallery case.

## Components

Reusable building blocks. Ownership: **shared** — belongs in a primitive, reused everywhere;
**app** — built in one surface, move it into a primitive when a second surface needs it; **mixed** —
a primitive paints it, the surface wraps behaviour around it.

| File | What it is | Status | Used in |
| --- | --- | --- | --- |
| [Accordion section](components/accordion.md) | Titled group that expands and collapses with a quick animation. | shared | [midi-functions](views/midi-functions.md) |
| [Automation mode chip](components/automation-mode-chip.md) | Read / Touch / Latch / Write automation mode control on a channel. | mixed | [track-header](views/track-header.md), [mixer](views/mixer.md), [channel-strip](views/channel-strip.md) |
| [Callout](components/callout.md) | Anchored pop-over that edits one object in place: macro mapping, modulation amount, plugin and parameter pickers, point value. | app | [fx-rack](views/fx-rack.md), [track-lanes](views/track-lanes.md), [mixer](views/mixer.md), [channel-strip](views/channel-strip.md), [arrangement](views/arrangement.md) |
| [Caption and heading](components/caption-and-heading.md) | Section heading, caption and readout text styles; the three type roles used inside surfaces. | mixed | [channel-strip](views/channel-strip.md), [fx-rack](views/fx-rack.md), [settings](views/settings.md), [side-drawers](views/side-drawers.md), [startup](views/startup.md) |
| [Checkbox](components/checkbox.md) | A boolean setting with a visible label ("Skip first note", settings switches). | shared | [fx-rack](views/fx-rack.md), [settings](views/settings.md) |
| [Chip](components/chip.md) | A small toggle that shows or hides something attached to its owner, and says how much there is. | shared | [track-header](views/track-header.md), [mixer](views/mixer.md), [fx-rack](views/fx-rack.md), [arrangement](views/arrangement.md), [transport-bar](views/transport-bar.md), [window](views/window.md) |
| [Colour picker](components/colour-picker.md) | The one colour chooser, used for tracks, folders, markers, blocks, lanes and articulations. | app | [track-header](views/track-header.md), [arrangement](views/arrangement.md), [track-lanes](views/track-lanes.md), [fx-rack](views/fx-rack.md), [mixer](views/mixer.md) |
| [Detach button](components/detach-button.md) | Glyph that reopens the current panel in its own resizable window; T41 lower-panel controls (Detach, Half / Full, Close). | shared | [fx-rack](views/fx-rack.md), [ara-editor](views/ara-editor.md), [piano-roll](views/piano-roll.md), [mixer](views/mixer.md) |
| [Empty state](components/empty-state.md) | Large glyph over one line of text, centred in a view that has nothing to show yet. | shared | [piano-roll](views/piano-roll.md), [mixer](views/mixer.md), [arrangement](views/arrangement.md) |
| [Fader](components/fader.md) | Linear slider with satin cap and recessed travel; the character element of the UI. | shared | [channel-strip](views/channel-strip.md), [mixer](views/mixer.md), [fx-rack](views/fx-rack.md), [arrangement](views/arrangement.md) |
| [Glyph toggle](components/glyph-toggle.md) | Glyph-only raised toggle: triplet and dotted feels, section chevrons. | shared | [midi-functions](views/midi-functions.md), [tool-row](views/tool-row.md) |
| [Hierarchy rail](components/hierarchy-rail.md) | Continuous folder rails in the header gutter; no other connector lines. | shared | [track-header](views/track-header.md) |
| [Icons](components/icons.md) | The single vector glyph set, plus tool and stretch cursors. | shared | [tool-row](views/tool-row.md), [transport-bar](views/transport-bar.md), [track-header](views/track-header.md), [mixer](views/mixer.md), [fx-rack](views/fx-rack.md), [side-drawers](views/side-drawers.md), [scroll-zoom-bars](components/scroll-zoom-bars.md) |
| [Knob](components/knob.md) | Rotary control for trim, pan, send level, node parameters and macros. | shared | [channel-strip](views/channel-strip.md), [mixer](views/mixer.md), [fx-rack](views/fx-rack.md), [knob-cell](components/knob-cell.md) |
| [Knob cell](components/knob-cell.md) | Knob with caption and value stacked: the one way a knob is labelled. | app | [fx-rack](views/fx-rack.md), [track-lanes](views/track-lanes.md), [mixer](views/mixer.md), [channel-strip](views/channel-strip.md) |
| [Level meter](components/level-meter.md) | T11 stereo post-fader peak, hold and clip in mixer strips and at the sequencer header edge. | shared | [track-header](views/track-header.md), [channel-strip](views/channel-strip.md), [mixer](views/mixer.md) |
| [List row](components/list-row.md) | One row in a vertical list: browser, presets, plugins, playlist, recent sessions, commands, settings. | app | [side-drawers](views/side-drawers.md), [startup](views/startup.md), [command-palette](views/command-palette.md), [settings](views/settings.md), [callout](components/callout.md) |
| [Menu button](components/menu-button.md) | Shows one current value and opens a menu of choices. | mixed | [tool-row](views/tool-row.md), [channel-strip](views/channel-strip.md), [fx-rack](views/fx-rack.md), [plugin-window](views/plugin-window.md), [settings](views/settings.md) |
| [Menus](components/menus.md) | Context menus, pop-up menus and the in-window menu bar; every command row projects the registry. | shared | every surface, [window](views/window.md) |
| [Mute and solo](components/mute-solo.md) | The only two lamps of headers and strips: mute in oxblood, solo in yellow. | shared | [track-header](views/track-header.md), [channel-strip](views/channel-strip.md), [mixer](views/mixer.md) |
| [Note](components/note.md) | A MIDI note in the roll and inline in arrangement parts, in seven states. | shared | [piano-roll](views/piano-roll.md), [arrangement](views/arrangement.md) |
| [Notice box](components/notice-box.md) | Once-on-open notice: quarantined plugins, missing audio, stopped recording. Not a general dialog. | shared | [quarantine-notice](views/quarantine-notice.md), [arrangement](views/arrangement.md), [ara-editor](views/ara-editor.md) |
| [Performance readout](components/performance-readout.md) | Compact CPU / Disk readout; a dash when data is missing, never a fictional value. | shared | [transport-bar](views/transport-bar.md) |
| [Power button](components/power-button.md) | On/off (bypass) for inserts, sends, racks, nodes, Monitor FX slots and the sidechain. | shared | [channel-strip](views/channel-strip.md), [mixer](views/mixer.md), [fx-rack](views/fx-rack.md), [side-drawers](views/side-drawers.md), [plugin-window](views/plugin-window.md) |
| [Range overlay](components/range-overlay.md) | The one marquee and range-selection look, for parts, lanes, takes, nodes and strips. | shared | [arrangement](views/arrangement.md), [piano-roll](views/piano-roll.md), [mixer](views/mixer.md), [track-lanes](views/track-lanes.md), [fx-rack](views/fx-rack.md) |
| [Scroll and zoom bars](components/scroll-zoom-bars.md) | Always-visible scroll bars on editing surfaces, carrying their zoom buttons. | mixed | [arrangement](views/arrangement.md), [piano-roll](views/piano-roll.md), [mixer](views/mixer.md) |
| [Search field](components/search-field.md) | Filter field with search glyph, live hint, clear button and match wash. | shared | [arrangement](views/arrangement.md), [mixer](views/mixer.md), [tool-row](views/tool-row.md) |
| [Segment display](components/segment-display.md) | Seven-segment position counters, read as one hardware counter; T41 two-row format readout. | shared | [transport-bar](views/transport-bar.md) |
| [Segmented control](components/segmented-control.md) | Exclusive choice among two to four options shown at once. | mixed | [side-drawers](views/side-drawers.md), [settings](views/settings.md), [track-lanes](views/track-lanes.md), [fx-rack](views/fx-rack.md) |
| [Selection edge](components/selection-edge.md) | The one selected-row, strip, macro or node cue. | shared | [track-header](views/track-header.md), [mixer](views/mixer.md), [channel-strip](views/channel-strip.md), [list-row](components/list-row.md), [fx-rack](views/fx-rack.md), [settings](views/settings.md) |
| [Shadows](components/shadows.md) | Lift shadow while dragging objects and the region shadow chrome casts into a margin (T41: `paint::region_shadow` under the transport, `paint::lift_shadow` under a dragged header). | shared | [piano-roll](views/piano-roll.md), [arrangement](views/arrangement.md), [transport-bar](views/transport-bar.md), [fader](components/fader.md), [knob](components/knob.md) |
| [Slot button](components/slot-button.md) | One instrument, insert or send slot: shows what is loaded and opens it; T37 adds the Late warning state. | shared | [channel-strip](views/channel-strip.md), [mixer](views/mixer.md) |
| [Project dialog](components/project-dialog.md) | Modal recovery and discard confirmation. | app | [startup](views/startup.md) |
| [Sweep bar](components/sweep-bar.md) | Indeterminate progress bar: session loading, ARA render. | shared | [startup](views/startup.md), [ara-editor](views/ara-editor.md) |
| [Tabs](components/tabs.md) | Editor / Mixer / FX Racks tab buttons in the status row; T41 toggle tabs (Editor \| Mixer). | shared | [window](views/window.md) |
| [Tap button](components/tap-button.md) | A send's pre/post-fader glyph: the tap branches above or below the fader cap. | shared | [channel-strip](views/channel-strip.md), [mixer](views/mixer.md) |
| [Text button](components/text-button.md) | Flat labelled button for actions that need words (Open…, Replace…, Store A, TAP). | shared | [settings](views/settings.md), [startup](views/startup.md), [side-drawers](views/side-drawers.md), [fx-rack](views/fx-rack.md), [transport-bar](views/transport-bar.md), [callout](components/callout.md) |
| [Text field](components/text-field.md) | Typed name or exact value: inline renames, position counter, knob values, node fields. | mixed | [track-header](views/track-header.md), [track-lanes](views/track-lanes.md), [fx-rack](views/fx-rack.md), [settings](views/settings.md), [transport-bar](views/transport-bar.md), [channel-strip](views/channel-strip.md), [command-palette](views/command-palette.md) |
| [Tool button](components/tool-button.md) | Borderless glyph button for editing tools, transport controls, settings, fit and close. | shared | [tool-row](views/tool-row.md), [transport-bar](views/transport-bar.md), [window](views/window.md) |
| [Track state button](components/track-state-button.md) | Record arm, MIDI monitor and inline note editing, identical on every surface. | shared | [track-header](views/track-header.md), [channel-strip](views/channel-strip.md), [mixer](views/mixer.md) |
| [Width knob](components/width-knob.md) | Stereo width, drawn as what it does. | shared | [channel-strip](views/channel-strip.md), [mixer](views/mixer.md) |
| [Value slider](components/value-slider.md) | Bounded number with a unit: drag, type, step with arrows, Option-click reset; box or track form. | shared | [transport-bar](views/transport-bar.md), [edit-history](views/edit-history.md), [midi-functions](views/midi-functions.md) |
| [Window frame](components/window-frame.md) | In-app frame of Clef tool windows and modal dialogs; never the OS title bar. | shared | [settings](views/settings.md), [command-palette](views/command-palette.md), [edit-history](views/edit-history.md), [multitrack-session](views/multitrack-session.md), [transport-bar](views/transport-bar.md) |

## Views

Composed surfaces. `app` — composed for one surface; `mixed` — primitives plus surface glue. The
file's `impl` says whether Clef code exists.

| File | What it is | Status | Made of | Composed with |
| --- | --- | --- | --- | --- |
| [Clef multitrack session](views/multitrack-session.md) | Rust/egui instrument rows, MIDI recording, and arrangement editing with adjacent coloured tracks, track-height zoom, scroll bars and one playhead (T40). | app | [list-row](components/list-row.md), [selection-edge](components/selection-edge.md), [range-overlay](components/range-overlay.md), [menus](components/menus.md), [scroll-zoom-bars](components/scroll-zoom-bars.md), [notice-box](components/notice-box.md) | [timeline-rulers](views/timeline-rulers.md), [edit-history](views/edit-history.md) |
| [ARA editor](views/ara-editor.md) | Lower-panel editor for an audio part edited in an ARA plugin, plus unavailable notice and Render ARA question. | app | [detach-button](components/detach-button.md), [icons](components/icons.md), [notice-box](components/notice-box.md), [sweep-bar](components/sweep-bar.md), [text-button](components/text-button.md) | — |
| [Arrangement](views/arrangement.md) | Timeline with MIDI and audio parts, global bands, and a pinned Master row. | mixed | [callout](components/callout.md), [chip](components/chip.md), [colour-picker](components/colour-picker.md), [fader](components/fader.md), [menus](components/menus.md), [note](components/note.md), [notice-box](components/notice-box.md), [range-overlay](components/range-overlay.md), [scroll-zoom-bars](components/scroll-zoom-bars.md), [search-field](components/search-field.md), [shadows](components/shadows.md) | [chord-lane](views/chord-lane.md), [timeline-rulers](views/timeline-rulers.md), [tool-row](views/tool-row.md), [track-header](views/track-header.md), [track-lanes](views/track-lanes.md) |
| [Articulation band](views/articulation-band.md) | Articulation directions band inside the piano roll, between locator band and grid. | app | — (see its Anatomy) | — |
| [Audio editor](views/audio-editor.md) | Lower-panel editor for plain audio parts: transients, warp markers and the Audio functions drawer. | app | [fader](components/fader.md), [glyph-toggle](components/glyph-toggle.md), [icons](components/icons.md), [menu-button](components/menu-button.md), [sweep-bar](components/sweep-bar.md), [text-button](components/text-button.md) | [ara-editor](views/ara-editor.md), [midi-functions](views/midi-functions.md), [timeline-rulers](views/timeline-rulers.md) |
| [Channel strip](views/channel-strip.md) | The selected channel's full strip in the left drawer, one scrolling column; T41: bottom half of the left column. | mixed | [automation-mode-chip](components/automation-mode-chip.md), [callout](components/callout.md), [caption-and-heading](components/caption-and-heading.md), [fader](components/fader.md), [knob](components/knob.md), [knob-cell](components/knob-cell.md), [level-meter](components/level-meter.md), [menu-button](components/menu-button.md), [mute-solo](components/mute-solo.md), [power-button](components/power-button.md), [selection-edge](components/selection-edge.md), [slot-button](components/slot-button.md), [tap-button](components/tap-button.md), [text-field](components/text-field.md), [track-state-button](components/track-state-button.md), [width-knob](components/width-knob.md) | — |
| [Chord lane](views/chord-lane.md) | The one global Chords lane: chord events, scale row, chord editor, Write chords to track. | app | [callout](components/callout.md), [checkbox](components/checkbox.md), [icons](components/icons.md), [menu-button](components/menu-button.md), [menus](components/menus.md), [segmented-control](components/segmented-control.md), [text-button](components/text-button.md) | [arrangement](views/arrangement.md) |
| [Command palette](views/command-palette.md) | Spotlight-style search over the whole command registry (⇧⌘P); remapping in Commands / Keyboard Shortcuts. | app | [list-row](components/list-row.md), [text-field](components/text-field.md), [window-frame](components/window-frame.md) | — |
| [Controller lanes](views/controller-lanes.md) | T10 Velocity and MIDI CC0–127 lanes with programmable interpolation. | mixed | — (see the view) | [piano-roll](views/piano-roll.md) |
| [Edit History](views/edit-history.md) | Edit menu and a revision slider for undoing or redoing multiple edits. | app | [menus](components/menus.md), [caption-and-heading](components/caption-and-heading.md) | [multitrack-session](views/multitrack-session.md) |
| [FX rack](views/fx-rack.md) | FX Racks tab: boxes, nodes, cables, macros and modulators. | app | [callout](components/callout.md), [caption-and-heading](components/caption-and-heading.md), [checkbox](components/checkbox.md), [chip](components/chip.md), [colour-picker](components/colour-picker.md), [detach-button](components/detach-button.md), [fader](components/fader.md), [icons](components/icons.md), [knob](components/knob.md), [knob-cell](components/knob-cell.md), [list-row](components/list-row.md), [menu-button](components/menu-button.md), [power-button](components/power-button.md), [range-overlay](components/range-overlay.md), [segmented-control](components/segmented-control.md), [selection-edge](components/selection-edge.md), [text-button](components/text-button.md), [text-field](components/text-field.md) | — |
| [MIDI functions drawer](views/midi-functions.md) | MIDI functions drawer (Quantize, Humanize, Transpose, Note operator …) and its collapsed rail. | app | [checkbox](components/checkbox.md), [glyph-toggle](components/glyph-toggle.md), [icons](components/icons.md), [menu-button](components/menu-button.md), [text-button](components/text-button.md), [text-field](components/text-field.md) | [piano-roll](views/piano-roll.md), [tool-row](views/tool-row.md) |
| [Mixer](views/mixer.md) | Provisional T11 mixer bank, docked Master, shared fader/meter dB scale, T12 insert chains, the T35 instrument Plugin slot with bypass/offline, and T15 buses, sends and output routing. | mixed | [automation-mode-chip](components/automation-mode-chip.md), [callout](components/callout.md), [chip](components/chip.md), [colour-picker](components/colour-picker.md), [fader](components/fader.md), [icons](components/icons.md), [knob](components/knob.md), [knob-cell](components/knob-cell.md), [level-meter](components/level-meter.md), [menus](components/menus.md), [mute-solo](components/mute-solo.md), [power-button](components/power-button.md), [range-overlay](components/range-overlay.md), [scroll-zoom-bars](components/scroll-zoom-bars.md), [search-field](components/search-field.md), [selection-edge](components/selection-edge.md), [slot-button](components/slot-button.md), [tap-button](components/tap-button.md), [track-state-button](components/track-state-button.md), [width-knob](components/width-knob.md) | — |
| [Piano roll](views/piano-roll.md) | T10 docked MIDI editor; since T40 one real-piano keybed and one scrolling 128-pitch grid for all selected clips. | mixed | [note](components/note.md), [range-overlay](components/range-overlay.md), [scroll-zoom-bars](components/scroll-zoom-bars.md) | [arrangement](views/arrangement.md), [controller-lanes](views/controller-lanes.md), [midi-functions](views/midi-functions.md), [timeline-rulers](views/timeline-rulers.md) |
| [Plugin window](views/plugin-window.md) | Original plugin editor attachment and presentation, macOS Dock behavior, and planned sidechain header. | app | [menu-button](components/menu-button.md), [power-button](components/power-button.md) | — |
| [Quarantine notice](views/quarantine-notice.md) | What the composer sees when a project opens with quarantined plugins. | app | [caption-and-heading](components/caption-and-heading.md), [icons](components/icons.md), [notice-box](components/notice-box.md), [text-button](components/text-button.md) | — |
| [Settings](views/settings.md) | The one Settings window: sidebar, search and categories; Audio page Applies to (all projects or this project). | app | [caption-and-heading](components/caption-and-heading.md), [checkbox](components/checkbox.md), [list-row](components/list-row.md), [menu-button](components/menu-button.md), [segmented-control](components/segmented-control.md), [selection-edge](components/selection-edge.md), [text-button](components/text-button.md), [text-field](components/text-field.md) | — |
| [Side drawers](views/side-drawers.md) | Right-hand drawers: Browser (Tracks, Racks, Plugins, Audio), Playlist, Monitor FX. | app | [caption-and-heading](components/caption-and-heading.md), [chip](components/chip.md), [icons](components/icons.md), [list-row](components/list-row.md), [power-button](components/power-button.md), [segmented-control](components/segmented-control.md), [text-button](components/text-button.md), [text-field](components/text-field.md) | — |
| [Clef Single-Instrument Project](views/single-instrument-project.md) | Historical Rust/egui workspace for the T03 single-instrument project; superseded by the multitrack session. | app | [list-row](components/list-row.md), [menu-button](components/menu-button.md), [notice-box](components/notice-box.md), [text-button](components/text-button.md), [text-field](components/text-field.md) | — |
| [Startup](views/startup.md) | Recent projects and crash recovery. | app | [caption-and-heading](components/caption-and-heading.md), [list-row](components/list-row.md), [text-button](components/text-button.md), [project-dialog](components/project-dialog.md) | — |
| [Timeline rulers](views/timeline-rulers.md) | Bars/beats ruler and loop band with continuous range preview and shared active Loop colour. | mixed | — (see its Anatomy) | — |
| [Tool row](views/tool-row.md) | Editing tools, shape and snap controls, fit toggles; T41: track search, create glyph and right-aligned fits under the transport. | mixed | [glyph-toggle](components/glyph-toggle.md), [icons](components/icons.md), [menu-button](components/menu-button.md), [scroll-zoom-bars](components/scroll-zoom-bars.md), [tool-button](components/tool-button.md) | — |
| [Track header](views/track-header.md) | Arrangement headers with arm, Mute, Solo (drag-paint), inline rename, drag-to-reorder, right-edge stereo meters and the Master docked at the bottom. | mixed | [automation-mode-chip](components/automation-mode-chip.md), [chip](components/chip.md), [colour-picker](components/colour-picker.md), [hierarchy-rail](components/hierarchy-rail.md), [icons](components/icons.md), [level-meter](components/level-meter.md), [mute-solo](components/mute-solo.md), [selection-edge](components/selection-edge.md), [slot-button](components/slot-button.md), [text-field](components/text-field.md), [track-state-button](components/track-state-button.md) | — |
| [Track lanes](views/track-lanes.md) | Automation, take and macro lanes recessed under their owner track. | app | [callout](components/callout.md), [chip](components/chip.md), [colour-picker](components/colour-picker.md), [knob-cell](components/knob-cell.md), [menus](components/menus.md), [range-overlay](components/range-overlay.md), [segmented-control](components/segmented-control.md), [text-field](components/text-field.md) | — |
| [Transport bar](views/transport-bar.md) | Transport band with region shadow: glyphs, tempo/meter, position and second counter, format readout, Snap, click, Stop Mode, MIDI record mode. | mixed | [chip](components/chip.md), [icons](components/icons.md), [menu-button](components/menu-button.md), [performance-readout](components/performance-readout.md), [segment-display](components/segment-display.md), [shadows](components/shadows.md), [text-button](components/text-button.md), [text-field](components/text-field.md), [tool-button](components/tool-button.md) | [timeline-rulers](views/timeline-rulers.md) |
| [Window](views/window.md) | Window grid, chrome rows, drawers, lower panel and status row; the in-window menu bar (T18); the T41 session layout with the left column and Editor / Mixer status toggles. | mixed | [chip](components/chip.md), [menus](components/menus.md), [tabs](components/tabs.md), [text-button](components/text-button.md), [tool-button](components/tool-button.md) | [tool-row](views/tool-row.md), [transport-bar](views/transport-bar.md) |

## Legacy reference (C++)

61 of the 63 component and view files quote the original Composer C++/JUCE/Tracktion
implementation: a `reference:` symbol, `source:` paths such as `Source/Arrangement.cpp`, a ```cpp
block, or JUCE prose. **That code is not in this repository and is not an API.** Those files carry
`impl: planned` and a reference-only banner.

Read their prose for design intent, translate it into Rust/egui, and never port a class, a
LookAndFeel or a file layout. Built Clef surfaces include the
[multitrack session](views/multitrack-session.md) and the historical
[single-instrument project](views/single-instrument-project.md), both inline in `crates/ui/src/lib.rs`.

## Maintenance

A new or renamed file in `components/`, `views/`, `snapshots/` or `snapshots/egui/` must be added
here and to the table in [README](README.md) in the same change. Each file's frontmatter (`status`,
`impl`, `used-in`, `components`, `snapshot`) is the source for its row: keep the frontmatter
correct and this index can follow it. Migrate a component to the new template when its primitive
lands, per the [contract](contract.md#migration-from-the-legacy-library).
