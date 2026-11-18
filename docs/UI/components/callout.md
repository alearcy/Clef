---
name: Callout
kind: component
reference: none yet (JUCE CallOutBox)
status: app
impl: planned
reference-source: [Source/RackEditor.cpp (MappingEditor, ModulationEditor), Source/PluginPicker.h, Source/AutomationControls.h (AutomationPicker, AutomationPointEditor), Source/AudioImportViews.h, Source/AudioProcessViews.h]
tokens: [panel, field, text, secondary, peacock]
used-in: [fx-rack, track-lanes, mixer, channel-strip, arrangement]
snapshot: snapshots/callout-plugin-picker.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Callout

## Purpose
A small anchored editor for one object, instead of a separate window: macro mapping, modulation
amount, plugin picker (Replace…), automation parameter picker, point value editor.

## Anatomy
A solid `panel` box with a 1-pixel `divider` edge pointing at its anchor (kit `drawCallOutBoxBackground`: no translucency, no shadow); bold title, then the content (search field, list rows,
fields); action buttons bottom right (Cancel, primary).

## Behaviour
Escape or a click outside closes and discards unless the content commits live; Return commits.
One commit is one undo step.

## Do / Don't
- Don't open a separate window for a single-object edit.

## Plugin picker (UI10.7, `Source/PluginPicker.h` over `Source/PluginList.h`)
The one way to choose a plugin. It opens directly beside the slot that asked (no menu step, no second
pop-up): focused search, Maker | Category segment, a "New rack" row first, FAVOURITES, RECENT, then
every plugin grouped by maker (the Browser's Plugins list). One row per plugin, its installed formats
in the caption with the preferred one first (Settings › Plugins › Preferred format, AU by default on
macOS); the row's context menu loads another format. No menu ever lists plugins.

Entry points: an empty instrument slot (strip and mixer), "Add audio effect…" and the mixer's "+", insert-before/after and Load audio effect… in a slot's menu, New FX send…, a Monitor FX add, a rack's Add node › Plugin… and Replace…. Recent plugins and the preferred format are per-user settings (`ProjectSession::recentPlugins`, `preferredPluginFormat`).

## Audio import (D05, `Source/AudioImportViews.h`)
`ImportProgressView` is a call-out drawn the same way (panel, divider edge, an arrow at its anchor) that lives
in the arrangement instead of a CallOutBox, so it updates at the timer rate while the session keeps playing:
"Importing N files", one row per file with its state, a recessed groove with the `accent` fill under the file
being written, Cancel import. `RefusedFilesView` is the content of a CallOutBox: warning glyph, "N files were
not imported", FILE and REASON rows, the primary OK (Return or Escape closes).

## Normalise and Strip Silence (D72, `Source/AudioProcessViews.h`)
Approved boards 2 and 3 of [D72](https://claude.ai/artifact/WVbhEKQzLmSgenbcg1M35P); snapshots
`normalise-analysing.png`, `normalise-callout.png`, `strip-silence-callout.png`. Both hang from the first selected
audio part in time (a scrolled-out part leaves them over the timeline) and share `AudioProcessView`: a bold 13-px
title with "· N audio parts" in 11-px `secondary`, the parts measured off the message thread as the call-out opens
(*Analysing… N %* over the D05 import groove, the primary flat and disabled), Cancel and the lit primary
("Normalise ↩", "Strip ↩") bottom right. Every number is a [value slider](fader.md#value-slider) with an 11-px
`secondary` caption (never a typed field); choices are raised text buttons side by side, the chosen one `peacock`.
- **Normalise** (248 wide): Peak | Loudness, Target (−1.0 dBFS / −23.0 LUFS), Each part | Together with more than one
  part, then the result: what was measured (11 px), the gain in 13-px `text` ("Gain +4.2 dB", a range for Each
  part, "… on every part" for Together), parts limited to +24 dB or silent by name, and in `danger` with the
  warning glyph "Peaks over 0 dBFS on N parts (max +1.8 dBFS)".
- **Strip Silence** (272 wide): Threshold, Minimum silence, Pre-roll, Post-roll, Fade in, Fade out (times on a
  logarithmic travel; a typed plain number is milliseconds, "1.2 s" seconds), then "N pieces from M parts" (13 px)
  and, in 11-px `danger`, the part (or count) that is entirely silent and will be removed. While it is open the
  arrangement previews every analysed part (see [arrangement](../views/arrangement.md)).
- Return applies (one undo step) and closes; Escape or Cancel closes without a change and clears the preview. Their
  commands are unavailable while either call-out is open. The last values are application settings.
