---
name: Note
kind: component
reference: composer::style::paintNote, NoteState
status: shared
impl: planned
tokens: [capLight, oxblood, danger, text]
used-in: [piano-roll, arrangement]
snapshot: snapshots/kit-gallery.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Note

## Purpose
A MIDI note in the roll, and inline in arrangement parts.

## Anatomy
A rounded (2-pixel) rectangle in the track colour with an optional label (`foregroundFor`). The label starts
3 pixels past the resize-handle width on every note, so the primary note's handle never overlaps it
and the label does not shift when selection changes.

## States
| State | Rendering |
| --- | --- |
| normal | Track colour |
| hovered | Fill lift only |
| selected | Roll: track colour with a 1.5-pixel `capLight` (white) outline inside the bounds. Inline in arrangement parts: a `capLight` fill, the other notes in the part's ink at 60 %; no red fill (composer, 2026-11-13: red belongs to out-of-scale rows and danger marks) |
| primary | Selected plus a 3-pixel resize handle at each end (one note only) |
| muted | 35 % fill, dimmed label |
| recordingPreview | Transient oxblood fill with a `danger` outline, replaced by committed notes |
| ghost | 30 % fill, faint track-colour outline (proposed note under the pointer) |

Extras: outside its track's first Range Guard, a 1.5-pixel dashed `danger` edge; erase target, a
solid `danger` edge; notes of non-active parts at 70 %; ghost parts at 22 % with no label, never
hit-tested; take notes the comp doesn't play at 35 %.

## Behaviour
Lifts with a [lift shadow](shadows.md) while dragged. Identity, selection and quantization stay in
the application.
