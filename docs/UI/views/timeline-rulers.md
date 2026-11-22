---
name: Timeline rulers
kind: view
reference: paintRollRuler, rollGridLevels
status: mixed
impl: app
reference-source: [Source/TimelineRows.h, Source/LocatorBand.h, Source/Arrangement.cpp, Source/PianoRoll.cpp]
snapshot: snapshots/timeline-rulers.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Timeline rulers

T05 implements the bars/beats ruler and loop band above the instrument lanes. The ruler locates
the playhead and supports keyboard beat steps. The loop band supports draw, move, resize, beat snap,
Option for tick precision, and Escape to cancel. The lanes and ruler share scroll and zoom.
After a valid release, the loop band holds the submitted range until the session confirms or
rejects it. See [T25](../../specs/T25-ui-flicker.md).

The loop band and grips use the same `LOOP_ACTIVE` yellow as the transport's Loop glyph while
looping is enabled, including during drag and acknowledgement. When looping is off, the band
uses a dim `ACCENT` fill and `ACCENT` grips. See [T34](../../specs/T34-transport-glyphs.md).

Shared by the arrangement and the piano roll.

- **Timeline rows** (D44), fixed order: Bars, Timecode, Time, Samples. The first row keeps the
  ruler height (24); each further row adds 16 with 11-px `secondary` labels. Row names sit
  right-aligned in the column left of the ruler. Labels thin out with zoom; every row locates on
  click; its context menu is the registry's ticked row list.
- **Chord band** (D06.3, piano roll only): 16 pixels between the marker row and the locator band,
  present while the project has chords. Each chord is a `field` box over its range with a 2-pixel
  `accent` left edge and "name · numeral" in 10 px `text`; read-only, a click locates to its start.
  Caption "Chords" in the column.
- **Locator band** (D90): three 10-pixel rows, Loop, Punch, Project, with 9-px `secondary` names.
  Loop uses the state colours above. Punch uses `danger` at 60 % on and 25 % off. The edges are
  2 pixels. Project is a
  faint `secondary` span with 2-pixel `text` grips (`secondary` when following content). Drag empty
  space to draw, the body to move, within 5 pixels of an edge to resize; snap, Alt free, Escape
  cancels. The band never scrubs.
- Grid tiers are suppressed, not aliased: bars always; beats above 7 px per beat; subdivision above
  7 px per subdivision (`minimumGridPixels`).

## Clef Rust/egui — T41 round 3

[T41 round 3](../../specs/T41-session-window-layout.md#round-3--gui-review-2026-11-22) (S11). In the arrangement, each ruler's unit is named right-aligned in the column over the
track headers: "Bars" beside the bars ruler, and the second ruler's unit beside it. A click on either
label opens the second-ruler menu: None, Time, Samples, SMPTE 24 / 25 / 30. The second ruler
(`Viewport::unit_ruler`, 18 points, `canvas`) sits under the bars ruler and above the loop band, its
marks on round values of its unit at least 96 points apart (`unit_step`), labelled in 10-point
`secondary`; it is display only. Its unit is independent of the transport's second counter and is
saved with the project (`ProjectFormat::second_ruler`, project version 8); the SMPTE frame rate is
shared with the counter. The piano roll keeps the bars ruler only.

**Evidence:** `snapshots/egui/t41-rulers-{1024,760}.png`; unit test
`the_second_ruler_marks_round_values_of_its_unit`.
