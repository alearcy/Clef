---
name: Track header
kind: view
reference: composer::style::layoutHeaderControls, HeaderColumns, HeaderSlot, HeaderRowKind, paintGroupRail
status: mixed
impl: planned
reference-source: [Source/Arrangement.cpp (Controls::resized, ~line 289)]
components: [mute-solo, track-state-button, chip, automation-mode-chip, slot-button, level-meter, hierarchy-rail, text-field, selection-edge]
snapshot: snapshots/track-header.png
snapshots: [snapshots/track-kinds-headers.png]
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Track header

## Clef T11 meter

Each instrument header reserves a full-height, frameless 9-point stereo meter at the right edge,
after its other controls. The meter shows the same post-fader peak, hold and clip state as its
mixer strip. The hover label identifies the track and indicates clipping. Clear the clip latch
from the mixer strip or the mixer command. See the [760-point header snapshot](../snapshots/egui/t11-header-760.png).

The Master header is docked below the instrument track bank, with its meter aligned to the same
right edge. See the [pinned Master in a long bank](../snapshots/egui/t11-header-long-760.png).

## Purpose
The fixed left column of every arrangement row: identity and the controls of track, folder, bus,
FX return and master rows. It stays fixed during horizontal scroll.

## Anatomy
- 4-pixel track-colour stripe at the left (3-pixel `stripe` on folders), then the hierarchy gutter
  with one [rail](../components/hierarchy-rail.md) per enclosing folder; nested rows step right
  12 pixels per depth as a whole.
- Name ([inline rename](../components/text-field.md) on double-click), 13 px at full density,
  11 px compact. D100: on MIDI and audio tracks the kind glyph (`trackMidi`, `trackAudio`, in
  `secondary`, never a lamp) leads the name, 14 px on rows of 70 and taller, 12 px otherwise, 6 px
  before the name (`snapshots/track-kinds-headers.png`). Folder rows add a chevron and folder glyph, name on the top line, subtitle under it.
- Controls, right to left before the meter: Solo, Mute (MONO on master), monitor, arm, the
  automation mode chip, A n, T n, ◆N, inline notes (see Layout). No overflow button: since UI10
  Fix 6 the row's context menu holds every track action. Input, Output and FX route slots on large rows; their menus and pickers open beside the slot
  itself (`menuAnchor::ScopedAnchor`), never beside the inspector's copy.
- A 9-pixel stereo [level meter](../components/level-meter.md) fixed at the right edge, full height,
  8 pixels clear of the last control, with no frame around its two bars.
- The list always keeps the arm and monitor columns, since the master's MONO spans them; a list
  with only the master never lets MONO cover the mode chip.
- Selected: `panel` with the [selection edge](../components/selection-edge.md); never a colour wash.

## Clef T26 take lanes

The provisional Clef track row adds the `T n` chip beside the record-arm target when the track has
two or more takes. Its accessible name is “Take lanes, n takes” and it shows the on state with the
accent fill. The per-track record mode lives in the selected-track inspector as a `Record mode`
menu (`Global`, `Merge`, `Replace`, `New take`), not in the header; the transport keeps the global
mode chip. Take lanes are specified in [track lanes](track-lanes.md#take-lane-d66-t26).

## Density
`HeaderDensity`: at 30 pixels and above, controls are 20-pixel targets with 13-px names; below 30,
16-pixel targets with 11-px names. Below `minimumHeaderWidth` 160 the columns leave rather than overlap the name;
their commands stay in the context menu. Folder rows use the track row heights (`folderHeight` 24
is only the floor).

## Rules
- Controls occupy fixed columns, so they line up down the whole track list.
- Controls that do not fit leave for the track context menu, which offers the same commands.
- Every channel header carries the meter; controls always sit left of it.

## Layout (UI10.5)
`layoutHeaderControls` places every control on the name line in fixed right-aligned columns, right
to left, ending 8 pixels before the meter: S, M (22 each), monitor, arm (22 each), automation mode (58), A n
(32), T n (30), ◆N (30), inline notes (22), 2 pixels apart. The arrangement computes the list's
`HeaderColumns` once (arm and notes if any track takes input, T n if any has two takes, ◆N if any pins
macros) and every row keeps those columns, so a row without a control leaves its column empty.
Columns leave whole from the end of the order as the list's header narrows (mode and A n from 300,
T n 360, ◆N 400, notes 440) while the name keeps 72 pixels; nested rows use the list's width, so the
indent never removes a column. Targets are 20 pixels on every D18 height, centred on the name line
(the name line is 22 pixels on rows of 70 and taller, the whole row otherwise). Large rows add the
route slots on a second line: a MIDI track's `In: MIDI <input>`, `Out:` and `Plugin: <instrument>`
(the Plugin slot); an audio track's `In: <audio input>`, `Out:` and `FX: <inserts>` (D100). Each track
has one monitor button: MIDI monitoring on a MIDI track, audio monitoring on an audio track (lit while
monitoring, `A!` badge when its input is unavailable). An audio track shows no notes glyph and no T n;
its arm stays in its column and arms it for audio recording (D106): audio tracks arm independently, any number at
once, while a MIDI arm replaces the previous MIDI arm only; a recording track squares its target. Folders show M, S and A n; routing rows no arm, monitor,
notes or T n; the master shows MONO across the arm and monitor columns, M, the mode and A n. The
default header column is 320 pixels (user decision, 2026-11-11). Hidden controls stay in the
track context menu as the same commands.

## Context menu (UI10 Fix 6)
Right-click on a header (or a mixer strip) opens one menu, `Arrangement::trackMenu`, in sections:
the track's name (Rename, Track Height ▸, colours); **Play and Record** (Mute, Solo or MONO on the
master; Record Enable, MIDI Monitoring, Record Mode ▸, take lanes and New Take on MIDI tracks; Record
Enable and Audio Monitoring on audio tracks); **Input** (MIDI Input… on MIDI tracks;
Audio Input… and Configure Audio Input Groups… on audio tracks); **Routing** (Plugin… on MIDI tracks,
Audio Effects…, Sends…, Output…, Audio Interface Output… on MIDI and audio tracks, Route to New Bus,
Send to New FX Track; the master only its effects and output); **Editing** on MIDI tracks (Inline
Note Editing, Inline Velocity Lane); **Organise** (preset, hide, pin, folders);
then Delete Track in `danger` (never on the master). On/off entries carry a tick, every entry is a
registry command with its live shortcut (see [track state button](../components/track-state-button.md)).

## Empty space (D100)
Below the last track row: double-click creates a MIDI track, Cmd+click an audio track, right-click
opens the creation menu at the pointer (New MIDI Track, New Audio Track, separator, New FX Track,
New Bus Track, New Folder (groups the selection), separator, Add Multiple Tracks…), each a registry
command with its live binding. The mixer bank's empty space takes the same three gestures; the
FX/bus dock takes none.

## Clef T41 controls and selection

Left to right after the name: the take-lane chip when present, record arm, Mute, Solo, then the
meter. Mute and Solo are the [mute and solo](../components/mute-solo.md) lamps
(`LAMP_WIDTH` 25), the same controls and commands as the track's mixer strip. Monitor is not shown
until audio tracks exist ([T14](../../specs/T14-audio-recording.md)). The selected header draws the
[selection edge](../components/selection-edge.md) around the whole header, not around the name
slot; its name row ([list row](../components/list-row.md) in header mode) paints no edge of its own.

### T41 round 2 interaction

- **Rename in place**: double-click the name (or Rename Track) edits it in the header. Enter or a
  click elsewhere keeps it, Tab keeps it and renames the next track (Shift+Tab the previous one),
  Escape drops it.
- **Drag to reorder**: press on the header outside its controls and drag past 4 points. The
  header lifts (lift [shadow](../components/shadows.md)) and follows the pointer; the other
  tracks slide out of its way over 120 ms. Release moves the track (one undo step); Escape
  cancels and the tracks slide back. No insertion line is drawn.
- **Drag-paint Mute and Solo**: pressing a lamp sets the opposite state; dragging over the same
  lamp of other tracks sets theirs too; release commits all of them as one undo step. Arm stays
  one track at a time. The mixer strips paint the same way.
- **Context menu**: Rename Track, Choose Instrument, Remove Track….

## Clef T41 round 3 column width

[T41 round 3](../../specs/T41-session-window-layout.md#round-3--gui-review-2026-11-22) (S10): the divider between the track headers and the lanes can be dragged (resize-column
cursor, a 1-point `accent` line while hovered or dragged), and View → Wider / Narrower Track Column
steps it by 20 points. The width stays between `minimumHeaderWidth` (160) and `maximumHeaderWidth`
(400) and always leaves the lanes 240 points; the default stays 220 (176 below 550 points). The
tool row's search field follows the column. Window session only.
