---
name: Quarantine notice
kind: view
status: app
impl: planned
reference-source: [Source/QuarantineViews.h (QuarantineNotice), Source/MainComponent.cpp (showQuarantineNotice)]
components: [notice-box, text-button, icons, caption-and-heading]
snapshot: snapshots/quarantine-notice.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Quarantine notice (D88)

Shown once, over the workspace, when a project opens with quarantined plugins. Approved board
"Opening a project with quarantined plugins" of
[Plugin Quarantine (D88)](https://claude.ai/artifact/5W86cB31xSVHVKFFW5Q2UE).

## Anatomy
A solid `panel` box, 560 wide, centred, with a 1-pixel `divider` edge (the call-out's surface, no
shadow), padded 16: the warning glyph (16, `danger`) and "N plugins are quarantined" at 16 bold; two
lines of `secondary` body text; 11-px uppercase column captions Plugin, Place, Reason; one 22-pixel
row per quarantined slot (plugin in `danger`, place in `text`, reason in `secondary`), at most twelve
then "… and N more"; bottom right "Show quarantined plugins…" and the primary `peacock` "Continue ↩".

## Behaviour
Return, Escape, Continue or a click outside the box closes it. "Show quarantined plugins…" runs that
command (Settings › Plugins) and closes it. While it shows it takes the keyboard. The status row
keeps the one-line form ("2 slots hold quarantined plugins in this project."), which is also the only
report when a plugin is quarantined while the project is open.

## Do / Don't
- The box is the kit [notice box](../components/notice-box.md) since D05, which reuses it for missing audio
  (approved board). Don't reuse it for other warnings without a mockup: it is not a general dialog.
