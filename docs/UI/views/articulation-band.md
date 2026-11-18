---
name: Articulation band
kind: view
status: app
impl: planned
reference-source: [Source/PianoRollArticulations.h]
snapshot: snapshots/articulation-band.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Articulation band (D63)

Between the locator band and the note grid. A 24-pixel `panel` header with fold chevron, rack/Split
name and the current articulation in `secondary`; 4 pixels under it, one 20-pixel row per
articulation of the track's single Articulation Split, names 11-px `secondary` in the fixed caption
column (grows up to half the roll width); 8 pixels before the note grid. Held spans in the track (or
row) colour, configured defaults subdued, explicit starts with a grip, selection oxblood with an
`accent` edge. Draw inserts, Select moves, Erase deletes; snaps to note attacks of the active part,
Alt free, Escape cancels. Rows scroll within a bounded height. Visual reference:
`UI/Mockups/D63-Articulation-Lanes.html`.
