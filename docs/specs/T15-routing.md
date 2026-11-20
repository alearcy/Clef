# T15 — Buses, Sends, Groups, and Routing

**Status:** Done — completed on 2026-11-21. Specification and written UI proposal approved on 2026-11-20; Stop and Tails added and approved on 2026-11-21. See [Completion](#completion-2026-11-21).
**Priority:** P1.
**Date:** 2026-11-20.
**Task:** [T15](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T11 mixer; T12 effects.
**UI proposal:** [T15 routing proposal](../UI/proposals/T15-routing.html) (UI Canvas design
`d_mwovwiw007m6`, revision 5). Provisional Clef UI; the [T18](../specs/T18-ui-foundation.md)
design-system acceptance is still outstanding.

## Decisions

The interview on 2026-11-20 agreed:

1. **One bus type.** A bus is an audio-only summing channel used as a group or as an FX
   return. There is no separate FX-return type; the role is the name and the way the user
   feeds it.
2. **One routing level.** Instrument tracks output to Master or to one bus. Every bus outputs
   to Master. Buses are not nested and cannot output to another bus.
3. **No fixed send count.** A channel has no fixed send slots. Each instrument track may have
   at most one send to each bus. A project-wide bound keeps the device callback bounded (see
   [Realtime bounds](#realtime-bounds)).
4. **Solo in place with upstream propagation.** Soloing a bus makes the tracks that output to
   it audible. A send from a soloed track keeps its destination bus audible, so FX returns
   are not cut by solo. Mute wins over solo.
5. **Routing in the mixer.** Each strip has a Sends section and an Output menu. The track
   context menu exposes the same commands. The arrangement does not show bus rows; buses
   live in the mixer bank.
6. **Compact send row.** One line per send: pre/post tap glyph, destination slot, level
   knob, bypass glyph. The dB value appears only in a popup anchored to the knob, never as a
   permanent second line. Three send rows are visible before the section scrolls.
7. **Value-only dB popup.** On hover and while the knob is dragged, the popup shows only the
   current dB value and updates live with the gesture. It is rendered in the tooltip layer,
   so it stays visible above the send list's scroll area and cannot be clipped.

## Goal

Turn the direct-to-master mix into a small routing graph that supports groups, FX returns and
sends, with defined solo/mute behaviour, delay compensation across the graph, and rejection
of invalid routing without losing the last valid project.

## Scope

- Bus track kind: stereo summing channel with name, inserts, Gain, Pan, Width, fader, mute,
  solo and meter. No instrument, clips, recording, arm or take lanes.
- Per-channel output assignment: Master or one bus.
- Sends from instrument tracks to buses: destination, level, pre/post-fader, bypass. No fixed
  slot count; one send per destination bus per track.
- Bus creation from the mixer and from a track's Output menu, including **New Bus…** and
  **Route to New Bus** (one undo step).
- Sends section and Output menu in the mixer; commands also in the track context menu.
- Solo-in-place across tracks, buses and returns; mute precedence.
- PDC across track inserts, bus inserts and master inserts; graph changes publish at block
  boundaries off the device callback.
- Persistence in project schema v4 with migration from v3, v2 and v1.
- Undo/redo for output assignment, sends, bus creation and bus removal.
- Bus metering after the bus fader.
- Failure reporting for invalid output/send targets, duplicate sends, a bus with sources,
  and a deleted or missing bus.

## Out of Scope

- Nested buses, bus-to-bus outputs, folder tracks, sidechain routing, a full routing matrix.
- Sends from buses (only instrument tracks send).
- Automation of send level, output or routing.
- Multiple outputs per channel, mono buses, surround.
- Offline render/export (T13).

## Signal Path

```text
instrument track
  instrument worker -> Gain (trim) -> inserts -> pre-fader tap ---- send gain ----+
                                               |                                  |
                                               v                                  |
                                        Width -> Pan -> fader -> mute/solo gate    |
                                               |                                  |
                                               +-------- post-fader tap --- send --+
                                               |                                  |
                                               v                                  v
                                         direct to Master                    bus sum
                                                                                  |
                                                                       Gain -> inserts
                                                                                  |
                                                                       Width -> Pan
                                                                          -> fader
                                                                       -> mute/solo gate
                                                                                  |
                                                                                  v
                                                                             Master sum
                                                                                  |
                                                                       Master inserts
                                                                                  |
                                                                       Pan -> fader
                                                                                  v
                                                                          meter / output
```

- **Pre-fader tap** is after Gain and inserts, before Width/Pan/fader. It is a stereo tap
  with no pan applied.
- **Post-fader tap** is after Width, Pan and fader.
- Both taps are gated by the track's mute/solo gate. A muted track sends nothing.
- Send gain is applied to the tap before it reaches the bus sum.
- A bus receives direct outputs and sends at unity gain; its own Gain, inserts, Width, Pan,
  fader and mute/solo gate follow, then it goes to Master.
- The metronome/click bypasses tracks and buses, is summed into the master path before the
  master inserts, and follows the existing master gain.

## Model

Project schema version becomes **4**.

```rust
pub enum TrackKind {
    Instrument,
    Bus,
}

pub struct Send {
    pub id: u64,
    pub target: u64,   // bus track id
    pub level: i16,    // fader units: -601 (silence) .. +60 (0.1 dB steps)
    pub pre: bool,     // true = pre-fader, false = post-fader
    pub enabled: bool,
}

pub struct Track {
    // existing fields
    pub kind: TrackKind,          // serde default Instrument
    pub output: Option<u64>,      // None = Master, Some(id) = bus id
    pub sends: Vec<Send>,         // instrument tracks only
}
```

Rules validated by `Project::validate`:

- Every track id is unique and below `next_id`; buses count toward `MAX_TRACKS`.
- A bus has `plugin = None`, empty `state`, no clips in the MIDI arrangement, no take
  lanes, and no `instrument_bypassed`/`instrument_offline`.
- `output` is `None` or an existing bus. An instrument track may output to a bus; a bus must
  output to Master (`None`). No self-routing or cycles are representable.
- Sends exist only on instrument tracks. `target` is an existing bus, never the source's own
  track, and duplicate targets on one track are rejected.
- `level` is within `Control::Fader` bounds; `id` is unique across tracks, inserts and sends.
- Project-wide send edges are at most `MAX_SEND_EDGES` (512). This is not a UI slot count; it
  keeps the device callback's per-sample work bounded. The edit reports a clear error when
  the budget would be exceeded.
- A bus with at least one direct source or send cannot be removed. The edit is refused with
  the number of sources that must be reassigned first.

Migration:

- v1 and v2 decode as today, then every track is `Instrument`, `output = None`, `sends = []`.
- v3 decodes with the same defaults.
- Saving writes v4. Reading a v4 file without the new fields uses the same defaults, so a
  hand-edited or older tool's file still opens.

## Solo and Mute

Let `solo_active` mean at least one unmuted channel has `solo` set.

- **No solo:** every unmuted track and bus is audible.
- **Mute:** a muted channel never contributes and never propagates solo. A channel with both
  mute and solo is silent and does not count as soloed.
- **Upstream set:** with solo active, an instrument track is in the set when it is soloed or
  when its output bus is soloed. A bus is in the set when it is soloed or when an in-set
  track outputs to it.
- **Bus audibility:** a bus is audible when it is in the set, or when an audible send targets
  it, or when a soloed bus is its destination.
- **Direct output:** an instrument track's direct output is audible only when the track is in
  the set.
- **Send audibility:** a send is audible when its source track is in the set, or when its
  destination bus is soloed. A send from a non-soloed track into a soloed FX return is
  therefore heard without making the source's dry output audible.
- **FX returns:** a bus made audible only by sends passes only the audible sends; its other
  direct inputs stay gated at their source.
- **Master:** always audible; its fader, pan and inserts still apply.

The matrix gates are computed in `PreparedMix` off the device callback. The device callback
only applies the prepared matrices and send gains.

## Realtime Bounds

- `MAX_SEND_EDGES = 512` project-wide; one send per destination bus per track.
- Bus count is bounded by `MAX_TRACKS` (128). The callback's bus scratch buffers are
  preallocated; it never allocates, locks, formats text, logs or waits.
- Insert chains and bus topology are prepared and published off the callback. Graph changes
  are published at a block boundary. A change during playback may produce a bounded
  transition, counted by the existing transition counters; it must not stall or crash the
  callback.
- Bus insert workers remain isolated plugin workers. The callback does not call plugins.
- The routing stage keeps a dry fallback so a late bus/master worker cannot silence healthy
  paths. A missed block is metered as a transition, not as audio corruption.

## PDC

- Track workers keep their existing alignment to the maximum track latency.
- A bus's path latency is its insert-chain latency. Buses with shorter chains are delayed to
  the maximum bus-chain latency.
- Master inserts add their chain latency after the bus sum.
- Total reported latency is
  `max_track_latency + lead + max_bus_chain_latency + master_chain_latency`, where `lead` is
  the existing worker scheduling lead. The transport, recording compensation, metronome and
  meters use this value.
- When the graph or a chain latency changes, the engine republishes the routing stage and
  total latency at a block boundary; the existing transition and stale-output protections
  apply.
- A direct-to-Master path with no inserts keeps its current zero-routing-latency behaviour.

## Stop and Tails (added 2026-11-21)

Approved by the user on 2026-11-21, implemented and confirmed by listening the same day.

Raised in the listening pass: Stop cut the reverb tail of a bus. Decisions taken with the user
on 2026-11-21:

- **Stop lets tails ring.** Instrument release tails and every effect tail (track inserts, bus
  inserts, master inserts) keep sounding after Stop until they decay. Stop releases held clip
  notes (the scheduler's existing clip clear on a generation change) and stops scheduling new
  clip events; it no longer fades or closes the output.
- **A second Stop is a panic.** Stop while already stopped fades the output to silence over
  5 ms, sends the panic MIDI events (all notes off, sustain off) to every instrument, and
  keeps the output gate closed until Play, Record or live audition.
- **Play over a tail.** Play while a tail is ringing does not cut it; the tail decays under the
  new playback.
- Tracked inside T15.

Cause and design. Stop did two things that cut tails: the transport faded and closed the output
gate, and the engine advanced the packet-validity generation, so track and routing packets
already rendered ahead (up to the scheduling lead) were discarded. The discard left a gap of
about one lead, after which the decaying tail returned; the T12/T35 "Stop crackle" was that
dip and return, and the closed gate hid it. The engine already keeps the validity generation
unchanged on Play from stopped. The change:

1. The callback advances the packet-validity generation only for a discontinuity while
   playing (Locate or a loop/range change during playback), not on the transition to
   stopped. Track, bus and master streams then stay contiguous through Stop and through a
   following Play.
2. A Stop request while playing no longer fades or closes the gate. Record end and stop frame
   are still captured at the request.
3. Stop while stopped (panic) keeps the existing 5 ms fade and closed gate and adds the panic
   MIDI to every track worker.

Accepted consequences and limits:

- Audio already rendered ahead of the Stop (at most the scheduling lead, about 21 ms with a
  256-frame buffer) still plays, including clip content in that window; then notes release.
- The panic does not reset plugin internal state. If Play follows the panic while a long effect
  tail is still decaying inside the plugin, its remainder can be heard when the gate reopens.
- Locate during playback keeps its existing bounded transition.

Acceptance:

- Automated: Stop while playing keeps the transport gain at 1 and leaves the validity
  generation unchanged; packets rendered before the Stop are consumed without a gap (a
  continuous track tone through Stop produces no zero run); bus/master routing packets stay
  matched through Stop; Stop while stopped fades to 0, queues the panic and keeps the gate
  closed until Play or audition; Locate while playing still discards stale packets; no
  callback allocation.
- Listening (user, `T12-listening-v2`): the bus reverb tail and an instrument release ring
  out after Stop with no click, dip or return; a second Stop silences everything; Play during
  a tail keeps the tail under the new playback.

Implementation: `Renderer::request_stop` stops at once without a fade; while stopped it counts
a panic and fades to 0 (`Renderer::panics`). `Stop` no longer closes the gate; Play and
audition reopen it after a panic. The callback publishes the panic count on
`Clock::panics`, and each track worker calls `Scheduler::panic`, which replaces the next block
with the panic MIDI set and forgets held voices. The callback advances the packet-validity
generation only when both the previous and the new projection are playing.

Automated evidence (2026-11-21): `stop_keeps_the_output_open_and_a_second_stop_is_a_panic`
(replaces `stop_fades_out_and_keeps_the_requested_stop_frame`),
`only_a_discontinuity_while_playing_discards_rendered_audio`,
`recording_clock_and_stop_keep_rendered_tail_packets_without_allocating` (replaces
`recording_clock_and_stop_flush_old_packets_without_allocating`; the pre-Stop block now plays,
no callback allocation), `panic_replaces_the_block_with_the_panic_set_and_forgets_held_voices`.
Full suite: 227 passed, 26 ignored; clippy and fmt clean.

## Persistence and Undo

- Output assignment, send add/remove/level/pre-post/bypass/destination, bus creation and bus
  removal are project edits.
- A continuous send-level gesture is one undo entry; Escape restores the gesture's starting
  value.
- **Route to New Bus** creates the bus and assigns the source in one undo step.
- Undo/redo restores the previous routing without asking for confirmation; redo of a removed
  bus restores the bus and its sends.
- A refused edit leaves both the project and the last valid routing untouched.

## UI Proposal

The written proposal is [T15-routing.html](../UI/proposals/T15-routing.html); the UI Canvas
design is `d_mwovwiw007m6`, revision 5. It shows three states: normal mixer, Output menu, and
solo on Drum Bus.

### Mixer layout

```text
Mixer
  toolbar: Mixer title, + Bus
  bank: instrument strips left, bus strips docked right before Master
  status: sample rate, buffer, PDC, solo, selection

instrument strip
  Gain
  Inserts (Plugin row, effects, Add effect…)
  Sends
    [tap pre/post] [destination slot] [level knob] [bypass]
    …
    Add send…
  Pan / Width
  Fader + meter
  Mute / Solo
  Output: Master ▾ | Bus name ▾
  Name

bus strip
  Gain
  Inserts (effects, Add effect…)
  Inputs (read-only source count and names)
  Pan / Width
  Fader + meter
  Mute / Solo
  Output: Master
  Name with BUS role tag

master strip
  Inserts
  Pan
  Fader + meter
  MONO
  Output: Device
  Name
```

- The Sends/Inputs section has one fixed height on every strip, and every strip has the same
  width and output row height, so Pan, fader, meter and name line up across instruments,
  buses and Master. The send list scrolls inside its section.
- The send row is one line and width-bounded: the `PRE` / `POST` tap toggles pre/post, the
  slot opens the destination menu, the compact knob changes level, the bypass glyph
  enables/disables. No permanent dB text and no strip widening. **Add send…** uses the same
  full-width field row and exact height as **Add effect…**.
- The send knob popup shows only the value, for example `-2.9 dB`, never the channel or
  routing label. It stays visible while the knob is dragged and follows the live level. It is
  rendered in the tooltip layer, not inside the scrolling send list.
- **Add send…** opens a menu that first offers **New Bus Send…** and then the buses that do
  not already receive a send from this track. **New Bus Send…** creates the bus and the send
  as one undo step, so a send can be created without leaving the send menu. With no buses the
  new-send action is still available and no dead rows appear.
- The Output menu lists Master and all buses, ticks the current destination, and ends with
  **New Bus…**. A bus's output menu shows Master only. A track's context menu offers the
  same commands and **Route to New Bus**.
- Buses are selected and renamed from their mixer strip; they do not appear as arrangement
  rows. Selecting a bus retargets the mixer focus but does not arm or load an instrument.
- Clicking anywhere on a strip selects the channel, not only its name box. Mixer and
  sequencer selection are synchronized: selecting an instrument in the mixer highlights and
  reveals it in the sequencer, and selecting it in the sequencer highlights and reveals its
  mixer strip.
- Insert and Plugin power buttons are centred in the gutter between the slot and the strip's
  trailing edge, so the bypass column is balanced rather than flush right.
- Static labels across the UI are not mouse-selectable and do not show a text cursor; only
  editable text fields accept a caret and selection.
- Long names truncate in the strip and are complete in the tooltip and accessible name.
- Keyboard: Tab reaches every send control; Enter/Space on the slot opens the destination
  menu; Space/Enter on the tap toggles pre/post; the knob follows the existing fader
  conventions; Delete removes a focused send. Escape closes the popup and menus.
- States: no buses, one send, three sends, more than three sends (scroll), pre, post,
  bypassed, output menu open, bus selected, bus soloed with upstream tracks, bus with
  sources refusing removal.

The UI change protocol applies: the proposal is approved; the implementation must add the
gallery cases and goldens for the new states at 1024 and 760 points, and update
[the mixer view](../UI/views/mixer.md) and [channel strip](../UI/views/channel-strip.md)
documentation in the same change.

## Commands

New or extended registry commands (titles in English, as the rest of the registry):

- `Add Bus`
- `Route to New Bus`
- `Output Master` / `Output to Bus…` (menu-driven destination choices)
- `Add Send…`
- `Remove Send`
- `Send Pre-Fader` / `Send Post-Fader`
- `Bypass Send`

Existing `Rename`, `Move Up`, `Move Down` and `Remove Track…` apply to buses where sensible;
bus removal is refused while it has sources.

## Failure and Edge Cases

- Sending to a missing or deleted bus is impossible after validation; a project that fails
  validation is rejected rather than partially loaded.
- Removing a bus with sources is refused with a clear message; the last valid routing stays.
- Adding a send to a bus that already has one selects the existing send instead of creating
  a duplicate.
- Output to the current bus is a no-op and does not create an undo entry.
- A bus with no inputs exports/plays silence and can be removed.
- An empty project has Master only; `+ Bus` is still available.
- A bus insert failure follows T12: the slot is marked failed, its dry signal passes, and
  the rest of the graph keeps running.
- A send level is clamped to the fader bounds; the popup shows the exact value including
  `-∞`.
- The master output remains fixed; buses cannot be assigned an audio-device output.

## Implementation Progress (2026-11-20)

- Model schema v4 with `TrackKind`, `Send`, per-track `output`, validation, v1/v2/v3
  migration and round-trip tests.
- Engine `PreparedMix` now publishes instrument channels, buses, sends and the solo set.
  The callback applies pre/post taps and bus matrices; the routing worker processes bus and
  master insert chains, aligns direct and bus paths, publishes bus meters and maintains the
  reported total latency. `Control::Send` provides the send-level knob range and formatting.
- Session actions for `AddBus`, `SetOutput` and `SendEdit` (add, remove, destination,
  pre/post, bypass, preview/commit level), undo entries, bus-removal refusal, persistence
  and bus meters in the mixer snapshot.
- Mixer UI: instrument bank on the left, bus bank docked immediately before Master; equal
  strip width and height across instruments, buses and Master; one-line width-bounded send
  rows with PRE/POST tap, destination menu, compact level knob and bypass; Add send… at the
  same height and in the same field style as Add effect…; Output menu with Master/bus/New
  Bus; `+ Bus`; and the shared command registry entry for **Add Bus**. The send knob popup
  shows only the live dB value and stays open while dragging. Clicking anywhere on a strip
  selects the channel and synchronizes mixer and sequencer selection. Insert and Plugin power
  buttons are centred in the gutter before the strip edge. Static labels are not selectable
  and show no text cursor. **Add send…** creates a new bus send or picks an existing bus;
  `Action::AddBusSend` records the bus and send as one undo step.
- Gallery: `t15_gallery` renders `buses`, `solo`, `long-sends` and `empty` at 1024 and 760.
  Snapshots are under `docs/UI/snapshots/egui/t15-*`.

## UI Feedback Pass (2026-11-20)

The second GUI review asked for these mixer corrections, implemented without changing the
approved routing model:

- The send knob popup shows only the dB value and stays open with the live value while the
  knob is dragged.
- Add send uses the shared Add field primitive, so its height matches Add effect exactly.
- Insert and Plugin power buttons sit in a compact gutter between the slot and the strip's
  trailing edge, and the send row uses the same power glyph instead of a text `B`.
- Static labels are no longer mouse-selectable and no longer show a text cursor; editable
  fields keep their native caret and selection.
- Clicking anywhere on a strip selects its channel, not only the name box; selection stays
  synchronized with the sequencer.
- Add send offers **New Bus Send…** and the existing buses in the same menu. The new action
  creates the bus and its send as one undo entry.

Evidence: the T15 gallery snapshots were regenerated, and the focused tests now cover the
Add send/Add effect height equality, bypass centring, whole-strip selection, the Add send
menu, the live value-only popup, and the one-step bus-plus-send undo.

## Crash Regression (2026-11-20)

The first live run crashed in the CoreAudio IO thread with a stack-guard protection fault
inside `RoutingInput::finish` → `rtrb::Producer<RoutingPacket>::push`. `RoutingPacket`
carried `buses: [[Frame; BLOCK]; MAX_TRACKS]` (about 128 KiB) by value and `finish` copied it
with `let packet = *self.pending`, overflowing the audio thread's small stack.

Fix: `RoutingPacket::buses` is now a heap `Vec<[Frame; BLOCK]>`; the callback moves the
buffer into the packet instead of copying it, keeps a spare buffer, and the routing worker
returns consumed buffers through a free ring. No audio-callback path constructs or copies a
64–128 KiB packet on the stack. Regression tests:
`routing_packet_moves_bus_buffers_through_the_handoff`,
`routing_handoff_fits_on_a_small_audio_thread_stack` (64 KiB stack), and the existing
allocation-free callback tests.

## Listening Pass Findings (2026-11-20)

- **Send bypass ignored.** In the mixer event dispatch, the `SendEnabled { enabled }` field
  shadowed the strip's `enabled` guard, so a bypass click (`enabled: false`) fell through and
  was dropped; only re-enabling worked. Fix: the strip guard now wraps the whole dispatch
  (`mixer_event`), and no event arm carries its own guard. Regression:
  `send_bypass_and_enable_both_reach_the_session`.
  Retest after that fix: the user still reports that bypass does not work. Added evidence:
  `t15_send_bypass_glyph_click_reaches_the_session` (a pointer click on the rendered glyph
  dispatches one edit) and a `SetEnabled` false/true round trip in
  `buses_outputs_sends_and_bus_removal_refusal_round_trip_without_workers`; the engine maps a
  disabled send to gain 0 and skips it. Open: what the user hears on bypass (a reverb tail
  continuing on the bus is expected; the send input stops), and whether the retest used a
  build after the fix. Rebuilt `target/debug/clef` on 2026-11-21 for the retest.
- **Send to a bus with a reverb insert.** Resolved by the user: the send works. The reported
  problem is the Stop behaviour: Stop fades the whole master output over 5 ms and keeps the
  output gate closed until Play or audition (a T12/T35 decision that removed a Stop crackle),
  so bus effect tails are cut. The user expects effect tails to continue after Stop; see
  [Stop and Tails](#stop-and-tails-added-2026-11-21-draft-awaiting-approval).

## Completion (2026-11-21)

The user confirmed on 2026-11-21, with `validation-output/T12-listening-v2.clef`:

- Listening: effect and instrument tails ring out after Stop; the send bypass works on the
  rebuilt binary.
- The remaining manual GUI pass (mixer, Output menu, solo states) and listening pass
  (pre/post, solo returns, bus PDC) were confirmed by the user at closure.

Automated evidence at completion: `cargo test --locked` 227 passed, 26 ignored; clippy and
`cargo fmt --all --check` clean.

Known limits carried forward (not blocking closure, by the user's decision):

- No native effect-fixture regression for bus-chain PDC; the routing math and dry handoff
  have unit coverage only.
- Send budget rejection at the project bound and a direct v1/v2 bus-shaped project test are
  untested.
- The full keyboard/accessibility interaction matrix for send controls, the Output menu and
  the dB popup is not covered beyond presence and bounds.
- The gaps listed below remain as recorded.

## Known Gaps at Closure

- Bus-chain PDC has unit coverage for the routing math and dry handoff, but no native
  effect-fixture regression yet. The T12 ignored native effect tests remain the model.
- The track context menu still lacks dedicated **Output…**, **Sends…** and **Route to New
  Bus** entries; the mixer exposes the same actions. The command registry has **Add Bus**.
- Bus strips show the number of sources, not their names.
- Solo gating is enforced by meters and the engine; the mixer does not visually dim excluded
  strips in this provisional pass.
- T18 design-system acceptance is still outstanding; this is provisional mixer work.

## Verification

Automated evidence recorded on 2026-11-20:

- `cargo test --locked`: 191 passed, 20 ignored.
- `cargo clippy --all-targets --locked -- -D warnings`: no issues.
- `cargo fmt --all --check`: clean.
- Model: `bus_output_and_sends_round_trip_and_reject_invalid_routing`,
  `v3_metadata_migrates_to_v4_defaults`.
- Engine: `bus_solo_propagates_upstream_and_send_returns_stay_audible`,
  `send_matrix_keeps_pan_and_fader_without_the_solo_gate`,
  `bus_output_and_send_sum_in_the_callback_without_allocation`,
  `routing_packet_moves_bus_buffers_through_the_handoff`,
  `routing_handoff_fits_on_a_small_audio_thread_stack`, and the existing
  `master_handoff_keeps_the_dry_signal_aligned_and_drops_late_wet_blocks`.
- Session: `buses_outputs_sends_and_bus_removal_refusal_round_trip_without_workers`,
  `add_send_to_new_bus_is_one_undo_step`.
- UI: `t15_mixer_exposes_bus_sends_output_and_add_bus_at_both_widths` (now checks Add send
  and Add effect share a height, the compact 14×20 insert bypass is centred, and the send
  bypass is the 14×16 glyph),
  `t15_buses_dock_right_and_strips_keep_one_width_and_height`,
  `t15_mixer_strip_click_selects_the_channel_and_reveals_it`,
  `t15_add_send_menu_can_create_a_bus_or_pick_an_existing_one`,
  `compact_send_knob_keeps_a_value_only_popup_visible_while_dragging`; the existing T11/T12
  gallery layout tests still pass.
- Gallery renders: [buses 1024](../UI/snapshots/egui/t15-buses-1024.png),
  [buses 760](../UI/snapshots/egui/t15-buses-760.png),
  [solo](../UI/snapshots/egui/t15-solo-1024.png),
  [long sends](../UI/snapshots/egui/t15-long-sends-1024.png),
  [empty](../UI/snapshots/egui/t15-empty-1024.png).

Verification not recorded at closure (see Completion for the user's decision):

- Native bus-chain PDC with the effect fixture: different track and bus chain latencies,
  direct and bus paths landing on the same sample, total-latency transition on a graph
  change. The routing math and dry handoff have unit coverage; the plugin-fixture leg is
  pending.
- Send budget rejection at the project bound, and a direct test of a v1/v2 project with a
  bus-shaped track table (the existing v1/v2 tests still pass).
- Accessibility pass for every send control, the Output menu and the dB popup, including
  keyboard pre/post, destination, bypass and remove; the current gallery test covers presence
  and horizontal bounds, not the full interaction matrix.
- Manual GUI pass of the mixer, Output menu and solo states; listening pass for pre/post,
  solo returns and PDC.

## Open Questions

- Whether sends should also be available from buses in a later task. This specification says
  no; the routing graph stays one level.
- Whether folders and nested buses should follow. Deferred; they need a separate graph design.
- Whether automation of send level and output is included in the automation task. Deferred.
