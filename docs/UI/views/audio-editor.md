---
name: Audio editor
kind: view
status: app
impl: planned
reference-source: [Source/AudioEditorPanel.h, Source/AudioEditorPanel.cpp, Source/AudioEditorDrawer.h, Source/MainComponent.cpp (audioEditorPart, audioEditorShown)]
components: [text-button, glyph-toggle, menu-button, fader, sweep-bar, icons]
views: [timeline-rulers, midi-functions, ara-editor]
tokens: [panel, surface, canvas, text, secondary, accent, peacock, oxblood, divider, grid, barLine, capLight, automationLane]
used-in: [window]
snapshot: snapshots/audio-editor.png, snapshots/audio-editor-group.png, snapshots/audio-slice-arrangement.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Audio editor (D73)

The lower panel's Editor content for a plain (non-ARA) audio part, where the piano roll is for MIDI and the
plugin's view is for ARA. Approved boards 1–4 of [Audio Quantise (D73)](https://claude.ai/artifact/8YZXAVuvF7p8Nz1atwwJSL).
Snapshots: `snapshots/audio-editor.png` (one part, Warp preview), `audio-editor-group.png` (two parts, Slice
preview from the guide), `audio-slice-arrangement.png` (the pieces on the arrangement).

## Purpose
Quantise audio to the grid by warping or slicing at its transients, and edit transients and warp markers by hand.

## Anatomy
- **Header**, 28 px, `panel`, as the [ARA editor](ara-editor.md)'s: a 4-px strip in the first track's colour,
  "Audio" (13 bold), the track and the parts (11, `secondary`), "Warp · N markers" at the right for one part.
- **Audio functions drawer**, 232 wide at the left, D102's anatomy (see [midi-functions](midi-functions.md)) with
  one section, **Quantize**: Method (Warp · Slice raised choices, the chosen `peacock`), Guide (a menu, only with
  several parts), Sensitivity, Grid (menu and the triplet/dotted [glyph toggles](../components/glyph-toggle.md)),
  Strength, Swing, Window, Dead zone and, for Slice, Crossfade: [value sliders](../components/fader.md#value-slider).
  A readout ("14 transients move · all parts move together"), Cancel and Apply; PREVIEW in the header while a
  preview runs, Cancel shown only then.
- **Timeline**: a 112-pixel label column, the Bars and Time rows (40), the transient band (16, `automationLane`),
  then one lane per part (at least 96 high): a 14-pixel warp row and the waveform in the track colour over a 10 %
  body, drawn through the part's warp map. A lane also draws the other pieces of the same file on its track (a
  Slice leaves several).
- **Transient band** (the guide's): detected ticks `accent` (2 px), hand-placed ticks `capLight` with a 6-px dot,
  selected ones `danger` with an `oxblood` dot. The sweep bar runs along its foot while detection runs. Caption
  "Transients", or "Guide cuts" with several parts.
- **Warp markers**: a pentagon handle in the warp row (`accent`, selected `oxblood` with a `capLight` edge) and a
  dashed `accent` line at 65 % through the waveform.
- **Guide chip**: "GUIDE" (10 bold `accent` on `field`) under the guide lane's names.
- **Preparing**: "<part> · Preparing warp…" at the lane's top left and the waveform at 45 % only while the part
  has nothing to play (its copy missing). A marker edit never dims it: the waveform redraws through the new map at
  once while the previous copy keeps playing until the new one is ready.

## Behaviour
- Shown in the Editor tab while the active arrangement part is a plain audio part and no MIDI part is selected;
  double-click on the part, Return (Open part) or Toggle editor opens it. Its lanes are the selected plain audio
  parts; a new selection takes the active part as its guide.
- A part shown for the first time is detected at its stored sensitivity (50 % by default).
- Band: click on empty space adds a transient; click selects (Shift adds), drag moves, Option-click removes;
  Delete removes the selection (*Delete selected markers*). Warp row: click selects a handle, drag moves it
  between its neighbours (the status row shows the move in ms), Escape cancels. A double-click in the waveform adds
  a marker snapped to the nearest transient within 6 px (Option free).
- Every drawer change previews (D102's preview: rewritten in place, one undo step on Apply, Cancel restores).
  Sensitivity re-detects instead; a running preview follows the new transients.
- The wheel scrolls the lanes vertically when they outgrow the panel (a scroll bar at the right; Shift or a sideways
  swipe moves in time), otherwise in time; Cmd + wheel zooms around the pointer (to 16 px per sample at 48 kHz).
- Double-click on one part of a selection (editor open or closed) opens the whole selection with that part as guide.

## Differences from the mockup
- The ghost (original-position) ticks of the preview and the dashed cut lines across a group's lanes are not drawn;
  the moved waveforms show the result.
- Dead zone reads in % of half a grid step, as D102's, not in ms.

## Code
```cpp
audioPanel.show(audioEditorParts(), session.activeArrangementPart()); // from MainComponent::refresh()
```
