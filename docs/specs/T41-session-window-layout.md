# T41 — Session Window Layout Cleanup

- **Status** — In Progress — 2026-11-22: round 3 (GUI review) approved by the user with the
  proposed defaults and implemented; awaiting the user's GUI check. 2026-11-22: round 2 (GUI review) approved by the user with the proposed defaults. 2026-11-21: Specification and proposed answers to the open questions
  approved by the user on 2026-11-21.
- **Priority** — P1 (user request: cleaner main window before further feature work).
- **Task** — [T41](../../TASKS.md)
- **Dependencies** — Builds on [T18](T18-ui-foundation.md) primitives and
  [T40](T40-editing-surfaces-cleanup.md) arrangement. Related exploration:
  [T42](T42-multi-project-tabs.md).

## Goal

Reorganise the main session window so the transport, tools, track headers, inspector, and
diagnostics each have one clear place, follow [docs/UI](../UI/README.md), and stop showing
debug text in the working area.

## Problems observed (2026-11-21, user screenshot `T12-listening-v2`)

1. Fit Width and Fit Height are one-way: after pressing them the tracks grow and there is no way
   back. The [tool row](../UI/views/tool-row.md) already specifies that pressing again returns to
   the previous view, lit `accent` while active.
2. Track headers are tall but only show name, a truncated plugin caption, and one glyph. Mute,
   Solo, Arm, and Monitor are missing.
3. The selected track header draws the [selection edge](../UI/components/selection-edge.md)
   only around its name slot, so the edge stops short of the arm control and the meter.
4. The transport is a floating box, not full width, and does not stand apart from the rest of the
   window.
5. Snap and the fit glyphs share space with the transport area instead of sitting in a tool row.
6. "Audio active" is shown under the project title.
7. The selected-track inspector floats at the right, half height.
8. The inspector shows a debug MIDI status line ("MIDI input unavailable / not selected /
   selected track").
9. A standalone Settings button duplicates the menu entry.
10. Device and engine diagnostics (sample rate, buffer, lead, plugin latency, clipped samples,
    Track diagnostics) sit as plain text at the bottom of the window.
11. The vertical zoom controls are not aligned with the horizontal ones at the corner they share.

## Decisions (2026-11-21)

Interview answers from the user, 2026-11-21:

1. **Fit glyphs are toggles.** Pressing Fit Width or Fit Height again restores the view saved
   before the fit. Each glyph is lit `accent` while its fit is active. Any manual zoom of that
   axis ends the fit state, and the saved view is discarded.
2. **Track header controls.** Every instrument header shows Arm, Mute, and Solo, in the
   order of the [track header](../UI/views/track-header.md) anatomy (right to left before the
   meter: Solo, Mute, Arm), using the existing primitives. Monitor is deferred to
   [T14](T14-audio-recording.md) (user decision, 2026-11-21): Clef has no monitor function yet,
   and it is defined for audio tracks. Header controls reuse
   ([mute and solo](../UI/components/mute-solo.md),
   [track-state button](../UI/components/track-state-button.md)).
3. **Selection edge.** The selected header draws the selection edge (the 1.5-point `accent`
   outline) around the whole header, not around the name slot only. Correction recorded
   2026-11-21: the first draft called the outline itself wrong; the selection edge is an outline
   by design.
4. **Transport.** Full window width, separated from the rest by a region shadow
   ([shadows](../UI/components/shadows.md), "Region"), as in the Composer DAW reference. The
   region shadow is implemented as a primitive, with its doc and golden.
5. **Tool row under the transport.** Snap, Fit Width, Fit Height, Add Instrument Track, and a
   track search field move into a [tool row](../UI/views/tool-row.md) below the transport. Add
   Instrument Track and the search field sit at the left, above the track headers.
6. **Remove "Audio active".**
7. **Project title stays where it is.** Multi-project tabs are a separate exploratory task,
   [T42](T42-multi-project-tabs.md).
8. **Left column.** The inspector moves to the left. One column of `inspectorWidth`: the
   inspector in the top half, the selected track's [channel strip](../UI/views/channel-strip.md)
   in the bottom half, each with its own scroll. The channel strip reuses the mixer strip code.
9. **Remove the debug MIDI status line** from the inspector.
10. **The in-window menu bar stays** (File / Edit / … is Clef's own, not the OS menu).
11. **Remove the Settings button.** Settings remains in the menu. Commands… stays.
12. **Transport format readout.** The transport shows the sample rate and the recording bit depth,
    for example `48 kHz · 24-bit`. Finding during implementation (2026-11-21): Clef records MIDI
    only, so no recording bit depth exists yet (the engine's 24-bit WAV writer is the offline
    render, not a recording format). The transport shows the device sample rate now; the bit
    depth joins it when [T14](T14-audio-recording.md) defines the audio recording format.
13. **Status bar.** The remaining diagnostics (requested and actual buffer, lead, plugin latency,
    clipped samples) move to one compact status row at the bottom of the window, `secondary`
    text. Track diagnostics becomes a panel opened from the Window menu.
16. **Editor and Mixer buttons in the status bar** (added at approval, 2026-11-21). Centred in the
    status row, two clickable labels behave as buttons: **Editor** opens the editor of the selected
    track (the MIDI piano roll today, the audio editor once T14 exists) and **Mixer** opens the
    mixer. Each is lit while its view is open and closes it on a second click.
14. **Master row unchanged** for now.
15. **Zoom controls.** The horizontal zoom controls stay as they are. The vertical zoom controls
    move so that both pairs meet cleanly at the shared bottom-right corner of the arrangement.

## Round 2 — GUI review (2026-11-22)

The user checked the first implementation in the running app and left 15 comments on the
screenshot (Claude Design canvas "T41 GUI review"). Interview answers the same day are marked
**(user)**; items marked **(proposed)** are defaults written for approval.

### Layout fixes

R1. **Transport margins.** The transport band keeps the 12-point side margins of the rest of
    the window instead of bleeding to the window edges; region shadow unchanged.
R2. **Snap moves into the transport**, after the musical position.
R3. **Tool row order.** Track search first, then a create-tracks glyph as tall as the search field
    (24 points), coloured orange **(user)**: the existing `CREATE` role (#E0913A, D101: the "+" that creates a track), so no new token; the fit toggles move to the
    right end of the row.
R4. **Left drawer toggle.** A glyph at the left end of the status row shows and hides the left
    drawer; hidden, the drawer gives its width to the arrangement. Saved per window session only
    **(proposed)**.
R5. **Editor | Mixer** are flat clickable texts with a divider between them, not buttons: the
    [tabs](../UI/components/tabs.md) look, `accent` while shown, `secondary` otherwise, and a
    click on the shown one hides it.
R6. **Drawer split by content.** The inspector takes the height of its content; the strip area
    takes the rest. No empty half.
R7. **Two strips, as in Logic.** The strip area shows the selected track's strip and, beside it,
    the strip of its output destination (its bus, else Master).
R8. **Inspector buttons removed.** Rename, Move Up / Down, Remove Track, Open Editor and Close
    Editor leave the inspector. Remove Track stays a command and a header context-menu entry;
    plugin editors open from their slot in the strip.
R9. **"Audio active."** no longer appears in the status message.

### Interaction

R10. **Inline rename.** Double-click a header's name to edit it in place. Enter commits; Tab
     commits and starts renaming the next track (Shift+Tab the previous one); Escape cancels.
R11. **Drag to reorder.** Press on a header (not on its controls) and drag vertically past
     4 points: the header lifts and follows the pointer, and the other headers slide out of its
     way with a short animation (about 120 ms ease-out) **(user: animation, no accent line)**.
     Release moves the track as one undo step; Escape cancels and everything slides back.
R12. **Drag-paint Mute and Solo** **(user: not Arm; Arm stays single-track)**. Pressing a lamp
     toggles it; dragging over the same lamp of other tracks (vertically in headers,
     horizontally in the mixer) sets them to the same new state. The gesture is one undo step
     **(proposed)**.

### Create tracks dialog

R13. The create glyph (and a Track menu entry "New Tracks…", **(proposed)**) opens a dialog
     **(user)**: number of tracks (1 up to the project limit), type (Instrument / Bus), name
     (numbered "Name 1…n" when more than one), and for Instrument tracks an optional instrument
     plugin loaded on each new track. Add Instrument Track and Add Bus stay quick commands.

### Counters and project format

R14. **Wider position display.** The segment display holds bar.beat.tick and a second counter
     the user picks by clicking it **(user)**: time `h:mm:ss.mmm`, samples at the running rate,
     or SMPTE `hh:mm:ss:ff`. SMPTE frame rate is chosen in the same menu: 24, 25, 30, default
     25 **(proposed)**. The choice is saved with the project **(proposed)**.
R15. **Format readout.** Beside the counters, on two small rows: the project sample rate
     (`48 kHz`) and bit depth (`24-bit`). Clicking either opens a Project Format dialog.
R16. **Project Format dialog.** Sample rate sets the rate of the project's audio override
     (T39) **(user)**, creating the override from the current settings when the project has
     none, and restarts audio at that rate; a rate the device refuses leaves everything unchanged
     with a clear error. Bit depth: 16, 24, or 32-bit float, default 24 **(user)**, saved in the
     project and used by audio recording once T14 exists. Project format version 7 adds the bit
     depth and the counter choice; older projects load with the defaults.

## Round 3 — GUI review (2026-11-22)

The user left 15 comments on four screenshots (Claude Design canvas "Clef screenshot review":
Mixer, Editor + Piano Roll, Editor with no clip, Editor after round 2). Interview answers the
same day are marked **(user)**; items marked **(proposed)** are defaults written for approval.

### Bottom panel: Editor and Mixer

S1. **The mixer docks like the editor.** It opens in the bottom panel under the arrangement,
    not over the whole workspace. Editor and Mixer are exclusive **(user)**: opening one closes
    the other, as the Editor | Mixer tabs in the status row already show.
S2. **Panel window controls.** The right end of the panel's tool row holds three glyph buttons,
    the same for Editor and Mixer: **Detach** (moves the panel into its own OS window, an egui
    viewport, which can sit on another monitor), **Half / Full** (toggles the panel between half
    the editing column's height and the whole of it, the arrangement hidden; lit `accent` while
    Full), and **Close** (X). The piano roll's text "Close" button is removed **(user)**.
    **(proposed)** The detached window has the same row with Attach in place of Detach; closing
    it closes the panel; the next open uses the last mode (docked or detached). Mode and
    Half / Full are window-session state, not saved in the project.
S3. **No panel titles.** The "Mixer" and "Piano Roll" headings are removed.
S4. **"+ Bus" removed** from the mixer **(user)**: buses are created from New Tracks… and the
    Add Bus command.

### Piano roll

S5. **Fit glyphs right-aligned** in the piano roll tool row, just left of the S2 controls.
S6. **MIDI Functions drawer.** The MIDI Functions toggle opens a drawer inside the editor, left
    of the keyboard, instead of a row above the grid. Its sections (Quantize, Transpose,
    Humanize) are stacked vertically, each an accordion that expands and collapses with a
    quick animation (about 120 ms ease-out, as R11) **(proposed)**. Several sections may be open
    at once; open state is window-session only **(proposed)**.
S7. **Velocity handles.** Each velocity stem ends in a filled dot (lollipop) that is the grab
    target; the dot takes the note colour, `accent` outline when selected **(proposed)**.
S8. **Keyboard shading.** A light shadow gives the keys depth without skeuomorphism: white keys
    get a faint bottom-edge shade, black keys a short drop shadow onto the white keys
    **(proposed)**, from existing shadow tokens.

### Empty states

S9. **One empty-state primitive**, centred horizontally and vertically in its area: a large
    icon (48 points, `secondary`) above one line of text. Used by the editor with no clip
    selected ("Select a MIDI clip in the arrangement."), the mixer with no tracks ("No tracks
    yet."), and the arrangement with no tracks ("No tracks yet.") **(proposed wording)**, each
    with its own icon (piano keys, faders, track lanes) **(proposed)**. Doc and golden in
    `docs/UI/components/`.

### Arrangement

S10. **Resizable track column.** The divider between track headers and timeline can be dragged;
     a command (View → Wider / Narrower Track Column, **proposed**) steps it by 20 points. Width
     range 160–400 points, default the current width **(proposed)**; saved with the window
     session only **(proposed)**.
S11. **Ruler labels and a second ruler.** The left end of each ruler, above the track headers,
     carries a label naming its unit ("Bars"). A context menu on the label adds or removes a
     second ruler below the first whose unit is Time, Samples, or SMPTE, chosen from the same
     menu. Its unit is independent of the transport's second counter **(user)** and saved with
     the project **(proposed)**, so the project format version moves to 8.

### Transport

S12. **Format readout as digits.** The sample rate and bit depth use the transport's segment
     digits like the counters, on two rows: sample rate on top (`48.0` with `kHz`), bit depth
     below (`24` with `bit`) **(user layout; proposed unit captions)**. Clicking still opens
     Project Format (R15).

Not changed: the transport's second counter (R14) remains independent of the S11 ruler.

## Scope

In scope: the items above, their UI documentation updates (transport bar, tool row, track header,
window, multitrack session, channel strip, scroll and zoom bars, shadows, and `docs/UI/index.md`),
and the gallery goldens for new or changed primitives.

Out of scope: multi-project tabs (T42), Master row changes, clip colours and MIDI preview
rendering, menu bar changes.

## Acceptance criteria

1. Fit Width and Fit Height toggle: second press restores the previous zoom and scroll on that
   axis; lit while active; a manual zoom clears the lit state. Covered by a unit test.
2. Each instrument header shows working Arm, Mute, and Solo controls, dispatched through
   the command registry, with tooltips that show live shortcuts.
3. The selected header shows the selection edge around the whole header.
4. The transport spans the window width and casts a region shadow; nothing else uses it as a rule.
5. The tool row sits under the transport with Add Instrument Track, track search, Snap, and the fit
   glyphs; the transport no longer hosts them.
6. "Audio active", the inspector MIDI debug line, and the Settings button are gone.
7. The left column holds the inspector (top half) and the selected track's channel strip (bottom
   half); the right inspector is gone.
8. The transport shows `<rate> kHz` (the bit depth follows with T14); the status row shows the other diagnostics;
   Window → Track Diagnostics opens the diagnostics panel.
9. The status row shows centred Editor and Mixer buttons that toggle the piano roll of the
   selected track and the mixer, lit while open.
10. The vertical and horizontal zoom controls meet at the corner without overlap or gap.
11. `cargo fmt`, `clippy -D warnings`, and `cargo test` pass; gallery goldens updated; the user
    checks the GUI.

## Resolved questions (approved 2026-11-21)

1. **Track search behaviour.** Does typing filter the visible tracks, or only highlight and scroll
   to the first match? Decision: filter, Escape clears.
2. **Default track height.** With header controls added, what is the minimum row height that
   keeps all four controls visible? Decision: the header density rule in the track header doc;
   below 30 points the controls switch to compact targets.
3. **Left column toggles.** Should the inspector and channel strip each be hideable (giving their
   space to the other or to the timeline)? Decision: not in this task.

## Progress

- 2026-11-21 — Interview with the user; draft specification written.
- 2026-11-21 — Specification approved; Editor and Mixer status buttons added.
- 2026-11-21 — Monitor deferred to T14 (user decision). Implementation complete; awaiting the
  user's GUI check.

- 2026-11-22 — GUI review (15 comments), round 2 interview and approval; round 2 implemented;
  awaiting the user's GUI check.
- 2026-11-22 — GUI review round 3 (15 comments on four screenshots): interview, round 3 drafted
  (S1–S12) and approved by the user with the proposed defaults; implemented; awaiting the user's
  GUI check.

## Round 2 implementation notes

- R1 the transport sits in the window's 12-point side margins; R2 Snap after the counters
  (`push_id("transport-snap")`, the roll keeps its own); R3 `tool_button::create_button`
  (`Icon::Plus`, `CREATE`), fits right-aligned.
- R4 `Icon::Drawer` + `ToggleLeftDrawer` (`view.left-drawer`); R5 `tabs::toggle_tabs`.
- R6 the inspector's measured content height sizes the drawer's top part (≤ 60 %); R7
  `App::selected_strips` draws the selected strip and its output's (bus or Master); R8 inspector
  buttons removed, header context menu added; R9 the session no longer sets "Audio active.".
- R10 `ArrangementUi::renaming` + `RenameEnd`; Rename Track starts it.
- R11 `RowLayout` animates each track block's top relative to the scrolled origin (scrolling
  never lags); the dragged block draws last with `paint::lift_shadow`; `dropped` holds the new
  order until the session's matches (or nothing is in flight); `Action::MoveTrack`.
- R12 `MixerUi::paint` / `paint_lamp` / `finish_paint`; session `Action::CommitMixBatch` records
  one `Edit::MixBatch`.
- R13 session `Action::AddTracks` checks the limit before adding anything; UI `NewTracks`
  dialog (`track.new`).
- R14–R16 model `ProjectFormat` (`BitDepth`, `SecondCounter`, `smpte_fps`), project version 7;
  `Timeline::nanos`; `second_counter_text`; session `Action::SetFormat`; Project Format dialog
  sends `Action::Settings { project: true }` for the rate (`transport.project-format`).

## Round 3 implementation notes

- S1/S2 `App::editing_column` docks the lower panel (`App::lower_panel`: the mixer or the roll)
  under the arrangement with the existing resize gutter; `App::panel`
  (`primitives::panel_controls::PanelState`, window session) holds Detach and Full. Full skips the
  tool row and arrangement and disables clip commands. `App::detached_panel` draws the panel in an
  immediate egui viewport (`panel_viewport()`); closing that window closes the panel.
  `finish_paint` waits for the pointer in either viewport. New commands `view.panel-full`,
  `view.panel-detach` (View menu). Opening the mixer closes the roll and the reverse.
- S3/S4 the mixer and roll headings are gone; `Event::AddBus` and the mixer's "+ Bus" are removed.
  "Show Mixer" reads "Hide Mixer" while open.
- S5 the roll's tool row is a fixed row: tools left, fits right before `panel_controls::at_right`.
- S6 `PianoRoll::functions_drawer` (208 points, `panel`) left of the keyboard; sections use the new
  `primitives::accordion::section` (120 ms ease-out); `PianoRoll::sections` holds the open state.
- S7 velocity heads `ROLL_VELOCITY_HEAD`; S8 `paint_front_shade` with `KEY_FRONT_SHADE` and a
  layered black-key shadow.
- S9 `primitives::empty_state` with `Icon::Keys`, `Icon::Faders`, `Icon::Lanes`.
- S10 `ArrangementUi::track_column`, `transport::header_width(width, column)`,
  `MAXIMUM_HEADER_WIDTH` (400) in foundations; divider drag in `timeline_surface`; commands
  `view.track-column-wider` / `-narrower`.
- S11 `ProjectFormat::second_ruler` (project version 8; version 7 loads hidden);
  `timeline::ruler_labels` and `Viewport::unit_ruler` with `unit_step`.
- S12 `segment_display::two_row_readout`; `transport::rate_digits`, `bit_digits`.

## Implementation notes

- `arrangement::FitToggle` holds the view saved by a fit and the fitted value; arrangement and
  piano roll both use it.
- Header: `primitives::mute_solo::lamp` for Mute and Solo dispatches `MixerMute` / `MixerSolo`
  on the track's target through `run_mixer_command`; the header owns the selection edge and
  `ListRow::header()` no longer paints one.
- `primitives::transport::band` replaces the boxed bar; `paint::region_shadow` paints the falloff.
- `App::tool_row` (transport.rs) holds + Track, the track search (`ArrangementUi::track_search`),
  Snap, and the fit toggles; the timeline no longer draws them.
- `App::workspace` lays out the body with child UIs on fixed rectangles (left column, editing
  column, or mixer) and then the status row; `App::selected_strip` reuses the mixer strip.
- Engine `diagnostics()` / `rate()` split the summary; `SessionView::rate` carries the rate.
- New registry commands: `view.editor` (Show Editor, ⌘⇧E) and `window.track-diagnostics`.

## Verification

Automated (2026-11-21):

- `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`: clean.
- `cargo test --locked`: 277 passed, 26 ignored. New tests:
  `a_fit_toggles_back_and_a_manual_zoom_ends_it`, `track_search_filters_by_name_ignoring_case`,
  `the_rate_readout_shows_kilohertz_and_hides_while_audio_is_off`,
  `the_editor_toggle_opens_the_selected_roll_from_the_mixer_and_closes_it_again`,
  `the_status_row_puts_errors_before_recording_and_messages`; the accessibility walk covers the
  new `t41` stage.
- Goldens: `docs/UI/snapshots/egui/t41-{session,editor,mixer,search,fit}-{1024,760}.png`
  (`cargo run -p clef-ui --example t41_gallery -- docs/UI/snapshots/egui <1024|760>`); the
  t08–t40 project-window goldens were re-rendered for the new layout.

Round 2 automated (2026-11-22): fmt, clippy clean; `cargo test --locked` 282 passed, 26 ignored.
New tests: `new_tracks_adds_numbered_tracks_of_one_kind_and_refuses_past_the_limit`,
`a_reorder_drag_opens_a_gap_at_the_drop_slot_and_drops_before_that_track`,
`second_counters_read_time_samples_and_smpte`, `nanos_follow_the_tempo_map`,
`the_project_format_round_trips_and_version_6_projects_get_the_default`; the accessibility walk
covers the new counter and format controls. All project-window and primitive goldens
re-rendered (t08–t41).

Not covered by automated tests: drag-paint, drag-to-reorder and its animation, inline rename
keys, the two dialogs' layout, and a real device's response to a sample-rate change.

GUI: pending the user's check of round 2 in the running app.

Round 3 automated (2026-11-22): `cargo fmt --all --check`, `cargo clippy --all-targets --locked
-- -D warnings` clean; `cargo test --locked` 286 passed, 26 ignored. New tests:
`the_lower_panel_docks_one_view_and_full_height_hides_the_arrangement`,
`the_track_column_keeps_its_range_and_leaves_room_for_the_lanes`,
`the_second_ruler_marks_round_values_of_its_unit`,
`the_format_readout_reads_kilohertz_and_bits_as_digits`; the version 7 format load is covered in
`the_project_format_round_trips_and_version_6_projects_get_the_default`. The t15 add-bus check
became `t15_mixer_exposes_bus_sends_output_and_panel_controls_at_both_widths`. New goldens
`t41-{mixer-full,functions,no-clip,rulers,empty}-{1024,760}.png`; t08–t41 project-window goldens
and t18 primitive goldens re-rendered (the t11/t12/t15 mixer stages show the full-height mixer
without the drawer).

Not covered by automated tests: the detached window on a real display or second monitor, the
accordion animation, the column divider drag, and the shading as seen on a Retina screen.

GUI: pending the user's check of round 3 in the running app.
