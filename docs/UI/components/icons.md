---
name: Icons
kind: component
reference: composer::style::Icon, drawIcon, toolCursor, toolCursorImage, stretchCursor, stretchCursorImage
status: shared
impl: gallery
tokens: [text, accent, secondary, disabled]
used-in: [tool-row, transport-bar, track-header, mixer, fx-rack, side-drawers, scroll-zoom-bars]
snapshot: snapshots/egui/t18-icons-1024.png
api: ui::primitives::icons::Icon
evidence: [t18-icons]
---

> **Migrated to Rust/egui (T18 Phase 3, 2026-11-21).** The Clef section at the end of this file is
> the implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

# Icons

## Purpose
One vector glyph set for every glyph-only control. Glyphs carry no fill tile, border or label.

Clef's transport implements six vector glyphs in `crates/ui/src/primitives/transport.rs`.
`Glyph::{Start, Play, Stop, Record, Loop, Metronome}` is rendered by
the [tool button](tool-button.md) using the shared tool metrics. See the
[transport gallery](../../specs/T34-transport-glyphs.md) for the rendered set.

## Anatomy
`enum class Icon`: select, draw, erase, paint, folder, chevronRight, chevronDown, plus, close, play,
stop, record, rewind, loop, settings, metronome, punch, minus, fitWidth, fitHeight, marker,
arrangement, chain, chainBroken, sendPre, sendPost, panelLeft, scissors, conductor, shape, capture,
search, warning (D88: a triangle with its mark, drawn in `danger` for a quarantined plugin), chord
(D06: three stacked note heads on a stem, the chord lane's header glyph), hidden (D101: an eye
crossed out, a view-preset-hidden track shown by a search), triplet and dotted (D102: a bracket over
a 3, and a note with its dot — the feel toggles of the MIDI functions drawer and step input), trackMidi
and trackAudio (D100: a keyboard with two black keys and a waveform stroke, the track kind leading the
name in a track header, drawn in `secondary`; never a lamp), and reverse (D72: an arrow running back to a start
bar, the reversed audio part's name-row badge in the part's ink at 75 %, 11 px), muted (D106: a speaker
crossed out, after a muted part's name) and layers (D106: two stacked parts, the "N muted below" badge of a
part that covers muted pieces).
`drawIcon(g, icon, area, colour)` strokes the glyph into `area`.

## Metrics
16-pixel glyph inside a 28-pixel target (`toolTarget`) for tools and transport; 14-pixel glyphs in
lane headers (`laneHeaderButton`). Cursors are 26-pixel artwork with a dark halo.

## States
Colour is the only state: `text` normal, a lifted tone on hover, `accent` (or lamp colour) active,
`disabled` disabled.

## Behaviour
`toolCursor(icon)` returns the cached cursor for draw, paint and erase; select returns the platform
arrow. `toolCursorImage` gives the same artwork for documentation.
`stretchCursor()` (D71) is the part-edge stretch pointer shown while Control is held over a part's edge band:
a horizontal double arrow with a compressed zigzag in the middle, black (1.6 px) on a white halo (3.6 px) like
the platform's resize cursors, 24 points drawn at 2× (`stretchCursorImage`, 48 pixels), hotspot at its centre.

## Do / Don't
- Do add a new glyph to the kit enum and `drawIcon`, with a test, when a surface needs one.
- Don't draw a glyph inline in an app component (today `DetachButton`, `PowerButton`, `TapButton`
  and `TrackStateButton` still do; UI10.3 moves them into the set).
- Don't use emoji or text characters as glyphs, except the documented ▾ ▸ ✕ ~ ◆ marks in chips and
  menus.

## Clef Rust/egui — T18 Phase 3

**API**

```rust
pub enum Icon { Start, Play, Stop, Record, Loop, Metronome, FitWidth, FitHeight, Search, Close, Check, Triplet, Dotted }
impl Icon { pub const ALL: [Icon; 13]; pub fn paint(self, painter: &egui::Painter, center: egui::Pos2, size: f32, color: egui::Color32); }
```

The one vector glyph set (formerly `tool_button::Glyph`). Strokes use `TOOL_STROKE`; the host gives
the colour from the lamp rules in foundations. Drawn by the [tool button](tool-button.md), the
[glyph toggle](glyph-toggle.md), the [search field](search-field.md) and the [checkbox](checkbox.md).
Glyphs carry no accessible node of their own: the control that draws one names itself.

**States:** `text`, `secondary`, `accent`, `disabled` (the gallery shows every glyph in each).
**Not yet drawn:** the legacy set's select, draw, erase, folder, chevrons, plus, settings, punch,
marker and the rest are added when a Clef surface needs them.

**Evidence:** gallery `snapshots/egui/t18-icons-{1024,760}.png` and `t18-icons-1024@2x.png`
(Retina); accessible names checked by `gallery::tests::every_gallery_control_has_an_accessible_name`.

## Clef T41 round 2

`Icon::Plus` (the create glyph, drawn in `CREATE` by `tool_button::create_button`) and
`Icon::Drawer` (a window with its left pane: show or hide the left drawer).

## Clef T41 round 3

[T41 round 3](../../specs/T41-session-window-layout.md#round-3--gui-review-2026-11-22): `Icon::Detach` and `Icon::Attach` (a window with an arrow leaving it or entering it: the
lower panel's own-window control), `Icon::PanelFull` (a frame with its upper part raised: half or
full height, lit `accent` while full), and three empty-state glyphs drawn at 48 points:
`Icon::Keys` (an octave), `Icon::Faders` (three faders) and `Icon::Lanes` (stacked track lanes).
`Icon::ALL` holds 21 glyphs.
