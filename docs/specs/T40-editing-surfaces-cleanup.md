# T40 — Editing Surfaces Cleanup: Arrangement and Piano Roll

- **Status** — Done — 2026-11-21. Specification and UI proposal approved by the user on
  2026-11-21; implementation, automated evidence and the user's GUI check complete.
- **Priority** — P1 (user request: cleaner UI before further feature work).
- **Task** — [T40](../../TASKS.md)
- **Dependencies** — [T18](T18-ui-foundation.md) Phase 1 first (user decision, 2026-11-21): the
  theme, parity test, and first primitives land before this surface work. Builds on T09 (arrangement),
  T10 (piano roll), and the [dense editing surfaces](../UI/contract.md#dense-editing-surfaces) rules.

## Goal

Turn the arrangement and the piano roll from provisional form layouts into continuous editing
surfaces: one playhead across all lanes, adjacent tracks, real scrolling in both axes, horizontal
and vertical zoom, and a realistic piano keybed.

## Problems observed (2026-11-21)

Code references are to the state on 2026-11-21.

1. **Piano roll does not scroll vertically.** Each clip shows a fixed pitch window
   (`Lowest pitch` and `Rows` fields) in a fixed-height rect; the wheel reaches an outer
   `ScrollArea` that rarely overflows. Shift+wheel steps one semitone per frame regardless of
   delta. Horizontal movement is only a slider; there is no wheel scroll or anchored wheel zoom.
2. **Keybed is not a piano.** Every pitch is one uniform row; black keys are a dark rect inside
   their own row, white keys do not widen behind them.
3. **Playhead is fragmented.** `Viewport::paint_playhead` clips a line to each rect (ruler, each
   lane, Master, each pitch grid, each controller lane), so it breaks at every gap, header, and
   take lane.
4. **Tracks are separated.** Each track is an egui row with the global `item_spacing`
   (12, 8), so lanes have 8-pixel gaps and the lane sits 12 pixels from its header.
5. **No vertical zoom.** Track rows are fixed at 44 pixels; the roll's row height is
   `grid height / Rows`. Horizontal zoom is a slider below the tracks.

## Decisions (2026-11-21)

Interview answers from the user, 2026-11-21:

- **Order:** T18 Phase 1 first (done; look accepted 2026-11-21), then this task.
- **Piano roll is one grid.** One keybed and one pitch/time grid covering all 128 pitches with real
  vertical scrolling. Every selected clip draws in that grid in its track colour; the active clip is
  in front and is the edit target. `Lowest pitch` and `Rows` are removed.
- **Scroll and zoom bars follow the reference** [component](../UI/components/scroll-zoom-bars.md):
  always visible; horizontal −/+ at the right end of the horizontal bar, vertical −/+ stacked at the
  bottom of the vertical bar; ⌘-wheel zooms horizontally and ⌘⇧-wheel vertically at the pointer.
  Applies to the arrangement and the piano roll.
- **No divider between tracks.** Adjacent track rows touch with no line between them.
- **Track identity is a colour stripe.** A 4-point stripe in the track colour at the left of each
  header tells adjacent tracks apart.
- **Track colours are stored.** A new track field holds its colour, assigned at creation by cycling
  the nine `theme::track::COLOURS`; it stays stable when tracks are reordered. Projects without the
  field get colours in track order on load. A colour picker is a later task.
- **Clips and notes use the track colour** instead of `peacock`: clips, unselected notes, velocity
  bars and controller curves (foundations, track colours). Labels use `foreground_on`.
- **Arrangement vertical zoom is uniform:** one track height for all tracks, 24–320 points
  (the D18 Micro…Jumbo range), default 44, steps of ×1.25, saved with the project.
- **Piano-roll view is saved with the project, once:** horizontal zoom and pitch-row height are
  stored like the arrangement view (view state: no undo step, no dirty document). Scroll is not
  stored; it follows the opened clip.
- **Opening a clip centres its notes vertically;** an empty clip centres on C4 (MIDI 60).
- **Fit buttons are included:** fit-to-width and fit-to-height glyph buttons for the arrangement and
  the piano roll.
- **Keybed behaviour is unchanged** apart from geometry: clicking a key auditions the pitch under
  the pointer exactly as today, with black keys hit where they are drawn.

## Requirements

### Shared

1. **Single playhead.** Each surface paints its playhead once, after its lanes: from the top of the
   ruler to the bottom of the last visible lane (arrangement: through tracks, take lanes and the
   Master row; roll: through ruler, grid and controller lanes). It never crosses headers or the
   keybed. `Viewport::paint_playhead` per rect is removed.
2. **Scrolling.** Plain wheel or trackpad scrolls vertically and horizontally by the gesture's delta;
   Shift-wheel scrolls in time. Headers and keybed move vertically with their lanes; the ruler moves
   horizontally with them. The wheel is consumed by the surface, so no outer scroll area moves.
3. **Zoom.** ⌘-wheel and pinch zoom time continuously, anchored at the pointer; ⌘⇧-wheel zooms the
   vertical axis in whole ×1.25 steps, anchored at the pointer row. The −/+ buttons step ×1.25,
   anchored at the playhead when visible, otherwise at the view centre. Ranges: time 8–240 points
   per quarter (unchanged); arrangement track height 24–320; roll pitch row 6–24 (default 12).
4. **Scroll and zoom bars** sit along the bottom and right edges of each surface's lane area, the
   corner between them empty. The roll's bar leaves the toolbar.
5. **Fit.** Fit-to-width zooms and scrolls time so the content fits the visible width
   (arrangement: bar 1 to the end of the last clip or the loop end, whichever is later; roll: the
   selected clips). Fit-to-height sets the arrangement track height so all tracks fit (clamped to
   24–320) or the roll row height so the selected clips' note range fits (clamped to 6–24, plus one
   row of margin). Both are glyph [tool buttons](../UI/components/tool-button.md) bound to registry
   commands (Fit Width, Fit Height), placed after Snap in each surface's control row.
6. **Viewport transform.** One transform per surface drives drawing and hit-testing in both axes,
   and only visible tracks, notes and clips are painted (dense editing rules).

### Arrangement

7. **Adjacent tracks.** Header column and lane column share one row geometry: rows touch, no item
   spacing, no divider. Take lanes sit directly under their track at their own heights.
8. **Header stripe.** A 4-point stripe in the track colour at the header's left edge; the
   [selection edge](../UI/components/selection-edge.md) still marks the selected track.
9. **Track height** applies to track rows (not to take lanes, which keep their own heights) and the
   header content (name, caption, arm, chip, meter) stays centred; below 44 the caption line hides.
10. **Master row** stays docked below the scrolling tracks; the playhead continues through it.

### Piano roll

11. **Keybed.** A real piano: seven equal white keys per octave (each 12/7 rows tall), black keys 57 %
    of the keybed width and one row tall, drawn over the white keys with a faint shadow; C keys
    named ("C4"), names hidden when rows are under 9 points. 56 points wide.
12. **Grid.** White-key rows lighter, black-key rows on `canvas`; the B–C seam darker as the octave
    rule. Notes of every selected clip, active clip in front; inactive clips' notes at reduced
    opacity. Clicking an inactive clip's note activates that clip. The clip chips above the grid
    keep choosing the active clip.
13. **Controller lanes** show the active clip and share the grid's horizontal transform.
14. **Opening and switching** a clip scrolls time to its start (unchanged) and centres its notes
    vertically (C4 when empty).

### Model and persistence

15. `Track` gains a `colour` (RGB, `serde(default)`); `ArrangementView` gains `track_height`;
    the project gains a `PianoRollView { zoom_milli, row_height }`. All are validated on load and
    written as view state. The project format becomes version 6; version 5 files load with default
    view values and colours assigned in track order. Older Clef builds cannot open version 6 files
    (as for every format bump).

## UI proposal

Arrangement (lane area shown; transport and details panel unchanged):

```
row: Snap ▾  Add Instrument Track  [fit ↔] [fit ↕]
┌ header column ───────────┬ ruler ───────────────────────────────┬─┐
│▌Strings      ◎  T2  ▮▮   │ ███ MIDI ██        │                  │▲│  ▌ = 4-pt track-colour stripe
│▌Piano        ◎      ▮▮   │      ███ MIDI ███  │  (playhead: one  │█│  rows touch: no gap, no line
│▌Bass         ◎      ▮▮   │                    │   line, ruler →  │ │
│  take lane 1             │   ░░ take ░░       │   Master)        │−│
│                          │                    │                  │+│
├──────────────────────────┼────────────────────┼──────────────────┼─┤
│ Master               ▮▮  │                    │                  │ │
│                          │ ◀━━━━━━━━━━━░░░░░░░░░░░░░░░░░░░░░ − +│ │
└──────────────────────────┴──────────────────────────────────────┴─┘
```

Piano roll:

```
row: Piano Roll  Snap ▾  Follow  MIDI Functions  [fit ↔] [fit ↕]  Close
row: clip chips  [Strings · 1.1]  [Piano · 1.2]
┌ keybed ┬ ruler ──────────────────────────────────────────┬─┐
│▀▀▀▀▀ ▏ │ grid: all 128 pitches, vertical scroll          │▲│
│█████   │   ▬▬ active clip notes (track colour)           │█│
│▀▀▀▀▀ ▏ │   ▭▭ other selected clips, dimmed               │ │
│C4      │                  ┃ playhead: ruler → last lane  │−│
│        │                                                 │+│
├────────┼─────────────────────────────────────────────────┼─┤
│Velocity│ ▮ ▮▮ ▮                                          │ │
├────────┼─────────────────────────────────────────────────┤ │
│        │ ◀━━━━━━━━░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ − + │ │
└────────┴─────────────────────────────────────────────────┴─┘
```

Gallery cases (1024 and 760): arrangement with five tracks at default height, at 24 and at 160,
playhead mid-view, take lanes open; roll with one clip, two clips (active and dimmed), empty clip
centred on C4, row heights 6 and 24, playhead mid-view.

## Progress and evidence (2026-11-21)

Implemented and verified. The user checked the running app on 2026-11-21 (wheel, trackpad and
zoom gestures, both surfaces) and reported everything working.

**Model and session**

- Project format 6: `Track::colour`, `ArrangementView::track_height` (24–320, default 44),
  `Project::piano_roll_view` (`zoom_milli`, `row_height` 6–24, default 12). Version 2–5 files load
  with default views and colours in track order. New tracks, buses and takes-to-new-track take
  `next_track_colour(track count)`.
- `Action::PianoRollView` is view state like `Action::ArrangementView`: validated, never dirty,
  saved silently when leaving a clean saved project.

**Arrangement** (`transport.rs::timeline_surface`)

- Rows are laid out on one geometry with zero spacing inside a clipped region scrolled by
  `ArrangementUi::v_offset`; no egui `ScrollArea`. Header: 4-point colour stripe, then name row,
  take chip, arm and meter in visual and focus order, the meter on the edge.
- Wheel, ⌘-wheel/pinch and ⌘⇧-wheel through `scroll_zoom_bars::wheel`; vertical bar beside the
  rows, horizontal bar under the Master lane; Fit buttons and the four Fit commands.
- One playhead via `Viewport::playhead` from the ruler top to the Master bottom; the ruler only
  reports the tick it shows (drag preview included).
- Clips, edit previews and recording backgrounds use the track colour; clip titles hide under
  32 points of height.

**Piano roll** (`piano_roll.rs`)

- `PitchGrid` maps all 128 pitches with `row` and `scroll`; `pitch_editor` allocates one keybed
  and one grid, paints every selected clip (inactive clips at 45 %) and edits the active one; a
  click on an inactive clip's note activates it. `Lowest pitch` and `Rows` are gone.
- Real keybed (`paint_keybed`, `keybed_key`): seven equal white keys per octave, black keys 57 %
  wide over them with a faint shadow, C names from 9-point rows; clicking moves the keybed cursor.
- Opening, the first selected clip and the clip chips centre the notes vertically (C4 when empty),
  re-centred once after the controller block is measured.
- The grid fills the height left after the ruler, MIDI functions and controller lanes (measured
  on the previous frame); horizontal bar at the bottom, vertical bar beside the grid; one
  playhead from the ruler to the last controller lane. The comp strip and value lanes no longer
  paint their own playhead.

**Deviation recorded**

- Take lanes were not added to the gallery cases (their fixture needs recorded takes); the
  take-lane layout is covered by the existing T26 tests, which pass.

**Evidence**

- Automated: `rtk cargo test --workspace --locked` — 575 passed, 124 ignored. New tests:
  `version_5_projects_get_colours_in_order_and_default_views`, `new_tracks_cycle_the_nine_hues`,
  `new_tracks_take_cycling_colours_and_the_roll_view_is_view_state`,
  `zoom_keeps_the_anchored_tick_and_scroll_moves_by_points`,
  `rows_map_pitches_and_back_through_the_scroll`,
  `keybed_hits_black_keys_where_drawn_and_white_keys_by_their_wide_body`,
  `arrangement_and_roll_each_paint_one_continuous_playhead`; existing roll tests now drive the
  one-grid `pitch_editor`.
- `rtk cargo clippy --all-targets --locked -- -D warnings` and `rtk cargo fmt --all --check` clean.
- Gallery: `docs/UI/snapshots/egui/t40-{arrangement-five,arrangement-height-24,
  arrangement-height-160,roll-empty,roll-rows-6,roll-rows-24}-{1024,760}.png`; all T07–T40
  surface snapshots regenerated.
- GUI: checked by the user in the running app on 2026-11-21, no issues. Listening and hardware
  evidence not applicable.

## Out of scope

Per-track height; colour picker; fold and scale rows; articulation band; parts column; inline
(D49) keybeds; a rewrite of the top toolbar, transport, or selected-track panel.

## Acceptance criteria

- The playhead is one continuous line from the ruler to the last lane in both surfaces, during
  playback and scrolling (snapshot and frame test: one vertical playhead segment per surface).
- Tracks touch with no gap or line; each header shows its colour stripe; clips and notes use the
  track colour (snapshots).
- The roll scrolls through all 128 pitches with the wheel; the keybed matches the white/black key
  geometry above (unit test on key rectangles; snapshot).
- ⌘-wheel and ⌘⇧-wheel zoom at the pointer, −/+ and fit buttons work in both surfaces (frame
  tests for anchoring and clamping; manual GUI check with mouse and trackpad).
- Track height, roll zoom and row height survive save and reopen without dirtying the document;
  version 5 projects load with assigned colours (model tests).
- Existing T09/T10 editing tests still pass; `rtk cargo test --locked`, clippy and fmt clean.
- The user reviews the regenerated snapshots and the running app.

## Verification intent

Unit tests for the viewport transform, anchored zoom, fit, keybed geometry and colour assignment;
frame tests for the single playhead and wheel handling; gallery snapshots at 760 and 1024; manual
GUI check of wheel, trackpad and pinch.

## Open questions

None at specification time.
