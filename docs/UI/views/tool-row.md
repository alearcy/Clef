---
name: Tool row
kind: view
reference: ToolButton, Icon
status: mixed
impl: planned
reference-source: [Source/MainComponent.cpp]
components: [tool-button, menu-button, scroll-zoom-bars, glyph-toggle]
snapshot: snapshots/tool-row.png, snapshots/step-input.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Tool row

The 28-pixel row above the arrangement and above the piano roll. Left: Select, Draw, Paint, Erase,
Shape (sine glyph, `peacock` while armed) and the "Shape: Sine · Grid ▾" menu button. Then the snap
controls: the arrangement shows the project grid after a `secondary` "Snap" label; the roll its value
and feel boxes. Right end: `fitWidth` and `fitHeight` glyphs (lit `accent` while pressing again
returns to the previous view); in the lower panel also the close glyph. Arrangement and roll tool
states are independent; both rows keep the same order.

**Heights** (composer, 2026-11-12): every bodied control of a tool row — menu buttons, combo boxes,
toggle and text buttons, chips, fields, the readout well — is `metrics::controlHeight` (24) tall,
centred in the 28-pixel row; only the tool, fit and close glyphs keep their 28-pixel `toolTarget`
square. The piano roll's row: tools, the Shape menu, a 1-pixel `divider`, SCALE (CHORD TONES retired 2026-11-13), then
the snap and length controls (Legato moved into the MIDI functions drawer, D102, 2026-11-13); the
Selection readout sits right before the fits.

**Step input** (D103, composer 2026-11-12): the **STEP** chip (52 wide, `peacock` while on) follows
Length. While it is on, Length gives its place to the step controls: the step length menu
(64 wide, "1/8 ▾", STEP is its caption), the triplet and dotted [glyph toggles](../components/glyph-toggle.md)
and "Velocity: Keyboard ▾" (140 wide: From keyboard, Fixed values, Other…). The status row shows the
step state and the rest, tie and back keys with their live bindings. Snapshot `step-input.png`.
