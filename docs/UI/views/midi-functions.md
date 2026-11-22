---
name: MIDI functions drawer
kind: view
status: app
impl: app
reference-source: [Source/MidiFunctionsDrawer.cpp, Source/MidiFunctionsDrawer.h, Source/MainComponent.cpp]
components: [text-button, glyph-toggle, menu-button, checkbox, text-field, icons]
views: [piano-roll, tool-row]
snapshot: snapshots/midi-functions.png, snapshots/midi-functions-sections.png, snapshots/midi-functions-collapsed.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# MIDI functions drawer

## Purpose
D102: Quantize, Humanize, Transpose, Scale quantize, Note operator, Strum and Legato for the selected notes (else every note of the
selected MIDI parts), with a live preview. Mockup:
https://claude.ai/artifact/5BCECRPQKTsLA6D4ZeugKa (boards 1–2, revised after the composer's comments of
2026-11-12).

## Anatomy
- A `surface` drawer at the MIDI editor's left edge, left of the keybed and the parts column, from
  below the tool row to the editor's bottom; the roll starts 8 pixels to its right. 232 wide.
- Title row: "MIDI functions" (13 bold) and the collapse chevron (the kit chevron, mirrored). Under it
  the target readout (11 px): `accent` ("5 notes · 1 part", "All notes of 3 parts") or `secondary`
  ("Nothing to process").
- Seven accordion sections (Quantize, Humanize, Transpose, Scale quantize — board 5 — Note operator,
  Strum, Legato — board 4, added 2026-11-13): a 28-pixel header with the
  chevron (down open, right closed), the name in 12 bold and, while that section previews, PREVIEW in
  10 bold `accent` at the right. Rows are 28 high: a 64-pixel `secondary` caption, then the control.
- Choices are raised [text buttons](../components/text-button.md) side by side, the chosen one
  `peacock` (Keep · End · Fixed, Chromatic · In scale, Nearest · Up · Down, Count · Grid, Up · Down) — never the recessed segmented
  well (composer, 2026-11-12). Feels are a [glyph toggle](../components/glyph-toggle.md) pair.
- Values are [value sliders](../components/fader.md#value-slider) (composer, 2026-11-13: sliders, not
  − + steppers), the unit in the value text ("80 %", "± 10 ms", "+2", "120 tick"); Semitones/Degrees,
  Octaves and Curve fill from zero.
- Note operator draws its repeats for the current values on a nominal note: a 40-pixel `canvas` strip
  with one `selection` (oxblood) bar per repeat, its width the gate and its height the velocity.
- Scale quantize: "Move to" and, under it, the readout "3 of 14 notes outside the scale" (11 px
  `secondary`).
- Bottom of each open section: Cancel and Apply (Humanize: Random and Apply). Apply is lit `peacock`
  while its section previews; Cancel shows only then. Other sections' Apply is disabled meanwhile.
- **Collapsed** (snapshot `midi-functions-collapsed.png`): a 24-pixel `surface` rail by the keybed with
  the expand chevron and "MIDI FUNCTIONS" (10 bold `secondary`, vertical). A click on the rail opens it.

## Behaviour
Changing a value previews (the roll and playback show the result, recomputed from the original
notes); Apply or Return commits one undo step, Cancel or Escape restores the notes and the section's
values as they were when the preview began. Any other command, a press outside the drawer and the
transport bar, a key the roll handles, another section's preview, recording or a document change ends
the preview the same way, with no history. Every action is a registry command (MIDI functions, Quantize, Humanize,
Transpose, Quantise pitch to scale, Note operator, Strum, Legato, the six transpose steps); the chevrons' tooltips carry the live binding.
Values, open sections and the collapsed state are application settings.

## Differences from the mockup (resolved)
- The Note operator's velocity ramp is two stepper rows (Velocity, →) instead of one "100 % → 60 %"
  row: two steppers do not fit one 190-pixel row.

## Do / Don't
- Do keep the drawer for functions that modify notes; Step input lives in the tool row (D103).
- Don't add a tool-row button for the drawer: it collapses to its rail.

## Clef Rust/egui — T41 round 3

[T41 round 3](../../specs/T41-session-window-layout.md#round-3--gui-review-2026-11-22) (S6). The MIDI Functions toggle in the piano roll's tool row opens the drawer inside the
editor, left of the keyboard over the whole editor height (ruler, grid and lanes start 8 points to
its right). It is `inspectorWidth` (208) wide on `panel`, with its own vertical scroll. Clef's
sections today are Quantize (Strength %), Transpose (Semitones) and Humanize (Timing, Velocity
deviation), each an [accordion section](../components/accordion.md); Quantize starts open. The rest
of the legacy design above (target readout, previews, further sections) is not built yet.
