# T05 — Transport, Tempo Map, and Timeline

**Status** — Done · 2026-11-18

**Prerequisite** — [T04](T04-multitrack-session.md) complete

## Behavior

- Use the audio device sample clock and a project timeline at 960 ticks per quarter note. Start at 120 BPM and 4/4. Store a tempo and meter map with step changes on bar boundaries. While stopped, add, edit, and remove changes from a list addressed by bar number. Keep the initial entry editable and required.
- Play starts at the playhead. The Stop Mode setting chooses **Keep Position** (default) or **Return to Play Start**. The latter restores the start of the most recent Play after any initial loop normalization, including after a seek or loop wrap. Stop leaves audio enabled for live instruments. Return to Start moves to bar 1. Click or drag the ruler to locate, including during playback. With audio disabled, locating and editing work and Play is disabled. A device loss stops transport. A reopened project starts stopped at bar 1 with audio disabled.
- The loop uses a half-open range. Start with bars 1–5 and Loop off. The ruler has a loop band for drawing, moving, and resizing the range. Snap to beats by default and to ticks with Option. Escape cancels an edit. If Play starts outside an enabled loop, start at its left locator. Lock map and loop-range edits during playback.
- Metronome and one-bar count-in default off. Count-in is audible independently of the metronome and holds the playhead until playback begins. The built-in click accents the downbeat and plays through the selected stereo output pair. Stop and locate cancel a running count-in.
- Save the tempo/meter map, loop range and toggle, click, count-in and Stop Mode settings. Open existing projects with the defaults. Show bars, beats, ticks and the playhead over empty lanes aligned with the existing track headers. Provide horizontal scroll and zoom, with the ruler and lanes on one viewport transform.
- Map project positions to exact sample offsets within blocks, including tempo changes and loop wraps. Reuse the current clock for live MIDI timestamps and expose the same map and block spans to future clip playback and offline rendering. Keep live monitoring separate from transport gating. Stop and locate invalidate queued timeline events and release transport-owned notes and sustain. Derive the visual playhead from the output clock and available latency instead of a separate UI timer.

## Provisional egui proposal

```text
Return to Start | Play | Stop | Loop | Metronome | 1 Bar | BPM / Meter | Bars.Beats.Ticks | Transport Settings
Track headers                  | Bars and beats ruler
                               | Loop locator band
Existing instrument rows       | Aligned lanes, grid, and playhead
                               | Horizontal scroll and zoom
```

Use the [transport bar](../UI/views/transport-bar.md), [timeline ruler](../UI/views/timeline-rulers.md), and [arrangement](../UI/views/arrangement.md) as references. Compose tool buttons, toggle chips, tempo/meter controls, position display, and scroll/zoom controls. Use the command registry for keyboard and pointer actions. Indicate Play, Stop, count-in, audio unavailable, loop and click states with accessible names and theme states. At narrow widths keep the transport on one row and move secondary settings into an accessible menu.

Out of scope are clips, recording, markers, punch, automation, tempo ramps and plugin transport sync.

## Acceptance

- Verify deterministic sample positions at 44.1 and 48 kHz across tempo/meter changes, count-in, loop wraps within a callback, both stop modes and locate. Check project round-trip and legacy defaults.
- Verify callback work stays bounded and allocation-free, stale transport events cannot leak, and live MIDI works with transport stopped. Exercise the click and count-in on the configured output pair.
- Inspect egui at 1024 and 760 points for layout, available controls, keyboard access, locator editing, playhead, and zoom/scroll alignment. Record evidence here.

## Verification

- Locked build, format, Clippy and tests passed. There were 59 passing tests and nine ignored native cases. Focused regressions cover tempo and meter offsets at 44.1 and 48 kHz, both Stop modes, latency projection across a loop, count-in cancellation, click output to the selected stereo pair, allocation-free callbacks, and project round-trip with defaults for older v2 files.
- The user accepted all six checks in the macOS app on 2026-11-18. The checks covered timeline layout and navigation, both Stop modes, loop locators and playback, tempo and meter changes, audible count-in and live MIDI with transport stopped, and project save and reopen.
