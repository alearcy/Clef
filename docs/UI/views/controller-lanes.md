---
name: Controller lanes
kind: view
reference: layoutLaneStack, layoutLaneHeader, paintLaneHeader, paintVelocityLane, paintControllerLane, controllerLanePosition, controllerName, isSwitchController
status: mixed
impl: planned
reference-source: [Source/PianoRoll.cpp (Velocity, Controller, RegionLane, LaneHeaders)]
snapshot: snapshots/kit-gallery.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Controller lanes

- Velocity opens the stack and is always present (72 default); one lane per MIDI controller follows
  it downwards (64 default, 40–240), each opened by a 4-pixel resize gutter. A new lane appears
  directly under velocity; "+ Add lane" is the last strip of the stack.
- Lane headers live in the keybed column (as wide as the longest name): name with number
  ("Modulation CC1"; a custom name replaces only the controller part), top/bottom values, collapse
  chevron, remove target when removable. Name click focuses, double-click renames, Tab moves on.
  Resize by the header's top border. Collapsed lanes keep a curve preview.
- Headers on `automationHeader` (`panel` while focused) over an `automationLane` band.
- Shape carries meaning: continuous = filled curve with points and bend handles; switch (64–69) =
  held steps, no handles. Every lane uses the part's track colour; `accent` marks only focus and
  selection. No data renders an explicit empty state.
- Region automation lanes of the shown part follow, with a REGION caption (9-px `accent`).
- Values crossing the kit are normalised 0…1; the host converts 0…127.
