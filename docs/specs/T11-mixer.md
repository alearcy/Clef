# T11 — Mixer and Master Section

**Status:** In Progress — implemented on 2026-11-19. Two-instrument listening verification is pending.
**Priority:** P1.
**Date:** 2026-11-17
**Task:** [T11](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T04 multitrack mix path.
**Review draft:** 2026-11-19. [Written UI proposal and mockup](#ui-proposal).

## Goal

Give the user a clear balance surface for instrument tracks and the master: gain, pan,
mute, solo, metering, and a defined output path. The mixer is the first surface that makes
gain staging and solo/mute semantics explicit.

## Proposed Scope

- Channel strips for every instrument track and the master.
- Instrument input Gain trim, pan, stereo Width, fader, mute, solo, and peak/hold/clip metering.
- Master output level and a clear direct-to-master routing path.
- Meter values driven by the realtime engine, not by UI-side estimation.
- Keyboard and command-registry integration for mute/solo and navigation.
- Persistence of mixer values in the project.

## Out of Scope

Buses, sends, groups, and the routing graph (T15); insert effects (T12); automation;
master limiter or loudness normalization unless explicitly agreed; channel EQ.

## Behavior Proposed for Review

- Use the existing stereo instrument paths. Pan is stereo balance on each instrument and
  the master. Center preserves both sides at unity. Moving toward one side attenuates the
  opposite side linearly in amplitude, reaching silence at the extreme. Separate mono
  channels, polarity, and configurable output routing are deferred.
- Add a **Gain** trim on each instrument strip, applied at the start of the mixer path
  after latency alignment. Proposed range: −24 to +24 dB, with a 0 dB default and reset.
  Keep the trim and fader independently adjustable. The track signal flows through Gain,
  Width, Pan, the fader, and the mute/solo gate in that order.
- Add **Width** beside Pan on each instrument strip. The proposed range is 0–200%,
  with a 100% default and reset. Scale the stereo side component while preserving the
  mid component. At 0% both channels carry the average of the original sides, at 100%
  stereo is unchanged, and at 200% the side component is doubled. Retain floating-point
  headroom through this stage. Master retains its fader and Pan, as in the mixer reference.
- Give the track and master faders a range from silence (−∞) to +6 dB, with 0 dB as the
  default and reset value. Smooth audible trim, gain, width, balance, mute, and solo changes with a short
  sample-rate-aware ramp. A continuous gesture makes one undo entry and Escape restores its
  starting value. Persist track trim, fader level, width, balance, mute, and solo, plus
  master fader level and balance. Open older projects with 0 dB trim and faders, 100% width,
  centered balance, and mute and solo clear.
- Solo is additive solo-in-place. With no solo active, all unmuted tracks are audible.
  With any solo active, only unmuted soloed tracks are audible. A track with both switches
  active is silent. Mute and solo gate audio after latency alignment, without interrupting
  MIDI, worker processing or other tracks. The metronome is independent of track mute and
  solo and passes through the master gain and balance.
- Meter each stereo track after gain, balance, mute, and solo. Meter the master after gain
  and balance, before the existing device clamp. An engine peak above 0 dBFS latches a clip
  indicator until reset. Use immediate attack, a proposed 20 dB/s display decay, and a
  one-second peak hold. Reset meter state when audio stops or the project changes. The
  callback publishes bounded peak data and clip observations. Display timing and dB
  conversion run outside the callback. Preserve floating-point headroom through the sum.

## UI Proposal

Provisional Clef UI pending the T18 design-system acceptance. Follow the [Mixer](../UI/views/mixer.md)
and [Channel strip](../UI/views/channel-strip.md) references for relative placement and visual
roles. This pass proposes one bank of instrument strips and a fixed Master strip. The inspector,
search, routing sections, insert slots, automation and macros are deferred.

```text
Project workspace
  View / command search → Show Mixer
  Mixer
    Horizontally scrollable instrument bank         Fixed Master
    ┌─────────────┐ ┌─────────────┐                ┌─────────────┐
    │    Gain     │ │    Gain     │                │             │
    │ Pan   Width │ │ Pan   Width │                │     Pan     │
    │ Fader ▏ L R │ │ Fader ▏ L R │                │ Fader ▏ L R │
    │   0.0 dB    │ │   0.0 dB    │                │   0.0 dB    │
    │  M      S   │ │  M      S   │                │             │
    │  → Master   │ │  → Master   │                │   Output    │
    │    Piano    │ │   Strings   │                │   Master    │
    └─────────────┘ └─────────────┘                └─────────────┘
```

| Region | Primitive and interaction |
| --- | --- |
| View entry and strip focus | Existing command registry for Show Mixer, previous and next channel, selected-channel Mute and Solo, and clip reset. Strip focus shares the arrangement's selected track. Master has focus without replacing the selected instrument. |
| Gain, Pan, Width, and fader | Gain and Pan knobs with values, the stereo fan [Width knob](../UI/components/width-knob.md) with percentage readout, and a vertical fader with dB readout. Gain sits above Pan and Width. Pan and Width use one row. Accessible names identify both channel and control. Focus supports arrow-key adjustment. Double-click or Option-click resets. |
| Meter and clip | Stereo level meter beside the fader using the shared scale. Show tick marks at +6, 0, −6, −12, −24, −36 and −48 dB, aligned to both controls. The clip indicator is an accessible reset target with a keyboard path. |
| Mute and solo | Shared M/S lamps, with independent labelled on/off states. No M/S controls on Master. |
| Channel footer | Direct-to-Master caption on instrument strips, Output caption on Master, name and selection edge. Long names truncate with full accessible text. |
| Sequencer track header | Reuse the track's stereo meter in a 9-point, full-height strip flush with the right edge of every track header. The header reserves space for the meter so the name and controls do not overlap it. Feed both views from the same post-fader reading and clip state. |
| Arranger Master | Keep a Master row beneath the vertically scrolling track bank and above the timeline navigation. Its header shows the Master stereo meter. The lane follows timeline zoom, horizontal position and playhead. |

Use the 92-point strip width and documented gaps. With one or few tracks the bank keeps the
same strip width. A long bank scrolls and keyboard navigation reveals the focused strip. At
760 points Master stays reachable and visible while the bank shows fewer strips. Test stopped,
playing, selected, muted, soloed, clipping, long names, and a long bank. When a track has no
active signal its meter is idle and its saved controls remain editable.

## Proposed Acceptance Criteria

- Deterministic audio checks cover independent trim and fader gain, stereo balance, mono,
  unity and expanded Width, the complete mute/solo matrix,
  click through Master, and peaks and clip reset. Verify final-only clamping, preserved latency
  alignment, and no callback allocation or blocking at the existing T04 entry points.
- Save and reopen a mix, open an older project, and verify defaults, exact value restoration,
  and undo and redo for discrete controls and complete gestures.
- Compare egui renders at 1024 and 760 points with the approved proposal. Verify scrolling,
  Master reachability, readable states, keyboard access, registry parity, aligned dB ticks,
  and an unobstructed, right-aligned sequencer meter on every track header.
- Verify that the arranger Master stays in place when scrolling a long track bank, and is visible
  in an empty project and with the Piano Roll dock open.
- Listen to two instrument tracks while adjusting trim, Width, faders, balance, mute and solo,
  then vary Master and reopen the project. Verify the mix and controls agree with the saved mix.

## Approval Gate

On 2026-11-19 the user approved the revised specification and layout with Gain and Width,
and added visible dB ticks and a compact meter at the right edge of every sequencer track header.
The user also requested a permanently visible Master row docked below the arranger tracks,
following the pinned bottom block in the arrangement reference.

## Implementation and Verification

- The session persists the instrument and Master settings and records each completed drag as one
  edit. Its transient previews apply to the device path and cancel on Escape. The project reader
  supplies neutral values when the mixer fields are absent.
- The engine publishes prepared coefficients through a bounded handoff, applies instrument
  controls after latency alignment, and ramps coefficient changes over approximately 5 ms.
  Master applies to the stereo sum and metronome. The two views use the same session meter reading.
- Automated tests cover exact project round trips, old metadata, preview and undo/redo, Width and
  balance math, mute and solo, metronome through Master, callback allocation and final clamping,
  peak hold and clip latch, pointer release, and fader/meter alignment. Accessibility bounds tests
  cover the docked Master and the right-aligned 9-point meters at both review widths.
- The egui gallery generated seven states at each width. Review the
  [playing mixer](../UI/snapshots/egui/t11-playing-1024.png),
  [long bank at 760 points](../UI/snapshots/egui/t11-long-bank-760.png), and
  [sequencer headers](../UI/snapshots/egui/t11-header-760.png),
  [pinned Master with a long track bank](../UI/snapshots/egui/t11-header-long-760.png), and
  [pinned Master with the Piano Roll dock](../UI/snapshots/egui/t11-header-dock-1024.png).
  The other states cover an empty mixer bank and stopped audio. The `t11_gallery` example
  regenerates the images. The egui test scrolls the track bank at both review widths and compares
  Master bounds before and after. It also checks an empty arranger and the open Piano Roll dock
  at 480 points of window height.
- On 2026-11-19, the locked test suite passed 164 tests with 12 ignored. Build, formatter, Clippy,
  and diff checks passed. Graphify was refreshed after the code changes.
- The isolated mixer commit passed 142 locked tests with 12 ignored. The gallery images were
  regenerated from the isolated mixer change on top of T34.
- The two-instrument listening pass with a saved and reopened project remains open.
