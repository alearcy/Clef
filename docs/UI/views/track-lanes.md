---
name: Track lanes
kind: view
reference: colours::automationHeader/automationLane/automationOutside, roll::laneGutterHeight
status: app
impl: app
reference-source: [Source/AutomationLane.cpp, Source/AutomationControls.h, Source/Arrangement.cpp, Source/PinnedMacros.h]
components: [segmented-control, chip, knob-cell, text-field, range-overlay, colour-picker, menus]
snapshot: snapshots/track-lanes.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Track lanes

Lanes recessed directly under their owner track. Each opens with a 4-pixel `panel` gutter that also
crosses the header column; the seam under a lane resizes it within the track heights (24–320).

## Automation lane (D61, D62)
Header on `automationHeader`, indented one step, track-colour stripe. Row 1: parameter name ▾
(picker), R (lane read, `peacock` when on), ✕ (hide). Row 2: Region | Track
[segments](../components/segmented-control.md) and the value at the playhead with units. A 40-pixel
lane keeps one row. Band on `automationLane`: the curve in the track colour (or the lane's own
colour) over a 22 % fill, ringed points, oxblood selected points, a hollow 7-pixel bend handle at
each segment's middle, held steps for discrete parameters, `disabled` when R is off. Region lanes
paint each part as a 10 % band, `automationOutside` elsewhere with the Track curve as a dashed
`disabled` guide. While writing: a 2-pixel `danger` stroke behind the playhead, "· writing" in the
header. "+ Add lane" sits alone in a 28-pixel gap under the last lane, flat 10-px `secondary` text.

## Take lane (D66, T26)

40-pixel `automationLane` band, header indented with the 2-pixel stripe, “Take n” (11 bold) and
“Plays …” (10 `secondary`), three-dot vertical lane menu with a filled hover background and no
border. 8 % track-colour band per take, 24 %
where the comp plays it. The take's note marks are drawn at their attack and duration, mapped low to
high, full-strength on the comp sections and dimmed elsewhere. Comp by Select drag (range overlay with an `accent` readout chip), cuts dragged at ↔ edges.

**Clef T26:** the `T n` chip sits in the track row beside the arm target and is hidden below two
takes. It toggles every lane of the track and saves immediately as view state, with no undo step.
A lane is track-wide: lane n is the n-th take index of every clip of the track, so a comp range,
Use whole take, Delete take, Flatten comp, and Take to new track act on every clip holding that
index in one step. Deletion shifts later lane names and heights down.

The header name opens an inline rename on double-click; Enter commits, Escape cancels, a blank
name restores “Take n”. The three-dot menu is drawn in the accent palette rather than a font glyph;
it and the transport record-mode button share the filled `field` hover with no border.
Double-clicking a clip's lane section opens its take in the piano roll. A single click outside the name applies Use whole take. The seam under the
lane resizes it from 24 to 320 points in 4-point steps and saves on release. The lane menu carries
Rename…, Edit in editor, Use whole take, Delete take, Flatten comp, and Take to new track.

Cuts are grabbed within 4 points on a lane with the horizontal-resize cursor and an `accent` edge
on both lanes involved; the drag snaps to the grid, Option frees it with a one-tick minimum, and
Escape cancels. A 3-point `accent` strip in the piano roll marks the ranges where the comp plays
the edited take. Tracks have no colours yet, so lane bands use the `accent` role. Clef has no Track
Height command set, so the seam is the only height control.

## Macro lane (D59.2)
A recessed `canvas` band with the track stripe, "MACROS" caption and 56 × 64
[knob cells](../components/knob-cell.md) in rows as wide as the header.
