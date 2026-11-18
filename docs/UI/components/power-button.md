---
name: Power button
kind: component
reference: composer::style::PowerButton, composer::style::StepButton
status: shared
impl: planned
tokens: [accent, secondary, disabled]
used-in: [channel-strip, mixer, fx-rack, side-drawers, plugin-window]
snapshot: snapshots/shared-controls.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Power button

## Purpose
On/off (bypass) for inserts, sends, racks, nodes, Monitor FX slots and the sidechain.

## Anatomy
The power glyph only: `accent` when on, `secondary` when bypassed, `disabled` when unavailable.
`paintGlyph` paints it for surfaces that draw rows instead of hosting buttons. `onAccentFill` inverts
it inside an accent-filled control.

Stepper: `composer::style::StepButton` is the same bare-glyph treatment for the − / + of a stepped
value field (the channel strip's Delay, the Add Tracks count): `secondary` at rest, `text` under the
pointer, the shared glyph hover. The LookAndFeel creates it for every `IncDecButtons` slider, so a
stepper is never two joined text buttons.

## Metrics
Channel strip column 26; mixer insert 16, mixer send 14.

## Behaviour
A registry command or session edit, one undo step. A node's power button is mappable to a macro.

## Do / Don't

## Code
```cpp
composer::style::PowerButton power { "Insert 2 power" };
power.setToggleState(!bypassed, juce::dontSendNotification);
// A painted row (Monitor FX) uses the glyph alone:
composer::style::PowerButton::paintGlyph(g, area, on, available, over);
```
