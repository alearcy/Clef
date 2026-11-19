---
name: Plugin window
kind: view
status: app
impl: planned
reference-source: [Source/ProjectSession.cpp (HeaderedEditor), Source/SidechainHeader.h]
components: [power-button, menu-button]
snapshot: snapshots/plugin-window-header.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Plugin window

A plugin's own editor is never restyled. Only a plugin with a sidechain input gets a 28-pixel `panel`
header bar above it (D58): a [power button](../components/power-button.md) for the sidechain, then a
`field` selector "Sidechain: <sources>" (or "Sidechain: none") opening the source menu (channels
with Pre-fader and Enabled toggles, Input submenu, None). Each change is one undo step.

## Clef T02 Translation

The approved Rust validation prototype hosts original VST3/CLAP editors in an
eframe-owned window, with no DAW header controls. `crates/plugin-host/src/native.rs` borrows eframe's
NSView and owns a disposable child surface for plugin ABI embedding. Native glue
creates no application window or control. The surface is detached and discarded
at editor close; callbacks from discarded graphics layers are cleared before the
plugin destroys their owners. The SDK VST3 bridge creates/destroys the plugin view
on every open/close and leaves the parent window and event loop to eframe. Host
resizes and plugin resize requests are negotiated on the main thread; queued
requests from a detached view are discarded. No native application window is
constructed by Clef. The broader sidechain/product surface above is outside T02.

The worker requests a hidden window at startup. On editor open it attaches the original view,
applies the viewport geometry, and prepares the embedded view for display before requesting
visibility and focus. Closing the editor cancels a pending reveal. See [T25](../../specs/T25-ui-flicker.md).
On macOS the worker runs as an accessory process so the Dock represents the
main Clef app even when several plugin workers are running.
