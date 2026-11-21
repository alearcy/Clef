---
name: Text field
kind: component
reference: LookAndFeel (TextEditor colours); app AmountField, inline rename editors
status: mixed
impl: gallery
reference-source: [Source/RackEditor.cpp (AmountField), Source/Arrangement.cpp, Source/PianoRoll.cpp, Source/MarkerList.cpp]
tokens: [field, text, accent, controlEdge]
used-in: [track-header, track-lanes, fx-rack, settings, transport-bar, channel-strip, command-palette]
snapshot: snapshots/egui/t18-text-field-1024.png
api: ui::primitives::text_field::TextField
evidence: [t18-text-field, primitives::text_field::tests]
---

> **Migrated to Rust/egui (T18, 2026-11-21).** The Clef section at the end of this file is the
> implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

# Text field

## Purpose
Typing a name or an exact value: inline renames (tracks, parts, lanes, takes, racks), the position
counter, typed knob values, node fields, search fields.

## Anatomy
A native `juce::TextEditor` with a `field` fill and `controlEdge` edge; focus shows the `accent` edge.
Search fields carry the `search` glyph at the left.

## Behaviour
- Native editing is kept: selection, clipboard, IME, undo inside the field.
- Return commits, Escape restores the previous value; an invalid value is rejected, the previous
  complete value restored and the reason sent to the status line.
- Inline rename opens on double-click over the name, with the name's own font and position; Tab and
  Shift-Tab walk the next names in row order.
- A typed value replaces a readout in place (knob value line, counter); no popup, no second field.

## Do / Don't
- Don't open a dialog to rename.
- Don't restyle a text field with a track colour.
- Don't use a text field to enter a numeric parameter: every number is a [value slider](fader.md#value-slider), whose value text is typed in place on double-click (composer, 2026-11-13, global rule).

Raised (UI10 Fix 7, variant B for fields): the same body, top light and under-shadow as a raised button (`ControlDepth::raised`); hover lifts the body to `field`, focus keeps the `accent` edge while typing. See control depth in [foundations](../foundations.md).

## Clef Rust/egui — T18

**API**

```rust
impl<'a> TextField<'a> {
    pub fn new(value: &'a mut String, name: &'a str) -> Self; // name: accessible label
    pub fn hint(self, hint: &'a str) -> Self;
    pub fn width(self, width: f32) -> Self;      // default: available width
    pub fn enabled(self, enabled: bool) -> Self;
    pub fn id(self, id: egui::Id) -> Self;       // stable id for focus requests
    pub fn show(self, ui: &mut egui::Ui) -> egui::Response; // the TextEdit's response
}
```

A frameless `egui::TextEdit` (native selection, clipboard, IME) on a raised body, 24 high, text
inset `CONTROL_PADDING`. Hover lifts the body; focus draws the 1-point `ACCENT` edge while typing;
disabled is flat with `DISABLED` text. Callers keep Return/Escape handling. Used for the track
name, plugin, effect and command searches, and the file name. The inline take-lane rename keeps
its own editor because it uses the name's font and position; it moves here when the inline
variant is specified.

Evidence: the gallery cannot hold OS focus, so the focus edge is covered by
`focused_field_draws_the_accent_edge_and_carries_its_name`.
