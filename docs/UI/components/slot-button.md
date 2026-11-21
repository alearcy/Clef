---
name: Slot button
kind: component
reference: composer::style::SlotButton, setSidechainOutline, setQuarantineMark
status: shared
impl: gallery
reference-source: [Source/ChannelStrip.cpp, Source/Mixer.cpp] (callers)
tokens: [field, peacock, text, disabled, sidechain, accent, controlEdge, danger]
used-in: [channel-strip, mixer]
snapshot: snapshots/egui/t18-slot-button-1024.png
api: ui::primitives::slot_button::slot
evidence: [t18-slot-button]
---

> **Migrated to Rust/egui (T18 Phase 3, 2026-11-21).** The Clef section at the end of this file is
> the implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

# Slot button

## Clef T12

*Superseded where it differs by the T18 Phase 3 section at the end of this file.*

The provisional T12 insert slot is `clef_ui::primitives::inserts::slot`. It is one
20-point full-width row with a 16-point power column. The `role` argument separates the
fixed **Plugin** slot from effect slots. The Plugin slot is first on instrument tracks,
cannot be moved or removed, and adds the `Empty` (No instrument) and `Offline` states:

- Active: `peacock` fill, `text` label, `accent` power glyph.
- Bypassed: `peacock` fill, `disabled` label, `secondary` power glyph.
- Missing: `field` fill, `disabled` label, disabled power; the tooltip says *Missing*.
- Failed: `field` fill, `danger` label and 1.5-point `danger` edge, disabled power; the
  tooltip carries the cause.
- Late (T37, effect slots): the Active look (`peacock` fill, `text` label, live power) with
  a 1.5-point `danger` edge. Shown while more than 10 % of the effect's blocks in the last
  window of about 2 s missed their deadline; it clears after a window at or below 10 %. The
  tooltip and accessible name add "Late: N % of blocks in the last 2 s. Reduce the plugin's
  load (for example oversampling)." Late is a warning, not a failure: the effect keeps
  running and is never quarantined. Missing, Failed and Bypassed take precedence.
  Snapshot: [t12-late](../snapshots/egui/t12-late-1024.png).
- Empty: `field` fill, disabled "No instrument" label; opens the instrument picker.
- Offline: `field` fill and disabled label; the worker is released and the context menu
  offers Bring Instrument Online.
- Focus: a 1-point `accent` outline. Enter or Space opens the editor; the power glyph
  toggles bypass. The accessible name includes the channel, plugin name and state.
  Long names truncate visually while the tooltip and accessible name keep them whole.
- Pointer: click opens the editor, the power glyph toggles bypass, and dragging reorders
  the slot. The slot menu is supplied by the mixer surface.

The legacy sidechain outline, latency row and quarantine warning icon are not part of
this T12 pass. Evidence: the T12 gallery states in [mixer](../views/mixer.md#clef-t12)
and `gallery::tests::pointer_hover_and_click_open_the_effect_picker_at_the_visible_gap`.

## Purpose
One instrument, insert or send slot: shows what is loaded and opens it.

## Anatomy
A full-width row: `field` when empty, `peacock` when loaded (label `foregroundFor`), `disabled`
label when bypassed. A [power button](power-button.md) at its right end. A plugin hearing a
sidechain adds a 1.5-pixel `sidechain` outline (`setSidechainOutline`), tooltip naming the sources.
The instrument slot of a MIDI track is labelled **Plugin**. A loaded plugin may add a `secondary`
"Latency: n smp / x ms" row under it (inspector only).

## Quarantined (D88)
A slot whose plugin is quarantined (`setQuarantineMark`): `field` body like an empty slot, a
1.5-pixel `danger` outline on the body, the warning [icon](icons.md) before the label and the label
in `danger`, glyph and label centred as one group. Its power button is disabled and unlit. The
tooltip, and a left click, give the reason ("… is quarantined: failed to load. Its settings are
kept. Right-click to retry or release."). Distinct from missing (`disabled` label, "(missing)") and
from bypass (`disabled` label on the loaded fill). Snapshots: `snapshots/quarantine-slot.png`
(channel strip), `snapshots/quarantine-mixer-slot.png` (mixer strip). Approved as variant A of
[Plugin Quarantine (D88)](https://claude.ai/artifact/5W86cB31xSVHVKFFW5Q2UE).

## Metrics
26 high in the channel strip, 20 in mixer strips.

## Behaviour
Left click opens the plugin editor (or the picker when empty); right click opens the slot menu
(`onMenu`); on a quarantined slot the menu starts with Retry quarantined plugin, Release plugin from
quarantine and Show quarantined plugins…, a healthy hosted plugin's with Quarantine plugin (all
registry commands with live bindings). Drag and drop reorders and accepts browser drops (2-pixel `accent` bar where it lands).
An empty send "+" keeps the upper send row, disabled, so sends read apart from inserts.

Labels take the text colour the host sets on the button (`textColourOffId`): a bypassed slot sets `disabled`. The kit `LookAndFeel` honours a button's own text colour since UI10.3.

Raised (UI10 Fix 7): a body 2 px shorter than its bounds with a 1-pixel top light and a soft 2-pixel shadow under it (`paintControlBody`, `ControlDepth::raised`); hover lifts the body to `field` without an edge; lit or pressed keeps the body and shadow and drops the top light; disabled is flat `panel` with a `divider` edge. See control depth in [foundations](../foundations.md).

## Clef Rust/egui — T18 Phase 3

**API**

```rust
pub fn slot(ui: &mut egui::Ui, name: &str, channel: &str, role: &str, state: State, description: &str, edge_inset: f32) -> (egui::Response, bool, bool);
```

A 20-point slot: `peacock` while the processor runs (Active, Late) or is bypassed (with a `disabled`
name), `field` when missing, empty or offline; Late adds a 1.5-point `danger` edge and Failed a
`danger` name and edge; keyboard focus an `accent` edge; the [power button](power-button.md) sits at
the right. Returns the slot response, whether it was activated (click, Return or Space) and whether
the power button was pressed. Accessible: a button named "<channel>, <role> <name>, <description>",
so Late, Failed and Missing are said, not only drawn.

**States:** active, late, bypassed, missing, failed, empty.

**Evidence:** gallery `snapshots/egui/t18-slot-button-{1024,760}.png` and `t18-slot-button-1024@2x.png`
(Retina); accessible names checked by `gallery::tests::every_gallery_control_has_an_accessible_name`.
