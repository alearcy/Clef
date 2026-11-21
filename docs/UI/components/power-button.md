---
name: Power button
kind: component
reference: composer::style::PowerButton, composer::style::StepButton
status: shared
impl: gallery
tokens: [accent, secondary, disabled]
used-in: [channel-strip, mixer, fx-rack, side-drawers, plugin-window]
snapshot: snapshots/egui/t18-power-button-1024.png
api: ui::primitives::power_button::power_button
evidence: [t18-power-button]
---

> **Migrated to Rust/egui (T18 Phase 3, 2026-11-21).** The Clef section at the end of this file is
> the implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

# Power button

## Clef T12

*Superseded where it differs by the T18 Phase 3 section at the end of this file.*

The provisional T12 insert power button is drawn in
`clef_ui::primitives::inserts::power_button` as a vector glyph: a broken ring with a
vertical stem, 16 by 20 points. It uses `accent` when the slot is active, `secondary` when
bypassed, and `disabled` when the effect is missing or failed. Hover lifts it to
`capLight`; keyboard focus draws a 1-point `accent` ring. Enter or Space toggles bypass
when the button is enabled, and the accessible name identifies the channel and plugin.

For an instrument track the power is the Plugin slot's power: it bypasses the instrument
while its worker stays online, muting its output before Gain and the effect chain. The
Plugin slot's context menu also exposes the separate Offline state, which releases the
worker; Offline disables this power control. The same `clef_ui::primitives::inserts::bypass`
glyph is used at 14×20 for insert slots and 14×16 for the compact send row. Evidence: the
T12/T35 gallery states in [mixer](../views/mixer.md#clef-t35).

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
Channel strip column 26; mixer insert and mixer send 14.

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

## Clef Rust/egui — T18 Phase 3

**API**

```rust
pub fn power_button(ui: &mut egui::Ui, width: f32, height: f32, label: &str, lit: bool, ready: bool) -> bool;
```

The power glyph (an open arc and a stem, 1.4-point stroke): `accent` while the processor runs,
`secondary` when bypassed, `CAP_LIGHT` on hover, `disabled` when not ready; keyboard focus an `accent`
ring. Space and Return toggle it with focus. Accessible: a button named by `label` ("Strings, Bypass
insert Tape EQ") with the on state. Used by [slot buttons](slot-button.md) and compact send rows
(`inserts::bypass` re-exports it).

**States:** on, bypassed, not ready.

**Evidence:** gallery `snapshots/egui/t18-power-button-{1024,760}.png` and `t18-power-button-1024@2x.png`
(Retina); accessible names checked by `gallery::tests::every_gallery_control_has_an_accessible_name`.
