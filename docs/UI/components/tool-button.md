---
name: Tool button
kind: component
reference: composer::style::ToolButton
status: shared
impl: planned
tokens: [text, accent, danger, disabled]
used-in: [tool-row, transport-bar, window]
snapshot: snapshots/kit-gallery.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Tool button

## Purpose
Borderless glyph button for editing tools, transport controls, settings, fit and close.

## Anatomy
A JUCE `Button` that paints one [icon](icons.md). No text, tile, border, bevel, underline or fill.

## Metrics
28 × 28 target, 16-pixel glyph.

## States
- normal `text`; hover and down adjust the glyph tone; disabled `disabled`.
- toggled: `accent`, or the colour given by `setActiveColour` for a lamp (play accent green, record
  `danger`). A lit lamp stays lit while the button is disabled.
- keyboard focus: a separate focus circle, never confused with the active state.

## Behaviour
State and invocation come from the command registry:
`button.setCommandToTrigger(&manager, commandID, true)` or the app's `Commands` adapter. The tooltip
shows the command name and its live shortcut.

## Do / Don't
- Do keep Select, Draw, Paint, Erase (and Shape) in the same order on both editing surfaces.
- Don't add text labels or fills to show the active tool; only the glyph colour changes.

## Code
```cpp
composer::style::ToolButton select { composer::style::Icon::select, "Select" };
select.setCommandToTrigger(&commandManager, commandID, true); // registry owns state, shortcut and tooltip
```
