---
name: Power button
kind: component
reference: composer::style::PowerButton, composer::style::StepButton
status: shared
impl: app
tokens: [accent, secondary, disabled]
used-in: [channel-strip, mixer, fx-rack, side-drawers, plugin-window]
snapshot: snapshots/shared-controls.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Power button

## Clef T12

The provisional T12 insert power button is drawn in
`clef_ui::primitives::inserts::power_button` as a vector glyph: a broken ring with a
vertical stem, 16 by 20 points. It uses `accent` when the slot is active, `secondary` when
bypassed, and `disabled` when the effect is missing or failed. Hover lifts it to
`capLight`; keyboard focus draws a 1-point `accent` ring. Enter or Space toggles bypass
when the button is enabled, and the accessible name identifies the channel and plugin.

For an instrument track the power is the Plugin slot's power: it bypasses the instrument
while its worker stays online, muting its output before Gain and the effect chain. The
Plugin slot's context menu also exposes the separate Offline state, which releases the
worker; Offline disables this power control. Evidence: the T12/T35 gallery states in
[mixer](../views/mixer.md#clef-t35).

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
