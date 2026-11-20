# T39 — Always-On Audio and Global Audio Preferences

- **Status** — Done — 2026-11-20. Specification and UI proposal approved by the user on
  2026-11-20; open questions answered the same day. Implementation, automated and native evidence,
  and the user's GUI and hardware checks complete.
- **Priority** — P1 (user request).
- **Task** — [T39](../../TASKS.md)
- **Dependencies** — T03 (device selection, explicit enable/disable contract, superseded here),
  T04 (session lifecycle), [T38](T38-first-play-readiness.md) (background preload at engine
  start). Takes over the default-device, fallback, and persisted-preference parts of
  [T17](T17-device-resilience.md); T17 keeps hotplug, reconnection, and the remaining scope.

## Goal

Remove Enable Audio. The audio engine runs whenever Clef runs, using saved audio settings, so
after a restart the user can play at once.

## Decisions (interview 2026-11-20)

- Audio settings (output device, channel pair, sample rate, buffer, MIDI input) come from a
  **global** preference, with an optional **per-project override** chosen explicitly in Settings.
- Audio blocks already saved in existing projects are **ignored**: they do not become overrides.
- When the configured output device is unavailable at start, a message always tells the user;
  the engine then falls back: a missing project-override device tries the global device first,
  then the macOS default output; if there is none, audio stays off.
- There is no way to switch the engine off: Enable Audio and Disable Audio are removed.

## Scope and Acceptance Criteria

### A1 — Global preferences and project override

- Global audio preferences are stored in Clef's Application Support directory, next to the
  existing recents and quarantine data, written atomically. An unreadable or invalid file is
  reported in the message and treated as absent; it is not overwritten until the user applies
  settings.
- A project stores an audio override only when the user chose "This project" in Settings. A
  project without an override uses the global preferences.
- Existing project audio blocks are read and discarded; saving such a project writes no override.
  The project format version is raised and older versions keep decoding.
- With neither preferences nor an override (first launch), the engine uses the macOS default
  output, its first two channels, and the device's current sample rate with a 128-frame buffer,
  and no MIDI input. Nothing is written until the user applies settings.

### A2 — Automatic engine start

- The engine starts when Clef launches and whenever a project is opened or created, using the
  effective settings (override, else global, else first-launch defaults). Opening a project with a
  different effective configuration restarts the engine with it.
- T38's background preload runs on every engine start.
- Applying Settings restarts the engine with the new configuration automatically; the caption
  "Changes stop audio. Enable Audio explicitly after applying." is replaced.
- An engine start failure (device error, unsupported configuration) reports the reason and leaves
  audio off until the next apply, project open, or launch.
- Behaviors that today require enabled audio (playback, recording, live MIDI, editor opening)
  keep their rules; messages that say "Enable Audio…" are reworded to point to Audio/MIDI Settings.
- A device error during a session (T03 "Settings retained; Enable Audio explicitly") reports the
  error; the engine restarts on the next apply, project open, or launch. Automatic reconnection
  remains T17 scope.

### A3 — Missing device fallback

- If the effective output device is missing, the message names it and the device in use. A
  project override falls back to the global device when that is present, else to the macOS
  default output; global preferences fall back to the macOS default output. A fallback device
  keeps the requested rate and buffer when supported, else uses its current rate and a 128-frame
  buffer.
- The fallback never rewrites the global preferences or the project override.
- If there is no default output either, audio stays off and the message says so.
- A missing MIDI input is reported and the engine starts without MIDI input.

### A4 — Remove Enable and Disable

- Enable Audio and Disable Audio commands, their buttons, shortcuts, command-palette entries,
  and session actions are removed. Tests and UI docs that rely on them are updated.

### A5 — UI proposal (needs approval)

- Audio/MIDI Settings gains, at the top, a two-way [segmented control](../UI/components/segmented-control.md)
  labelled **Applies to** — **All projects · This project** — with an 11-px `secondary` caption:
  "All projects saves Clef's audio preferences. This project saves an override in the project
  file." It opens on This project when the current project has an override, else All projects.
- Choosing All projects for a project that has an override and applying removes the override.
- The header caption becomes "Applying restarts audio."
- The toolbar loses the Enable/Disable Audio button; no replacement control. The actual active
  configuration stays visible where it is today, including a fallback device name.
- Settings and toolbar UI docs, their index entries, and snapshots are updated with the change.

### Out of scope

- Hotplug detection, automatic reconnection, and device-change handling during a session (T17).
- Aggregate or multiple devices, per-track device routing.

## Verification Plan

- Automated: preferences round trip, atomic write, invalid file handling.
- Automated: effective-settings resolution (override, global, first-launch defaults); legacy
  project audio blocks are ignored and not re-saved; format version decoding.
- Automated: missing output → fallback to default with message and no preference rewrite; no
  default → audio off with message; missing MIDI input → engine without MIDI and a message.
- Automated: Open/New/Apply restart the engine; Enable/Disable no longer exist.
- GUI (user): Settings scope control, captions, toolbar without Enable, fallback visibility.
- Hardware (user): restart Clef with the Audient connected → audio active at once; disconnected →
  message and macOS default output.

## Implementation (2026-11-20)

- Model: `Project.audio` is `Option<AudioSettings>`, the explicit override; the project format is
  version 5 (`audio_override` key). Version 1–4 audio blocks are read and discarded.
- `clef-audio-io`: `Devices` reports the macOS default output and each device's current rate;
  `validate` no longer requires a MIDI input. The engine opens MIDI only when an input is set.
- `clef-session/src/audio.rs`: `Preferences` (`~/Library/Application Support/Clef/audio.json`,
  atomic write, invalid file reported and kept) and `resolve` (override → global device → macOS
  default; first-launch defaults; MIDI optional).
- Session: `start_audio` replaces Enable; `Action::Settings { audio, project }` writes the
  preferences or the override, stops and restarts audio. `Action::Enable`/`Disable` are removed.
  Workers follow the engine rate (`worker_rate`): a different rate captures their state and
  reloads them.
- UI: Enable/Disable commands and the toolbar button are removed; the toolbar shows Audio active /
  Audio off. Audio/MIDI Settings has the Applies to choice and the agreed captions.

Clarifications made during implementation:

- "Clef launches" means the session starts audio as soon as a project is open; the startup
  chooser has no project. Every Open/New/Recover restarts the engine, because leaving a project
  already releases all workers.
- The Applies to choice uses two egui selectable labels, the pattern the existing record-mode
  controls use, because the segmented-control primitive is still `impl: planned`.
- A MIDI input lost during a session is reported once and audio keeps running (MIDI optional).

## Verification Evidence

- Automated (2026-11-20): `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D
  warnings`, `cargo test --locked` — 223 passed, 26 ignored. New tests: model
  `audio_is_an_explicit_override_and_legacy_audio_blocks_are_ignored`; session `audio::tests`
  (resolution order, missing override → global → default, adapted fallback values, no output,
  missing MIDI, preferences round trip and invalid file kept).
- Native (2026-11-20, ignored `native_audio_starts_by_itself_and_settings_scope_global_or_project`,
  macOS default output): audio starts by itself on New with first-launch defaults and writes
  nothing; a global apply writes the preferences and restarts audio; a project apply stores an
  override, keeps the global preferences and marks the project dirty; a later global apply removes
  the override. T38's `native_t12_first_play_starts_on_an_aligned_graph` still reports 0
  transition frames on both passes.
- Not rerun: the older T03/T04 native suites (commercial plugins, virtual MIDI); they now start
  audio with `start_audio` and read the saved T03 output through a test helper.
- GUI (user, 2026-11-20): Settings Applies to choice, captions, and toolbar without Enable
  confirmed correct. The legacy `docs/UI/snapshots/settings.png` is not regenerated.
- Hardware (user, 2026-11-20): audio active on project open; global apply on the Audient EVO16
  persists across a Clef restart; fallback behavior confirmed correct.

## Open Questions

None. Answered 2026-11-20: a missing override device tries the global device before the macOS
default; first-launch defaults are confirmed (128-frame buffer, the device's current rate).
