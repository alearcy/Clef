---
name: Window frame
kind: component
status: shared
impl: gallery
api: ui::primitives::window_frame::{ToolWindow, frame}
tokens: [panel, divider, text]
used-in: [settings, command-palette, edit-history, multitrack-session, transport-bar]
snapshot: snapshots/egui/t18-window-frame-1024.png
evidence: [t18-window-frame]
---

# Window frame

## Purpose

The in-app frame of Clef's floating tool windows: Audio/MIDI Settings, Commands / Keyboard
Shortcuts, Edit History, Choose Instrument, Quarantined Plugins, file windows, Tempo and Meter Map
and Transport Settings. It also frames the modal confirmation, recovery and effect-picker dialogs.
It is never a repaint of the OS title bar; native window chrome stays untouched.

## API

```rust
pub fn frame() -> egui::Frame; // PANEL fill, 1-point DIVIDER edge, 4-point corners, 12 inset
impl<'a> ToolWindow<'a> {
    pub fn new(title: &'a str) -> Self;              // default width 360
    pub fn width(self, width: f32) -> Self;
    pub fn position(self, position: egui::Pos2) -> Self; // default position
    pub fn show<R>(self, ctx: &egui::Context, open: &mut bool, draw: impl FnOnce(&mut egui::Ui) -> R)
        -> Option<R>;
}
```

## Anatomy and behaviour

An `egui::Window` titled in `BODY` bold `TEXT`, closable, not collapsible, using `frame()`. No
shadow (foundations, depth). Dialogs built on `egui::Modal` pass `frame().inner_margin(16)`. The
window id stays derived from the title, so saved positions survive the migration.

## Usage

```rust
ToolWindow::new("Audio/MIDI Settings").width(580.0).show(ctx, &mut open, |ui| { /* … */ });
```

## Do / Don't

- Don't construct `egui::Window` in a surface; use `ToolWindow`.
- Don't add a shadow, a gradient or a second border.

## Evidence

[Gallery](../snapshots/egui/t18-window-frame-1024.png); every surface snapshot that opens a tool
window (T08 history, T09 arrangement split confirmation).
