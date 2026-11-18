---
name: Glyph toggle
kind: component
reference: composer::style::GlyphToggle
status: shared
impl: planned
tokens: [field, peacock, text, secondary, disabled, controlEdge]
used-in: [midi-functions, tool-row]
snapshot: snapshots/midi-functions-sections.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Glyph toggle

## Purpose
A glyph-only toggle on a raised control body, where a word would waste room: the triplet and dotted
feels (D102, D103). Two of them side by side replace a Straight · Triplet · Dotted choice: at most one
lit, neither lit means straight (composer, 2026-11-12).

## Anatomy
A raised `field` body (`paintControlBody`) with a 14-pixel [icon](icons.md) centred: `Icon::triplet`
(a bracket over a 3) or `Icon::dotted` (a note with its dot).

## Metrics
24 × 22 inside a `controlHeight` row; 4 pixels between a pair.

## States
At rest `secondary` glyph; hover lifts the glyph to `text` and adds the `controlEdge` edge; lit is a
`peacock` body with a `text` glyph (no top light); disabled is flat with a `disabled` glyph.

## Behaviour
The host owns the state and the exclusivity of a pair. The accessible name is the tooltip ("Triplet",
"Dotted"); hosts with a command add its live binding to the tooltip.

## Do / Don't
- Do use it for a feel or another binary choice with a well-known glyph.
- Don't use it where a word is clearer; never paint the glyph inline in an app component.

## Code
`style::GlyphToggle triplet { style::Icon::triplet, "Triplet" };` in `UI/ComposerStyleKit/Source/ComposerStyle.h`.
