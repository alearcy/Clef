---
name: Search field
kind: component
reference: SearchField, paintMatchHighlight, Icon::search, Icon::hidden
status: shared
impl: gallery
reference-source: [UI/ComposerStyleKit/Source/ComposerStyle.cpp, Source/Arrangement.cpp, Source/Mixer.cpp]
tokens: [field, text, secondary, accent, create]
used-in: [arrangement, mixer]
snapshot: snapshots/egui/t18-search-field-1024.png
api: ui::primitives::search_field::SearchField
evidence: [t18-search-field]
---

> **Migrated to Rust/egui (T18 Phase 3, 2026-11-21).** The Clef section at the end of this file is
> the implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

# Search field

## Purpose
Filtering a list by name as you type: the D101 track search in the arrangement's track band and the
mixer's own channel search.

## Anatomy
A raised [text field](text-field.md) 22 high (kept over the recessed, pill and underline variants of
mockup board 5, 2026-11-12):
the `search` glyph in the left indent (`secondary`, `accent` while it filters), the hint text in
`secondary` ("Find track"), the live binding right-aligned in 10-px `disabled` while it is empty
("⌘F", `setShortcutHint`), and while it holds text a 9-pixel `close` glyph
at the right (`secondary`, `text` on hover, pointing-hand cursor), painted by the field: no button tile.
In the arrangement its edge is the header column's edge (so the glass lands on the names' inset) and
the band's "+" is a flat `plus` glyph in `create` orange, in the global lanes' "+" column; in the mixer it starts at the first strip's edge.

## Behaviour
- The host filters on every change; matching is a trimmed, case-insensitive substring
  (`Source/TrackFilter.h`). The matched part of each shown name gets an `accent` wash at 45 %
  (`paintMatchHighlight`).
- Escape clears the text and returns focus to the surface it came from; a press on ✕ clears and keeps focus.
- A click elsewhere on the mixer focuses the mixer itself, never the field (the mixer accepts focus).
- A filter is view state: never saved, never an undo step, cleared when the document changes.
- A row hidden by a view preset (D54) that matches is shown with the `hidden` glyph (an eye crossed
  out) and its name in `secondary`; the arrangement adds a 9-px HIDDEN caption.
- Programmatic text (`setSearchText`) applies at once; JUCE's own change message is asynchronous.

## Do / Don't
- Don't filter the pinned bottom block or the mixer dock.
- Don't add a second search field style: reuse this one (the plugin list's field predates it).

## Clef Rust/egui — T18 Phase 3

**API**

```rust
SearchField::new(&mut text, "Find track").hint(..).shortcut("⌘F").width(240.0).id(id).show(ui) -> egui::Response
```

The [text field](text-field.md) in its search form (`TextField::search`): the 12-point `search`
[icon](icons.md) in the left indent (`secondary`, `accent` while the text is not blank), the hint in
`secondary`, the live binding right-aligned in `caption` `disabled` while empty, and a 9-point `close`
glyph at the right while it holds text (`secondary`, `text` on hover, pointing hand). Escape clears
the text and gives up focus; the clear glyph clears and keeps focus. Accessible: the field is named
by `name`; the clear glyph is a button named "Clear search". Filtering is the host's: trimmed,
case-insensitive substring.

**States:** empty with binding, filtering. **Adoption:** Choose Instrument's search, the Commands /
Keyboard Shortcuts window's search and the command palette field.

**Evidence:** gallery `snapshots/egui/t18-search-field-{1024,760}.png` and `t18-search-field-1024@2x.png`
(Retina); accessible names checked by `gallery::tests::every_gallery_control_has_an_accessible_name`.
