---
name: Tool button
kind: component
reference: composer::style::ToolButton
status: shared
impl: gallery
api: ui::primitives::tool_button::tool_button
tokens: [text, secondary, accent, danger, disabled, CAP_LIGHT, LOOP_ACTIVE]
used-in: [tool-row, transport-bar, window]
snapshot: snapshots/egui/t18-tool-button-1024.png
evidence: [t18-tool-button, t34 gallery, primitives::transport::tests]
---

> **Migrated to Rust/egui (T18, 2026-11-21).** The Clef section at the end of this file is the
> implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

# Tool button

## Purpose
Borderless glyph button used by the transport.

## API

```rust
pub fn tool_button(
    ui: &mut egui::Ui,
    glyph: Glyph,
    name: &str,
    enabled: bool,
    active: bool,
    keyboard_focus: bool,
) -> egui::Response
```

Each button uses an independent `TOOL_TARGET` (28 × 28 point) target and one centred
`TOOL_GLYPH` (16 point) [icon](icons.md). Open glyph paths use `TOOL_STROKE` (1.5 points).

## States

| State | Glyph | Target |
| --- | --- | --- |
| Rest | `TEXT` | Transparent |
| Hover | `CAP_LIGHT` | Transparent |
| Pressed | `SECONDARY`, or `DANGER` for Record | Transparent |
| Keyboard focused | Current state colour | Separate `ACCENT` focus circle |
| Disabled or unavailable | `DISABLED` unless lit | Transparent |
| Lit | `ACCENT` for Play or Metronome, `DANGER` for Record, `LOOP_ACTIVE` for Loop | Transparent |

## Behaviour

The caller supplies command availability, active state, and the accessible name and tooltip from
the command registry. Pointer clicks and keyboard activation return a clicked response. The
transport dispatches the command after drawing. Disabled controls keep their accessible name.

## Usage

```rust
if self.transport_tool(ui, Command::ToggleLoop, Glyph::Loop, timeline.looping) {
    chosen = Some(Command::ToggleLoop);
}
```

## Evidence

The [T34 gallery](../../specs/T34-transport-glyphs.md) renders the real controls. Frame tests
cover the pressed and released Record glyph, and workspace tests check disabled behaviour,
keyboard actions, and layout at both widths.

## Clef Rust/egui — T18

T18 moved the primitive from `primitives::transport` to `primitives::tool_button` (re-exported from
`transport` for existing callers). The keyboard focus circle now also follows the shared
keyboard-navigation flag (`paint::keyboard`). See the
[gallery](../snapshots/egui/t18-tool-button-1024.png) for rest, lit and disabled glyphs.

T40 adds the `FitWidth` and `FitHeight` glyphs (two bounds with arrows between them) used by the
arrangement and piano-roll fit buttons.
