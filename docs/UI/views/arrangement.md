---
name: Arrangement
kind: view
reference: paintFolderPreview, folderPartBounds, paintGlobalHeader, trackBandHeight, SearchField, paintMatchHighlight, paintRangeOverlay, paintLiftShadow, paintNote
status: mixed
impl: planned
reference-source: [Source/Arrangement.cpp, Source/ArrangementLane.cpp, Source/MarkerLanes.cpp, Source/ChordLane.cpp, Source/TempoLane.h, Source/AudioImportViews.h]
components: [note, range-overlay, shadows, chip, colour-picker, scroll-zoom-bars, search-field, menus, notice-box, callout]
views: [track-header, timeline-rulers, track-lanes, tool-row, chord-lane]
snapshot: snapshots/arrangement.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Arrangement

## Piano Roll Entry

Double-click a selected MIDI clip or press Enter on a focused clip to open the
[Piano Roll](piano-roll.md) below the timeline. The dock shows every selected clip in
its own lane. Double-clicking an empty arrangement lane continues to create a clip.

The live Clef MIDI editor is documented under
[T09 arrangement editing](multitrack-session.md#t09-arrangement-editing).

In Clef T11, the Master row stays docked beneath the vertically scrolling tracks and above the
timeline navigation. The header shows the shared Master stereo meter. Its lane uses the arranger
grid and playhead at the current horizontal position and zoom. The Master row is present in an
empty project and remains above the Piano Roll when the dock is open.

## Anatomy, top to bottom
1. [Timeline rulers](timeline-rulers.md) with the locator band.
2. **Global band**: tempo, marker, [chord](chord-lane.md) and arrangement lanes. Headers painted by `paintGlobalHeader`
   (`globalHeader`, the lane glyph, uppercase caption), lanes on `globalLane`. The arrangement track is a 24-pixel lane of rounded blocks in their colour, with a
   chain chip (welded/broken), LIST and "+" right-aligned in its header; marker headers carry "+".
2a. **Track band** (D101; a 1-pixel `divider` line at its top closes the global band, replacing the old 2-pixel seam; the track column's vertical divider skips the band): `trackBandHeight` (36) of `background`, sticky. In the
   header column the [search field](../components/search-field.md) ("Find track", ⌘F right-aligned; its edge on the
   column's edge), then a flat `plus` glyph in `create` orange (never confused with a lane's "+"), in the
   global lanes' "+" column, that opens the Tracks menu's creation commands (New MIDI, New Audio, FX, Bus, separator,
   Add Multiple Tracks…) with live bindings (`trackCreationMenu::build`, shared with the D100
   empty-space menu). While filtering, "N of M tracks · Esc clears" sits in
   `secondary` beside it, and an empty result reads "No tracks match "…"". Enclosing folders show open.
3. **Pinned block**: pinned root tracks and folders, sticky under the band, ending in a 2-pixel seam.
4. Track rows ([track header](track-header.md) + timeline) with their [lanes](track-lanes.md).
5. Bottom block: buses, FX returns and master, pinned.

## Timeline
`canvas` with the project grid (`grid`, bar lines `barLine`, nothing closer than 8 pixels). Parts
keep their track colour, selected parts add the selection outline and handles. A MIDI part resizes
from a 6-pixel end band in whole beats (Alt free); a trim that would cut a note is refused with a
status message. Folder stripes carry members' parts as dimmed miniatures (45 %), one band per
member, clipped to the stripe, never editable. A Select drag from empty space draws the
[range overlay](../components/range-overlay.md). Parts lift with a shadow while dragged.

## Audio parts (D05)
Approved boards: [Audio Editing (D05)](https://claude.ai/artifact/YAjpNzUt5aBSYYRVLAejvM). Snapshots:
`snapshots/audio-parts.png`, `audio-trim.png`, `audio-drop.png`, `audio-import-progress.png`, `audio-refused.png`,
`audio-missing-notice.png`.
- An audio part keeps the part grammar: track-colour fill, the 11-px name top left in `foregroundFor(fill)`,
  the same 1-pixel selection outline as a MIDI part. The waveform replaces the notes, in `foregroundFor(fill)`
  at 75 %, in the area under the name. From the Big height (76) upward a stereo file shows each channel in
  its own half with a faint seam (the same ink at 25 %); below that the channels draw as one combined
  envelope. The waveform is a filled, anti-aliased outline through each pixel column's peaks (a 3-column max,
  then a 5-column average, quadratic curves between columns), never per-pixel min/max lines; silence is a
  hairline at the centre (smooth waveform refinement, [board](https://claude.ai/artifact/LrruWW7ssXFAu4AACZuzpf),
  variant A).
- **Missing file**: the track colour at 20 % with a 1-pixel outline in the track colour, no waveform, the
  `warning` glyph (11 px, `danger`), the name in `text` and " · Not found" in `danger`; from 34 px of height
  a second line "Plays silence · Relink Missing Audio…" in 10-px `secondary`.
- **Trim**: the left and right 6-pixel bands of an audio part resize it (Alt free). While the left edge drags,
  the audio being cut away shows at 22 % of the track colour with its waveform at 55 %, and a `peacock` readout
  chip (10 px, 18 high, 21 px above the part) reads "Start h:mm:ss.mmm · offset +s.sss s". Neither edge passes
  the file's start or end.
- **Drops** (Finder, Browser Audio): the target lane gets a 1.5-pixel `accent` outline; a 2-pixel `accent` bar
  marks the start from the first ghost to the last; each file is a dashed `accent` ghost (10 % fill) as long as
  its audio, captioned name · h:mm:ss.s. Files beyond the first sit on "New audio track" ghost rows (Normal
  height, `background`, a dashed `barLine` header with the audio glyph) directly below the target, or below the
  last track over empty space. With several files a `field` badge by the pointer reads "N files · bar B". Over a
  track that refuses audio there is no ghost: a `field` chip by the pointer gives the reason in `danger`, which
  the status line repeats on drop.
- **Pending import**: each file waits at its place with the track colour at 30 % and a 55 % progress fill.
  After half a second the import call-out (`ImportProgressView`, 380 wide, panel with a divider edge and an
  arrow at the first pending part, kept inside the visible timeline, above the timeline in the arrangement)
  lists each file with its state and a groove for the running one, and Cancel import (Escape).
- **Refused files** after an import: a call-out (`RefusedFilesView`) with the warning glyph, "N files were not
  imported", how many did, FILE (`danger`) and REASON (`secondary`) rows and the primary OK.
- **Missing audio on open**: the kit [notice box](../components/notice-box.md) lists FILE, PARTS and LAST
  LOCATION with Later and the primary Relink… (Return); the status row keeps the one-line form.
- An audio part's menu has Copy, Rename Part..., Duplicate, Delete, the D70 fade rows (below) and Paste; double-click selects it and
  opens no editor.
- **Waveform zoom** (D05 refinement, board 4, `snapshots/waveform-zoom.png`, `waveform-zoom-callout.png`): one
  drawing scale, ×1 to ×16, for every audio part's waveform (the trim preview too; never a gain, never missing
  parts). Each channel area clips its waveform, so peaks past it are flattened at the edge. The `trackAudio`
  glyph fills the [scroll bars](../components/scroll-zoom-bars.md)' corner, `secondary` at ×1 and `accent`
  above; its tooltip gives the scale and the live binding of Waveform Zoom…. A click opens a
  [call-out](../components/callout.md) (`WaveformZoomView`, 112 × 236): bold "Waveform", a vertical kit
  [fader](../components/fader.md) on a log₂ scale with ×1 … ×16 marks in 9-px `secondary`, and the ×-readout
  (12 px). It follows the fader live; double-click or Option-click resets to ×1.

## Audio recording and muted parts (D106)
Approved board: [D106 Audio Recording](https://claude.ai/artifact/Fog4bUWpYRKwRA6v371uyw), boards 1–3.
- **Recording region**: each recording audio track shows D15's region (`selection` at 35 %, 1-pixel `danger` outline,
  "Recording · <file>.wav" in 11-px `danger`) with the incoming waveform filled in `danger` at 75 %, growing to the
  playhead; from the Big height a stereo input draws its two channels with a 25 % seam. A loop take draws its current
  pass. With punch on, the region spans only punch in to punch out. Each pixel column reads a fixed stretch of the file
  (the waveform never slides while the file grows); columns not yet written stay empty. Stop replaces it with the
  committed part.
- **Muted part** (Mute Part, or the covered piece of an older part after a recording): the track colour at 22 % with a
  dashed 1-pixel outline (3 on, 3 off) in the track colour, its waveform or notes in `text` at 40 %, the name in `text`
  followed by " · Muted" and the `muted` glyph (11 px). Muted parts paint first, under the parts that cover them; they
  neither overlap nor crossfade (D70 ignores them). Both part menus (audio and MIDI) list **Mute Part** after Duplicate,
  ticked while the part is muted, a registry command with its live binding (unassigned by default).
- **"N muted below"**: a part that covers muted pieces shows, in its lower-left corner, a `background` 55 % box (15 high,
  4 in) with the `layers` glyph and "N muted below" in 10-px `text` at 75 %. It drops on a part under 34 px or too
  narrow for it.

## Fades and crossfades (D70)
Approved boards: [Fades and Crossfades (D70)](https://claude.ai/artifact/8cNxVaNrXFtBMAjsB4rSW2). Snapshots:
`snapshots/fade-drag.png`, `fade-shape.png`, `crossfades.png`, `crossfade-move.png`, `settings-anti-click.png`.
- **Name**: an audio part's name starts 12 px in (D05: 7 px), clear of the corner handle; a fade in or a
  crossfade at the start pushes it to 6 px past its end, so the name never sits under a curve.
- **Fade**: a curve from the edge to the fade's length across the part's full height, in `foregroundFor(fill)`
  at 1.5 px; the audio outside it is dimmed with `canvas` at 45 %. The curve is Tracktion's own fade curve
  (`AudioFadeCurve::alphaToGainForType`), so the drawing is what plays. Shapes: Linear, Convex (a new fade),
  Concave, S-curve.
- **Handles**: a 7-px square in each top corner (fill the part's ink, 1-px `capLight` edge), on hover and on a
  selected part, at the fade's end once it has one; while dragged, `capLight` with an ink edge. The top 10 px
  of the edge band are the handle; the rest of the D05 trim band still trims. No handle on a part narrower than
  24 px or on a crossfaded edge.
- **Drag**: the fade's end follows the pointer's travel, free; with Cmd it snaps to the grid and the grid line
  lights in `accent` (1 px, the row's height). A `peacock` readout chip (the D05 trim chip) reads
  "Fade in 1.400 s", "· grid" when snapped.
- **Option + vertical drag** on a curve (within 8 px of it, or in the handle's 10-px band) steps through the
  shapes every 12 px, around the list; a `field` chip with a `divider` edge and the lift shadow shows a 16-px
  slot per shape (12-px glyph, the current one on `peacock`) and "Fade in · Concave" ("Crossfade · Equal gain"
  on a crossfade, two slots). Every fade chip sits above the part, or 3 px inside its top on the first visible
  row, where above would be under the band.
- **Crossfade**: across exactly the overlap of two audio parts on one track, the later part on top: the two
  curves cross in the part's ink, the overlap shaded with `canvas` at 18 %, the earlier part's end a 1-px ink
  line at 50 %. Hover shows the chip "Crossfade · Equal power · 0.800 s". Option-drag toggles Equal power /
  Equal gain. A removed crossfade sums: only the earlier part's hidden end, as a dashed 1-px ink line at 50 %.
  The overlap paints over both parts, so neither the earlier part nor a part being dragged hides it.
- **While parts are dragged** every audio part draws the fades the move would leave: the X forms as two parts
  meet, follows the overlap and disappears once they part, before the release (`crossfade-move.png`).
- A **missing** part keeps its fades: the curves in the track colour, no dimming. The waveform zoom never
  scales the curves. Anti-click fades (Settings › Editing) are not drawn.
- The audio part menu adds Fade In to Playhead, Fade Out from Playhead, Crossfade Selected Parts, Remove Fades
  (`danger`), then the Fade In Shape, Fade Out Shape and Crossfade Shape submenus, each row a registry command
  with its live binding, ticked on the shape the selection shares; a submenu is disabled without that fade.

## Part-edge time stretch (D71)
Approved boards: [D71 Part-Edge Stretch](https://claude.ai/artifact/MunZusFeJH89xRSmwgJvDv). Snapshots:
`snapshots/stretch-hover.png`, `stretch-midi.png`, `stretch-audio.png`, `stretch-badge.png`.
- **Target**: with Control held, a part's left and right 6-pixel edge bands (MIDI parts too, which have no left
  trim band) show the kit [stretch cursor](../components/icons.md) and the band lifts with the part's ink at
  18 %. A D70 fade handle keeps its place in the band. Without Control the band trims or resizes as before.
- **Drag**: Control + drag past 3 px stretches; Control-click without moving opens the part menu on release.
  The preview scales the content into the new bounds (notes drawn from the unstretched length, the waveform
  over the same file range, the fades scaled), a dashed 1-px ink line at 50 % marks where the dragged edge was,
  and a `peacock` readout chip (the D70 fade chip, 10 px, 18 high, 21 px above the part, inside its top on the
  first visible row) sits at the dragged edge, left-aligned to a left edge and right-aligned to a right one:
  "Stretch 125 % · 2.2.000 · grid" for MIDI (bars.beats.ticks length), "Stretch 125 % · 6.250 s" for audio,
  "· limit" once the 25–400 % bound stops it. A snapped edge lights its grid line in `accent` (1 px, the
  part's height plus 3 px each way). Escape cancels.
- **Badge**: a stretched audio part's name row adds `↔ 125 %` after the name in its ink at 75 % (the name
  ellipsises first; no badge under 24 px of spare row). MIDI parts keep no ratio and show none.
- **Preparing**: while Tracktion renders the stretched audio the waveform drops to 41 % ink (silent part) and a
  2-pixel bar runs under the name row, the track at 22 % ink, the done fraction in full ink.
- The audio part menu adds **Reset Stretch** in its own group after the fade shape submenus, a registry command
  with its live binding (none by default), disabled unless a selected audio part is stretched.
- D73 adds **Quantize Audio…** (⌥Q, opens the [audio editor](audio-editor.md) or applies its drawer), **Reset
  Warp** and **Set as Guide Part** right after Reset Stretch, registry commands with their live bindings.
  Double-click on a plain audio part opens the audio editor; a Slice leaves separate parts crossfaded by D70.
  A warped part's waveform is drawn from its original file through the warp map, so it never waits for its copy.

## Reverse, part gain, normalise and strip silence (D72)
Approved boards: [D72 Strip Silence, Normalise, Reverse](https://claude.ai/artifact/WVbhEKQzLmSgenbcg1M35P).
Snapshots: `snapshots/audio-reverse.png`, `audio-gain-badges.png`, `strip-silence-preview.png` (the call-outs:
[callout](../components/callout.md)).
- **Reversed**: the part's waveform draws its file region right to left; fades stay on the edges they were on.
  While Tracktion renders the reversed copy the part shows the D71 preparing bar and its waveform at 41 % ink.
- **Gain**: the waveform draws scaled by the part's gain, on top of the waveform zoom (peaks past a channel area
  flatten at its edge); 0 dB draws as before.
- **Badges** after the name, in the part's ink at 75 %, 6 px apart, in this order: the kit `reverse` glyph
  (11 px), D71's `↔ 125 %`, the gain (`+4.2 dB`, `−6.0 dB`, one decimal, a true minus). The name ellipsises first;
  badges then drop from the right (gain, stretch, reversed) once fewer than 24 px of row would remain.
- **Tooltip**: an audio part with a badge names them all: "Cello take 3.wav · Reversed · Stretch 125 % · Gain +4.2 dB".
- **Strip preview** (while the Strip Silence call-out is open): what would go is dimmed with `canvas` at 60 %, each
  new piece edge is a 1-px `capLight` line across the part, and the threshold is a dashed 1-px `accent` pair around
  each channel's centre, scaled like the waveform (zoom and gain). The document does not change until Strip.
- The audio part menu adds **Reverse**, **Normalise…**, **Reset Part Gain**, **Strip Silence…** in their own group
  after Reset Stretch, registry commands with their live bindings (none by default); Reset Part Gain is disabled
  unless a selected audio part has a gain.

## ARA parts (D87)
Approved board 1 and 4 of [ARA Support (D87)](https://claude.ai/artifact/348XW3KKYWjvwpLsSwXPtH). Snapshot:
`snapshots/ara-parts.png`. The editor is the [ARA editor](ara-editor.md).
- **Badge**: an audio part edited in an ARA plugin closes its name row with the plugin's name ("Melodyne") in the
  part's ink at 75 %, after the D71 and D72 badges; it is the first badge to drop on a narrow part. The waveform
  stays the original file's.
- **Plugin unavailable** (not scanned, or quarantined): the part plays its original file and the badge leads the
  row instead (it drops last): a 14-px `field` pill, 2-px corners, with the D88 1.5-px `danger` outline, the kit
  `warning` glyph (10 px) and the plugin's name at 10 px, both in `danger`.
- **Rendered** (Render ARA to Audio, while the part can still return to the plugin's edits): the plugin's name
  stays, in the part's ink at 45 %; the tooltip ends "· ARA: Melodyne, rendered" (`snapshots/ara-rendered-part.png`).
  Edit with that plugin returns the part to its edits; Remove ARA forgets them and the badge goes.
- **Tooltip**: "Verse take 3.wav · ARA: Melodyne" after the other badges; unavailable,
  "BV double.wav · ARA: VocAlign Ultra is not installed · Plays the original audio" (or "is quarantined").
- The audio part menu adds, in their own group after Delete: **Edit with <plugin>** when one ARA plugin is scanned
  ("Edit with ARA Plugin", disabled, when none is), or the submenu **Edit with ARA Plugin** with one row per
  plugin, the part's own ticked; **Render ARA to Audio**; **Remove ARA** (`danger`, as every Remove row). They are
  registry commands with their live bindings (none by default). Commands search lists "Edit with <plugin>" once
  per plugin in place of the generic row.
- An ARA plugin is in no slot, so the same group ends with D88's **Quarantine plugin** on a live ARA part and
  **Release plugin from quarantine** on a part whose plugin is quarantined.
- Double-click on an ARA part opens its editor; on any other audio part it does nothing, as before.
- An edge stretch (D71) of an ARA part shows no preparing bar: the plugin stretches, no proxy is rendered.

## Slip editing (D104)
No mockup (waived by the composer); every visual reuses an existing one. Snapshots: `snapshots/slip-midi.png`,
`slip-audio.png`, `slip-audio-limit.png`.
- **Target**: with Cmd + Option held (no Control), a part's body, clear of its 6-pixel edge bands and the D70 fade
  handles, shows the JUCE dragging-hand cursor. Without both keys the body behaves as before.
- **Drag**: Cmd + Option + drag past 3 px slides the content under fixed edges; Cmd + Option-click without moving
  is the Cmd-click toggle. The part's own notes or waveform draw where the slip puts them, clipped to its edges
  (no ghost beyond them: it overlapped neighbouring parts). The D71 readout chip sits above the part's left edge:
  "Slip +0.1.240" for MIDI (bars.beats.ticks), "Slip −0.512 s · −0.1.024" for audio, "· limit" once the file
  (audio) or the part's start (MIDI's first event) stops it, "· grid" with Shift, which lights the grid line the
  grabbed point snapped to in `accent` as D71 does. Escape cancels.

## Sample zoom (D105)
Mockup: https://claude.ai/artifact/AcwED72hqk1mM1NRAivNJ2 (approved 2026-11-14). Snapshots:
`snapshots/sample-zoom-dots.png`, `sample-zoom-peaks.png`, `sample-zoom-split.png`.
- **Depth**: the horizontal zoom reaches 384 000 px per beat (16 px per sample at 48 kHz, 120 bpm). The axis is a
  window of at most 4 194 304 px that moves with the view, so positions stay exact three hours in; the
  [scroll bars](../components/scroll-zoom-bars.md) span the whole arrangement.
- **Waveform**: from 256 samples per pixel up, the saved overview as before; from 1 to 256, the same filled shape
  from the file's own peaks (a 5-column max keeps a single-sample click at full height); below 1 sample per pixel, a
  1.25-px line through the samples in the waveform ink, a 0.5-px zero hairline at 37 % of the ink, and a 2.25-px
  dot on each sample once samples are 6 px apart. Stereo halves draw one line each; the combined area draws the
  channels' mean.
- **Sample snap**: once a sample is 2 px wide, audio trims, resizes, moves, splits, slips, fade ends and edge
  stretches land on whole samples of the project clock; Alt frees them (the slip's Shift still snaps to the grid);
  MIDI keeps its grid.
- **Readouts**: the trim chip adds the sample ("Start 2:59:10.0211 · sample 516 001 011 · offset +0.0021 s"); the
  scissors at sample depth show the pointer as a dashed `text` line at 35 % beside the `accent` cut line and a
  `peacock` chip "Split · h:mm:ss.ssss · sample n" at the cut. The mockup's ring at the cut point is not drawn: the
  line and chip already mark it.
- **Ruler**: rows keep their format; Bars adds beats, then sixteenths and ticks between bars ("24.3", "24.3.240");
  Time reaches tenths of a millisecond; Timecode adds subframes (80 per frame, "…:ff.40"); Samples reaches single
  samples.

## Rules
Visual folders and summing-bus folders are distinct roles; never convert implicitly. Folder collapse
is view state.
