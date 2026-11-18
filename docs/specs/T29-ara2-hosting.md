# T29 — ARA 2 Hosting and Audio Part Editing

**Status:** Planning draft — not approved for implementation. Product behavior is inherited
from the Composer DAW; a Clef feasibility probe, implementation interview, and approval are
required first.
**Priority:** P3.
**Date:** 2026-11-18
**Task:** [T29](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T14 audio tracks, media, and audio clips; T13 offline render; T16 plugin
catalog, quarantine, and plugin details; T08 undo/redo. ARA host probe gates the rest.

## Reference decisions

The product decisions were taken and verified in the external Composer DAW project. It is not
part of this repository and is never an API:

- `~/Documents/ChatGPT/DAW/Specs/D87-ARA-Support.md` — DONE 2026-11-14, with its approved
  mockup (boards 1–4) and implementation evidence.
- `E02-ARA-Probe.md` — the Tracktion ARA 2 probe with Melodyne 5.4.2 and ARA SDK 2.3.0. Its
  findings are requirements in D87.
- `D88-Plugin-Sandboxing.md` (quarantine; ARA stays in process), `D05-Audio-Editing.md` (audio
  parts and project package), `D14-MIDI-Editor-Panel.md` (bottom editor panel),
  `D71-Part-Edge-Time-Stretch.md`, `D72-Strip-Silence-Normalise-Reverse.md`, and
  `D17-Commands-And-Keybindings.md`.
- [ARA editor](../UI/views/ara-editor.md) and [arrangement](../UI/views/arrangement.md) §ARA
  parts are the Clef UI references.

Clef inherits the behavior, not the C++ implementation. The Composer DAW relied on
Tracktion/JUCE's ARA host; Clef has no ARA host today. Tracktion, JUCE, and Melodyne classes
are historical references, never APIs.

## Critical feasibility gate

The Composer DAW could enable an existing ARA host. Clef builds its VST3 hosting on its own
Steinberg SDK bridge and isolated workers, so ARA 2 requires new host-side work:

- Integrate the Celemony ARA SDK into the native bridge or a dedicated host component, with the
  ARA host callbacks (document controller, model, audio access, plugin entry) owned by the
  worker that hosts the plugin.
- Confirm the ARA SDK license and pin the exact revision before implementation.
- Prove with the installed Melodyne (or another ARA 2 plugin) that a Clef worker can detect an
  ARA factory, create a document and playback region, analyze, play, render offline, save and
  restore the archive, and follow tempo and position.
- Confirm the worker model can host ARA in process with the plugin while keeping the
  application UI, project, and device callback isolated, and that a crash on open is caught by
  T16's quarantine path.

No implementation starts until this probe passes and its evidence is recorded here. A failed
probe records the limitation and stops the task; it does not fall back to an in-house pitch
editor.

## Goal

A composer corrects the pitch and timing of a recorded voice or solo instrument without leaving
the project: select an audio clip, open it in Melodyne (or another ARA 2 plugin) in the editor
panel, edit, and hear the result in context at once.

## Inherited behavior

### Entering ARA

- **Edit with ARA plugin** (`audio.ara.edit`) works on one or more selected audio clips. With
  one ARA plugin scanned it is used directly and the menu item reads “Edit with Melodyne”; with
  several the item is a submenu of plugin names, and Commands search lists one entry per
  plugin. Unavailable with no ARA plugin scanned, no audio clip selected, or when every
  selected clip is already in ARA with that plugin.
- Entering ARA is one undo step for all selected clips. The clips keep position, length, offset,
  fades, gain, colour, and name. The editor panel opens on them.
- A stretched clip keeps its length: the stretch ratio is handed to the plugin, which now does
  the stretching. A reversed clip is already a rendered file and enters ARA as that file.
- Clips already in ARA with a different plugin switch only after a confirmation, because that
  discards their edits in the first plugin (undoable).

### Editing

- Double-click on an ARA clip, or **Open editor** with it selected, shows the plugin's editor in
  the bottom panel. Double-click on a non-ARA audio clip keeps its current behavior.
- The panel shows one editor per ARA plugin. Selecting ARA clips in the arrangement tells the
  plugin which clips are selected, so its view follows the arrangement; clips of several
  tracks can be shown together when the plugin supports it.
- The panel header carries the plugin's name and the detach button. Detaching moves the editor
  to the floating plugin window; re-attaching puts it back.
- The editor needs a minimum height dictated by the plugin. The panel grows to it when opened
  and cannot be dragged smaller while the editor is shown; the previous height returns when
  the panel shows MIDI again.
- While the plugin's editor has focus, keys go to the plugin, except the transport keys Clef
  keeps (play/stop, record, locate), as the plugin-window rules define.
- Edits made in the plugin are heard immediately, are part of the project state, and are saved
  and restored with the project. They are **not** steps in Clef's undo history: the plugin
  keeps its own undo. Clef's undo covers entering, leaving, moving, trimming, stretching, and
  deleting the clip, and restores the plugin's edits with it.

### Arrangement operations on an ARA clip

- Move, copy, duplicate, split, trim, slip, fade and crossfade, gain, mute, colour, and rename
  work as on any audio clip. A copy carries its own copy of the plugin's edits.
- Edge stretch: same gesture and limits; the plugin stretches, no proxy is rendered.
- Reverse, strip silence, and normalize on an ARA clip ask: “Render ARA edits first?” — Render
  and continue, or Cancel.
- Signal order: plugin output, then the clip's gain and mute, then its fades. A per-note gain
  set in the plugin and the clip's gain add up and never overwrite each other. Render ARA to
  audio writes the plugin's output only; the clip keeps its gain and fades as editable values.
- Tempo, meter, and key changes reach the plugin. A clip stays where Clef puts it; the plugin
  does not re-stretch content by itself when the tempo changes.

### Leaving ARA

- **Render ARA to audio** (`audio.ara.render`): renders each selected ARA clip through the
  plugin into a new file in the project media location, at the file's sample rate and channel
  count, and replaces the clip with a normal audio clip on that file with the same position,
  length, fades, and gain. One undo step; undo restores the ARA clip with its edits.
- **Remove ARA** (`audio.ara.remove`): the clip returns to the original file and the plugin's
  edits are discarded. One undo step; undo restores them.

### Appearance and failure

- An ARA clip shows a badge with the plugin's name in its header strip; the waveform is the
  original file's.
- Missing or quarantined plugin: the clip plays the original file, its badge takes the danger
  treatment, and its tooltip names the plugin. The ARA state stays in the project untouched and
  becomes live again when the plugin is available. Opening the editor shows a notice in the
  panel instead of an editor. Remove ARA stays available; Render ARA to audio does not.

### Commands

`audio.ara.edit`, `audio.ara.render`, and `audio.ara.remove` go through the shared command
registry: Actions list, Commands search, assignable keystrokes, the clip context menu, and the
application menu path defined by T18, with live bindings and context-aware availability. No
default keystroke.

## Clef requirements

### Hosting and isolation

- ARA plugins run in process with the Clef worker that hosts them, never in the application
  process. The device callback never calls into ARA and never blocks on it.
- All ARA model changes happen on the worker's message thread. While a document is being
  edited, that clip outputs silence for the affected block (or follows the host's try-lock
  policy), and the rest of the session keeps playing. No allocation or lock is added to the
  callback.
- A crash guard entry is written before an ARA plugin is instantiated, so T16's quarantine
  catches a plugin that brings the worker down on open. ARA state is never silently dropped by
  a quarantine decision.
- The ARA host extension is part of the native bridge contract: small C ABI, explicit thread
  affinity and lifetimes, no C++ exception or Rust panic crossing it. Follow
  [native/AGENTS.md](../../native/AGENTS.md).

### State and persistence

- The project stores each ARA clip's plugin identity, the ARA archive, and the clip association
  needed to restore the document. Save/open restores the edited state; the analysis is not
  redone from scratch when the plugin and archive are available.
- The project schema migrates earlier documents. A project without ARA clips round-trips
  unchanged; a project with ARA clips opened without the plugin keeps the ARA state on save.
- Autosave and crash recovery (T07) capture the ARA archive with the same policy as plugin
  state; the cost is bounded and recorded.
- Two clips on the same file each get their own edits (separate audio modifications).

### Realtime and offline render

- ARA playback is read through the plugin in the worker; clip gain, mute, and fades stay in
  Clef's path. Sample-accurate positioning and PDC are preserved.
- T13's offline render and the Render ARA to audio path read ARA clips through the plugin and
  include them in the render.
- An ARA clip never uses a stretch proxy. Leaving ARA restores proxy use where the clip type
  requires it.

### Interaction with neighbouring tasks

- T14 provides audio clips, media storage, and the missing-media path.
- T26/T27 introduce takes and comping. The reference makes the ARA commands unavailable on
  clips with recording takes and on MIDI clips. The interview decides the reverse rule: whether
  comping is refused on an ARA clip or ARA is refused on a clip with takes, with a clear reason.
  A different rule (for example, ARA on a flattened comp) needs an explicit decision.
- T13 supplies the render path; T16 supplies the catalog, quarantine, and missing-plugin
  resolution; T08 supplies undo for the commands that are undoable.
- No in-house pitch or timing editor is built.

## Out of scope

An in-house pitch or timing editor; drawing analyzed notes on the clip; audio-to-MIDI from ARA
notes; ARA on tracks instead of clips; AU ARA (VST3 ARA first; macOS only); Windows; alignment
plugins (VocAlign, Auto-Align); other ARA plugins beyond the one verified; freeze and bounce.

## Open questions

- The probe outcome: can the existing worker host ARA 2, or does it need a dedicated host
  process or an in-process exception? Record the evidence and limits.
- ARA SDK license, exact revision, and whether the SDK is a required build dependency.
- VST3 only, or AU ARA later; whether the scanner marks ARA-capable plugins separately.
- Which Melodyne version is verified, and what evidence counts for another ARA plugin.
- Project schema layout for the ARA archive; one bump or reuse the plugin-state representation.
- Autosave cost and whether the archive is captured on every autosave.
- Editor panel height policy, detach behavior, and keyboard passthrough details.
- Whether general clip fades/crossfades exist when T29 starts; if not, whether T29 depends on
  that capability or excludes the fade operations from the ARA signal order.
- Interaction with clips that have takes (T27) and with flattened comps.
- Analysis progress reporting, timeouts, and failure messages.
- Maximum ARA documents per project and memory limits.
- Command names and categories, if they differ from the reference.

## UI proposal requirement

The ARA editor panel, the clip badge and menu entries, the unavailable-plugin notice, the
render question, and the detached editor require a written UI proposal derived from
[ARA editor](../UI/views/ara-editor.md), [arrangement](../UI/views/arrangement.md), and
[plugin window](../UI/views/plugin-window.md), with user approval before implementation. The
reference mockup and snapshots are evidence of intent, not a pixel target.

## Verification intent

### Probe

- An ARA 2 factory is detected in a Clef worker; a document, audio source, and playback region
  are created; analysis returns note content.
- Offline render through the plugin has the expected length and is not silent.
- Save, destroy, and reopen restores the ARA archive; tempo and position changes reach the
  plugin.
- Manual GUI check shows the plugin editor and an audible pitch edit; transport position
  follows.

### Implementation

- Entering ARA keeps position, length, offset, fades, and gain, disables the stretch proxy, and
  is one undo step; undo and redo restore the non-ARA and ARA states.
- An ARA clip renders offline with signal where the clip sits; moving it moves the signal.
- Save and reopen restores ARA clips with their plugin and analyzed notes without re-analysis.
- Edge stretch to 150 % yields rendered signal 1.5 times as long, with no proxy file.
- Render ARA to audio produces a normal clip on a new file whose content matches the ARA render
  within tolerance; undo restores the ARA clip.
- Remove ARA returns a normal clip on the original file; undo restores ARA.
- With the plugin description unavailable, the project opens, the clip plays the original file,
  is flagged, and a save keeps the ARA state.
- Command availability follows the selection; each command appears in the registry.
- No allocation, lock, or blocking call is added to the device callback; a quarantined ARA
  plugin leaves the session playing.
- GUI, listening, and hardware evidence stay separate from automated checks.

## Approval gate

This document is a planning draft. The product decisions are the referenced Composer DAW
specification; the ARA host probe, a Clef implementation interview for the open questions, an
approved specification, and an approved written UI proposal are required before implementation.
