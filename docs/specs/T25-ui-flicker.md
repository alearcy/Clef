# T25 — UI Flicker and Plugin Editor Presentation

Status: Done. Scope approved and final behavior accepted by the user on 2026-11-18.

## Report

- A previous loop range flashes when the pointer is released after dragging a new range.
- The selected-track details sidebar and transport bar exhibit brief visual artifacts on pointer clicks and releases while playback is stopped, including button clicks.
- Opening the Pocket Strings VST3 editor briefly shows a black window before the original editor appears.
- The supplied `Playhead.clef` project has 15 instrument tracks and a saved loop range with looping off. It contains Glass Strings VST3 on Instrument 1 and Zebra2 CLAP on Instrument 2. Pocket Strings VST3 is installed separately and can be exercised on an unused track in a test copy.

## Agreed behavior

- Present a valid loop preview continuously from drag through release and session confirmation. Escape, invalid edits, and rejected session edits restore the last committed range. Keep the existing snapping and editing rules.
- Keep the selected-track details and transport bar visually stable through a loop edit and ordinary pointer clicks, including button press and release with playback stopped. Preserve their layout, controls, and status feedback.
- Present the original VST3 and CLAP editors without an intervening blank window. Keep the eframe window, worker isolation, and the existing editor controls and lifecycle. A failed open leaves the editor hidden and reports the failure through the existing path.

## Initial findings

- `Viewport::loop_band` takes the gesture on release before choosing the range to paint. The paint falls back to the committed timeline. `App::timeline_surface` submits the new range after that paint, and the UI receives the authoritative session view asynchronously.
- `App::main_surface` renders the transport before the timeline, then renders selected-track details. A loop edit can change `busy` inside the timeline render, exposing different command states in the same pass. The upper audio row can also add and remove its working indicator. These candidates cover edits sent to the session. Reproduce with a click that sends no session action and with a button that does to determine whether a shared presentation issue is involved.
- The worker requests visibility after attachment, but attachment alone provides no evidence that a plugin's view has presented content. The pinned eframe integration can also show its root viewport during its first render independently of Clef's visibility request. Establish when the black frame occurs to identify the responsible path.

## Acceptance

- Draw, move, and resize a loop range. Observe the release frame and the update to the committed range. Verify continuity with audio disabled and enabled while stopped, and verify Escape and a rejected edit restore the committed range.
- Use a copy of `Playhead.clef` to observe selected-track details and transport at rest during pointer press and release, both on a button and where a click sends no session action. Repeat during a loop edit. Record the triggering action and whether any flash or shift remains.
- On macOS, open, close, and reopen Pocket Strings VST3. Compare the first open after worker startup with subsequent opens. Use Glass Strings VST3 and Zebra2 CLAP from the test project as controls for the shared lifecycle. Verify that a failed editor open surfaces the error without revealing an empty editor window.
- Record automated state or lifecycle coverage and visual observations in this specification.

## Implementation and verification

- The loop band paints the new range on release and uses the submitted range until the session acknowledges it. The session remains authoritative and a failed submission displays the previously committed range.
- The main workspace paints before dispatching actions triggered in its controls. Pending operations shorter than 100 ms keep the current button and track-details appearance. Longer operations show the existing working feedback. Commands continue to obey the session busy gate. Unchanged session snapshots and repeated native title commands no longer request idle redraws.
- The worker applies the viewport geometry while hidden. A later frame lays out the embedded view and flushes its layer transaction before requesting visibility and focus. Close, hide, and error cleanup cancel a queued reveal.
- Automated coverage checks the actual egui locator shapes at release and during session acknowledgment, and verifies that the backend suppresses unchanged idle updates while acknowledging no-op commands.
- macOS smoke checks used a temporary copy of Playhead with Pocket Strings added to Instrument 3. The host returned successful editor open/close states for Pocket Strings VST3 and open states for Glass Strings VST3 and Zebra2 CLAP. Clicks and a transport toggle returned stable final frames. The desktop tool samples settled frames and did not commit its synthetic drag, so it could not establish the absence of brief sidebar/transport or plugin flashes, or verify the physical drag. These remain visual acceptance items.

## Follow-up after user testing

- The user confirmed the loop range works without artifacts. Pressing a button still briefly changes the appearance of other buttons.
- The previous delay covered session actions shorter than 100 ms. Longer actions still dimmed otherwise available buttons, and the audio transport acknowledgment could dim its controls immediately. The workspace now evaluates button appearance against project state and command eligibility. Transient session and audio transport waits gate dispatch while preserving unrelated button appearance. The delayed working indicator still signals longer session actions.
- An egui frame regression compares the actual paint of unrelated workspace and transport buttons before and during each wait, and checks that the corresponding commands are still gated.
- The user confirmed that only Stop still changed brightness. Stop now follows the playback phase and an unacknowledged Play request. Pending settings and locate commands keep it dimmed while playback is stopped. The engine regression covers settings acknowledgment, cancellation before Play is acknowledged, and confirmed playback. The frame regression includes Stop.

## Final acceptance

- The final build passed 63 automated tests with nine native cases ignored. Formatting, build, and Clippy passed.
- The user explicitly confirmed all three visual checks on 2026-11-18: the loop range stays stable on release, unrelated buttons keep their brightness including Stop, and Pocket Strings VST3 opens and reopens without a black flash. The earlier desktop smoke checks established editor lifecycle and settled UI state.
- The [UI contract](../UI/contract.md#interaction-frame-regressions) carries forward the checks for future changes.
