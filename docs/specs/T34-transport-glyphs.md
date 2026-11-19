# T34 — Transport Glyphs and Loop Colour

Status: Complete
Created: 2026-11-19
Completed: 2026-11-19
Depends on: T05, T06, T25

## Request

Begin replacing transport buttons with individual borderless glyph buttons that show state through colour. Record turns red when pressed. Loop and the loop band share the same yellow while looping is enabled.

## UI proposal

```text
transport row
  tool buttons: Return to Start | Play | Stop | Record
  record mode menu
  tool button: Loop
  wide only: Metronome tool button | 1 Bar | BPM and meter
  musical position | Transport Settings

timeline
  bars and beats ruler
  loop band: visible interval and endpoint grips
```

Use a 28-point click target and a 16-point vector glyph for each tool button. Return to Start uses a left-pointing triangle and start bar, Play a right-pointing triangle, Stop a square, Record a circle, Loop two repeat arrows, and Metronome a metronome outline. Each target has its own command, accessible name, tooltip with the current shortcut when assigned, and keyboard focus. The glyphs have no button frame or background tile. The existing keyboard-only focus cue remains separate from the frame.

| State | Glyph or band |
| --- | --- |
| Available | `TEXT` glyph, with hover and press tone feedback |
| Unavailable | `DISABLED` glyph. Record keeps its reason tooltip |
| Play or Metronome active | `ACCENT` glyph |
| Record pressed or recording | `DANGER` glyph during the accepted press and through preparation, count-in and capture. It returns to its idle or disabled colour when recording ends or the request is cancelled or rejected |
| Loop active | A shared yellow (`#E5C34F`, proposed `LOOP_ACTIVE` token) on the glyph, interval and endpoint grips |
| Loop inactive | Resting glyph and the existing dim band preview with accent grips |

The loop band uses the same active colour while the range is dragged, submitted, or confirmed. At compact widths Metronome stays in Transport Settings. The record-mode menu, count-in label, tempo and meter, position counter, and Transport Settings keep their current presentation. Transport commands, enablement, stop mode, snapping, range editing and range acknowledgement retain their current behavior.

## Acceptance criteria

- Each converted control has a separate target and dispatches its existing command through the command registry. Keyboard operation, accessible names, toggle states and shortcut tooltips are preserved.
- The converted buttons have no frame or tile at rest, on hover, or when pressed, active or disabled.
- Record is red on press and remains red for an accepted recording request through count-in and capture. Verify cancellation or rejection and Stop restore its state.
- The Loop glyph and visible band use the same yellow when enabled. Verify both toggle transitions and drag, release and range acknowledgement frames.
- Compare real egui renders at 1024 and 760 points with the approved layout and states. Check for overlap, clipping, and unrelated button flicker. Record automated and visual evidence separately.
- Update the transport bar, timeline rulers, tool button, icons, foundations and UI index alongside implementation.

## Approval

The user approved the six glyphs, the yellow and the Record lamp timing on 2026-11-19.
The user confirmed closure on 2026-11-19 after reviewing the transport screenshot.

## Verification

- Frame tests verified that Record stays red across pointer press and release and while
  recording is active, including count-in. A real session rejection clears the submitted
  state. The workspace test checked the Record and Stop accessibility actions and the transport
  layout at both widths. A separate regression checked unrelated button paint during
  acknowledgement.
- Loop-band frame tests checked the active and inactive colours across drag, release, pending
  acknowledgement, confirmation and restoration of the committed range.
- Workspace checks passed, including `cargo check --locked`, `cargo build --locked`,
  `cargo test --locked` (166 passed, 12 ignored), `cargo fmt --all --check`, and
  `cargo clippy --all-targets --locked -- -D warnings`.
- The isolated T34 change also passed formatting and workspace tests (131 passed, 12 ignored).
  The gallery images below were generated from that isolated change.
- The native eframe gallery rendered the real workspace components. All eight images were
  inspected for transport alignment and clipping, matching active Loop colours, and the red
  Record glyph during count-in and capture. The gallery uses fixture states. Live MIDI
  recording and listening were outside this UI verification.

| Gallery state | 1024 points | 760 points |
| --- | --- | --- |
| Stopped | [1024](../UI/snapshots/egui/t34-stopped-1024.png) | [760](../UI/snapshots/egui/t34-stopped-760.png) |
| Looping enabled | [1024](../UI/snapshots/egui/t34-looping-1024.png) | [760](../UI/snapshots/egui/t34-looping-760.png) |
| Count-in | [1024](../UI/snapshots/egui/t34-count-in-1024.png) | [760](../UI/snapshots/egui/t34-count-in-760.png) |
| Recording | [1024](../UI/snapshots/egui/t34-recording-1024.png) | [760](../UI/snapshots/egui/t34-recording-760.png) |
