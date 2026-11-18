---
name: Side drawers
kind: view
status: app
impl: planned
reference-source: [Source/Browser.cpp, Source/PlaylistPanel.cpp, Source/MonitorFxPanel.cpp, Source/PluginList.cpp, Source/PresetList.cpp, Source/AudioFileList.cpp]
components: [list-row, segmented-control, text-field, power-button, chip]
snapshot: snapshots/side-drawers.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Side drawers

Full-height drawers from below the transport to the status row. Left: the
[channel strip](channel-strip.md). Right, one at a time: Playlist, Monitor FX, Browser.

- **Browser** (D94): 248 wide (200–400 by its left edge), × closes, BROWSER chip in the status row.
  Bold title; a `field` well with Tracks | Racks | Plugins | Audio segments (`peacock` active), each with its
  own search. Racks: TEMPLATES and MY RACKS groups, bold categories with ▾/▸ and counts, indented
  presets with FX/Instrument captions. Plugins: FAVOURITES, INSTRUMENTS, EFFECTS by maker, format
  caption, ☆/★ (`solo`). Drags outline their target in `accent` or show a 2-pixel `accent` bar where
  they land. Library path as a 10-px `secondary` footer.
- **Browser › Audio** (D05, `snapshots/browser-audio.png`): a FOLDERS heading, then each folder the composer
  added (per-user settings) as a bold ▾/▸ row with its file count, subfolders indented 12 px, files as
  22-pixel rows captioned "m:ss · FORMAT" (rate and channels in the tooltip). The search filters file names and
  opens the folders holding a match. Cmd-click builds a multi-selection; rows drag onto the arrangement and
  Return imports onto the selected track at the playhead. A top-level folder's menu: Show in Finder, Rescan
  Folder, Remove Folder from Browser (`danger`). Empty state: two lines of `secondary` text and "Add folder…".
  **Details** (D05 refinement, board 4, `snapshots/browser-audio-details.png`): a 158-pixel panel at the foot,
  under a 1-pixel `divider`, replaces the path footer. The selected file's name (12-px bold), then 16-pixel
  label/value rows (label 62 px in `secondary`, value in `text`): Format ("WAV · 24-bit integer", "WAV · 32-bit
  float", "M4A · compressed"), Rate, Channels, Length (h:mm:ss.mmm), Size. A rate other than the project's adds a
  10-px line under Rate: the `warning` glyph in `solo` and "Project runs at 48 kHz: resampled" in `secondary`;
  more than two channels shows "N · can't be imported" in `danger`; a converted format adds "Converted to a 32-bit
  float WAV on import". The folder (home as ~) closes the panel in 10-px `secondary`. Several files: the count and
  the totals of length and size; nothing selected: "Select a file to see its details."
- **Playlist**: `inspectorWidth`, 26-pixel entries (index, colour bar, name, length); the playing
  entry an `accent` outline.
- **Monitor FX** (D60): slot rows painted with the power glyph; excluded from renders.
