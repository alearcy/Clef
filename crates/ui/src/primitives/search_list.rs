//! Search list (T41 round 6): a search field over a filtered list of choices that the keyboard
//! drives. Typing filters by label; ↑/↓ move the highlight over enabled matches (it starts on
//! the current choice, else the first match); Return picks the highlighted choice; Escape
//! cancels. The pointer works as on any list. Used by the New Tracks instrument menu, the
//! insert effect picker and Choose Instrument.
use super::{list_row::ListRow, search_field::SearchField};
use eframe::egui::{self, Align, Id, Key, Modifiers, Ui};

/// One choice.
pub struct Entry {
    pub label: String,
    /// Read by assistive technology instead of `label` when set (the plug-in path, say).
    pub accessible: Option<String>,
    pub enabled: bool,
    /// The choice in effect now, ticked and highlighted first.
    pub current: bool,
}

impl Entry {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            accessible: None,
            enabled: true,
            current: false,
        }
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn current(mut self, current: bool) -> Self {
        self.current = current;
        self
    }
    pub fn accessible(mut self, name: impl Into<String>) -> Self {
        self.accessible = Some(name.into());
        self
    }
}

/// What the composer did this frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// The index into the entries given.
    Picked(usize),
    Cancelled,
}

/// The highlight between frames, and the pass it was last shown in.
#[derive(Clone, Default)]
struct State {
    query: String,
    highlight: Option<usize>,
    pass: u64,
}

/// Whether `label` matches `query`, ignoring case.
pub fn matches(label: &str, query: &str) -> bool {
    label.to_lowercase().contains(&query.trim().to_lowercase())
}

/// Draws the search field and the list (at most `max_height` tall). `empty` is the caption
/// shown when nothing matches. Returns what the composer chose, if anything.
pub fn show(
    ui: &mut Ui,
    id: Id,
    search: &mut String,
    hint: &str,
    entries: &[Entry],
    max_height: f32,
    empty: &str,
) -> Option<Outcome> {
    let pass = ui.ctx().cumulative_pass_nr();
    let mut state = ui
        .data(|data| data.get_temp::<State>(id))
        .unwrap_or_default();
    // Shown again after a gap: a fresh opening, which focuses the field.
    let opened = state.pass + 1 < pass;
    let shown: Vec<usize> = (0..entries.len())
        .filter(|&index| matches(&entries[index].label, search))
        .collect();
    let selectable: Vec<usize> = shown
        .iter()
        .copied()
        .filter(|&index| entries[index].enabled)
        .collect();
    if opened || state.query != *search || state.highlight.is_none_or(|h| !selectable.contains(&h))
    {
        state.highlight = selectable
            .iter()
            .copied()
            .find(|&index| entries[index].current)
            .or_else(|| selectable.first().copied());
        state.query.clone_from(search);
    }
    // The keys are read before the field so its editing never sees them.
    let (down, up, enter, escape) = ui.input_mut(|input| {
        (
            input.count_and_consume_key(Modifiers::NONE, Key::ArrowDown),
            input.count_and_consume_key(Modifiers::NONE, Key::ArrowUp),
            input.consume_key(Modifiers::NONE, Key::Enter),
            input.consume_key(Modifiers::NONE, Key::Escape),
        )
    });
    // Each press moves one enabled match, stopping at either end.
    let mut moved = false;
    if down + up > 0 && !selectable.is_empty() {
        let last = selectable.len() as isize - 1;
        let at = state
            .highlight
            .and_then(|h| selectable.iter().position(|&index| index == h))
            .map_or(if down > 0 { -1 } else { last + 1 }, |at| at as isize);
        let next = (at + down as isize - up as isize).clamp(0, last);
        state.highlight = Some(selectable[next as usize]);
        moved = true;
    }
    let mut outcome = if escape {
        Some(Outcome::Cancelled)
    } else if enter {
        state.highlight.map(Outcome::Picked)
    } else {
        None
    };
    let field = SearchField::new(search, hint).id(id.with("query")).show(ui);
    if opened {
        field.request_focus();
    }
    egui::ScrollArea::vertical()
        .id_salt(id.with("list"))
        .max_height(max_height)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for &index in &shown {
                let entry = &entries[index];
                let mut row = ListRow::new(&entry.label)
                    .enabled(entry.enabled)
                    .selected(state.highlight == Some(index));
                if entry.current {
                    row = row.trailing("✓");
                }
                if let Some(name) = &entry.accessible {
                    row = row.accessible(name);
                }
                let response = row.show(ui);
                if moved && state.highlight == Some(index) {
                    response.scroll_to_me(Some(Align::Center));
                }
                if response.clicked() {
                    outcome = Some(Outcome::Picked(index));
                }
            }
            if shown.is_empty() {
                super::text::caption(ui, empty);
            }
        });
    state.pass = pass;
    ui.data_mut(|data| data.insert_temp(id, state));
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(
        ctx: &egui::Context,
        search: &mut String,
        entries: &[Entry],
        keys: &[Key],
    ) -> Option<Outcome> {
        let mut outcome = None;
        let events = keys
            .iter()
            .map(|&key| egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            })
            .collect();
        let _ = ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                outcome = show(
                    ui,
                    Id::new("list"),
                    search,
                    "Search",
                    entries,
                    200.0,
                    "None",
                );
            },
        );
        outcome
    }

    #[test]
    fn arrows_skip_disabled_matches_and_return_picks_the_highlight() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let entries = [
            Entry::new("Kontakt 7 · VST3"),
            Entry::new("Opus · VST3").current(true),
            Entry::new("Falcon · VST3").enabled(false),
            Entry::new("SINE Player · VST3"),
        ];
        let mut search = String::new();
        assert_eq!(run(&ctx, &mut search, &entries, &[]), None);
        // The highlight starts on the current choice, then skips the disabled one.
        assert_eq!(
            run(&ctx, &mut search, &entries, &[Key::ArrowDown, Key::Enter]),
            Some(Outcome::Picked(3))
        );
        assert_eq!(
            run(
                &ctx,
                &mut search,
                &entries,
                &[Key::ArrowUp, Key::ArrowUp, Key::Enter]
            ),
            Some(Outcome::Picked(0))
        );
        // Typing filters and moves the highlight to the first match.
        search.push_str("si");
        assert_eq!(
            run(&ctx, &mut search, &entries, &[Key::Enter]),
            Some(Outcome::Picked(3))
        );
        assert_eq!(
            run(&ctx, &mut search, &entries, &[Key::Escape]),
            Some(Outcome::Cancelled)
        );
        search.push_str("zzz");
        assert_eq!(run(&ctx, &mut search, &entries, &[Key::Enter]), None);
    }
}
