---
name: Track state button
kind: component
reference: composer::style::TrackStateButton
status: shared
impl: gallery
tokens: [panel, field, oxblood, accent, text]
used-in: [track-header, channel-strip, mixer]
snapshot: snapshots/egui/t18-track-state-1024.png
api: ui::primitives::track_state::record_arm
evidence: [t18-track-state]
---

> **Migrated to Rust/egui (T18 Phase 3, 2026-11-21).** The Clef section at the end of this file is
> the implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

# Track state button

Reference images: `Specs/Assets/D19/record-reference.png`, `monitor-reference.png`.

## Purpose
Record arm, MIDI monitor and (D49) inline note editing, identical on every surface.

## Anatomy
A rounded 3-pixel tile with a glyph: record is a target (outer ring and inner ring; a filled dot
when armed, a square while a take records), monitor a speaker, notes three staggered bars. Armed
record fills `oxblood`; monitor and notes fill `accent` with a dark glyph when on. Off: `panel`,
`field` on hover. In the inspector they sit at the right of the name row.

## Metrics
22 × 20 in headers and the channel strip name row.

## Approved change (UI10, 2026-11-11)
No filled tile: the glyph is the lamp, as in the transport. Armed turns the target `danger` with a
filled centre, a recording take squares it, monitor on turns the speaker `accent`. M and S stay the
only filled lamps. Implemented in UI10.3 (`TrackStateButton::glyphColour` gives the lamp colour for a state).

## Audio arm (D106)
An audio track's record button arms it like a MIDI track's (`danger` target with a filled centre, squared while its take
records), but independently: several audio tracks can be armed and record in the same pass.

## Badge (UI10 Fix 6)
`setBadge(text, colour)` adds a second state as an 8-px bold mark in the glyph's lower-right corner,
on a `panel` knock-out. D100: an audio track's monitor is its audio input monitoring (manual mark: the
wave lit while on, the X when off) and shows a `danger` **A!** while it monitors an unavailable input;
a MIDI track never monitors audio, so it carries no badge; `disabled` while the button is disabled.

## Monitor Auto / On / Off (D67, approved 2026-11-12)
`setMonitorMark(MonitorMark::automatic)` replaces the wave with a bold 9-px **A** after the speaker:
`accent` while the Auto track monitors (toggle on), `secondary` while it waits. With the manual mark,
On draws the wave and Off an **X** after the speaker (it was a slash). A recording take draws On.
Disabled draws `disabled`: the wave when on, else the X, never the A, so it cannot read as Auto idle.
The D24 badge stays in the lower-right corner. Hosts set lamp, mark and wording with
`showMonitorState` (`Source/MonitorButtonState.h`). Approved board:
[D67 Automatic Monitoring](https://claude.ai/artifact/QEHnZ665SSL5voPRhg1NHS); real render
`snapshots/monitor-states.png` (Auto live, Auto idle, On, Off, recording, disabled).

## Clef Rust/egui — T18 Phase 3

**API**

```rust
pub fn record_arm(ui: &mut egui::Ui, track: &str, enabled: bool, armed: bool, recording: bool, keyboard: bool) -> bool;
```

The record target as the lamp (UI10, no filled tile): outer ring and small inner ring in `secondary`
when off, `danger` with a filled centre when armed, a `danger` square while its take records,
`disabled` when disabled; hover draws a `field` hover frame, keyboard focus an `accent` one. The
accessible name says what a click does ("Arm Piano for MIDI recording", "Disarm Piano",
"Recording MIDI on Piano") and the armed state is exposed as selected, so the state never depends
on colour alone. Returns true when clicked.

**States:** off, armed, recording, disabled. **Adoption:** record arm in the arrangement track
headers (`transport.rs`). Monitor and notes buttons are not in Clef yet.

**Evidence:** gallery `snapshots/egui/t18-track-state-{1024,760}.png` and `t18-track-state-1024@2x.png`
(Retina); accessible names checked by `gallery::tests::every_gallery_control_has_an_accessible_name`.
