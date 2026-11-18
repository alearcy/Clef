---
name: ARA editor
kind: view
status: app
impl: planned
reference-source: [Source/AraEditorPanel.h, Source/MainComponent.cpp (araShown, editWithAra, afterAraRender)]
components: [detach-button, text-button, notice-box, sweep-bar, icons]
tokens: [panel, canvas, text, secondary, danger]
used-in: [window]
snapshot: snapshots/ara-editor-panel.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# ARA editor (D87)

The lower panel's Editor content for an audio part edited in an ARA plugin (Melodyne). Approved boards 2–4 of
[ARA Support (D87)](https://claude.ai/artifact/348XW3KKYWjvwpLsSwXPtH). Snapshots: `snapshots/ara-editor-panel.png`
(the plugin's own view is native and renders black in a component snapshot), `ara-unavailable-panel.png`,
`ara-render-question.png`. The part's badge and menu rows are in [arrangement](arrangement.md).

## Purpose
Where the piano roll is for a MIDI part, the plugin's editor is for an ARA part: select the part, open the
editor, edit, hear it in context.

## Anatomy
- **Header**, 28 px, `panel`, right under the panel's drag divider, in the tool row's place: a 4-px strip in the
  track's colour; the plugin's name at 13 px bold in `text`; then at 11 px in `secondary` the track and the
  selected ARA parts ("Lead vocal · Verse take 3.wav, Chorus take 1.wav"); at the right the
  [detach button](../components/detach-button.md) (20 × 20, as on a rack header) and the panel's own close
  button.
- **Body**: the plugin's editor, edge to edge. It is plugin-owned and never restyled. A resizable editor fills
  the body; a fixed-size one is centred at the top.
- **Detached**: the body is `canvas` with one centred `secondary` line, "Melodyne is open in its own window."; the
  editor is in the floating [plugin window](plugin-window.md).
- **Plugin unavailable** (not scanned, or quarantined): the header shows the kit `warning` glyph (14 px) and the
  plugin's name, both in `danger`, and no detach button. The body is `canvas` with a block 520 wide, centred:
  the `warning` glyph (16) and "<Plugin> is not installed" or "<Plugin> is quarantined" at 16 px bold; two lines
  of 13-px `secondary` text saying the part plays its original audio and its ARA edits stay in the project;
  bottom right one [text button](../components/text-button.md), **Remove ARA**, its label in `danger`.

## Metrics
Header 28. The panel is never shorter than the header plus the height the plugin's editor accepts (the editor
answers a resize with its own minimum); 240 is assumed until it answers. While the ARA editor shows, the panel
keeps a height of its own: the MIDI editor's height returns when a MIDI part is edited again.

## Behaviour
- The Editor tab shows the ARA editor while the active arrangement part is an ARA audio part and no MIDI part is
  active. Double-click on an ARA part, Return (Open part) or Toggle editor opens it.
- One editor per ARA plugin: the instance already showing keeps the editor and follows the selected ARA part
  (the plugin is told which part is selected); a part of another plugin replaces it.
- The detach button moves the editor to the plugin window and back (its glyph shows the arrow coming back in
  while detached); closing that window re-attaches it.
- The embedded editor is the component Tracktion creates as that plugin's window
  (`ProjectSession::embeddedPluginEditor`), so the engine closes it whenever the part's ARA instance goes: delete,
  undo, leaving ARA, another document.
- The plugin's native view takes the keyboard. Play / Stop, Stop and Go to project start are picked up before it,
  under their live bindings (`AraKeyMonitor`, macOS); every other key, Record included, stays with the plugin.
- A native view sits above every component of the window, so the editor hides while a question or Commands
  search shows and returns when it closes.
- **Render ARA edits first?** Reverse, Normalise… and Strip Silence… on an ARA part ask before they run: the
  [notice box](../components/notice-box.md) in its question form, heading "Render ARA edits first?", one
  sentence why and what rendering does, **Cancel** and the primary **Render and Reverse ↩** (the command's name).
  Return renders and continues; Escape cancels. A part whose plugin is unavailable is refused in the status row
  instead. Switching parts from one ARA plugin to another asks the same way ("Edit with <plugin>?").
- While Render ARA to Audio runs the window takes no input and every command is unavailable: the workspace is
  dimmed with `background` at 72 % and a `panel` box 440 wide with a `divider` edge shows "Rendering ARA to
  audio…" (16 px bold), the parts' names (13 px, `secondary`) and the [sweep bar](../components/sweep-bar.md).

## Do / Don't
- Don't draw the plugin's content, analysed notes or a pitch editor of our own: the plugin's view is the editor.
- Don't add a second way to leave ARA: Render ARA to Audio and Remove ARA are registry commands.

## Code
```cpp
araPanel.show(part);                 // from MainComponent::refresh(), with the ARA part to show
session.embeddedPluginEditor = [this](tracktion::Plugin& plugin) { return araPanel.createEmbedded(plugin); };
```
