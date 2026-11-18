# T28 — FX Rack and Virtual Cable Graph

**Status:** Planning draft — not approved for implementation. Product behavior is inherited
from the Composer DAW; the Clef implementation interview and approval are still required.
**Priority:** P2.
**Date:** 2026-11-18
**Task:** [T28](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** T12 insert effects and worker hosting; T15 routing and PDC; T08 undo/redo;
T04 track chain; T16 plugin catalog and quarantine where available.

## Reference decisions

The product decisions were taken and verified in the external Composer DAW project. It is not
part of this repository and is never an API:

- `~/Documents/ChatGPT/DAW/Specs/D59-FX-Composer.md` — DONE 2026-11-05. The first
  implementation inherits phases **D59.1 (racks and graph)** and **D59.3a (instrument racks
  and MIDI routing nodes)**. D59.2 (macros), D59.4 (modulators, pump, snapshots), D59.5 (rack
  browser), and D59.3b (articulation split, range guard, CC remap, arpeggiator) are later
  decisions and are not part of the first approval.
- `D58-Sidechain-Routing.md` and `D26-FX-And-Bus-Tracks.md` are related reference decisions;
  sidechain routing stays outside this task unless the Clef interview includes it.
- [FX rack](../UI/views/fx-rack.md) is the Clef UI reference; it currently describes the whole
  D59 surface, including the deferred macro and modulator parts.

Clef inherits the behavior, not the C++ implementation. Tracktion `RackType`,
`RackInstance`, `ConnectedNode`, and the Composer node classes are historical references:
Clef owns its rack model, graph compilation, and realtime path in Rust. The legacy node types
and their symbols are never APIs.

## Goal

Let a track's signal path be built from nodes connected by virtual cables: audio nodes,
instrument racks, and native MIDI routing nodes, with parallel branches, summed inputs,
latency compensation, bypass, undo, and persistence. The rack is the container in the track
chain; the graph is what the user edits.

## Inherited behavior

### Rack container and chain

- A rack is a container in the track chain: an insert, or the instrument (“Plugin”) slot for
  an instrument rack. A track may hold several racks; the signal runs through them in slot
  order (Out of one rack feeds In of the next). No cable is drawn between rack boxes.
- The editor is a free graph with cables in the lower panel's **FX Racks** tab beside the piano
  roll and mixer. One box per rack of the selected track, in chain order, each box holding its
  own graph. A “New rack” box at the end offers empty, template, and preset.
- A new rack is transparent: Rack In feeds Rack Out. Rack In and Rack Out are fixed rails with
  audio and MIDI pins; a rack with no nodes passes its input through.
- Box header: power glyph (rack bypass), slot label (PLUGIN, INSERT n), editable name, total
  latency in samples, menu, and detach. A rack can be detached into its own resizable window.
- Removing an insert removes its rack; undo restores both in one step.

### Graph and cables

- Nodes are boxes with a power glyph, name, and kind tag. Pins sit on the box edges: audio
  (stereo), MIDI, and sidechain/modulation where the phase provides them. Audio pins are
  `accent`, MIDI pins and cables use the `midi` role; a stereo pin is two channel connections,
  a MIDI pin is one.
- A drag from an output pin to a compatible input pin creates a cable. Dropping on an
  incompatible pin or a connection that would create a cycle does nothing. Deleting a node
  removes its cables. A cable is selected by click and deleted with the shared delete command.
- Several cables into one input are summed. The graph aligns the latency of parallel branches
  before the sum, so a direct path and a processed path stay phase-aligned.
- Selection takes the shared accent edge. Bypass dims a node and its cables; a bypassed node
  passes its input through (for MIDI, unchanged). Double-clicking a plugin node opens its
  original editor.
- The graph is a larger canvas that scrolls both ways inside its box. Nodes move by drag;
  positions, bypass, and fold state are saved with the rack.
- Any node can fold to one line, keeping its pins; the graph never reflows. Fold/unfold
  selected, fold all, unfold all, and focus next/previous rack are commands.
- While the transport plays (or a track sounds from live input), dots travel active cables;
  stopped, cables are still.
- Duplicate copies selected nodes with their values and the cables among them; a marquee and
  Shift-click select several nodes, which move, duplicate, bypass, and delete together, one
  undo step each.

### Audio nodes

Native audio nodes are an accepted exception to “no built-in plugins”: they are routing and
utility nodes, not a general internal plugin suite.

- **Gain**: gain (dB fader), pan, width, phase invert L/R.
- **M/S Encode**: L/R in, Mid and Side out. **M/S Decode**: Mid and Side in, L/R out.
- **Crossover**: 2–5 bands, Linkwitz-Riley 24 dB/oct, phase-coherent sum with allpass
  compensation, one stereo output pin per band, draggable split frequencies with unit
  readouts.
- **Mix**: Dry and Wet stereo inputs, one Wet knob, equal-power law.

### Instrument racks and MIDI nodes

- The Plugin slot menu offers “New instrument rack” (transparent) and, when the slot holds an
  instrument, “Wrap in rack”: the same plugin instance moves into a new rack, cabled MIDI In →
  instrument → Rack Out, with state and window kept. One undo step each. Removing the slot
  removes the rack; one undo step restores both.
- Choosing an instrument from the slot while it holds a rack adds the instrument to the rack.
  Adding an instrument cables MIDI In → instrument and its main output → Rack Out, summed with
  the others; the empty rack's In → Out passthrough is replaced, not summed with. Extra outputs
  of a multi-output instrument stay free.
- MIDI nodes live only in instrument racks; insert racks keep the audio node menu. An
  instrument rack's node menu has Plugin… (instruments, audio effects, MIDI effects), the audio
  nodes, and a MIDI section.
- Native MIDI nodes, all with note-on/note-off pairing (a note-off goes where its note-on went,
  whatever changed in between) and bypass that passes MIDI unchanged:
  - **Key Split**: 2–8 zones, one output pin per zone, split points as notes, draggable on a
    keyboard view or typed, overlap width in semitones, velocity scaled across the overlap
    (equal power or linear law).
  - **Velocity Split**: Soft and Loud outputs, low/high overlap (1–127), velocity scaled by the
    chosen law.
  - **Velocity Curve**: curve (−100 % compress … +100 % expand), min and max output velocity,
    seeded random so a render from the same start repeats.
  - **MIDI Filter**: Pass or Block for Notes, Pitch bend, Aftertouch (channel and poly),
    Program change, and a list of CC numbers.
  - **Transpose**: −48…+48 semitones, optional scale lock (tonic and mode), nearest scale note,
    ties downward.
  - **Channel**: listen (All or 1–16; other channels dropped) and send on (Keep or 1–16).
  - **MIDI Merge**: one more input pin than connected cables (from 2), one output.
- Numeric values (splits, overlap, velocities, curve, random, semitones) are parameters ready
  for macros and automation; modes, switches, keys, and CC lists are plain node settings. Every
  edit is one undo step.

### Commands

All commands go through the shared command registry, Actions list, Commands search, and
assignable keystrokes:

- Toggle FX racks (tab), New rack on selected track, Bypass selected nodes, Add node.
- Delete selection removes selected nodes or cables while focus is inside the tab.
- New instrument rack, Wrap instrument in rack, Rename rack, Detach rack, Fold/unfold and
  focus-rack commands, Duplicate selection.

No command gets a default keystroke unless the interview agrees one.

## Clef requirements

### Model and graph compilation

- The project document owns each rack (identity, slot position, name, bypass), its nodes
  (kind, plugin identity where hosted, parameter values, position, fold state), and its cables
  (source pin, destination pin).
- A graph is validated off the callback: pin compatibility, no cycles, bounded node and cable
  counts, bounded fan-out and fan-in. An invalid edit is refused with a reason and leaves the
  last valid graph intact.
- Compile the graph to a bounded processing plan with explicit buffer routing, branch latency
  alignment, and sum points. Graph changes are prepared off the callback and swapped at a
  buffer boundary; the callback never walks editable model data.
- Plugin nodes reuse T12's isolated workers, failure deadlines, state capture, and editor
  path. A failing plugin node is bypassed or silenced without taking down the graph or the
  callback; the rest of the rack keeps playing.
- Native nodes perform bounded DSP with preallocated state. No allocation, lock, formatting,
  logging, or I/O on the callback. The execution location of native nodes (engine callback
  versus a dedicated DSP path) is an implementation decision recorded in the approved
  specification; it must preserve sample accuracy and PDC.
- MIDI nodes preserve note identity and pairing across every transformation; a bypass passes
  MIDI unchanged. The MIDI path stays sample-accurate and does not add unbounded per-event
  work.
- PDC covers racks, plugin latency, parallel branches, and the path from track to master. A
  latency change while playing is applied without a click and with explicit behavior for
  already-sounding material.

### Persistence and undo

- Save/open round-trips racks, graphs, positions, bypass, fold state, names, parameter values,
  plugin identities, and state. Schema migration keeps earlier projects valid.
- Every node, cable, position, bypass, rename, wrap, and removal edit is one undo step. View
  state (fold, scroll, selected node) is saved but not undoable.
- Quarantined plugin nodes follow T16: the node is outlined in the danger treatment, its name
  and kind tag say quarantined, its pins and cables stay as saved, and its node menu carries
  the quarantine commands. A quarantined node does not run.

### Interaction with neighbouring tasks

- T12 supplies effect hosting; T28 adds the rack container and graph around it. Do not
  duplicate the effect worker or chain state.
- T15 supplies the routing and PDC foundation. T28's graph is inside a rack, not a replacement
  for buses and sends.
- T22's remaining native MIDI tools are designed as rack nodes after T28; M/S encode/decode
  and the utility nodes in this task must not be duplicated there.
- T29 ARA is independent of racks; an ARA clip is not a rack node.

## Out of scope

Nested racks; racks on clips; cables between rack boxes; audio-rate or polyphonic modulation;
MIDI output to hardware from a rack; macros, snapshots, modulators, pump, and the rack browser
(D59.2/D59.4/D59.5) in the first implementation; the articulation split, range guard, CC remap,
and arpeggiator nodes (D59.3b, owned by T22); sidechain routing unless the interview includes
it; ARA; internal instrument/effect plugins beyond the native utility nodes.

## Open questions

- Where native audio and MIDI nodes execute, and how that meets the callback rules without
  adding latency.
- Whether Clef hosts third-party MIDI effects in this task or only native MIDI nodes.
- Maximum racks per track, nodes per rack, cables per pin, and total plugin nodes.
- How branch latency is measured and aligned, and how PDC reacts to a graph change during
  playback.
- Whether a plugin node gets its own worker or shares the track's worker pool, and the failure
  policy for a node.
- Graph-change policy during playback: mute the rack for a block, crossfade, or apply at the
  next buffer.
- Parameter model and whether macro/automation support is Phase 2 of T28.
- Sidechain pins and routing: defer entirely or reserve the pin model now.
- Schema layout and version bump; coordinate with T16 identity and T15 routing.
- Which parts of the UI proposal are implemented in the first phase, given that
  [fx-rack](../UI/views/fx-rack.md) also describes macros and modulators.
- Command names and categories, if they differ from the reference.

## UI proposal requirement

The FX Racks tab, rack boxes and headers, graph canvas, nodes, pins, cables, node menus,
folding, detach, selection, duplicate, and the instrument-rack flow require a written UI
proposal derived from [fx-rack](../UI/views/fx-rack.md), [plugin window](../UI/views/plugin-window.md),
and the listed components, with user approval before implementation. The proposal must mark
which parts are in the first phase and which are deferred.

## Verification intent

- Rack create/delete on a track with undo/redo; node add/remove/move; legal and illegal
  connections (cycle, wrong type); stereo pin equals two channel connections; fan-out and
  summed fan-in.
- Save/reopen keeps graph, positions, bypass, fold state, names, and plugin identities.
- Crossover band sum nulls against the input through the split allpasses below the agreed
  tolerance; M/S encode plus decode nulls against the input; Mix law matches the agreed curve.
- Branch latency alignment and PDC are sample-checked; a graph change during playback does not
  click or leave stale audio.
- A crashing plugin node is isolated; the rest of the graph keeps playing and the failure is
  reported.
- Bypass is sample-identical to the input path at the same latency; MIDI bypass is unchanged
  and keeps note pairing.
- No realtime allocation, lock, or unbounded work on the callback; CPU and latency stay within
  the agreed budget.
- Commands registered with correct availability; real-component snapshots compared with the
  approved boards.
- Automated, GUI, listening, and hardware evidence stay separate.

## Approval gate

This document is a planning draft. The product decisions are the referenced Composer DAW
specification; a Clef implementation interview for the open questions, an approved
specification, and an approved written UI proposal are required before implementation.
