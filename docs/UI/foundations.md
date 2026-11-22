---
name: Foundations
kind: foundations
platform: Rust + egui (eframe)
tokens: crates/ui/src/theme.rs (T18; interim crates/ui/src/lib.rs)
reference: legacy Composer C++ kit (not in this repository)
---

# Foundations

The look is called **Composer Dark** (historical name; Clef's one theme): minimal, calm and dense,
with physical fader affordances giving it character. Near-black surfaces with a slight green
undertone, soft-white text, one restrained peacock accent for active controls, oxblood as the
counterpoint. Large areas stay quiet. No heavy bevels, glossy panels, decorative cards, neon,
gradients for decoration or stock widget looks. It is a native desktop workstation, not a web page:
no CSS, no HTML implementation, no web fonts.

All sizes are egui points: logical pixels at `pixels_per_point = 1.0`, scaled by the window. The
values below are the design source of truth for Clef. `crates/ui/src/theme.rs` carries the same names and
values, and a test fails when they drift (`theme::tests`, T18). The legacy C++ kit is not in
this repository and does not override this file.

`theme::{metric, roll, text, track}`, `theme::foreground_on` and `theme::install_fonts` exist since
T18 Phase 1 (2026-11-21); `theme::routing_accent` and `theme::cursor` are still planned.

## Colour roles

Use the role, never the hex. Each role is one constant in `crates/ui/src/theme.rs`; the Rust name in the
first column is the canonical one. A colour that is not in this table does not exist: do not
introduce one, and do not compute one at a call site.

| Rust constant | Role | Hex | Use |
| --- | --- | --- | --- |
| `BACKGROUND` | background | #101A1C | Window and gaps between strips/regions |
| `PANEL` | surface | #172326 | Panels, headers, strips, drawers, menus' hosts |
| `CANVAS` | canvas | #111C1F | Editing surfaces: timeline, roll grid, rack graph |
| `FIELD` | field | #1F2F32 | Controls: buttons, fields, chips, menus, slot rows, selected list rows (1.16:1 on panel, the lightest that keeps danger and disabled text above their floors) |
| `TEXT` | text | #E3E8E5 | Primary text and glyphs |
| `SECONDARY` | secondary | #A4B5AE | Captions, units, inactive values, shortcut hints |
| `DISABLED` | disabled | #74847D | Disabled text and glyphs, unavailable data (3.5:1 on field) |
| `ACCENT` | accent | #78B5AA | Active control glyph, focus, selection edge, play lamp, audio pins |
| `PEACOCK` | content | #336A64 | Filled active control (primary button, lit segment, lit chip and every lit transport button: READ, 1 BAR, TAP, MONITOR FX), highlighted menu row, default content; text on it 5.0:1 |
| `OXBLOOD` | selection | #882820 | Selected points and segments fill, mute lamp, FX role accent (piano-roll notes: white outline since 2026-11-13) |
| `DANGER` | danger | #C7847C | Readable error/destructive text, record lamp, Write mode, punch |
| `LOOP_ACTIVE` | loop active | #E5C34F | Active Loop glyph and loop band |
| `SOLO` | — (token only) | #D8B23C | Solo lamp only, with a dark label |
| `DIVIDER` | divider | #2B3B3D | Menu and call-out edges, separators, a disabled control's edge. Never a region rule |
| `GRID` | grid | #243236 | Beat and subdivision lines |
| `BAR_LINE` | barLine | #3A4D50 | Bar lines, dashed empty-node edges |
| `CONTROL_EDGE` | controlEdge | #3A4D50 | Hover edge of field controls and the checkbox box (1.8:1 on panel) |
| `CAP` | cap | #CCD3D1 | Fader cap and knob body satin |
| `CAP_LIGHT` | — (token only) | #EDF1EF | Cap highlight |
| `CAP_DARK` | modulation | #8C9994 | Cap shade, modulation cables and arcs, LFO curves |
| `GLOBAL_HEADER` | globalHeader | #131E21 | Global band (tempo, markers, arrangement) headers |
| `GLOBAL_LANE` | globalLane | #14221F | Global band lanes |
| `AUTOMATION_HEADER` | — (token only) | #070D0F | Recessed lane headers under a track (automation, takes, CC) |
| `AUTOMATION_LANE` | — (token only) | #0C1416 | Recessed lane bands |
| `AUTOMATION_OUTSIDE` | — (token only) | #070D0F | Region lane outside every part |
| `OUT_OF_SCALE` | — (token only) | #C75A50 | Piano-roll row outside the scale in force (D06.3, revised 2026-11-13): a 3-pixel edge at the row's visible left, the row tinted at 5 % |
| `SIDECHAIN` | sidechain | #A98BD0 | Sidechain outlines, pins and cables only |
| `MIDI` | midi | #C08A4E | MIDI pins and cables, velocity bars in nodes, range bands |
| `CREATE` | create | #E0913A | The "+" that creates tracks only (D101; T41: the tool-row create glyph) |

`theme::routing_accent(role)` gives routing channels their accent: FX oxblood, bus peacock, master
accent, tracks secondary. Role accents tint only a name box, a fader cap and cables, at 10 % when
used as a wash.

### Track colours

Nine muted hues (`theme::track::COLOURS`), minimum pairwise CIELAB distance 21.7 (test floor 19).
Every hue labels at 4.5:1 or more with `foreground_on` (see [Contrast](#contrast)): Peacock
#217264, Ochre #AE763F, Slate blue #4466A3, Lavender #9377B0, Sage #5E8F62, Oxblood #A83C2C,
Steel blue #6E9AB8, Brass #8F8A34, Dusty rose #B46E80. T18 moved Peacock and Slate blue darker
(light label) and Ochre, Lavender and Dusty rose lighter (dark label), in CIELAB lightness only;
the T40 values were #2E7D6E, #A8703A, #4A6BA8, #8B6FA8 and #B06A7C. Users may also choose any
colour through the [colour picker](components/colour-picker.md) — never a fixed swatch list.

Since T40 the colour is stored with each track (`Track::colour`), assigned at creation by cycling
the nine hues (`clef_model::project::TRACK_COLOURS`; `theme::track::COLOURS` is built from it).
Loading a project maps a track that still carries one of the five T40 values exactly to its
replacement; any other stored colour is kept.

A track colour appears on: the 4-pixel stripe at the left of its arrangement header, its parts,
unselected notes, velocity bars, controller curves, folder previews, the mixer name box tint. It
never washes a track header, channel strip or mixer strip background. A child without a colour
shows its nearest coloured folder's.

### Lamps and state colour

- Mute and solo are the only lamps of headers and strips: mute oxblood, solo yellow with a dark label.
- The transport owns the two global lamps: play accent green, record danger red, painted as the
  glyph's own colour while running or recording. A lit lamp stays lit while its button is disabled.
- The active Loop glyph and the loop band share `LOOP_ACTIVE`.
- Every other active glyph is accent (tools, punch, metronome, fit buttons). Never turn a lamp
  into a filled tile, border or underline.
- Selected piano-roll notes keep their track colour with a white (`CAP_LIGHT`) outline
  (decision, 2026-11-13); the arrangement's inline notes are filled white when selected; points and
  segments take the oxblood selection fill; arrangement parts keep their colour and add an
  outline/handles. The interface accent never follows the selected track's hue.
- Label colour on any fill comes from `theme::foreground_on(fill)`: the WCAG contrast picks
  `TEXT` or `BACKGROUND`. Never hard-code white on a track colour.

### Contrast

WCAG 2 AA (T18 Phase 3): 4.5:1 for text, 3:1 for focus edges, glyphs and other non-text cues that
identify a control. `theme::CONTRAST` lists the pairs the surfaces draw; a test fails any pair
below its requirement and any drift between this table and the code. The ratio column is the
weakest measured pair (for track hues, the weakest hue). `foreground_on` rows name the fill.

| Foreground | Background | Requirement | Ratio |
| --- | --- | --- | --- |
| `TEXT` | `BACKGROUND` | text 4.5 | 14.28 |
| `TEXT` | `PANEL` | text 4.5 | 12.98 |
| `TEXT` | `CANVAS` | text 4.5 | 14.00 |
| `TEXT` | `FIELD` | text 4.5 | 11.21 |
| `SECONDARY` | `BACKGROUND` | text 4.5 | 8.26 |
| `SECONDARY` | `PANEL` | text 4.5 | 7.51 |
| `SECONDARY` | `CANVAS` | text 4.5 | 8.10 |
| `SECONDARY` | `FIELD` | text 4.5 | 6.49 |
| `ACCENT` | `BACKGROUND` | text 4.5 | 7.58 |
| `ACCENT` | `PANEL` | text 4.5 | 6.89 |
| `ACCENT` | `FIELD` | text 4.5 | 5.95 |
| `DANGER` | `PANEL` | text 4.5 | 5.38 |
| `DANGER` | `FIELD` | text 4.5 | 4.65 |
| `foreground_on` | `PEACOCK` | text 4.5 | 5.00 |
| `foreground_on` | `OXBLOOD` | text 4.5 | 7.14 |
| `foreground_on` | `SOLO` | text 4.5 | 8.73 |
| `foreground_on` | `ACCENT` | text 4.5 | 7.58 |
| `foreground_on` | track hues | text 4.5 | 4.60 |
| `ACCENT` | FIELD (focus edge) | non-text 3 | 5.95 |
| `DISABLED` | `PANEL` | exempt: inactive control | 4.09 |
| `DISABLED` | `FIELD` | exempt: inactive control | 3.53 |
| `CONTROL_EDGE` | `FIELD` | exempt: decorative; controls are identified by label or glyph | 1.56 |

## Typography

System sans (SF Pro on macOS, weight 700 for bold), loaded into egui at startup from the system
font as `FontFamily::Name("sf-pro")`. No bundled web font and no substitute: egui's default font is
a development fallback only, never shipped. `theme::text` exposes the six styles below; surfaces use
a style, never `RichText::size`.

| Rust style | Size | Use |
| --- | --- | --- |
| `HEADING` | 16 | Window and page headings |
| `BODY` | 13 | Default control and row text, track names (full density) |
| `SMALL` | 11 | Captions, compact names, lane names, ruler row labels |
| `CAPTION` | 10 | Chip words, knob values, column captions, footers |
| `TINY` | 9 | Locator band labels, tiny captions |
| `MENU` | 14 | Menu rows, startup lists |

egui's own `TextStyle::Heading` is configured to 18 by default: that default is wrong here. Set
`Heading` to `HEADING` (16) in the startup style, or use the token explicitly.

Numbers that the composer reads (dB, ms, samples, beats) always carry their unit and stay readable
beside the control; nothing is truncated — a node or column grows instead, and an ellipsised name has
a full tooltip.

## Metrics

Window and chrome (`theme::metric`; token names map to constants in SCREAMING_SNAKE_CASE, for
example `controlHeight` → `CONTROL_HEIGHT`):

| Token | Value |
| --- | --- |
| body / small / heading (font) | 13 / 11 / 16 |
| toolTarget | 28 |
| toolGlyph | 16 |
| toolStroke | 1.5 |
| controlHeight | 24 |
| nesting | 16 |
| stripe | 3 |
| channelWidth | 92 |
| channelGap | 1 |
| minimumHeaderWidth | 160 |
| maximumHeaderWidth | 400 |
| folderHeight | 24 |
| transportHeight | 40 |
| menuBarHeight | 28 |
| inspectorWidth | 208 |
| statusHeight | 24 |
| tabHeight | 28 |
| workspacePadding | 12 |
| minimumWindowWidth | 1024 |
| minimumWindowHeight | 640 |
| railWidth | 2 |
| controlBodyInset | 2 |
| controlPadding | 8 |
| chipHeight | 20 |
| selectionEdge | 1.5 |
| scrollBar | 12 |
| menuRow | 26 |
| menuSeparator | 9 |
| menuCaption | 22 |
| menuTick | 16 |
| menuPadding | 4 |

Piano roll (`theme::roll`):

| Token | Value |
| --- | --- |
| keybedWidth | 56 |
| rulerHeight | 24 |
| pitchRowDefault / Minimum / Maximum | 12 / 6 / 24 |
| noteRadius | 2 |
| handleWidth | 3 |
| playheadWidth | 1 |
| velocityLaneHeight | 72 |
| controllerLaneHeight | 64 |
| laneMinimumHeight / laneMaximumHeight | 40 / 240 |
| laneGutterHeight | 4 |
| laneHeaderButton | 14 |
| velocityBarWidth | 3 |
| minimumGridPixels | 7 |

Corners: 2 points on ordinary controls (`theme::metric::CORNER_CONTROL`). Track heights follow the
D18 presets (Micro 24 … Jumbo 320).

## Depth

Depth belongs to the satin fader caps, knob bodies, recessed travel slots and (since UI10 Fix 7)
the raised and recessed field controls described below, plus soft shadows drawn only by primitives
([shadows](components/shadows.md)): caps and knobs carry a short shadow kept inside their own
bounds; black piano keys cast a faint shadow on the white deck; notes and parts lift while dragged;
the transport bar casts a short falloff into its margin. The startup screens' panel and damask
backdrop ([startup](views/startup.md)) are the one bounded container and the one texture in the app.
Never a shadow on menus, popups or cards, never a shadow used as a rule. Every linear travel is a
`field` groove with a `divider` rim and an inner shadow, because a bare fill disappears on one of
the two dark surfaces.

**Control depth (UI10 Fix 7).** `field` is only 1.16:1 against `panel`, so a fill alone never
carries a control's shape; soft depth does, painted by the primitive helper
`ui::paint::control_body` (T18):
- **Raised** — every field control you press or type into: text and slot buttons, mute/solo at
  rest, combo boxes, menu buttons, mode chips, text fields, value boxes and editable values (the
  user moved fields from recessed to raised, variant B, on 2026-11-11). A 1-pixel top light (white
  5 %) and a tight blurred drop shadow (black 35 %, 1 px down, 3 px blur) that falls mostly in the
  strip under the body. T18 Phase 3 (2026-11-21, user choice "C" of a shadow study) replaced the
  earlier hard two-line shadow (black 55 % then 24 %) and the 9 % top light as dated.
- **Recessed** — only travels and tracks other controls sit in: fader grooves and segment tracks.
  An inner top shadow (black 50 % over 3 px) and a 1-pixel bottom light (white 7 %).
- **One body height.** Raised and recessed controls share `controlBody`, 2 px shorter than the
  bounds, in every state; the strip under a raised body holds its shadow and stays empty under a
  recessed one, so buttons and fields side by side line up.
- **Hover** lifts the body to `field` and never adds a border; keyboard focus keeps its `accent`
  edge.
- **Lit** fills (peacock, oxblood mute, solo, accent) and pressed buttons keep the body and its
  shadow and drop the top light, so they read as pushed in without changing size. **Disabled** is flat `panel` with a `divider` edge.
- Checkboxes keep their `controlEdge` box; knobs keep their cap shadow with a `divider` rim.
  Glyph-only buttons (tools, power, arm, monitor, tap) stay bare glyphs.

Trade-off, accepted by the user: at rest the top light and the shadow are faint cues, below the
always-on edge's 1.81:1 (fainter still since T18 Phase 3); the label, the depth and the hover fill
carry the target. Depth is a control's own, never a region rule or a card.

## Separation

**Never separate regions with a rule.** No long thin line between chrome rows, panels, the channel
strip and the arrangement, or between rolled surfaces. Surfaces already differ: use margins and
background roles, plus at most a region shadow inside the spacing. A line is allowed only as a
control's own edge (field or meter outline, fader travel slot, meter ticks, fader index), a menu
separator, and a bounded hierarchy guide inside a track header. Sections inside a panel are grouped
with spacing, never section rules. Two seams are deliberate exceptions: the 2-pixel global-band seam
and the 2-pixel pinned-block seam.

## States

| State | Rendering |
| --- | --- |
| normal | Role colours as specified per component |
| hover | Glyph or text tone lifts slightly; rows lift to `field`. Never a new border |
| down | Glyph tone deepens |
| active / on | Glyph turns `accent` (or its lamp colour); filled controls use `peacock` |
| selected (row, header, strip) | The [selection edge](components/selection-edge.md): 1.5-pixel `accent` at 80 % inside the bounds; headers and strips keep `panel`, text-only rows add `field` |
| selected (content) | Piano-roll notes: white outline over the track colour; inline notes: white fill; points, segments: oxblood fill |
| focus (keyboard) | A separate focus circle or ring, only for keyboard navigation (`show_focus_ring`: a focus given by a click draws none, since the click already changes the control's state and colour; text fields keep their focus edge while typing); never used as selected decoration |
| disabled | `disabled` colour, still semantically disabled for accessibility |
| unavailable data | Dash or "unavailable" in `disabled`/`secondary`, never a fake value |
| destructive | Menu entries starting with Delete, Remove, Clear (and Replace/Write where marked) in `danger` |

Contrast (tested): primary and secondary text at least 4.5:1 on every surface, text on `peacock` at
least 4.5:1, active non-text glyphs at least 3:1, `disabled` at least 3.5:1 on `field` (readable but
clearly below `secondary`), and the hover `field` body distinguishable from `panel`.

## Pointer

The armed tool is the cursor over editing surfaces (pencil, brush, eraser from `theme::cursor`),
drawn with a dark halo; Select keeps the platform arrow. Resize edges, the ruler hand and text fields
keep their own cursors; Control over a part's edge band shows Clef's stretch cursor (D71,
[icons](components/icons.md)).

## Interaction conventions

- A level control returns to its documented default on double-click and on Option-click.
- Alt bypasses snap on every drag; Escape cancels a drag or staged preview.
- A completed gesture is one undo step; view state (fold, visibility, sizes, scroll) never creates an
  undo step and never dirties the document unless stated.
- Context menus are short projections of the command registry; command search covers discovery.
  Never hide frequent editing behind deep menu trees.
- Numeric parameters are [value sliders](components/fader.md#value-slider): drag, ⇧-drag fine, double-click the value to type it, double-click the travel to reset. Never a bare input field for a number (decision, 2026-11-13).
- Native text fields keep native editing (selection, clipboard, IME).
- Static labels are not mouse-selectable and never show a text cursor; only editable text fields
  accept a caret and selection.
- OS window chrome, native file dialogs and plugin-owned editors are never restyled.

## Commands

Every user action is a command in the shared registry (`crates/ui/src/commands.rs`; today in `crates/ui/src/lib.rs`):
listed in Actions / keyboard shortcuts and in Commands search, with a user-assignable keystroke,
context-aware enablement, and the live binding shown in menus and tooltips (omitted when unbound).
A button, glyph, chip or menu row invokes a command; it never carries private behaviour.

## Accessibility

Every control has an accessible title and description; glyph-only controls name themselves in their
title and tooltip. Labels are available independently of colour. Keyboard focus order follows the
visual order; Tab walks inline-renamable names in row order.
