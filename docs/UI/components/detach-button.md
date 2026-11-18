---
name: Detach button
kind: component
reference: composer::style::DetachButton
status: shared
impl: planned
tokens: [text, accent]
used-in: [fx-rack, ara-editor]
snapshot: snapshots/shared-controls.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Detach button

## Purpose
Opens the current panel in its own resizable window: a frame with an arrow leaving its top right.

## States
Toggled on (the panel is detached, D87's ARA editor): the arrow's head is at the frame's centre, pointing back
in: the button attaches the panel again. The tooltip says which ("Open the editor in its own window", "Attach the
editor to the panel").
