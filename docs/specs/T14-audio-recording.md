# T14 — Audio Tracks and Recording

**Status:** Planning draft — not approved for implementation.
**Priority:** P1.
**Date:** 2026-11-17
**Task:** [T14](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T05 timeline; T11 mixer; media-storage decision before T04 schema v2 freeze.

## Goal

Add audio tracks and recording to the session: select an input, monitor, record to disk,
display the waveform, and reference the recorded media safely. This is the first task that
requires a durable decision about where audio media lives.

## Proposed Scope

- An audio track type alongside instrument tracks.
- Input channel selection and recording-device configuration.
- Monitoring through the track output, with explicit latency behavior.
- Recording to disk with a defined file format and naming policy.
- Waveform display for recorded and referenced audio.
- Project references to media, with missing-file detection.
- Count-in and punch scope agreed in the interview.
- Save/open, missing-and-relink behavior, and no partial recording published as complete.

## Out of Scope

Comping and take lanes (T26/T27); warp/time-stretch; ARA (T29); audio editing beyond
waveform display; automatic sample-rate conversion; cloud media.

## Open Questions

- **Media storage:** single project file versus project package/folder or sidecar media
  directory. This must be decided before schema v2 freezes in T04.
- Recording format, bit depth, and channel count.
- Punch recording and count-in scope.
- Input monitoring through the plugin chain and how latency is reported.
- Takes and comping are T26/T27; this task must leave recording, file identity, and clip
  structure ready for them.
- Missing media relink UI and project-open behavior.
- Disk streaming and buffering for long recordings.
- Waveform cache policy. Evaluate per-channel min/max peaks at multiple resolutions, built in
  the background and selected by zoom. Derived data should be rebuildable from the source media.
  A missing or stale cache should leave the waveform pending while playback and project loading
  proceed. Agree on source identity, invalidation, and disk limits before implementation.
- Maximum recording length and disk-space failure behavior.
- Whether audio clips share the T09 clip grammar.

## UI Proposal Requirement

Audio-track headers, waveform display, recording states, and the missing-media surface
require a written UI proposal derived from `docs/UI/` and user approval before
implementation.

## Verification Intent

Deterministic capture; media references and paths; missing and relinked files; save/reopen;
no partial recordings published as complete; monitoring and PDC; disk failure handling.

## Approval Gate

This document is a planning draft. The media-storage decision, an interview, an approved
specification, and an approved written UI proposal are required before any implementation.

## Carried over from T41 (2026-11-21)

- **Track Monitor control.** T41 left Monitor out of the instrument track header because Clef has
  no monitor function yet; define it here with audio input monitoring and add it to the header
  between arm and Mute ([track header](../UI/views/track-header.md)).
- **Recording bit depth in the transport.** The T41 transport shows the device sample rate; once
  the audio recording format is decided, the readout becomes `<rate> kHz · <bits>-bit`.

