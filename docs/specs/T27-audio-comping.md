# T27 — Audio Comping and Take Lanes

**Status:** Planning draft — not approved for implementation. Product behavior is inherited
from the Composer DAW; the Clef implementation interview and approval are still required.
**Priority:** P2.
**Date:** 2026-11-18
**Task:** [T27](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T14 audio tracks, recording, and media storage; T26 take and comp model;
T08 undo/redo; T09 clip editing.

## Reference decisions

The product decisions were taken and verified in the external Composer DAW project. It is not
part of this repository and is never an API:

- `~/Documents/ChatGPT/DAW/Specs/D76-Audio-Takes.md` — DONE 2026-11-18. It extends the MIDI
  take model in `D66-MIDI-Takes.md` and reuses `D70-Fades-And-Crossfades.md` and
  `D106-Audio-Recording.md`.
- `D87-ARA-Support.md` records the reference behavior for ARA parts; T29 owns ARA in Clef.
- [Track lanes](../UI/views/track-lanes.md), [arrangement](../UI/views/arrangement.md), and
  [audio editor](../UI/views/audio-editor.md) are the Clef UI references.

Clef inherits the behavior, not the C++ implementation. Tracktion `WaveCompManager`, its
rendered comp file, and the Composer storage format are historical references: Clef owns its
take model, comp playback, and crossfade path in Rust.

## Goal

Record several audio passes, keep every pass as a take, and assemble a click-free comp with
selection and crossfades, without flattening or deleting source media.

## Inherited behavior

### Record modes for audio

- T26's effective mode (the track's mode, or the global one when the track is on Global)
  applies to every recorded audio track of the pass. MIDI keeps T26's rules unchanged.
- **Merge**: T14's current behavior — a new clip on top, the covered material muted.
- **Replace**: as Merge, but the covered pieces are removed instead of muted; their files stay.
  Loop: the last complete pass replaces the loop range. A track that recorded nothing removes
  nothing.
- **New take**: the recording becomes a new take of the audio clip under it. A recorded range
  touching several audio clips of the track first joins them into one clip; their content and
  the gaps between them become take 1. The clip grows to cover the recorded range; older takes
  are silent in the grown span. On empty space a new ordinary clip is created. Loop: one take
  per complete pass, in one clip covering the loop range, the last pass active. Punch: the take
  covers the punch range.
- A new take becomes active over its recorded range; the rest of the comp is unchanged.
- Stop commits the whole pass (audio and MIDI, takes, joins, comp) as one undo step, “Record”.

### Takes and comp

- A take is what one pass recorded on that clip: one or more audio regions (file, source
  offset, position, length). Take 1 of a joined clip keeps its former clips as regions,
  including their gains, fades, and crossfades within it.
- A clip with two or more takes plays its comp: sections, each a time range assigned to one
  take. Inside a section the active take plays; where that take has no audio the comp is
  silent.
- **Comp crossfade**: every cut between two sections plays an equal-power crossfade centered on
  the cut. The default length is a project setting, **Comp crossfade**, 10 ms, adjustable
  0–200 ms (0 = hard cut), saved with the project, undoable. A cut keeps the default until its
  own length is dragged; then it keeps its own value. Shape follows the project's crossfade
  shape commands (Equal power / Equal gain). A crossfade never extends past the neighbouring
  cut's crossfade or the clip's edges; it shortens to fit.
- Take lanes, the “T n” chip, comp by Select drag, cut dragging, Use whole take, Delete take,
  Flatten comp, Take to new track, lane names, and lane heights: as T26, for audio clips too.
  If Clef adopts tracks that hold both MIDI and audio clips, one lane set serves both kinds.
- **Flatten comp** turns the comp into ordinary audio clips: one per section (per region inside
  a section), at the same times, with the cut crossfades becoming ordinary crossfades of the
  same length and shape. Take to new track creates an audio track holding the take's regions as
  ordinary clips.
- Delete take and Flatten remove takes from the model only; files are never deleted.
- **Cut drag**: clamped at the neighbouring cut, one grid step minimum; Alt free with sample
  precision. On the track row the cut shows the crossfade X; dragging its edge changes only
  that cut's length, one undo step, “Comp crossfade”, with a millisecond readout.

### On a take lane

- **Slip**: Cmd-drag on a take's audio moves that take's audio inside the clip without moving
  the clip or the comp; snapped to the grid, Cmd-Alt free. Audio slid beyond the take's file is
  silence. One undo step, “Slip take”.
- **Take gain**: a value slider in the lane header, −30 to +12 dB, 0.1 dB steps, double-click
  resets to 0 dB. It applies to that take wherever it plays, in every clip of the lane. One undo
  step, “Take gain”, with a readout chip when not 0 dB.
- **Solo take**: an S toggle in the lane header. While on, the track plays that take over the
  whole of every clip instead of the comp; one solo lane per track. View state only: not saved,
  not undoable, the comp is unchanged. It ends on any comp edit of the track or by toggling.
- Fades on the clip's outer edges, move, copy, duplicate, split (every take and the comp
  split), resize, delete, mute, and automation work on a clip with takes as on any clip.
- Processing that would rewrite the source (reverse, normalize, quantize, stretch, gain to
  file) is refused on a clip with takes with the reason “Flatten the comp first”. The audio
  editor shows the comp read-only.

### Take groups

- Audio tracks recorded in the same New take pass form one take group. Their clips made by that
  pass share take indices and the comp: a comp drag, cut move, cut crossfade length, Use whole
  take, Delete take, and Flatten on any member act on all members, in one undo step. Slip, take
  gain, and solo stay per track.
- A later New take pass onto a group's clips joins the group again when all its members record;
  if only some record, the recorded tracks get their take and are unlinked from the rest with
  a notice, in one undo step.
- **Unlink takes** dissolves the group of the selected clips; each keeps its comp. The lane
  header shows a link glyph with the number of linked tracks while a group exists.

### Commands

All commands go through the shared command registry, Actions list, Commands search, and
assignable keystrokes, with live bindings:

- T26's commands extend to audio clips.
- Solo Take, Unlink Takes, Comp Crossfade Shape: Equal Power / Equal Gain.
- Take gain and the Comp crossfade setting are value sliders; Reset Take Gain is a command.

No command gets a default keystroke unless the interview agrees one.

## Clef requirements

### Media and model

- T14 must settle media storage before this task starts. Takes reference recorded files; the
  project document stores take regions, comp sections, per-cut crossfade length and shape,
  lane state, and take-group membership. Comping never rewrites or deletes source files.
- Coordinate one take-schema bump with T26. Projects without audio takes open unchanged.
- A missing media file follows T14's missing-media path; a comp references only files that
  remain in the project.

### Playback and realtime

- Comp playback is prepared off the device callback as a bounded set of clip segments and
  gain ramps. The callback performs no allocation, lock, formatting, logging, or I/O.
- Cut crossfades are sample-accurate equal-power ramps centered on the cut; the sum of the two
  sides stays within the agreed tolerance and produces no click at 0 ms or at the default
  length.
- PDC, latency compensation, and the existing anti-click path are unchanged. Comp edits during
  playback are applied at a buffer boundary with no partial plan audible.
- Flatten and Take to new track are model operations prepared off the callback; they add no
  realtime work.

### Undo and commands

- Each recording, take creation, comp drag, cut move, crossfade length or shape change, slip,
  take gain, rename, delete, flatten, unlink, and take-to-track action is one undo step.
- Lane visibility, lane heights, and solo-take listening state follow T26's view-state rules.

### Interaction with neighbouring tasks

- T14 provides audio clips, recording, monitoring, and media references. T27 extends them.
- T26's take lanes and comp grammar are shared; do not create an audio-only lane model.
- T27 inherits clip fades and crossfades from the reference (D70). Clef has not yet agreed
  general clip fades; if they do not exist when T27 starts, they are a prerequisite or T27 must
  exclude them. T27 itself implements only the crossfade at comp cuts.
- T29 (ARA) must define its interaction with clips that have takes; T27 does not add ARA.

## Out of scope

General clip fades and crossfades beyond comp cuts; warp/time-stretch; ARA (T29); automatic
alignment of takes; whole-track playlists; a rendered comp file; cleaning unused media;
comping across tracks not recorded together; hybrid MIDI/audio tracks if Clef has not adopted
them; tape-style loop record into one group.

## Open questions

- T14's media-storage decision and its effect on take regions and file identity.
- Whether general clip fades exist when T27 starts, or whether T27 carries the minimum fade
  path needed by comp cuts; how a later general fade task reuses it.
- Exact schema layout for regions, sections, cut lengths, and take groups; one bump with T26.
- How the engine schedules several overlapping regions and gain ramps without per-block work.
- Whether Clef adopts hybrid tracks; if not, take lanes stay per track type and a later task
  reconciles them.
- Take-group membership and undo when only some members record.
- Sample-precision rules for cut dragging and crossfade shortening.
- Refusal copy and which processes are available when T27 starts.
- Command names and categories, if they differ from the reference.

## UI proposal requirement

Audio take lanes (waveforms, active and dimmed sections, take gain, S toggle, link glyph), the
clip row with cut crossfades and length drag, the slip gesture, the record-mode surfaces, the
refusal reason, and the Comp crossfade setting require a written UI proposal derived from
[track lanes](../UI/views/track-lanes.md), [arrangement](../UI/views/arrangement.md), and
[audio editor](../UI/views/audio-editor.md), with user approval before implementation.

## Verification intent

- Comp selection and Use whole take play the expected take; each cut crossfades with equal
  power at the project length, sample-checked in a render; a dragged cut length and a shape
  change apply to that cut only; 0 ms is a hard cut; crossfades shorten to fit.
- Cut drag snaps to the grid; Alt is free; the attached crossfade moves with it.
- Slip, take gain, and Solo take match the render; solo leaves the comp and undo history
  untouched.
- Two audio tracks recorded together comp together, sample-aligned in the render; Unlink
  dissolves the group; a partial pass unlinks with the notice.
- Flatten yields ordinary clips and crossfades rendering identically to the comp; Take to new
  track; Delete take; no file deleted.
- Part operations keep takes; refused processing gives the reason.
- One undo step per action, redo; save/reopen keeps takes, comp, cut lengths and shapes, slip,
  gains, and groups; an older project opens unchanged.
- No realtime allocation or lock added to the callback; PDC unchanged.
- Commands in the registry with live bindings; real-component snapshots compared with the
  approved boards.
- Automated, GUI, listening, and hardware evidence stay separate.

## Approval gate

This document is a planning draft. The product decisions are the referenced Composer DAW
specification; T14's media-storage decision, a Clef implementation interview for the open
questions, an approved specification, and an approved written UI proposal are required before
implementation.
