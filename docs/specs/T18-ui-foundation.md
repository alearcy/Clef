# T18 — egui Design System, Commands, and Accessibility

**Status:** Done — completed 2026-11-21. Phases 1–3 were approved, implemented and accepted by
the user (Phase 3 including the debug primitive gallery and the new raised shadow); the user
closed the task the same day. Components still `impl: planned` belong to views that do not exist
yet (FX rack, chord lane, ARA, audio editor) and are built with those views.
**Priority:** P1 from 2026-11-21 (was P2).
**Date:** 2026-11-17
**Task:** [T18](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** Ongoing UI work; `docs/UI/` authority.

## Goal

Extract and formalize the UI foundation before the application grows more editing surfaces:
design tokens, reusable egui widgets, menus, focus and keyboard navigation, accessible
names, contrast, scaling, command palette, and shortcut editing. UI documentation must be
updated in the same change.

## Proposed Scope

- Extract T03's colors, metrics, and one-off widgets from `app.rs` into shared modules.
- Implement or consolidate reusable components selected from `docs/UI/components/`.
- Menus, context menus, tooltips, and keyboard accessibility.
- Accessible names and focus states for all controls; no label relying only on color.
- Contrast and scaling behavior, including window resizing.
- Command palette and user-assignable shortcuts through the command registry.
- UI documentation updates and mapping to the egui implementation.

## Out of Scope

A visual redesign; adopting native AppKit controls; web or mobile UI; a full component
inventory unrelated to implemented views.

## Open Questions

- Accessibility target and whether accesskit/screen-reader support is required now.
- Which custom-painted egui controls need accessible fallbacks.
- High-contrast and scaling requirements, and minimum supported window sizes.
- Keyboard-only workflows and focus order for complex views.
- Shortcut conflict policy and whether user remapping is in scope.
- Which `docs/UI` components are translated now versus when a view needs them.
- ~~How the egui implementation is documented relative to the legacy C++ kit.~~ Resolved
  2026-11-17: the C++ kit is reference-only and is not in this repository; the
  [UI contract](../UI/contract.md) defines the egui documentation format.

## UI Proposal Requirement

This task is itself the UI foundation. It still requires a written proposal for any new
or changed surface and user approval before implementation, per `docs/UI/README.md`.

## Documentation normalization (2026-11-17)

Planning work completed before implementation, so the task has an unambiguous starting point:

- `docs/UI/README.md` no longer names C++ as authority. `crates/ui/src/theme.rs` and
  `crates/ui/src/primitives/` are the target authority; `docs/UI/foundations.md` holds the values;
  `crates/ui/src/lib.rs` is explicitly interim until T18 lands.
- `docs/UI/contract.md` defines the four layers (tokens, primitives, surfaces, evidence), the
  required sections of a primitive and a surface document, the gallery and golden policy, the
  rules for agents, and a worked text-button contract.
- `docs/UI/foundations.md` keeps every token value but is Rust-first: Rust constant names, egui
  text styles, system SF Pro loading, no JUCE units.
- All 63 component and view files were normalized: `kit:` became `reference:`, legacy paths became
  `reference-source:`, and every file gained `impl: planned | app | gallery`. 61 files carry a
  reference-only banner.
- `docs/UI/index.md` states that `status` is design ownership and `impl` is availability; no
  component is `gallery` yet.

Decisions taken on 2026-11-17 (recorded in
[contract.md](../UI/contract.md#decisions-2026-11-17)): Rust tokens with a doc-parity test;
eframe screenshots for the gallery; the eight T04 primitives first; primitives before the T04
surface; the theme keeps the name Composer Dark.

## Phase 1 — First primitive batch (refreshed and approved 2026-11-21)

The 2026-11-17 proposal targeted the T04 surface. Since then T05–T39 added the arrangement,
piano roll, mixer and transport surfaces, `crates/ui/src/primitives/` (surface-level drawing
helpers), a minimal `theme.rs` (metrics plus a few colours), the SF Pro loader in
`primitives/project.rs`, and the snapshot registry in `crates/ui/src/gallery.rs`. The 2026-11-21
interview refreshed the scope:

- **Priority:** P1; Phase 1 gates [T40](T40-editing-surfaces-cleanup.md) (user decision).
- **Batch:** the eight primitives from 2026-11-17 plus the three T40 needs: tool button, menu
  button, and scroll and zoom bars.
- **Adoption:** each primitive replaces the raw egui widgets it covers on the existing surfaces in
  the same change. Layouts stay as they are; the look becomes consistent with the tokens.

**Deliverables**

1. `crates/ui/src/theme.rs` — every colour role from `foundations.md`, window/chrome and roll
   metrics, the six text styles, `foreground_on`, `routing_accent`, `track::COLOURS`. Colour
   literals elsewhere in `crates/ui/src` move into it.
2. Shared paint helpers — `control_body` (raised/recessed), `selection_edge`, shadow helpers.
3. Primitives in `crates/ui/src/primitives/`, each with every state, an accessible name and the
   tooltip rule: `text_button`, `notice_box`, `list_row`, `selection_edge`, `text_field`, `chip`,
   `window_frame`, `caption`, `tool_button`, `menu_button`, `scroll_zoom_bars`.
4. Gallery cases for each primitive and state in the existing `gallery.rs`, written to
   `docs/UI/snapshots/egui/`.
5. A token parity test that parses the colour, text-style and simple-metric tables in
   `foundations.md` and fails when either side drifts, plus a check that rejects colour literals
   outside `theme.rs`.
6. Adoption on existing surfaces: raw `egui::Button`, `TextEdit`, `ComboBox` and similar calls
   covered by a primitive are replaced; affected surface snapshots are regenerated.
7. Migrated docs for the eleven components (`impl: gallery`, Rust API, state matrix, usage,
   evidence), their `index.md` rows, and the surface docs whose look changed.

**Acceptance**

- `rtk cargo test --locked` passes, including token parity and primitive behaviour tests; a
  deliberate hex change in either place fails the parity test.
- The gallery writes one image per primitive per state; two runs on the same machine at
  `pixels_per_point = 1.0` are compared and the result recorded.
- Every primitive doc has the sections required by the contract; no component is `gallery`
  without its image.
- No surface calls a raw egui control that one of the eleven primitives covers.
- The user reviews the primitive gallery and the regenerated surface snapshots and accepts them
  as the intended look, not merely "not broken".

**Order:** theme and parity test → selection edge, caption, text button, tool button → list row and
notice box → text field, chip, menu button → scroll and zoom bars → window frame → adoption on
surfaces → gallery pass and doc migration.

**Out of scope:** the T40 editing-surface changes (playhead, adjacent tracks, single roll grid,
keybed, vertical zoom); window layout redesign (Phase 2); remaining components; command registry
extraction; a full accesskit audit; light theme; scaling beyond `pixels_per_point = 1.0`.

**Risks and open items**

- SF Pro: the loader reads `SFNS.ttf`; verify it renders correctly as a variable font in the
  first gallery pass. If not, choose between CoreText extraction and Helvetica Neue before goldens
  are committed.
- `window_frame` is an in-app frame for Clef dialogs, never a repaint of the OS title bar.
- Adoption touches every surface; snapshot regeneration is the regression check.
- Scroll and zoom bars land as a primitive here; wiring them into the arrangement and piano roll
  (wheel, ⌘-wheel, ⌘⇧-wheel, vertical zoom) is T40.

## Phase 1 progress and evidence (2026-11-21)

Done: implemented, look accepted and checked in the running app by the user on 2026-11-21.

**What changed**

- `theme.rs` holds every colour role (29), the window/chrome and roll metric tables, the six text
  styles, `track::COLOURS`, `foreground_on` and `install_fonts`. The former `colors` module in
  `lib.rs` is gone; `MIX_MUTE`/`MIX_SOLO` became `OXBLOOD`/`SOLO`. Five metrics the primitives
  needed were added to foundations: `controlBodyInset`, `controlPadding`, `chipHeight`,
  `selectionEdge`, `scrollBar`.
- SF Pro is now the first proportional face app-wide and egui's Heading/Body/Button/Small styles
  map to 16/13/13/11; before, only the startup panel used SF Pro and the rest used egui's default.
- `paint.rs`: `control_body` (raised/recessed), `hover_fill`, `under_shadow`, `selection_edge`, and
  the shared keyboard-navigation flag that gates focus edges. `routing_accent` and the lift shadow
  are not built: nothing needs them yet.
- Primitives: `text_button`, `tool_button` (moved from `transport`), `chip`, `menu_button`,
  `text_field`, `text` (caption and heading), `list_row`, `notice_box`, `window_frame`,
  `scroll_zoom_bars`; gallery stage `primitives::specimens`.
- Adoption: command buttons, dialog buttons, the track details panel, the quarantine notice, the
  instrument/effect pickers, the file window, Audio/MIDI settings, commands window, Edit History,
  tempo/transport windows, the take-lane chip, the snap/record-mode/beat-unit/controller menus, the
  startup chooser and recovery rows, and the arrangement/roll Scroll/Zoom sliders (now the
  horizontal scroll and zoom bar).
- Not adopted (owned by components outside this batch): context and Edit menus (menus), Applies to
  (segmented control), tempo/bar/tick numbers and the history revision slider (value slider),
  mixer slot, tap and output buttons, the inline take-lane rename, the roll's Lowest pitch/Rows
  fields (removed by T40).

**Findings**

- Track-colour labels: with Clef's `TEXT`/`BACKGROUND` labels the weakest hue (Peacock #2E7D6E)
  reaches 3.96:1, not the 4.9:1 foundations quoted from the legacy kit. Open accessibility item.
- Allocating a primitive inside a child `Ui` (`add_enabled_ui`, `push_id`) breaks wrapping in
  `horizontal_wrapped` rows; primitives allocate in the caller's `Ui` through `add_enabled`.
- The gallery window never holds OS keyboard focus, so text-field focus is proven by a frame test.

**Evidence**

- Automated: `rtk cargo test --locked` (workspace): 238 passed, 26 ignored on 2026-11-21; UI crate 70
  tests including `theme::tests` (colour, text-style and metric parity, colour-literal check,
  `foreground_on`) and primitive tests (text button accessibility and width, text field focus edge
  and label, scroll bar zoom button).
- `rtk cargo clippy --all-targets --locked -- -D warnings` and `rtk cargo fmt --all --check` clean.
- Gallery: `docs/UI/snapshots/egui/t18-*-{1024,760}.png`, 12 cases; two consecutive runs at
  1024 produce byte-identical PNGs, also identical to the committed files.
- Surface snapshots T07–T34 regenerated at 1024 and 760 after adoption.
- Visual acceptance: the user accepted the T18 gallery and regenerated snapshots as the base for
  T40 on 2026-11-21, and checked the running app together with T40 the same day.

## Phase 2 — Menus, controls, command registry and palette (draft 2026-11-21)

**Status:** approved by the user on 2026-11-21; implementation in progress. This
section, its UI proposal and the menu grouping were approved together.

### Interview decisions (2026-11-21)

| Topic | Decision |
| --- | --- |
| Batch | Menus, segmented control, value slider, command registry extraction, command palette. |
| Shortcut persistence | Remapped shortcuts persist in the global preferences, apply to every project, and can be reset to defaults. |
| Conflicts | Assigning a shortcut already used in an overlapping context asks first: "Already used by *X* — Replace / Cancel". |
| Unmodified keys | Allowed (Space, Backspace, single letters); a binding without Command/Ctrl/Option never fires while a text field has focus or an IME composition is active. |
| Palette vs Commands window | The palette (⌘⇧P) searches and runs commands; the existing Commands / Keyboard Shortcuts window is kept for remapping only. |
| Unavailable commands | Shown dimmed in the palette, not runnable. |
| Menu bar | A full in-window egui menu bar is added and replaces the provisional command-button row. |
| Value slider | Drag to change (Shift = fine), double-click to type (Return commits, Escape cancels), ↑/↓ step with focus (Shift = large step), Option-click resets to the default. |
| Piano roll keys | The roll's local shortcuts (⌘A, ⌘C/⌘V, ⌘D, Backspace/Delete …) move into the registry as roll-context commands: remappable and in the palette. |
| Accessibility | Accessible name and visible keyboard focus on every new primitive; keyboard navigation in menus, menu bar, segmented control and palette. No VoiceOver audit in this phase. |

### Scope

1. **Command registry** — `crates/ui/src/commands.rs` owns `Command`, its title, default
   shortcut, **context** (`Global`, `Arrangement`, `PianoRoll`, `Mixer`, `TextFree`), and the
   lookup used by dispatch, menus, tooltips, the palette and the Commands window. `lib.rs` keeps
   only `dispatch` and `enabled`. Every menu row, button tooltip and palette row reads its name,
   enablement and live shortcut from the registry (menus.md: "every menu is a projection of the
   command registry").
2. **Shortcut dispatch** — one dispatcher resolves a key event to at most one command: the most
   specific active context wins (roll or arrangement focus over global); two bindings conflict when
   they share the shortcut and their contexts can be active together (Global overlaps every
   context; Arrangement and PianoRoll do not overlap each other). The existing guards stay: Undo/Redo
   yield to text fields, PlayStop yields to mixer activation focus, Redo never falls through to Undo.
3. **Shortcut persistence** — the global preferences file (`crates/session/src/audio.rs`
   `Preferences`) gains a `shortcuts` map of command id → shortcut or `null` (explicitly unbound);
   only differences from the defaults are stored. Command ids are stable strings, independent of the
   enum's order. Unknown ids and unparsable shortcuts are ignored and reported once; a corrupt file
   keeps the existing fallback. "Restore Defaults" in the Commands window clears the map.
4. **Commands window (remap only)** — rows: title, shortcut, Set, Clear. Set captures the next key
   (Escape cancels, any key allowed). A conflict opens the Replace / Cancel confirmation; Replace
   unbinds the other command. Running commands from this window is removed (the palette owns it).
5. **Command palette** — ⌘⇧P (Ctrl-Shift-P elsewhere) and the menu bar's Commands… entry open a
   modal [window frame](../UI/components/window-frame.md) with a [text field](../UI/components/text-field.md)
   over [list rows](../UI/components/list-row.md): title left, live shortcut right in `secondary`.
   Case-insensitive substring match on the title, registry order; unavailable commands dimmed and
   skipped by ↑/↓. Return runs the highlighted command, Escape closes; the palette restores the
   focus and context it was opened from before dispatching, so context commands act where the user
   was.
6. **Menus primitive** — `primitives::menu`: rows 26 high, tick column, right-aligned live shortcut
   omitted when unbound, ▸ submenu, separators, section captions, `peacock` highlight, destructive
   labels (Delete, Remove, Clear; or marked) in `danger`, disabled rows `disabled`. Keyboard: ↑/↓
   move, Return/Space activate, → / ← open/close submenus, Escape closes, focus returns to the
   invoking control. Toggle menus (mixer Sections) stay open until Escape, a click outside or their
   button. Adoption: Edit menu, arrangement and mixer context menus, REC mode popup, mixer Sections,
   and the popups of the existing `menu_button` and chip primitives.
7. **Menu bar** — a 28-pixel in-window row (`menuBarHeight`) at the top of the project window,
   replacing the provisional New Project … Commands button row. Menus (all entries from the
   registry; final grouping in the UI proposal below): **File**, **Edit**, **Track**, **Transport**,
   **View**, **Window**; right-aligned Commands… and Audio/MIDI Settings. F10 or ⌃F2 focuses it
   from the keyboard; ←/→ move between menus. Not shown on the startup chooser.
8. **Segmented control** — `primitives::segmented`: two to four segments in a recessed `field`
   track, lit segment `peacock` with `foreground_on` label, hover lifts the label to `text`,
   disabled segments dimmed and skipped by ←/→, accessible name on the group and selected state on
   each segment. Adoption: Settings › Applies to.
9. **Value slider** — `primitives::value_slider` for bounded numbers with a unit and a formatter:
   behaviour per the decisions above; accessible name and value; focus edge. Adoption: tempo,
   bar/tick fields in the transport windows, and the Edit History revision slider (horizontal
   track form). The roll's Lowest pitch/Rows fields are excluded (removed by T40).
10. **Gallery and docs** — gallery cases for every new primitive state, the palette and the menu
    bar; `menus.md`, `segmented-control.md`, a new `value-slider.md`, `command-palette.md`,
    `window.md` (menu bar row), `edit-history.md` and `index.md` updated to `impl: gallery`/`app`.

### UI proposal (for approval)

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ File  Edit  Track  Transport  View  Window                Commands…  Settings│ 28 px, background
├──────────────────────────────────────────────────────────────────────────────┤
│ Untitled · transport bar · tool row · arrangement … (unchanged)              │
```

Proposed grouping (every entry is an existing registry command; shortcut shown when bound):

- **File:** New Project, Open Project…, Open Recent ▸, Save, Save As…, Recovery…, Close Clef.
- **Edit:** Undo *label*, Redo *label*, Edit History…, separator, Select All Clips, Duplicate,
  Split at Playhead, Delete.
- **Track:** Add Instrument Track, Add Bus, Rename Track, Move Track Up/Down, Remove Track…,
  separator, Choose Instrument, Open/Close Editor, Restart Instrument, Instrument Bypass/Offline,
  Record arm, Track Record Mode ▸, Show/Hide Take Lanes.
- **Transport:** Play / Stop, Return to Start, Record MIDI, Loop, Metronome, One-Bar Count-In,
  Record Mode ▸, Stop Mode ▸, Tempo and Meter Map…, Transport Settings….
- **View:** Show Mixer/Sequencer, Fit Arrangement to Width/Height, Fit Piano Roll to Width/Height.
- **Window:** Commands / Keyboard Shortcuts…, Command Palette…, Audio/MIDI Settings, Show
  Quarantined Plugins…, Rescan Plugins, Refresh Devices.

Palette (modal, 560 wide, centred in the upper third):

```text
┌ Command Palette ─────────────────────────────┐
│ [ Search commands                         ]  │
│ ▌Play / Stop                         Space   │ ← highlighted (peacock)
│  Return to Start                             │
│  Duplicate                            ⌘D     │ ← dimmed when unavailable
└──────────────────────────────────────────────┘
```

### Acceptance

- One registry is the only source of command titles, default shortcuts and contexts; no surface
  hardcodes a shortcut string or checks a key that a registry command owns (piano roll included).
- Remapping persists across restarts in the global preferences; Restore Defaults works; a corrupt
  or partial shortcuts section loads the defaults for the missing entries and reports once.
- A conflicting assignment asks Replace / Cancel; Cancel leaves both bindings unchanged.
- Unmodified bindings never fire while a text field has focus or IME is composing.
- Palette: opens with ⌘⇧P, filters, skips unavailable rows, runs with Return, restores focus and
  context before dispatch.
- Menus, menu bar, segmented control and value slider: every state in the gallery, keyboard
  operation covered by frame tests, accessible names asserted in tests.
- The provisional command-button row is gone; affected surface snapshots regenerated.
- `rtk cargo test --locked`, clippy and fmt clean; the user accepts the gallery, the menu bar and
  the regenerated snapshots as the intended look.

### Order

Registry extraction and dispatcher (with tests for every existing guard) → persistence and
Commands window remap/conflict → piano roll commands into the registry → menus primitive →
menu bar replacing the button row → palette → segmented control → value slider → adoption,
gallery, docs.

### Out of scope

macOS system menu bar (native AppKit is not allowed); VoiceOver audit; fuzzy matching and recent
commands in the palette; per-project shortcut sets; command offers per query (D61.3/D87);
light theme and scaling.

### Open questions

- Menu bar grouping above is a proposal; confirm or adjust.
- Whether Settings › Startup (Show chooser | Reopen last | Start empty) exists yet in Clef to adopt
  the segmented control; if not, only Applies to adopts it.

## Phase 2 progress and evidence (2026-11-21)

Done: implemented on 2026-11-21; the user reviewed the gallery, the regenerated surface snapshots
and the running app, and accepted them the same day.

**What changed**

- `crates/ui/src/commands.rs`: the registry (`Command`, `SPECS` with stable id, title, default
  shortcut and context), `Keymap` (live shortcuts, conflict check, exact matching, saved form),
  `captured` (⌘ on macOS and Ctrl elsewhere both become Command). `lib.rs` keeps `dispatch` and
  `enabled`; the old `Binding` list is gone. Contexts are Global, Arrangement, PianoRoll and Mixer;
  the planned `TextFree` context was not needed: the text guard applies to every binding without
  Command/Control/Option.
- Dispatcher (`App::shortcuts`): most specific context first; PianoRoll commands are consumed by
  the focused roll lane; Arrangement commands need arrangement focus; unmodified bindings never
  fire while a text field has focus or IME is composing; Undo/Redo yield to text fields; Play /
  Stop yields to mixer activation focus; nothing fires while the palette is open or a shortcut is
  being captured. Matching is exact, with two rules: roll step commands (move, resize, add note)
  match with or without Option (Option turns snap off), and an unmodified Backspace binding also
  accepts forward Delete.
- Piano roll: select all, delete, copy, paste, duplicate, add/audition note and the arrow steps
  (keybed cursor included) are registry commands (`roll.*`). ⌘C/⌘V keep working through the Copy
  and Paste events while they keep their default keys. The Insert key alias for adding a note was
  dropped (no Insert key on Mac keyboards). Space still auditions on the focused keybed (widget
  activation, not a command).
- Persistence: `crates/session/src/shortcuts.rs` reads and atomically writes `shortcuts.json`
  beside `audio.json` (version 1, 64 KiB cap, opaque id → shortcut text); `Action::SetShortcuts`
  and `Snapshot::shortcuts`. Unknown ids, unreadable shortcuts and clashing entries are skipped and
  reported once in the status line; a corrupt file loads the defaults and is replaced on the next
  change. A change sent while the session is busy is retried when it is idle.
- Commands / Keyboard Shortcuts is remap-only: Set, Clear, Restore Defaults, Replace / Cancel on
  a clash. The command palette (`crates/ui/src/palette.rs`, ⇧⌘P) runs commands.
- Primitives: `menu` (frame, `Item`, separator, caption), `menu_bar`, `segmented`,
  `value_slider`; `ListRow::trailing`. New metrics `menuRow`, `menuSeparator`, `menuCaption`,
  `menuTick`, `menuPadding` in foundations and `theme::metric` (parity test covers them).
- Adoption: menu bar replaces the command-button row (`crates/ui/src/menus.rs`); the Edit menu
  primitive in `history.rs` is gone; clip context menu, take-lane menu, insert/instrument slot
  menus, send destination, add-send and output menus, REC mode popup and every `menu_button`
  popup use the menu primitive; Settings › Applies to uses the segmented control; tempo, bar,
  beats, tick fields, MIDI function numbers and the history revision slider use the value slider.

**Deviations from the approved draft**

- Submenus (Open Recent ▸, Record Mode ▸, Stop Mode ▸, Track Record Mode ▸) became captioned,
  ticked sections: the menus primitive has no submenus. Open Recent and Recovery are not in File:
  they are unavailable with a project open and stay on the startup chooser.
- Mixer Sections and Settings › Startup do not exist in Clef, so they were not adopted.
- The palette highlights with the list row's selected state (field and selection edge), not
  `peacock`.
- The value slider's track form follows the pointer from the press and has no typing or reset;
  menu captions are regular weight (one SF Pro weight is loaded).

**Evidence**

- Automated: `rtk cargo test --locked` (workspace) 257 passed, 26 ignored on 2026-11-21; new tests
  `commands::tests::*` (unique ids, no default clashes, context overlap, saved-form round trip,
  partial/bad saved form, matching rules), `palette::tests::*`, `primitives::menu_bar::tests::*`,
  `primitives::segmented::tests::*`, `primitives::value_slider::tests::*`,
  `tests::unmodified_shortcuts_wait_while_text_is_typed`,
  `tests::palette_shortcut_opens_it_and_a_clashing_assignment_asks_first`,
  `shortcuts::tests::saved_shortcuts_round_trip_and_bad_files_fall_back` (session); existing
  `history_slider_previews_then_commits_on_release_and_supports_cancel` and
  `redo_shortcut_never_falls_through_to_undo` pass on the new widgets and dispatcher.
- `rtk cargo clippy --all-targets --locked -- -D warnings` and `rtk cargo fmt --all --check` clean.
- Gallery: `docs/UI/snapshots/egui/t18-{menu,menu-bar,segmented,value-slider,command-palette}-{1024,760}.png`;
  a second run at 1024 is byte-identical for all 17 T18 cases.
- Surface snapshots T07–T40 regenerated at 1024 and 760 (menu bar, menus, value sliders).
- Visual acceptance: the user accepted the T18 Phase 2 gallery, the regenerated snapshots and the
  running app on 2026-11-21. No listening or hardware evidence applies.

## Phase 3 — Accessibility, Retina, mixer controls and remaining components (approved 2026-11-21)

**Status:** done 2026-11-21 — approved with its UI proposal, implemented, and the look (gallery,
regenerated snapshots, new raised shadow) accepted by the user the same day.

### Interview decisions (2026-11-21)

| Topic | Decision |
| --- | --- |
| Batch | One phase covering accessibility, Retina and resize, the mixer controls and the planned components that existing views reference. |
| Screen reader | Accessible name, role and value on every control in the accesskit tree, asserted by automated tests. No manual VoiceOver session. |
| Scaling | Correct rendering at `pixels_per_point` 1.0 and 2.0 (Retina) and at the minimum window; no user UI zoom. |
| Contrast | WCAG AA: 4.5:1 for normal text, 3:1 for large text (≥ 18 px, or ≥ 14 px bold), non-text controls and focus edges; checked by an automated test over the token pairs. |
| Track hues | Darken the weak hues until their label reaches 4.5:1; the palette shifts slightly. |
| Mixer controls | Extract into primitives and align their gesture with the value slider. |
| Planned components | Checkbox, tabs, glyph toggle, track-state button, search field, icons, segment display, range overlay. |

### Scope

1. **Contrast test and fixes** — `theme.rs` gains `contrast(a, b)` (WCAG relative luminance) and a
   table of the foreground/background pairs the surfaces actually use (text, secondary, disabled
   on background/panel/field/raised; `foreground_on` labels on peacock, accent, oxblood, solo and
   every track hue; focus edge and control edge against their backgrounds). A test fails any pair
   below its AA threshold; `foundations.md` lists the pairs with their measured ratio and the
   parity test covers the table. Tokens below threshold are adjusted, `disabled` text included
   only if it is meant to be read (WCAG exempts inactive controls; the table records that
   exemption explicitly rather than skipping the pair).
2. **Track hues** — each hue whose `foreground_on` label is below 4.5:1 is darkened in CIELAB
   lightness only (hue and chroma kept) until it passes, while the minimum pairwise distance stays
   ≥ 19 (current 19.4). `theme::track::COLOURS` and `clef_model::project::TRACK_COLOURS` change
   together; a test checks they match.
3. **Accessible names audit** — a test helper walks the accesskit tree of every gallery and surface
   snapshot case and fails on any focusable or clickable node without a name, and on any slider
   without a value. Custom-painted controls in arrangement, piano roll, timeline, mixer, transport
   and inserts get `widget_info` where missing. Tests assert names and values for the new
   primitives. No colour-only label: lit states also change shape, glyph or text, checked per
   primitive in its state matrix.
4. **Retina and resize** — the gallery renders every T18 case and the main surface cases at
   `pixels_per_point = 2.0` as well (`*-1024@2x.png`); hairlines and focus edges snap to physical
   pixels at both scales (a paint helper owns the rounding). The window keeps its 760 × 480
   minimum; snapshots at 760 already cover the narrow layout, and a frame test resizes from 1024
   to 760 and back without panels overlapping or controls leaving the window.
5. **Mixer primitives** — out of `primitives/mixer.rs` into their own primitives, look unchanged:
   `fader`, `knob` (send and compact send), `width_knob`, `level_meter` (strip and header),
   `mute_solo` (the lamp), `power_button`, `slot_button`. Gesture aligned with the value slider:
   drag (Shift = fine), **double-click to type** (Return commits, Escape cancels), ↑/↓ and ←/→ step
   with focus (Shift = large step), Option-click resets. Today double-click resets: it becomes
   typing, and reset stays on Option-click only. The drag still commits one undo step on release.
   Value text and units stay as `text_value` formats them.
6. **Planned components** — each lands as a primitive with gallery states and a doc; adoption only
   where a current surface has the matching control:

   | Component | Adoption now |
   | --- | --- |
   | Icons | The `Glyph` set used by tool, transport and roll buttons moves into `primitives::icons`; every glyph in the gallery. |
   | Glyph toggle | Roll and arrangement tool row toggles that are on/off states. |
   | Track-state button | Record arm in the transport track list (`transport.rs`). |
   | Search field | Plugin picker search and the command palette field. |
   | Segment display | Transport bar/beat/tick counter (`primitives::transport::counter`), count-in state as text, not colour. |
   | Range overlay | Loop band in the timeline rulers; arrangement selection range if it draws its own band. |
   | Checkbox | Gallery only — no current surface has a boolean option. |
   | Tabs | Gallery only — no current window has tabs. |

   Not in this phase: hierarchy rail (no folder tracks in the model), callout, detach button, knob
   cell, sweep bar, automation mode chip, colour picker, performance readout, tap button, shadows
   doc beyond the existing helpers.
7. **Docs** — every component above to `impl: gallery` or `app` with Rust API, state matrix,
   keyboard, accessibility and evidence sections; `foundations.md` contrast table and track hues;
   `index.md` rows; affected view docs.

### UI proposal (for approval)

No layout changes. Visible differences:

- Track hues: slightly darker Peacock, Sage, Steel blue, Brass and any other hue below 4.5:1
  (exact values computed and shown in the gallery before adoption).
- Any token adjusted by the contrast test (listed with before/after ratios in the gallery case).
- Transport counter drawn as a segment display in the same place and width.
- Record arm drawn as a track-state button; loop band drawn by the range overlay.
- Mixer fader and knobs: same look; a double-click opens an inline value field in place of the
  readout.

```text
Mixer knob, double-click:      ┌──────────┐
   ( ◜ )  → typing →           │ -6.0  dB │  Return commits · Escape cancels
                               └──────────┘
```

### Acceptance

- Contrast test passes for every listed pair; a deliberate darkening of `TEXT` below 4.5:1 on
  `PANEL` fails it.
- All nine track hues label at ≥ 4.5:1 and keep pairwise distance ≥ 19; model and theme match.
- The accesskit walk finds no unnamed interactive node and no valueless slider in any gallery or
  surface case.
- Gallery cases for every new primitive and state at 1.0 and 2.0; two runs byte-identical.
- Mixer controls: frame tests for drag (and Shift fine), type-to-commit and Escape, arrow steps,
  Option-click reset, one undo step per drag.
- Resize frame test passes; snapshots at 760 and 1024 regenerated.
- `rtk cargo test --locked`, clippy and fmt clean; the user accepts the gallery, the new hues and
  the regenerated snapshots as the intended look.

### Order

Contrast function and table → token and hue fixes → accesskit walk and missing names → physical
pixel snapping and 2.0 gallery → icons → mixer primitives with the aligned gesture → glyph
toggle, track-state button, search field, segment display, range overlay → checkbox and tabs →
adoption, gallery, docs.

### Out of scope

Manual VoiceOver audit; user UI zoom; high-contrast theme; light theme; the components listed as
not in this phase; any layout change.

### Resolved questions (2026-11-21)

- Saved projects: a track whose colour is exactly one of the nine old default hues is mapped to
  the new hue on load; custom colours are kept. The mapping lives in the model with a test.
- Mixer: double-click opens typing; reset is Option-click only.
- Checkbox and tabs: primitive, gallery states and doc in this phase, no adoption.

## Phase 3 progress and evidence (2026-11-21)

Done: implemented on 2026-11-21; the user accepted the gallery, the regenerated snapshots and the
raised-shadow change the same day.

**What changed**

- Contrast: `theme::contrast`, `theme::Requirement` and the `theme::CONTRAST` pair table, mirrored
  by the new [Contrast](../UI/foundations.md#contrast) table in foundations (parity test included).
  Every text pair already met AA; the only failures were five track hues.
- Track hues: Peacock #2E7D6E → #217264 and Slate blue #4A6BA8 → #4466A3 (darker, light label);
  Ochre #A8703A → #AE763F, Lavender #8B6FA8 → #9377B0 and Dusty rose #B06A7C → #B46E80 (lighter,
  dark label). The weakest label is now 4.60:1; minimum pairwise CIELAB distance 21.7.
  `theme::track::COLOURS` is now built from `clef_model::project::TRACK_COLOURS`, so the two can
  never disagree. Loading a project maps the five retired T40 values exactly to their replacement;
  other stored colours are kept.
- Accessible names: `gallery::tests::every_gallery_control_has_an_accessible_name` walks the
  accesskit tree of every case of every gallery stage (97 cases). It found and fixed unnamed menu
  popups (now role Menu, named by their menu-bar title or "Menu") and a double interaction in the
  new tabs. The segment display sets its name explicitly (egui exposes a label's text only as value).
- Retina: the gallery takes a scale argument (`… <width> 2`) and writes `…-1024@2x.png` through the
  zoom factor; the T18, T09, T10 and T34 stages are rendered at 2.0.
- Resize: `gallery::tests::resizing_to_the_minimum_width_and_back_keeps_controls_in_the_window`
  alternates 1024 and 760 on the arrangement.
- Primitives: `icons` (from `tool_button::Glyph`, plus Search, Close, Check, Triplet, Dotted),
  `checkbox`, `tabs`, `glyph_toggle`, `track_state::record_arm`, `search_field::SearchField` (the
  text field's search form), `segment_display`, `range_overlay`, `fader`, `knob`, `level_meter`,
  `mute_solo`, `power_button`, `slot_button`. The mixer gesture stays one implementation
  (`mixer::Slider`) shared by fader and knobs.
- Mixer gesture: Shift drag at a quarter speed with an origin re-based on Shift changes (no jump);
  double-click opens an inline value field (Return commits, Escape or click-away cancels; units and
  "-inf" parsed by `mixer::parse_value`); arrows 0.5 dB / 1 and Shift-arrows 3 dB / 10; Option-click
  resets; double-click no longer resets.
- Adoption: search field in Choose Instrument, Commands / Keyboard Shortcuts and the palette; segment
  display for the transport counter (with a Count-in caption); range overlay for the arrangement and
  piano-roll marquees; track-state button for record arm.

- Raised depth (user request after reviewing the gallery, 2026-11-21: the outer shadow looked
  dated): a shadow study of five variants was rendered and the user chose "C". `paint::under_shadow`
  is now a tight blurred drop shadow (black 35 %, 1 point down, 3-point blur, epaint `Shadow`)
  instead of two hard lines (black 55 % and 24 %), and the top light drops from white 9 % to 5 %.
  Every raised control changes; all snapshots were regenerated.

**Deviations from the approved draft**

- Pixel snapping needed no Clef helper: epaint's tessellator already rounds lines, rectangles and
  text to physical pixels by default; the 2.0 gallery is the check.
- Glyph toggle has no adoption: Clef has no triplet/dotted feel, and the Follow and MIDI Functions
  toggles are words, which the glyph-toggle rule keeps as words.
- Range overlay replaces the marquee, not the loop band: the loop band keeps `LOOP_ACTIVE` per the
  lamp rules. The marquee edge changed from 100 % to 80 % `accent`.
- `CONTROL_EDGE` (1.56–1.99:1) is recorded as exempt (decorative), by user decision on 2026-11-21:
  controls are identified by their label or glyph, which meet AA.
- Hue direction: the decision was "darken"; darkening the three dark-labelled hues would have needed
  about −9 L* and broken the distance floor (18.9 < 19), so each hue moved toward its own label
  (user delegated the choice on 2026-11-21).
- egui's own ScrollArea bars have no accessible name; the walk exempts the ScrollBar role.
- The width knob is the `Control::Width` variant of the knob primitive, not a separate module.
- The resize test covers the arrangement only; the mixer's strips scroll by design and its docked
  Master is covered by `mixer_master_stays_docked_and_track_meters_line_up_at_both_widths`.

**Evidence**

- Automated: `rtk cargo test --locked` (workspace) 271 passed, 26 ignored on 2026-11-21. New tests:
  `theme::tests::{every_drawn_pair_meets_wcag_aa, contrast_table_matches_the_foundations_table,
  track_hues_stay_distinct}`, `project::tests::retired_default_hues_load_as_their_replacement_and_custom_colours_stay`,
  `gallery::tests::{every_gallery_control_has_an_accessible_name,
  resizing_to_the_minimum_width_and_back_keeps_controls_in_the_window}`,
  `primitives::mixer::tests::{shift_drag_moves_a_quarter_as_far_without_a_jump,
  double_click_types_a_value_and_option_click_resets, arrows_step_and_shift_arrows_take_the_large_step,
  typed_values_parse_back_in_each_unit}`, `primitives::specimens::tests::*` (tabs, checkbox,
  search field, segment display).
- `rtk cargo clippy --all-targets --locked -- -D warnings` and `rtk cargo fmt --all --check` clean.
- Gallery: 14 new T18 cases (`t18-{icons,checkbox,tabs,glyph-toggle,track-state,search-field,
  segment-display,range-overlay,fader,knob,level-meter,mute-solo,power-button,slot-button}`) at 1024
  and 760, every T18 case at 1024@2x; a second run of all 62 T18 images (1024 and @2x) is
  byte-identical to the committed files.
- Surface snapshots T07–T40 regenerated at 1024 and 760; T09, T10 and T34 also at 1024@2x.
- Visual acceptance: the user accepted the Phase 3 gallery, the snapshots and shadow "C" on
  2026-11-21, and checked the debug primitive gallery in the running app.
- Not done: manual VoiceOver session (out of scope by decision).

## Debug primitive gallery (2026-11-21)

User request: one in-app page with every primitive, scrollable, opened from the Window menu, to be
removed from release builds.

- **Window › Primitive Gallery (Debug)** opens a 760-wide tool window with a vertical scroll area
  that draws every T18 gallery case in order (`Specimens::show_all`). The four cases that draw over
  the whole window (notice box, question, window frame, command palette) get a Show/Hide button
  instead of opening by themselves.
- Debug builds only: the menu row and the window exist under `cfg!(debug_assertions)`. It is
  deliberately not a registry command, so it never appears in the palette, the Commands window or
  the saved shortcuts; this is the one menu row that is not a registry projection.
- The first run in the app hit egui's debug assertion "widget changed layer_id": the specimen menu
  bar and palette used the same fixed ids as the real ones. `menu_bar::menu_bar` now takes the bar
  id (`menu_bar::main_id()` for the project window) and `Palette::with_id` gives the specimen its
  own; gallery images are unchanged.
- User feedback on 2026-11-21: the overlays could not be closed and a detached dropdown appeared
  under the page. Each overlay now reports when it closes (notice and question answered or
  dismissed with Escape or a click outside, window frame closed, palette closed or run) and the
  page drops it; the specimen menu bar holds its Transport menu open only on the screenshot stage
  (`Specimens::for_screenshots`), so on the page it opens on click like the real one.
- Evidence: `primitives::specimens::tests::one_page_view_draws_every_inline_case_and_opens_overlays_on_request`;
  workspace tests, clippy and fmt clean. The user checked the page in the running app and
  confirmed it works on 2026-11-21 (the unbundled debug binary cannot be driven by the
  desktop-control tools).

## Verification Intent

Widget behavior and states; focus order; shortcut dispatch; contrast tokens; UI docs match
the implementation; no one-off control introduced where a shared component exists.

## Approval Gate

Each phase above was interviewed, specified, approved with its UI proposal and accepted before it
closed. The task is complete; further UI foundation work needs its own task.
