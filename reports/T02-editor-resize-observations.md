# T02 Editor Resize Observations — 2026-11-17

Computer-use inspection exercised the original plugin editors inside the eframe
window. Screenshots were observed in the task conversation; no screenshot artifact
is claimed here. Each test used the selected saved state and scheduled playback on
EVO16 outputs1–2 at48kHz/128frames,512sample transport lead.

## Pianoverse VST3

The editor restored AA Y7 and Width63.2. It advertises no user resize capability;
the host zoom control is disabled. Settings were inspected and closed, leaving the
same preset/parameter. No resize pass is claimed for an unsupported operation.
The observed final console report covered5,229,056 callback frames, zero missed or
stale blocks/output errors, and normal editor close/stop/worker exit.

## ZebraHZ VST3

The plugin's own right-click scale menu offered50–200%. Before correction, choosing
80% changed plugin content to904x640 while the host remained1130x800, leaving empty
space. The bridge's deferred resize path called the user-resize negotiation helper;
`canResize()==false` made that helper re-query the previous dimensions instead of
acknowledging the requested size.

The pinned SDK's `pluginterfaces/gui/iplugview.h` states that canResize refers to
user sizing, and onSize must follow a plugin resizeView request. The poll path now
acknowledges the requested rectangle through onSize regardless of user-draggability,
then forwards that accepted client area to eframe. Actual100%->80%->100% inspection
confirmed904x640 and1130x800 content areas with matching windows and no empty margin.
Final callback report:2,717,184frames, zero misses/stale blocks/output errors, normal
editor close/stop/worker exit. This supplies the before/after resize regression.

## ZebraHZ CLAP

The same original scale menu was exercised100%->80%->100% during playback. Both
plugin content and host client area changed to904x640 and back to1130x800, with no
stuck editor. Plugin-originated CLAP request_resize updates the host area without a
redundant plugin set_size call. Final callback report:3,224,960frames, zero misses/
stale blocks/output errors, normal editor close/stop/worker exit.

These are GUI and short live-counter observations. The separate600second and
physical MIDI reports establish their own durations; they are not inferred here.

## Explicit Preset Change and Fresh-Worker State Proof

For each ZebraHZ format separately, the original browser was opened during
scheduled playback, `Alternator Flute` was selected and visibly loaded, and the
browser's Restore operation returned to the agreed `initialize` sound. Master was
then changed from20.00to40.00 using the native editor, stopped, saved to a new
verification-only snapshot and restored by `recreate` in a fresh worker.

Both fresh workers visually showed Master40.00 and `initialize [Loaded]` in the
original browser, then produced nonzero scheduled audio. VST3's post-recreate
observation covered2,532,992callback frames with peak0.24154612 and zero missed/
stale blocks or output errors. CLAP's post-recreate observation covered1,958,144callback frames with peak0.23567021
and zero missed/stale blocks/output errors. Original Master20 snapshots
were restored before ending these sessions; no factory preset was overwritten.

Verification-only snapshots:
`validation-output/states/zebrahz-vst3-preset-change-master40.state` and
`validation-output/states/zebrahz-clap-preset-change-master40.state`.
The long scheduled, physical, and final offline runs consistently use the original
Master20 snapshots and their recorded SHA256 values; they are not silently relabeled
as tests of these new Master40 snapshots.
