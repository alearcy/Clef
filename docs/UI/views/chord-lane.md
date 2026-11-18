---
name: Chord lane
kind: view
reference: paintGlobalHeader, drawIcon (Icon::chord, Icon::record), LookAndFeel button painting
status: app
impl: planned
reference-source: [Source/ChordLane.cpp, Source/ChordEditor.h, Source/ChordWriteDialog.h]
components: [callout, text-button, menu-button, segmented-control, checkbox, menus, icons]
views: [arrangement]
snapshot: snapshots/chord-lane.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Chord lane

## Purpose
D06: the one global Chords lane in the arrangement's global band, under the marker lanes and above
the arrangement track. It holds harmony only (chord events and scale changes); it is heard only
through the instrument track chosen in **Play via** (D06.2), and its chords become notes through
Write chords to track.
Mockup: https://claude.ai/artifact/QCicLZHJHyQtvGv5TrqRWb, board 1 (lane), board 2 (editor) and
board 3 (Write chords to track, conflict prompt).

## Anatomy
- **Header** (`paintGlobalHeader`, `Icon::chord`, caption "CHORDS"): chevron (collapse), the
  **Played** display (a `canvas` well, the chord on any MIDI input in bold 12 px `accent`, caption
  "Played" in 10 px `secondary`), and the arm glyph (`Icon::record`, `danger` when armed,
  `secondary` otherwise). The second line, when the lane is at least 40 px tall, holds the
  **Play via** [menu button](../components/menu-button.md) ("Play via: Strings pad ▾", "Play via:
  None ▾"; 18 px high, up to 150 wide) and after it the scale at the playhead in 10 px `secondary`.
- **Scale row**: 15 px at the top of the lane. Each change is a 2-pixel `accent` edge and its scale
  name in 9 px `secondary`; with no change, "C major" from bar 1. Hidden while collapsed.
- **Chord row**: one rounded (2 px) event per chord on `field`; name bold 12 px `text`, numeral
  10 px `secondary` below when the lane is at least 26 px tall inside (a locked inversion adds
  "· locked" there). Selected: `selection`
  (oxblood) fill, 1-pixel `text` outline and 3-pixel `text` edge handles. While drawing, the range is a
  50 % `field` ghost with an `accent` outline. Erase marks crossed chords with a
  1.5-pixel `danger` outline. Bar lines `grid`; the playhead crosses the lane.

## Metrics
Default height 56, 40–160 by its lower edge, 20 collapsed (`chord::defaultHeight`, `minimumHeight`,
`maximumHeight`, `collapsedHeight`). Edge grab 4 px.

## Behaviour
- Gestures as parts: Draw drags on empty space to draw a chord (a click draws one grid step); it is
  written at once (the chord played on a MIDI input, else the tonic) and selected. The editor opens
  only when asked: a double-click or Edit Chord…. Select clicks (Shift adds), drags to move,
  drags an edge to lengthen or shorten; Alt bypasses snap; Escape cancels; Erase removes what it
  crosses. A placed chord trims the chords it covers (no overlaps). Each gesture is one undo step.
- Scale row: double-click (or a Draw click) adds a change carrying the scale in force; drag moves it;
  right-click or double-click a change for Key, Mode, Delete.
- Context menus are projections of the registry: Edit Chord…, Write Chords to Track…, Lock
  Inversion (ticked when locked), Copy, Cut, Duplicate, Delete Chord, Add Scale Change Here (at the
  chord's start, not the pointer); on empty space Add Chord Here, Add Scale Change Here, Paste; on
  the header Arm, Collapse, Hide, Play Chords via Selected Track, Stop Chord Playback, Add Chord /
  Scale Change at Playhead.
- Play via: the button's menu lists None and every instrument (MIDI) track, the current one ticked.
  With a track chosen, playback sends each chord, voiced with the last write settings, to that
  track's instrument beside its own parts; its mute and solo apply. While stopped, pressing a chord
  (and every change in the chord editor) sounds it there for as long as the button is held, at
  least half a second.
- Drag out: moving chords more than 10 px below the lane turns the move into a drag (a `field` chip
  naming the chords). Over a MIDI track's lane a dashed ghost in the track colour (35 % fill, 1-pixel
  dashed edge, caption "Drop: Dm9 · G7sus4 · G7") shows the range, held where the pointer grabbed
  the chord and snapped to the grid (Alt free): dragged straight down it sits exactly under the
  chord's own segment. The drop opens Write chords to track. Over the piano roll, ghost
  [notes](../components/note.md) preview the voicing and the drop writes them at once, no dialog.
- Arm: with the lane armed and no MIDI track armed, Record plays the transport and records only
  chords; with a MIDI track armed both record and the chords join the take's undo step.

## Chord editor (callout content)
Snapshot: `snapshots/chord-editor.png`. A [callout](../components/callout.md) anchored to the chord: bold 16 px title, "Scale: …" right;
a `canvas` preview well with the name (26 px), numeral and tones (12 px `secondary`), the MIDI lamp
(`accent` while a key is held on any input) and a two-octave keybed picture (`cap` / `background`
keys, lit tones `accent`). Then rows of lit [text buttons](../components/text-button.md) (lit =
`peacock`, kit `buttonOnColourId`): ROOT (12), QUALITY (maj, min, dim, aug, sus2, sus4, 5),
EXTENSION (—, 6, 7, 9, 11, 13, add9, 6/9) with a separate **maj7** toggle, ALTERATIONS (b5, #5, b9, #9,
#11, b13; at most two), BASS (a menu button: Root or a pitch). Two rows of diatonic buttons (triads,
sevenths) with name and numeral. Cancel and the primary OK at the bottom right. Return commits,
Escape or a click outside cancels. A chord played on any MIDI input fills the editor.

## Write chords to track (dialog)
Snapshot: `snapshots/chord-write-dialog.png`. A 640 × 498 dialog window (not modal). Bold 16 px
title; right, in 11 px `secondary`, "track · bars a–b · Dm9 → G7sus4 → G7". Three sections under
[section headings](../components/caption-and-heading.md) with 10 px `secondary` row captions:
- **Voicing** — INVERSION: a [segmented control](../components/segmented-control.md) Root | 1st |
  2nd | 3rd with the hint "first chord; voice leading picks the rest"; SPREAD: a row of lit
  [text buttons](../components/text-button.md) Close, Drop-2, Drop-3, Drop-2+4, Open; TOP NOTE: a
  [menu button](../components/menu-button.md) "Top note at or below: C5 ▾" (octave submenus);
  then [checkboxes](../components/checkbox.md) Voice leading, Separate bass, the menu button
  "Bass octave: 2 ▾" (enabled with Separate bass) and Double root. OUTPUT (D68): the checkbox
  **One track per voice** — each voice of the written chords goes to a new MIDI track below the
  target, top voice first. Without a MIDI track (no track, or not a MIDI one) the title reads "New
  tracks", the box is checked and disabled, and that forced choice is not stored.
- **Rhythm** — VALUE: lit text buttons Sustain, 1/1, 1/2, 1/4, 1/8, 1/16 and a segmented control
  Straight | Triplet | Dotted; GATE · VELOCITY: two stepper fields ("90 %", "72") with the hint
  "Gate applies to grid values only". Sustain disables the feel and the gate.
- **Preview** — a `canvas` well: the real voicing, read-only, one rounded bar per note in the target
  track's colour, a `grid` line and the chord name (10 px `secondary`) at each chord, and
  "Total movement: N semitones" bottom right. It follows every control live.
Bottom: what the write does in 11 px `secondary` ("Creates a part over bars 4–6", "Extends “name” to
cover bars 4–6", "Writes into “name”, bars 4–6", "Creates 3 tracks below Strings, bars 4–6", "Creates 3 tracks
at the end, bars 4–6"), then Cancel and the primary Write. Return writes,
Escape cancels. The settings are stored with the document on Write.

The **conflict prompt** is the kit alert: title "<track> already has notes there", message "N notes
in “part”. Replace removes them; Add keeps them and writes the chords on top.", buttons Replace
(default), Add, Cancel. It appears for the command, a drop and Stamp chord alike, only when the
written range already holds notes. One track per voice never asks: its tracks are new.

**Split chord into tracks** (D68, notes or parts → tracks) asks with the kit alert too: title "Split
into N tracks", message "Each voice goes to a new track below, top voice first. Keep leaves the
original notes in place; Delete removes them.", buttons Keep (default), Delete, Cancel. The part
menu lists the command after Duplicate, with its live binding.

**Quantise pitch to scale** (D69, notes or parts) has no dialog or alert: it reports "N notes moved to
the scale." in the status line. The part menu lists it after Split chord into tracks, with its live
binding.

## Do / Don't
- Do keep harmony on the lane and voicing in the write-to-track dialog (D06.2).
- Spread and rhythm value are rows of lit buttons, not segmented controls (five and six choices;
  the segmented control stops at four), as the chord editor's rows.
- Don't add a free-text name field: the name is printed from the builder.
- The builder rows are rows of lit buttons, not segmented controls: they hold 6–12 choices, where the
  segmented control stops at four (resolved against the approved mockup, which drew them as one row).
