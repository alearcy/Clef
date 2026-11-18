---
name: Track lanes
kind: view
reference: colours::automationHeader/automationLane/automationOutside, roll::laneGutterHeight
status: app
impl: planned
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

## Take lane (D66)
40-pixel `automationLane` band, header indented with the 2-pixel stripe, "Take n" (11 bold) and
"Plays …" (10 `secondary`), ▾ lane menu. 8 % track-colour band per take, 24 % where the comp plays
it. Comp by Select drag (range overlay with an `accent` readout chip), cuts dragged at ↔ edges.

## Macro lane (D59.2)
A recessed `canvas` band with the track stripe, "MACROS" caption and 56 × 64
[knob cells](../components/knob-cell.md) in rows as wide as the header.
