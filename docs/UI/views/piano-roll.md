---
name: Piano roll
kind: view
reference: paintKeybed, keybedKeyBounds, paintPitchRows, pitchRowBounds, paintKeyRows, paintScaleMark, paintRollGrid, paintRollRuler, paintPlayhead, paintNote
status: mixed
impl: planned
reference-source: [Source/PianoRoll.cpp, Source/PianoRoll.h]
components: [note, range-overlay, shadows, scroll-zoom-bars]
views: [timeline-rulers, controller-lanes, articulation-band, tool-row, midi-functions]
snapshot: snapshots/piano-roll.png, snapshots/piano-roll-harmony.png, snapshots/piano-roll-build.png, snapshots/piano-roll-fold.png, snapshots/midi-functions.png, snapshots/step-input.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Piano roll

The product's core surface.

## Anatomy
- **Parts column** (with more than one open part): 160 wide (100–320, hideable), 26-pixel rows with
  a 3-pixel stripe, a visibility mark (dot shown, ring ghost, dash hidden), part and track name;
  active row a 35 % selection fill.
- **Keybed**, 56 wide: a real piano — seven equal white keys per octave (12/7 rows each, the C key
  closing on the bottom of the C row), black keys 1.25 rows tall and 57 % wide. Names on C keys only.
  A sounding key lights in the track colour; the pointer's pitch gets a weaker `accent` fill. The
  keybed auditions on the selected track's instrument, left edge loudest, right edge softest.
  Inline (D49) keybeds are 20 wide with `compactLabels`.
- **Grid**: rows carry the pitch class (white-key rows lighter, black-key rows on `canvas`); seals at
  E–F and B–C, the B–C one darker as the only octave rule. Pitch rows 12 by default (6–24).
- **Notes**: see [note](../components/note.md). The selected part's end is a vertical line with a
  ruler grip (snap, Alt free, `accent` while dragged; refuses to cut notes).
- [Rulers](timeline-rulers.md) above, [articulation band](articulation-band.md) between the locators
  and the grid, [controller lanes](controller-lanes.md) below.

## Harmony (D06.3)
Mockup: https://claude.ai/artifact/QCicLZHJHyQtvGv5TrqRWb, board 4.
- **Scale rows** (revised 2026-11-13, mockup board 6 option C): a row outside the scale in force is
  tinted `outOfScale` (#C75A50) at 5 % and carries a 3-pixel `outOfScale` edge at its segment's visible
  left (the grid's left edge, and just after each scale change), segment by segment along the scale
  changes; the light/dark key shading stays; every in-scale key on the keybed carries a 3-pixel `accent` mark at
  its right edge (`paintScaleMark`, the scale at the playhead). The **SCALE** chip toggles both.
- **Fold** ([menu button](../components/menu-button.md) "Fold: Off ▾", Off · Scale · Used notes):
  folded rows close up and keep their pitch-class tint, with a darker seal where a gap closed
  (`paintPitchRows` over a row list); the keybed becomes one key per row (`paintKeyRows`: light keys,
  dark keys 57 % wide, names on C and after each gap). Scale keeps any row that holds a note.
- **Selection readout**: caption "Selection" (10 px `secondary`) and a `canvas` well with the chord
  in bold 12 px `accent` ("Dm9 · ii9"): the selected notes, else the open part's notes under the
  playhead; empty under two pitch classes.
- **Placement** (composer, 2026-11-12): Fold sits in the column beside the ruler's bars row, by the
  keys, with the meter right-aligned after it; SCALE (controlHeight, 60 wide) follows the tool buttons past a
  1-pixel `divider` line (a view toggle, not a tool; CHORD TONES and its out-of-chord marks were retired by the composer, 2026-11-13); the readout sits right-aligned before the
  fits and yields on a short row. All only invoke registry commands.
- **Scale row** (2026-11-13, board 5): 16 px `globalLane` above the chord band, caption "Scale", while
  the project has chords or scale changes. Each scale in force from its change: a 2-pixel `accent` edge
  and the scale name (10 px `secondary`), as the chord lane's scale row. Read-only; edit scales in the lane.
- The **chord band** is a drag source: a chord dragged from it onto the grid is written as notes,
  as from the chord lane; a click still locates.
- **Build** (⌃-drag up from a note; the modifier is a setting): ghosts are 1-pixel `accent` outlines
  with "M3 · E4" labels (10 px `accent`) beside them; a ladder (210 wide, `background` at 90 %) at the
  view's top right lists "3 · …", "Triad · C", "7th · Cmaj7 · Imaj7" … with the reached step on a
  `peacock` fill. Inside a Draw or Paint stroke the same modifier shapes the chord on the drawn note
  (its root keeps the selection fill); after letting go, Paint repeats the chord in every cell. ⌃⇧ steps by semitone and the ladder shows the interval. **Strum** (⌃⌥-drag) has no
  overlay: the notes move and the status bar reads "Up-strum: 24 ms per note".

## MIDI functions and step input (D102, D103)
- The [MIDI functions drawer](midi-functions.md) sits at the editor's left edge, before the parts
  column and the keybed; collapsed it is a 24-pixel rail.
- While step input is on, the playhead is the step cursor: a translucent `accent` band (12 %) one step
  wide follows it from the note area down through the lanes.

## Note menu (D69)

Right-click on a note opens the kit popup menu for the note selection: a note outside the selection
becomes the selection first; right-click on empty grid opens it only while notes are selected.
Entries, each a registry command with its live binding and availability: Quantise pitch to scale,
Split chord into tracks…, Chord track from selection, then (D102, after a separator) Quantize, Humanize,
Transpose, Note operator and Strum with the drawer's values. On macOS ⌃-click stays the chord gesture.

## Rules
Host owns zoom, scroll and quantization; the kit paints. Several open parts keep their colours,
non-active at 70 %, ghosts at 22 % never hit-tested.
