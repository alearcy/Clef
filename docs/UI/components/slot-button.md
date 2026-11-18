---
name: Slot button
kind: component
reference: composer::style::SlotButton, setSidechainOutline, setQuarantineMark
status: shared
impl: planned
reference-source: [Source/ChannelStrip.cpp, Source/Mixer.cpp] (callers)
tokens: [field, peacock, text, disabled, sidechain, accent, controlEdge, danger]
used-in: [channel-strip, mixer]
snapshot: snapshots/shared-controls.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Slot button

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

Raised (UI10 Fix 7): a body 2 px shorter than its bounds with a 1-pixel top light and a soft 2-pixel shadow under it (`paintControlBody`, `ControlDepth::raised`); hover adds the `controlEdge` edge; lit or pressed keeps the body and shadow and drops the top light; disabled is flat `panel` with a `divider` edge. See control depth in [foundations](../foundations.md).
