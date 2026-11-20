---
name: Clef Single-Instrument Project
kind: view
status: app
impl: app
source: [crates/ui/src/lib.rs, crates/session/src/lib.rs, crates/plugin-host/src/catalog.rs, crates/model/src/project.rs]
components: [text-button, menu-button, text-field, list-row, notice-box]
snapshot: pending
---

> **Implementation status (2026-11-17):** historical T03 surface, inline in `crates/ui/src/lib.rs` with
> stock egui widgets; superseded by [multitrack session](multitrack-session.md). Kept as the
> record of the first Rust/egui surface. See the [UI contract](../contract.md).

# Clef Single-Instrument Project

## Purpose

Historical T03 surface. T04 supersedes its single-track, eager-load and global-worker-failure
behavior; see the current [multitrack session](multitrack-session.md).

The T03 workspace supports one live instrument track. Its written proposal was
approved on 2026-11-17 in [T03](../../specs/T03-single-instrument-project.md).
It translates Composer Dark into Rust/egui; it does not inherit the legacy
arrangement, transport, mixer or startup chooser.

## Anatomy and Metrics

The eframe window opens at 1024 × 640 with a 760 × 480 minimum. Content has
12-point workspace padding; controls are at least 24 points high. The project
command row wraps at narrow sizes. Project identity and unsaved status precede
the audio status label (Audio active / Audio off); there is no Enable or Disable Audio
control since T39. The single panel contains the track name, instrument,
editor actions and routing summary. Status and actual callback configuration
follow the panel. Spacing and background roles separate regions; no region rules.

Audio/MIDI Settings, the searchable instrument picker, Commands/Keyboard
Shortcuts, file chooser and destructive-transition confirmations are egui
surfaces. Native plugin editors remain unchanged inside eframe-owned windows.
All Clef window/control ownership stays in eframe; the native application-quit
hook creates no windows or controls and retains winit's existing delegate.

## States and Behavior

- Empty: Add Instrument Track. A second track is unavailable.
- Ready: display plugin name/format and saved MIDI/output selections; audio runs.
- Active: Disable Audio and actual sample rate/callback range/transport lead.
- Busy: disable session actions while an asynchronous load/save/reconfiguration runs.
- Unavailable: retain plugin identity, opaque state and device choices; explain recovery.
- Worker failure: silence and stop the stream, then require explicit restart from saved state.
- Unsaved: include staged track names, editor exposure and live playing conservatively.
  Save stops audio and closes the editor before capturing state. Reopening the editor
  or enabling audio marks the project potentially modified again.

New, Open and Close protect changes with Save/Discard/Cancel. Cancelling or failing
Save stops the transition. Save As confirms replacement; ordinary Save rejects a
file changed outside the current session. Files contain versioned metadata and
opaque plugin state in one `.clef` container; plugin binaries/samples stay external.
The egui file chooser supports directory navigation and an absolute path field.

Settings stage changes until Apply. Supported rates, output pairs and buffers come
from the chosen device. Refresh updates devices without substituting missing
selections. Applying stops audio. Actual callback sizes are reported separately
from the requested buffer. Missing/disconnected devices never silently reroute.

Scanning is isolated, bounded and cancellable at application shutdown. The picker
shows individual instruments and their formats, including multiple instruments in
one bundle. Failures are readable in a bounded scrolling disclosure; Rescan retries.

## Commands and Accessibility

`crates/ui/src/lib.rs` owns shared command definitions, availability and dispatch. Buttons,
command search and shortcuts use them; parameterized selections enter the session
through its shared actions. Command bindings can be reassigned for the current
application session. Menus/tooltips show their live binding. Defaults: Command+N,
Command+O, Command+S, Command+Q and Command+K.

AccessKit and default fonts are enabled in eframe. Controls have text/accessibility
labels, keyboard focus and non-color status text. Native Quit requests are routed
to the same egui unsaved-change decision as closing the main window.

## Verification

Actual GUI screenshots and accessibility trees were inspected during T03. Separate
user listening/physical-MIDI confirmations and automated/native evidence are recorded
in the [T03 verification](../../specs/T03-single-instrument-project.md#completion--2026-11-17). No committed snapshot
is available yet; do not treat the legacy images as screenshots of this surface.
