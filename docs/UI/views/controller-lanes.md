---
name: Controller lanes
kind: view
reference: layoutLaneStack, layoutLaneHeader, paintLaneHeader, paintVelocityLane, paintControllerLane, controllerLanePosition, controllerName, isSwitchController
status: mixed
impl: gallery
reference-source: [Source/PianoRoll.cpp (Velocity, Controller, RegionLane, LaneHeaders)]
snapshot: snapshots/kit-gallery.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Controller lanes

## Clef T10

Velocity and open CC lanes follow the clip lanes in the lower editor and edit the focused
clip. Add a lane by controller number (CC0–127) and channel (1–16). Click in a CC lane
to add a point. Drag a point to move or change its value, or right-click it to remove it.
Velocity is drawn as bars with a horizontal extent line to each note's end, and follows
note selections. Dragging a velocity bar changes the value relative to the grabbed point,
so the value does not jump to the pointer height. Select events and use the arrow keys
to adjust them, or Delete to remove them.

Continuous CC segments can be linear or curved. Drag the bend handle or select the source
point and use Curvature and Linear. Recorded points keep their original held behavior until
an interpolation mode is applied. Switch lanes use steps. Discrete protocol events use
single markers. A lane can collapse or be removed independently of its clip data.

Tab reaches every point; the focused point becomes the selection and paints a focus ring.
Arrows move or change the selected points, Delete removes them, and Command-A selects the
whole lane. With the lane focused, Enter or Insert adds a point at the playhead holding the
value in force there.

Gallery cases at 760 and 1024 points cover [velocity](../snapshots/egui/t10-velocity-760.png),
[linear CC](../snapshots/egui/t10-linear-cc-1024.png),
[curved CC](../snapshots/egui/t10-curved-cc-760.png), and
[switch CC](../snapshots/egui/t10-switch-cc-1024.png).

## Legacy anatomy

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
