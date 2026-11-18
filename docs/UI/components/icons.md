---
name: Icons
kind: component
reference: composer::style::Icon, drawIcon, toolCursor, toolCursorImage, stretchCursor, stretchCursorImage
status: shared
impl: planned
tokens: [text, accent, secondary, disabled]
used-in: [tool-row, transport-bar, track-header, mixer, fx-rack, side-drawers, scroll-zoom-bars]
snapshot: snapshots/kit-gallery.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Icons

## Purpose
One vector glyph set for every glyph-only control. Glyphs carry no fill tile, border or label.

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
