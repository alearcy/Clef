---
name: Settings
kind: view
status: app
impl: planned
reference-source: [Source/SettingsPanel.h, Source/AudioSettingsPanel.h, Source/StartupChooser.h (StartupModeChoice)]
components: [list-row, text-field, segmented-control, menu-button, checkbox, caption-and-heading]
snapshot: snapshots/settings.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Settings (D96)

One Settings… window. A `panel` sidebar (180) with the search field and categories (Audio, MIDI,
Plugins, Editing, Automation, Keyboard shortcuts, Startup); the selected one a `field` row with the
selection edge; a match count in 10-px `secondary` while searching. On `background`, the page under a
16-px heading, or "Results for …" grouped by 11-px `secondary` category names. Rows: a 160-pixel
`secondary` label, the control, an optional 11-px caption under it. Spacing, never rules.

## Plugins › Quarantined plugins (D88)
Between Preferred format and the scanned list: the 160-pixel `secondary` label with an 11-px caption,
11-px uppercase column captions (Plugin, Format, Reason, Since), 26-pixel [list rows](../components/list-row.md)
(five visible, then the list scrolls; the selected row `field` with the selection edge), then **Retry**
and **Release** [text buttons](../components/text-button.md), disabled with nothing selected, and an
11-px caption. Empty: "No plugin is quarantined." in `disabled`. The selected row is the target of
the Retry and Release commands the buttons run. Snapshot: `snapshots/quarantine-list.png`.

## MIDI page: automatic monitoring (D67)
Below Chase and the tap trigger, **Automatic MIDI monitoring** is a four-way segmented control
(Manual · Arm · Selection · Arm or selection) with its caption. The D67 board drew a menu button; the
segmented control wins because the library gives up to four exclusive choices to it.

## Editing page: anti-click fades (D70)
Below Chord gestures, **Anti-click fades** is a [checkbox](../components/checkbox.md) labelled "On" with its
11-px caption; it runs the Anti-click Fades command (its tooltip shows the live binding). Snapshot:
`snapshots/settings-anti-click.png`.
