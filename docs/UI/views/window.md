---
name: Window
kind: view
reference: composer::style::layoutWorkspace, WorkspaceRects
status: mixed
impl: planned
reference-source: [Source/MainComponent.cpp]
components: [transport-bar, tool-row, tabs, chip, menus]
snapshot: snapshots/window.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Window

## Anatomy, top to bottom
1. macOS system menu bar (File, Edit, View, Window; settings in the app menu). On Windows a flat
   in-window menu row (`menuBarHeight` 28) with the project name, Commands… and the settings glyph.
2. [Transport bar](transport-bar.md), `transportHeight` 40, recessed, casting a region shadow.
3. Body:
   - **Left drawer**: the [channel strip](channel-strip.md) at `inspectorWidth` 208, full height from
     below the transport to the status row. Hidden, it gives its width back.
   - **Centre**: the [arrangement](arrangement.md) with its [tool row](tool-row.md) (28), and below a
     6-pixel drag divider the **lower panel**: Editor (piano roll, the [ARA editor](ara-editor.md) for an ARA audio part, or the [audio editor](audio-editor.md) for a plain one), Mixer or FX Racks, one at a time,
     chosen by the [tabs](../components/tabs.md) centred in the status row. The lower panel closes, maximises and keeps its working state;
     Editor and Mixer share a height of at most 42 % of the body (never below 160); FX Racks keeps
     its own height (the window minus 394 when opened), saved with the project.
   - **Right drawer**: Playlist, Monitor FX or Browser, one at a time, full height ([side drawers](side-drawers.md)).
4. Status row, `statusHeight` 24: the channel-strip toggle glyph at the left, the lower-panel tabs
   centred, the status message and the BROWSER toggle at the right (flat text: `text`, `accent`
   while shown).

## Metrics
`workspacePadding` 12; minimum window 1024 × 640. The left drawer appears from 1024 wide.

## Rules
- Regions are separated by spacing and background roles only, never rules.
- The project name lives in the window title on macOS.
- Open issue: the app still keeps its own grid instead of `layoutWorkspace` (tracked since UI03).

## Clef T02 Validation Host

The approved Clef prototype is a terminal runner with original plugin editors hosted by
**egui/eframe**, not this legacy DAW workspace. eframe owns the parent window and event loop;
Clef borrows its parent view through raw-window-handle only to attach the plugin ABI editor.
No direct AppKit application windows or controls are constructed by Clef. The parent window
is hidden when the editor is closed; the plugin editor itself is destroyed and recreated.
Window resizing is enabled only when the attached plugin reports support, with plugin size
negotiation. OS chrome and plugin-owned UI retain their original appearance. No custom
Clef controls or revised workspace design are approved by this prototype.

See [T02 specification](../../specs/T02-engine-validation.md) for the implementation and
verification limits, including the outstanding stale VST3 resize request failure and
processing/editor serialization. The original ZebraHZ VST3 editor has one visual smoke
check; this does not establish full editor acceptance for all mandatory plugins.

## Clef T03 Application

The approved single-instrument workspace is implemented in Rust/egui. See
[Single-Instrument Project](single-instrument-project.md) for its surface, commands,
states and verification. The T02-only UI scope above describes the earlier prototype.
