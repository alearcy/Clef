# T06 — MIDI Recording and Clip Playback

- **Status** — Done · 2026-11-19
- **Priority** — P0
- **Updated** — 2026-11-19
- **Task** — [T06](../../TASKS.md)
- **Prerequisites** — [T04 multitrack session](T04-multitrack-session.md) and
  [T05 transport](T05-transport-timeline.md)

## Goal

Record an instrument performance from the project MIDI input, hear it back in the
arrangement, and save a project that preserves the performance and alternative takes.

## Confirmed in the interview

- Arm one instrument track at a time. Use the project MIDI input. Live MIDI follows the
  armed track regardless of selection and follows the selected track again after disarm.
- Provide three selectable recording modes: Merge, Replace, and New take. Keep takes
  available for later comping.
- Capture notes with their MIDI channel and velocity, sustain (CC64), pitch bend, and
  modulation (CC1). Retain original timing with latency compensation. Reuse T05's
  optional one-bar count-in. Release held sounds on Stop.

## Recording Workflow

- Store a single project-wide mode, defaulting to Merge. The transport menu sets it for the
  next recording. Track-specific overrides and take-lane editing belong to
  [T26](T26-midi-comping.md). Store arm state in the session and open projects disarmed.
- Record starts from Stop at the current playhead using T05's loop normalization and
  optional count-in. Keep live monitoring during count-in and start capture at the first
  playback sample after it ends. Stop during count-in cancels the take. Record requires
  explicitly enabled audio, an available project MIDI input, and a ready armed instrument.
- Freeze the recording target, mode, loop and tempo/meter settings for a take. Keep track
  selection available and guard locate, target removal or replacement, and other commands
  that would interrupt capture. Stop finalizes the take as one project change and applies
  the current T05 Stop Mode. Disabling audio finalizes capture before shutting audio down.
- For a linear pass use the half-open range from actual capture start to Stop. With Loop
  enabled use the T05 loop range. Treat the first and last partial passes as passes and
  keep only passes with captured material. An empty recording leaves the project clean
  when it was clean and leaves existing material intact.
- During capture, keep the other tracks playing and keep the target track's live input
  active. Merge also plays existing material on the target. Replace and New take suppress
  the target's existing playback in the capture range until the take finishes.

## Mode Semantics

The rules adapt the Composer behavior referenced in [T26](T26-midi-comping.md#reference-decisions).

| Mode | Linear | Loop |
| --- | --- | --- |
| **Merge** | Add MIDI to the existing clip in its time range and create a clip where needed. Where there are takes, place new events in the take active at their position. | Add the captured events from every non-empty pass at their positions within the loop. |
| **Replace** | Replace active material within the recorded range. Remove notes by attack and controllers by event position. Keep material outside that range and preserve inactive takes. | Apply the last non-empty pass to the full loop range, including when Stop ends that pass early. An entirely empty recording changes nothing. |
| **New take** | Add a take to the clip touched by the recorded range. Join multiple touched clips, preserving existing takes, active sections, and gaps. Extend the clip as needed. In empty space, create a clip with its first take. | Keep one take per non-empty pass in one clip covering the loop. Activate the latest non-empty take over the loop. |

In New take, activate the new performance over the recorded range and preserve the
previous playback selection elsewhere. Store ordered takes with stable identities and
half-open active sections so T26 can expose them for comping. Playback chooses note attacks
and controller events by their active sections and plays the full duration of eligible
notes, including across take-section boundaries. Keep existing clip boundaries in Merge
and Replace where possible. A new clip covers the portion of the capture range that
requires it.

## Timing, Playback, and Recovery

- Timestamp incoming MIDI against T05's device clock and map it to the musical position
  the performer hears. Apply the shared output and compensated render-path latency once.
  Preserve channel, velocity, note duration, controller values, and ordering at identical
  positions. Use the project timing grid at its native resolution without quantization.
- Schedule clip playback with T05's tempo map and sample offsets into the correct track's
  isolated worker. Prepare bounded playback data and finalize captures off the device
  callback. Keep live and playback event ownership distinct so releasing one source does
  not accidentally release the other source's matching notes.
- At capture start use fresh note attacks and the supported live controller state. End a
  captured held note at Stop or the loop boundary. At a recording wrap, carry a still-held
  key into the next pass at the left locator until its physical release. Carry current
  controller state across passes.
- On playback start, locate, and loop wrap, restore supported controller state and active
  notes at the destination. Release stale playback voices and sustain before scheduling
  the new position. On Stop, clear the recording and playback voices and sustain on
  affected instruments. Fresh live MIDI remains playable with audio enabled.
- On input loss, device failure, worker failure, or capture overflow, stop the take and
  show the cause. Preserve reliable captured material as new takes when possible and
  leave the existing active arrangement intact. If no complete material can be recovered,
  report it. Retain accepted data on a recoverable failure and never apply a partial
  Replace.
- Save clips, takes, active sections, record mode, and T05 transport settings using the
  existing atomic project save. Open older projects with empty lanes and Merge mode. Keep
  T05's reopen behavior of audio disabled, transport stopped, and position at bar 1.
  Completed recordings mark the project dirty and receive the existing save/discard
  protection.

## Approved UI

Extend the [transport bar](../UI/views/transport-bar.md) and
[multitrack session](../UI/views/multitrack-session.md). Use the
[arrangement](../UI/views/arrangement.md) and [track header](../UI/views/track-header.md)
as design references.

```text
Multitrack session
  Transport row
    Return to Start | Play | Stop | Record | REC Merge ▾ | existing transport controls
  Track headers and aligned timeline lanes
    Instrument row | Record Arm target
    Timeline lane  | growing recording region, committed clips, playhead
  Session status
    Count-in or recording label, unavailable reason, or capture interruption
```

- Use the transport button for Record, the
  [track state target](../UI/components/track-state-button.md) for Record Arm, and the
  chip and menu pattern for the mode selector. Open the menu to show Merge, Replace, and
  New take with the current choice checked. Keep the change unavailable during a take.
- Expose ready, unavailable, count-in, and recording states. Use the danger lamp during
  capture. Fill the track target when armed and square its center while recording. Keep
  Stop visibly available throughout count-in and capture. Give Record, arm, and mode
  commands accessible names, tooltips, shared command dispatch, and keyboard access.
- Draw the growing region with the danger role and committed clips in their lanes using
  the ruler's existing viewport transform. Keep the playhead legible. Show the clip name
  and take count where they fit, and expose the track, musical range, and take count
  accessibly.
- Keep Record and mode selection beside Stop on one row at 1024 and 760 points. Put
  secondary controls in Transport Settings if needed at 760 points. Use the existing
  provisional egui surfaces and verify the proposal with the actual layout and states.

## Scope Boundaries

T26 owns take-lane display, manual take and comp editing, and track-specific record mode
overrides. T08 owns general undo/redo. Clip move, resize and split belong to T09, and
piano-roll editing to T10. Audio recording, punch, capture during existing playback,
quantization, retrospective capture, and automation are outside this proposal.

## Acceptance

- Fake MIDI at 44.1 and 48 kHz proves captured positions and playback sample offsets at
  different latencies, tempo changes, count-in, and loop wraps. Cover Note On/Off, channel,
  velocity, CC64, pitch bend, CC1, equal-time ordering, and held keys across transitions.
- Exercise all modes with and without loop, including partial and empty passes, multiple
  touched clips, active and inactive takes, Stop cancellation during count-in, and
  interruptions. Verify that no stale event or stuck note survives Stop, seek, or wrap.
- Verify arm routing independently of selection, healthy track continuation where
  applicable, and bounded callback work without allocation, waits, or locks.
- Verify round-trip, older-project defaults, retained takes, atomic save, and dirty-state
  protection. Run the repository gates and relevant T02–T05 regressions.
- Verify layout and keyboard operation at 1024 and 760 points. Exercise the record and
  arm states, mode choices, recording preview, and interruption feedback. Record
  automated results, macOS GUI evidence, and physical MIDI/listening evidence separately.

## Approval Gate

The user approved the recording semantics, timing and recovery rules, and written UI
proposal on 2026-11-18, including Replace over the full loop when the last pass ends early.

## Implementation

The project stores MIDI as an optional v2 metadata extension. Track IDs own sorted clips,
and stable take IDs own each section's events. Edits validate the candidate before it replaces
the arrangement. Interrupted capture adds separate takes and preserves existing active
material. Capture is bounded to 16,384 incoming messages per recording, with a separate bound
on generated loop material. Placement and the project format also enforce independent limits.

The transport publishes an absolute presentation start and its musical projection without
allocating on the audio callback. The start allows the existing worker lead and compensation
to deliver clip events at the same presentation samples as the click. Capture correlates the
incoming arrival with the device anchor and output latency. Brokers schedule and release MIDI
for their own instruments. A new playback generation invalidates stale buffered audio after
transport discontinuities. Capture assembly and plan construction run off the callback.

## Verification — 2026-11-19

- Deterministic tests cover both sample rates, tempo transitions, ordered note/controller
  capture, compensated sample positions, clipped held notes, sustain chase, loop wrap,
  stale packet rejection, allocation-free callback transitions, all three recording modes,
  interrupted take placement, persistence, and legacy defaults.
- The native T06 test uses the configured CoreAudio stereo output, an owned virtual CoreMIDI
  source, and the rebuilt TestSynth VST3. It verifies arm routing independently of selection,
  cancelling count-in, capture, disarm routing, Save/Open, lazy loading, scheduled clip
  events, and nonzero audio output from the reopened track. Removing the owned input
  during Replace stops audio and retains the interrupted performance as an inactive take
  while preserving the original active events.
- Locked check, build, format, Clippy and tests passed. The default suite ran 74 tests
  and skipped 10 explicitly selected native cases. The T06 native case and four T02/T03
  TestSynth regression cases passed when selected explicitly.
- The egui test checks control and clip labels, bounds, and focus and activation actions
  at 760 and 1024 points for stopped, count-in, and recording states. The initial
  automated validation used virtual MIDI. The user subsequently confirmed that the
  delivered recording behavior worked in their setup.

## Arrangement Note Preview — 2026-11-19

The user confirmed that the delivered T06 behavior works and requested two refinements to
recording feedback. The user approved this proposal on 2026-11-19.

```text
Instrument row
  Timeline lane
    Grid and committed MIDI clip with active note marks
    Loop recording: previous completed pass behind the current recording region
    Current recording region with live note marks
    Playhead
```

- Extend the existing timeline lane and recording preview primitives. Use the shared viewport
  to draw read-only horizontal note marks at their attack and duration, with pitch mapped from
  low to high. Keep marks clipped to the visible clip or recording region, below the label.
  Committed clips draw the notes selected by their active sections.
- Show captured notes as soon as their Note On reaches the session. Held marks grow to the
  current position and settle at Note Off or the pass boundary. Use the danger role for live
  marks and the clip foreground for committed notes. A controller-only region has no marks.
- With Loop enabled in Merge, Replace, or New take, retain the latest completed pass containing
  notes as a dimmed region behind the current pass. It shares the same time and pitch coordinates as the live
  marks. At wrap, the previous pass stays visible while the live region starts growing again.
  Before the first pass completes, any existing committed clip provides the background.
- Keep this change in T06 as recording and read-only arrangement feedback. T09 owns clip
  editing, T10 owns note editing, and T26 owns take lanes and comp selection.

Verify Note On, held growth and Note Off, ordinary Stop, Save/Open, the first and subsequent
loop wraps, and the shared clip geometry at both workspace widths.

### Note preview verification

- The session builds note previews from the capture's note pairing and loop projection.
  Preview data is transient. The egui painter uses the audible position to extend held
  marks and renders committed active notes from the project.
- The locked suite passed 77 tests. Focused tests cover immediate Note On, closing on
  Note Off, wrap with a quiet or controller-only pass, growth over the previous pass
  at the same pitch, and clipping and take selection on committed parts.
- The native CoreMIDI/TestSynth test passed with two loop passes. It observed the first
  pass in the background while the second note was held and released, then verified
  both takes on disk and playback after reopening.
- Check, build, format, Clippy, and bundle binary comparison passed.

The user accepted live note feedback and the New take overlay, and requested the same overlay
in Merge and Replace. The three modes share the preview path. The native CoreMIDI/TestSynth
regression passed and observed the previous pass after wrap in all three modes.

## Completion — 2026-11-19

The user accepted the delivered workflow and requested closure after receiving the
updated build with the previous-pass overlay in Merge, Replace, and New take. T06 was
removed from the open task index.
