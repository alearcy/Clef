---
name: Search list
kind: component
status: shared
impl: gallery
tokens: [field, text, secondary, accent, disabled]
used-in: [window, mixer, multitrack-session]
snapshot: snapshots/egui/t18-search-list-1024.png
api: ui::primitives::search_list::{show, Entry, Outcome}
evidence: [t18-search-list, primitives::search_list::tests::arrows_skip_disabled_matches_and_return_picks_the_highlight, gallery::tests::new_tracks_keeps_the_chosen_instrument]
---

# Search list

## Purpose
Choosing one item from a long list by typing and with the keyboard:
- the New Tracks instrument menu;
- the insert effect picker;
- Choose Instrument.

Added in [T41 round 6](../../specs/T41-session-window-layout.md#round-6--search-and-keyboard-in-plugin-pickers-2026-11-22), user request.

## Anatomy
- A [search field](search-field.md) (glyph and clear button) across the width.
- Under it, a scrolling list of [list rows](list-row.md), at most `max_height` tall.
- The highlighted row uses the list row's selected fill.
- The current choice carries a trailing ✓.
- Disabled rows (busy, quarantined) are `disabled`.
- When nothing matches, a caption replaces the rows.

## Behaviour
- **Opening:** the search field takes focus each time the list is shown after a gap.
- **Typing:** filters by label, ignoring case, so "vst3" matches a format. A new query moves the highlight to the current choice when it matches, else to the first enabled match.
- **↑ / ↓:** move the highlight over enabled matches, stopping at the ends. The list scrolls to keep the highlighted row centred.
- **Return:** picks the highlighted row.
- **Escape:** cancels.
- **Pointer:** a click picks a row, as on any list.
- **Keys:** the four keys are consumed before the field, so its editing never sees them.

## Clef Rust/egui

```rust
pub struct Entry { pub label: String, pub accessible: Option<String>, pub enabled: bool, pub current: bool }
pub enum Outcome { Picked(usize), Cancelled }
pub fn show(ui: &mut egui::Ui, id: egui::Id, search: &mut String, hint: &str,
            entries: &[Entry], max_height: f32, empty: &str) -> Option<Outcome>;
```

[Menu button](menu-button.md) `search_menu_button` hosts it in a menu that clicks inside keep
open. The menu closes on a pick, on a cancel or on a click outside.

## Do / Don't
- Use it for plug-in and other long lists. Use a plain [menu button](menu-button.md) for short fixed choices.
- Don't add a second focus model: the field keeps focus while the arrows move the highlight.
