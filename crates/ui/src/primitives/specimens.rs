//! T18 primitive gallery: every primitive in every state, on one stage per primitive family.
//! Hover and pressed bodies are painted through the same `paint::control_body` the widgets use,
//! because the gallery cannot hold a pointer over several controls at once.
use super::{
    checkbox::checkbox,
    chip::chip,
    glyph_toggle::glyph_toggle,
    icons::Icon,
    list_row::ListRow,
    menu::{self, Item},
    menu_bar,
    menu_button::{choice, menu_button},
    notice_box::NoticeBox,
    power_button::power_button,
    range_overlay::range_overlay,
    scroll_zoom_bars::{self, Axis},
    search_field::SearchField,
    segment_display::segment_display,
    segmented::Segmented,
    slot_button::slot,
    tabs::tabs,
    text,
    text_button::TextButton,
    text_field::TextField,
    tool_button::tool_button,
    track_state::record_arm,
    value_slider::ValueSlider,
    window_frame::ToolWindow,
};
use crate::{commands::Command, palette};
use crate::{
    paint::{self, BodyState, Depth},
    theme::{self, metric, text as style},
};
use clef_model::mixer::Control;
use eframe::egui::{self, Align2, Context, Ui, vec2};

pub const CASES: &[&str] = &[
    "text-button",
    "tool-button",
    "chip",
    "menu-button",
    "text-field",
    "caption",
    "list-row",
    "selection-edge",
    "scroll-zoom-bars",
    "notice-box",
    "notice-question",
    "window-frame",
    "menu",
    "menu-bar",
    "segmented",
    "value-slider",
    "command-palette",
    "icons",
    "checkbox",
    "tabs",
    "glyph-toggle",
    "track-state",
    "search-field",
    "segment-display",
    "range-overlay",
    "fader",
    "knob",
    "level-meter",
    "mute-solo",
    "power-button",
    "slot-button",
];

/// Cases drawn as floating windows or modal overlays; the one-page view opens them on request.
const OVERLAYS: [&str; 4] = [
    "notice-box",
    "notice-question",
    "window-frame",
    "command-palette",
];

/// Persistent text for the field specimens.
#[derive(Default)]
pub struct Specimens {
    /// The gallery stage: menus that a screenshot must show are held open.
    screenshot: bool,
    /// The overlay cases the one-page view currently shows.
    overlays: Vec<&'static str>,
    fields: [String; 3],
    searches: [String; 2],
    checks: [bool; 3],
    tab: usize,
    segments: [u8; 3],
    values: (u16, u16, usize),
    palette: Option<palette::Palette>,
}

fn state_row(ui: &mut Ui, label: &str, draw: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        let (rect, _) =
            ui.allocate_exact_size(vec2(120.0, metric::CONTROL_HEIGHT), egui::Sense::hover());
        ui.painter().text(
            rect.left_center(),
            Align2::LEFT_CENTER,
            label,
            style::font(style::SMALL),
            theme::SECONDARY,
        );
        draw(ui);
    });
}

/// A body painted in a forced state with a centred label, for hover and pressed specimens.
fn forced(ui: &mut Ui, label: &str, state: BodyState) {
    let width = ui
        .painter()
        .layout_no_wrap(label.into(), style::font(style::BODY), theme::TEXT)
        .size()
        .x
        + metric::CONTROL_PADDING * 2.0;
    let (rect, _) =
        ui.allocate_exact_size(vec2(width, metric::CONTROL_HEIGHT), egui::Sense::hover());
    let body = paint::control_body(ui.painter(), rect, Depth::Raised, state);
    let colour = state.lit.map_or(theme::TEXT, theme::foreground_on);
    ui.painter().text(
        body.center(),
        Align2::CENTER_CENTER,
        label,
        style::font(style::BODY),
        colour,
    );
}

impl Specimens {
    /// The specimens of the screenshot gallery, with menus held open.
    pub fn for_screenshots() -> Self {
        Self {
            screenshot: true,
            ..Self::default()
        }
    }

    /// Closes an overlay of the one-page view once it reports that it closed.
    fn close_on(&mut self, case: &str, closed: bool) {
        if closed {
            self.overlays.retain(|open| *open != case);
            if case == "command-palette" {
                self.palette = None;
            }
        }
    }

    /// Every case on one page, in gallery order (debug window). Overlay cases get a button that
    /// shows or hides them, since they draw over the whole window.
    pub fn show_all(&mut self, ui: &mut Ui, ctx: &Context) {
        for case in CASES {
            if OVERLAYS.contains(case) {
                let shown = self.overlays.contains(case);
                egui::Frame::new()
                    .fill(theme::BACKGROUND)
                    .inner_margin(metric::WORKSPACE_PADDING)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        text::heading(ui, case);
                        let label = if shown { "Hide" } else { "Show" };
                        if TextButton::new(label).toggle(shown).show(ui).clicked() {
                            if shown {
                                self.overlays.retain(|open| open != case);
                            } else {
                                self.overlays.push(case);
                            }
                        }
                        if shown {
                            self.case(ui, ctx, case);
                        }
                    });
            } else {
                ui.push_id(case, |ui| {
                    ui.set_width(ui.available_width());
                    self.show(ui, ctx, case);
                });
            }
            ui.separator();
        }
    }

    /// Draws one case's content without its stage.
    fn case(&mut self, ui: &mut Ui, ctx: &Context, case: &str) {
        match case {
            "text-button" => text_buttons(ui),
            "tool-button" => tool_buttons(ui),
            "chip" => chips(ui),
            "menu-button" => menu_buttons(ui),
            "text-field" => self.text_fields(ui),
            "caption" => captions(ui),
            "list-row" => list_rows(ui),
            "selection-edge" => selection_edges(ui),
            "scroll-zoom-bars" => scroll_bars(ui),
            "notice-box" => self.close_on(case, notice(ctx)),
            "notice-question" => self.close_on(case, question(ctx)),
            "window-frame" => self.close_on(case, window(ctx)),
            "menu" => menus(ui),
            "menu-bar" => bar(ui, self.screenshot),
            "segmented" => self.segmented(ui),
            "value-slider" => self.value_sliders(ui),
            "command-palette" => {
                let closed = self.palette(ctx);
                self.close_on(case, closed);
            }
            "icons" => icons(ui),
            "checkbox" => self.checkboxes(ui),
            "tabs" => self.tabs(ui),
            "glyph-toggle" => glyph_toggles(ui),
            "track-state" => track_states(ui),
            "search-field" => self.search_fields(ui),
            "segment-display" => segment_displays(ui),
            "range-overlay" => range_overlays(ui),
            "fader" => mixer_controls(ui, &[Control::Fader]),
            "knob" => mixer_controls(ui, &[Control::Gain, Control::Pan, Control::Width]),
            "level-meter" => level_meters(ui),
            "mute-solo" => mute_solo(ui),
            "power-button" => power_buttons(ui),
            "slot-button" => slot_buttons(ui),
            _ => {}
        }
    }

    pub fn show(&mut self, ui: &mut Ui, ctx: &Context, case: &str) {
        egui::Frame::new()
            .fill(theme::BACKGROUND)
            .inner_margin(metric::WORKSPACE_PADDING)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 10.0);
                text::heading(ui, case);
                ui.add_space(4.0);
                self.case(ui, ctx, case);
            });
    }

    fn checkboxes(&mut self, ui: &mut Ui) {
        let [off, on, disabled] = &mut self.checks;
        *on = true;
        *disabled = true;
        state_row(ui, "off", |ui| {
            checkbox(ui, "Skip first note", off, true);
        });
        state_row(ui, "on", |ui| {
            checkbox(ui, "Skip first note", on, true);
        });
        state_row(ui, "disabled, on", |ui| {
            checkbox(ui, "Skip first note", disabled, false);
        });
    }

    fn tabs(&mut self, ui: &mut Ui) {
        state_row(ui, "first shown", |ui| {
            tabs(
                ui,
                "Lower panel",
                &["Editor", "Mixer", "FX Racks"],
                &mut self.tab,
            );
        });
        state_row(ui, "middle shown", |ui| {
            let mut shown = 1;
            tabs(
                ui,
                "Lower panel two",
                &["Editor", "Mixer", "FX Racks"],
                &mut shown,
            );
        });
    }

    fn search_fields(&mut self, ui: &mut Ui) {
        let [empty, filtering] = &mut self.searches;
        if filtering.is_empty() {
            *filtering = "str".into();
        }
        state_row(ui, "empty, binding", |ui| {
            SearchField::new(empty, "Find track")
                .shortcut("⌘F")
                .width(240.0)
                .show(ui);
        });
        state_row(ui, "filtering", |ui| {
            SearchField::new(filtering, "Find track")
                .width(240.0)
                .show(ui);
        });
    }

    fn segmented(&mut self, ui: &mut Ui) {
        let options = [(0, "Tracks"), (1, "Racks"), (2, "Plugins")];
        let [first, second, disabled] = &mut self.segments;
        *second = 1;
        state_row(ui, "first lit", |ui| {
            Segmented::new("Browser", first, &options).show(ui);
        });
        state_row(ui, "middle lit", |ui| {
            Segmented::new("Browser", second, &options).show(ui);
        });
        state_row(ui, "two options", |ui| {
            let mut project = true;
            Segmented::new(
                "Applies to",
                &mut project,
                &[(false, "All projects"), (true, "This project")],
            )
            .show(ui);
        });
        state_row(ui, "disabled", |ui| {
            Segmented::new("Browser", disabled, &options)
                .enabled(false)
                .show(ui);
        });
    }

    fn value_sliders(&mut self, ui: &mut Ui) {
        let (tempo, disabled, revision) = &mut self.values;
        if *tempo == 0 {
            *tempo = 120;
            *disabled = 96;
            *revision = 2;
        }
        state_row(ui, "rest, unit", |ui| {
            ValueSlider::new("Tempo", tempo, 20..=400)
                .unit("BPM")
                .default_value(120)
                .show(ui);
        });
        state_row(ui, "disabled", |ui| {
            ValueSlider::new("Tempo", disabled, 20..=400)
                .unit("BPM")
                .enabled(false)
                .show(ui);
        });
        state_row(ui, "track form", |ui| {
            ValueSlider::new("Revision", revision, 0..=5)
                .track()
                .width(264.0)
                .show(ui);
        });
        state_row(ui, "track, disabled", |ui| {
            let mut value = 3_usize;
            ValueSlider::new("Revision", &mut value, 0..=5)
                .track()
                .width(264.0)
                .enabled(false)
                .show(ui);
        });
    }

    /// True when the palette closed (Escape, a click outside or a picked row).
    fn palette(&mut self, ctx: &Context) -> bool {
        let palette = self.palette.get_or_insert_with(|| {
            palette::Palette::new(None, false).with_id(egui::Id::new("specimen-palette"))
        });
        let rows: Vec<palette::Row> = [
            (Command::PlayStop, "Space", true),
            (Command::ReturnToStart, "", true),
            (Command::Record, "", true),
            (Command::DuplicateClips, "⌘D", false),
            (Command::SplitClips, "⌘E", false),
            (Command::Save, "⌘S", true),
            (Command::Palette, "⇧⌘P", true),
        ]
        .into_iter()
        .map(|(command, shortcut, enabled)| palette::Row {
            command,
            label: command.title().into(),
            shortcut: shortcut.into(),
            enabled,
        })
        .collect();
        palette.show(ctx, &rows).is_some()
    }

    fn text_fields(&mut self, ui: &mut Ui) {
        // The focused edge is covered by a frame test: the gallery window never holds OS focus.
        let [empty, filled, disabled] = &mut self.fields;
        if filled.is_empty() {
            *filled = "Strings".into();
            *disabled = "Piano".into();
        }
        state_row(ui, "empty, hint", |ui| {
            TextField::new(empty, "Search")
                .hint("Search plugins")
                .width(240.0)
                .show(ui);
        });
        state_row(ui, "rest", |ui| {
            TextField::new(filled, "Track name").width(240.0).show(ui);
        });
        state_row(ui, "disabled", |ui| {
            TextField::new(disabled, "Track name")
                .enabled(false)
                .width(240.0)
                .show(ui);
        });
    }
}

fn text_buttons(ui: &mut Ui) {
    state_row(ui, "rest", |ui| {
        TextButton::new("Save").show(ui);
        TextButton::new("Open Project…").show(ui);
    });
    state_row(ui, "hover", |ui| {
        forced(
            ui,
            "Save",
            BodyState {
                hovered: true,
                ..BodyState::default()
            },
        )
    });
    state_row(ui, "pressed", |ui| {
        forced(
            ui,
            "Save",
            BodyState {
                pressed: true,
                ..BodyState::default()
            },
        )
    });
    state_row(ui, "focused", |ui| {
        forced(
            ui,
            "Save",
            BodyState {
                focused: true,
                ..BodyState::default()
            },
        )
    });
    state_row(ui, "disabled", |ui| {
        TextButton::new("Save").enabled(false).show(ui);
    });
    state_row(ui, "primary", |ui| {
        TextButton::new("Open ↩").primary(true).show(ui);
    });
    state_row(ui, "toggle off / on", |ui| {
        TextButton::new("REC Merge").toggle(false).show(ui);
        TextButton::new("REC Merge").toggle(true).show(ui);
    });
    state_row(ui, "small, min width", |ui| {
        TextButton::new("TAP").small().min_width(44.0).show(ui);
        TextButton::new("READ")
            .small()
            .min_width(44.0)
            .toggle(true)
            .show(ui);
    });
}

fn tool_buttons(ui: &mut Ui) {
    let glyphs = [
        Icon::Start,
        Icon::Play,
        Icon::Stop,
        Icon::Record,
        Icon::Loop,
        Icon::Metronome,
    ];
    state_row(ui, "rest", |ui| {
        for glyph in glyphs {
            tool_button(ui, glyph, "Specimen", true, false, false);
        }
    });
    state_row(ui, "lit", |ui| {
        for glyph in glyphs {
            tool_button(ui, glyph, "Specimen", true, true, false);
        }
    });
    state_row(ui, "disabled", |ui| {
        for glyph in glyphs {
            tool_button(ui, glyph, "Specimen", false, false, false);
        }
    });
}

fn chips(ui: &mut Ui) {
    state_row(ui, "off", |ui| {
        chip(ui, "T 3", "Show 3 take lanes", false, true);
        chip(ui, "A 2", "Show 2 automation lanes", false, true);
    });
    state_row(ui, "on", |ui| {
        chip(ui, "T 3", "Hide 3 take lanes", true, true);
        chip(ui, "1 BAR", "Count-in", true, true);
    });
    state_row(ui, "disabled", |ui| {
        chip(ui, "T 3", "Take lanes unavailable", false, false);
    });
}

fn menu_buttons(ui: &mut Ui) {
    let choices = ["Bar", "Beat", "1/8", "1/16"];
    state_row(ui, "value", |ui| {
        menu_button(ui, "snap", "Snap", None, "Beat", true, |ui| {
            for option in choices {
                choice(ui, option, option == "Beat");
            }
        });
    });
    state_row(ui, "caption", |ui| {
        menu_button(ui, "rate", "Rate", Some("Rate"), "48 000 Hz", true, |_| {});
    });
    state_row(ui, "disabled", |ui| {
        menu_button(ui, "mode", "Record mode", None, "Merge", false, |_| {});
    });
}

fn captions(ui: &mut Ui) {
    text::heading(ui, "Page heading");
    text::section(ui, "Section heading");
    text::body(ui, "Body text: track names, rows and controls.");
    text::caption(ui, "Caption: maker · format, path, length.");
    text::unavailable(ui, "Unavailable: — (no device)");
    text::danger(ui, "Danger: the plugin stopped responding.");
}

fn list_rows(ui: &mut Ui) {
    let width = 320.0;
    ui.spacing_mut().item_spacing.y = 0.0;
    ListRow::new("Glass Strings").width(width).show(ui);
    ListRow::new("Glass Strings")
        .selected(true)
        .width(width)
        .show(ui);
    ListRow::new("Missing Synth (Not found)")
        .available(false)
        .width(width)
        .show(ui);
    ListRow::new("Disabled row")
        .enabled(false)
        .width(width)
        .show(ui);
    ui.add_space(8.0);
    ListRow::new("Song.clef")
        .detail("~/Music/Clef · 2 hours ago")
        .height(44.0)
        .width(width)
        .show(ui);
    ListRow::new("Strings")
        .detail("No instrument · Empty")
        .height(44.0)
        .selected(true)
        .header()
        .width(width)
        .show(ui);
}

fn selection_edges(ui: &mut Ui) {
    for (label, fill) in [
        ("header on panel", theme::PANEL),
        ("row on field", theme::FIELD),
    ] {
        state_row(ui, label, |ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(240.0, 44.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 2, fill);
            paint::selection_edge(ui.painter(), rect);
        });
    }
}

fn scroll_bars(ui: &mut Ui) {
    let width = ui.available_width().min(640.0);
    for (label, axis, enabled) in [
        (
            "start",
            Axis {
                offset: 0.0,
                visible: 200.0,
                total: 800.0,
            },
            true,
        ),
        (
            "middle",
            Axis {
                offset: 300.0,
                visible: 200.0,
                total: 800.0,
            },
            true,
        ),
        (
            "everything visible",
            Axis {
                offset: 0.0,
                visible: 800.0,
                total: 600.0,
            },
            true,
        ),
        (
            "disabled",
            Axis {
                offset: 100.0,
                visible: 200.0,
                total: 800.0,
            },
            false,
        ),
    ] {
        text::caption(ui, label);
        ui.allocate_ui(vec2(width, metric::SCROLL_BAR), |ui| {
            scroll_zoom_bars::horizontal(ui, label, axis, enabled);
        });
    }
    ui.allocate_ui(vec2(metric::SCROLL_BAR, 180.0), |ui| {
        scroll_zoom_bars::vertical(
            ui,
            "Pitch",
            Axis {
                offset: 40.0,
                visible: 32.0,
                total: 128.0,
            },
            true,
        );
    });
}

/// True when the notice was answered or dismissed.
fn notice(ctx: &Context) -> bool {
    let rows = [
        [
            "Glass Strings".to_owned(),
            "Strings".to_owned(),
            "Crashed during load".to_owned(),
        ],
        [
            "Old Piano".to_owned(),
            "Piano".to_owned(),
            "Stopped responding".to_owned(),
        ],
    ];
    NoticeBox::new(
        "specimen-notice",
        "This project contains quarantined plugins",
        "Saved states are retained. These instruments will not load until explicitly reactivated.",
        "Continue ↩",
    )
    .rows(&["Plugin", "Track", "Reason"], &rows)
    .secondary("Show Quarantined Plugins…")
    .show(ctx)
    .is_some()
}

/// True when the question was answered or dismissed.
fn question(ctx: &Context) -> bool {
    NoticeBox::new(
        "specimen-question",
        "Render ARA edits first?",
        "The plugin's edits are applied to a new audio file before Reverse runs.",
        "Render and Reverse ↩",
    )
    .question()
    .secondary("Cancel")
    .show(ctx)
    .is_some()
}

/// True when the window's close button was pressed.
fn window(ctx: &Context) -> bool {
    let mut open = true;
    ToolWindow::new("Audio/MIDI Settings")
        .width(420.0)
        .position(egui::pos2(80.0, 80.0))
        .show(ctx, &mut open, |ui| {
            text::section(ui, "Output");
            text::body(ui, "Built-in Output · 48 000 Hz");
            text::caption(ui, "Latency: 128 smp / 2.67 ms");
            ui.horizontal(|ui| TextButton::new("Close").show(ui).clicked())
                .inner
        })
        .unwrap_or(false)
        || !open
}

fn menu_rows(ui: &mut Ui) {
    Item::new("Undo Add Track").shortcut("⌘Z").show(ui);
    Item::new("Redo").shortcut("⇧⌘Z").enabled(false).show(ui);
    Item::new("Duplicate")
        .shortcut("⌘D")
        .highlighted(true)
        .show(ui);
    Item::new("Delete").shortcut("⌫").show(ui);
    menu::separator(ui);
    menu::caption(ui, "Record Mode");
    Item::new("Record Mode: Merge").ticked(true).show(ui);
    Item::new("Record Mode: Replace").ticked(false).show(ui);
    Item::new("Record Mode: New take")
        .ticked(false)
        .enabled(false)
        .show(ui);
}

fn menus(ui: &mut Ui) {
    // States: rest with shortcut, disabled, highlighted, destructive, separator, caption, ticks.
    menu::frame().show(ui, |ui| {
        menu::begin(ui);
        ui.set_max_width(280.0);
        menu_rows(ui);
    });
}

/// `held` keeps the Transport menu open for the screenshot; the one-page view leaves it to clicks.
fn bar(ui: &mut Ui, held: bool) {
    let id = egui::Id::new("specimen-menu-bar");
    if held {
        menu_bar::open(ui.ctx(), id, 3);
    }
    menu_bar::menu_bar(
        ui,
        id,
        &["File", "Edit", "Track", "Transport", "View", "Window"],
        |_, ui| {
            Item::new("Play / Stop").shortcut("Space").show(ui);
            Item::new("Return to Start").show(ui);
            menu::separator(ui);
            Item::new("Loop").ticked(true).show(ui);
            Item::new("Metronome").ticked(false).show(ui);
            menu::caption(ui, "Stop Mode");
            Item::new("Stop Mode: Keep Position").ticked(true).show(ui);
            Item::new("Stop Mode: Return to Play Start")
                .ticked(false)
                .show(ui);
            None::<()>
        },
        |ui| {
            TextButton::new("Settings").show(ui);
            TextButton::new("Commands…").show(ui);
            None
        },
    );
}

fn icons(ui: &mut Ui) {
    for (label, colour) in [
        ("text", theme::TEXT),
        ("secondary", theme::SECONDARY),
        ("accent", theme::ACCENT),
        ("disabled", theme::DISABLED),
    ] {
        state_row(ui, label, |ui| {
            for icon in Icon::ALL {
                let (rect, _) = ui.allocate_exact_size(
                    vec2(metric::TOOL_TARGET, metric::TOOL_TARGET),
                    egui::Sense::hover(),
                );
                icon.paint(ui.painter(), rect.center(), metric::TOOL_GLYPH, colour);
            }
        });
    }
}

fn glyph_toggles(ui: &mut Ui) {
    for (label, lit, enabled) in [
        ("rest", false, true),
        ("lit", true, true),
        ("disabled", false, false),
        ("disabled, lit", true, false),
    ] {
        state_row(ui, label, |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            glyph_toggle(ui, Icon::Triplet, "Triplet", lit, enabled);
            glyph_toggle(ui, Icon::Dotted, "Dotted", false, enabled);
        });
    }
}

fn track_states(ui: &mut Ui) {
    for (label, enabled, armed, recording) in [
        ("off", true, false, false),
        ("armed", true, true, false),
        ("recording", true, true, true),
        ("disabled", false, false, false),
    ] {
        state_row(ui, label, |ui| {
            record_arm(ui, "Piano", enabled, armed, recording, true);
        });
    }
}

fn segment_displays(ui: &mut Ui) {
    state_row(ui, "stopped", |ui| {
        segment_display(ui, "Position", "1.01.0000", false, 120.0);
    });
    state_row(ui, "running", |ui| {
        segment_display(ui, "Position", "12.03.0480", false, 120.0);
    });
    state_row(ui, "recording", |ui| {
        segment_display(ui, "Position", "12.03.0480", true, 120.0);
    });
    state_row(ui, "time", |ui| {
        segment_display(ui, "Time", "-0:01.250", false, 120.0);
    });
}

fn range_overlays(ui: &mut Ui) {
    state_row(ui, "marquee", |ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(320.0, 60.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 0.0, theme::CANVAS);
        range_overlay(ui.painter(), rect.shrink2(vec2(40.0, 12.0)), None);
    });
    state_row(ui, "with readout", |ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(320.0, 60.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 0.0, theme::CANVAS);
        range_overlay(
            ui.painter(),
            rect.shrink2(vec2(40.0, 6.0)),
            Some("Take 2 · 12.1.000 → 13.3.000"),
        );
    });
}

/// Every given control at its default, a low and a high value, and disabled.
fn mixer_controls(ui: &mut Ui, controls: &[Control]) {
    use super::mixer::Slider;
    for (index, control) in controls.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            let (min, max) = control.bounds();
            for (column, (label, value, enabled)) in [
                ("default", control.initial(), true),
                ("low", min + (max - min) / 5, true),
                ("high", max, true),
                ("disabled", control.initial(), false),
            ]
            .into_iter()
            .enumerate()
            {
                ui.push_id((index, column), |ui| {
                    ui.vertical(|ui| {
                        ui.add_enabled_ui(enabled, |ui| {
                            let _ = Slider {
                                control: *control,
                                value,
                                drag_start: None,
                                name: "Specimen",
                                master: false,
                                compact: false,
                            }
                            .show(ui);
                        });
                        ui.label(
                            egui::RichText::new(format!("{} {label}", control.label()))
                                .size(style::SMALL)
                                .color(theme::SECONDARY),
                        );
                    });
                });
            }
        });
    }
}

fn level_meters(ui: &mut Ui) {
    use clef_session::Meter;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        for (index, reading) in [
            Meter::default(),
            Meter {
                level: [-180, -240],
                hold: [-120, -150],
                clipped: false,
            },
            Meter {
                level: [40, 20],
                hold: [60, 50],
                clipped: true,
            },
        ]
        .into_iter()
        .enumerate()
        {
            ui.push_id(index, |ui| {
                super::level_meter::strip_meter(ui, "Specimen", reading, "");
                super::level_meter::header_meter(ui, 44.0, "Specimen", reading);
            });
        }
    });
}

fn mute_solo(ui: &mut Ui) {
    for (label, lit, enabled) in [
        ("off", false, true),
        ("lit", true, true),
        ("disabled", false, false),
        ("disabled, lit", true, false),
    ] {
        state_row(ui, label, |ui| {
            ui.push_id(label, |ui| {
                ui.horizontal(|ui| {
                    super::mute_solo::lamp(ui, "M", "Mute Specimen", false, lit, enabled);
                    super::mute_solo::lamp(ui, "S", "Solo Specimen", true, lit, enabled);
                });
            });
        });
    }
}

fn power_buttons(ui: &mut Ui) {
    for (label, lit, ready) in [
        ("on", true, true),
        ("bypassed", false, true),
        ("not ready", false, false),
    ] {
        state_row(ui, label, |ui| {
            power_button(ui, 14.0, 20.0, &format!("Specimen {label}"), lit, ready);
        });
    }
}

fn slot_buttons(ui: &mut Ui) {
    use super::slot_button::State;
    for (label, state, description) in [
        ("active", State::Active, "Active"),
        ("late", State::Late, "Late: 12 % of blocks"),
        ("bypassed", State::Bypassed, "Bypassed"),
        ("missing", State::Missing, "Missing"),
        ("failed", State::Failed, "Failed to load"),
        ("empty", State::Empty, "Empty"),
    ] {
        state_row(ui, label, |ui| {
            ui.push_id(label, |ui| {
                ui.set_width(200.0);
                let _ = slot(ui, "Tape EQ", "Specimen", "insert", state, description, 4.0);
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::accesskit::{Node, Role};

    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Runs `draw` for a few frames with `events` on the last one; returns the accesskit nodes.
    fn run(ctx: &Context, events: Vec<egui::Event>, mut draw: impl FnMut(&mut Ui)) -> Vec<Node> {
        let mut nodes = Vec::new();
        for frame in 0..3 {
            let output = ctx.run_ui(
                egui::RawInput {
                    events: if frame == 2 {
                        events.clone()
                    } else {
                        Vec::new()
                    },
                    ..Default::default()
                },
                |ui| draw(ui),
            );
            nodes = output
                .platform_output
                .accesskit_update
                .map(|tree| tree.nodes.into_iter().map(|(_, node)| node).collect())
                .unwrap_or_default();
        }
        nodes
    }

    fn context() -> Context {
        let ctx = Context::default();
        theme::install_fonts(&ctx);
        ctx.enable_accesskit();
        ctx
    }

    #[test]
    fn tabs_move_with_the_arrows_and_name_each_tab() {
        let ctx = context();
        let mut shown = 0;
        let nodes = run(&ctx, vec![], |ui| {
            tabs(ui, "Panel", &["Editor", "Mixer"], &mut shown);
        });
        assert!(nodes.iter().any(|n| n.label() == Some("Mixer")));
        let mut focus = egui::Id::NULL;
        let _ = ctx.run_ui(Default::default(), |ui| {
            focus = ui.id().with(("clef-tabs", "Panel")).with(0);
        });
        ctx.memory_mut(|memory| memory.request_focus(focus));
        run(&ctx, vec![key(egui::Key::ArrowRight)], |ui| {
            tabs(ui, "Panel", &["Editor", "Mixer"], &mut shown);
        });
        assert_eq!(shown, 1);
    }

    #[test]
    fn checkbox_flips_on_click_and_reports_its_state() {
        let ctx = context();
        let mut value = false;
        let mut rect = egui::Rect::NOTHING;
        run(&ctx, vec![], |ui| {
            rect = checkbox(ui, "Skip first note", &mut value, true).rect;
        });
        let at = rect.center();
        let click = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        run(
            &ctx,
            vec![egui::Event::PointerMoved(at), click(true)],
            |ui| {
                checkbox(ui, "Skip first note", &mut value, true);
            },
        );
        let nodes = run(&ctx, vec![click(false)], |ui| {
            checkbox(ui, "Skip first note", &mut value, true);
        });
        assert!(value);
        assert!(
            nodes
                .iter()
                .any(|n| n.role() == Role::CheckBox && n.label() == Some("Skip first note"))
        );
    }

    #[test]
    fn search_field_escape_and_clear_empty_the_text() {
        let ctx = context();
        let id = egui::Id::new("search");
        let mut text = String::from("str");
        let nodes = run(&ctx, vec![], |ui| {
            SearchField::new(&mut text, "Find track")
                .id(id)
                .width(240.0)
                .show(ui);
        });
        assert!(nodes.iter().any(|n| n.label() == Some("Clear search")));
        assert!(nodes.iter().any(|n| n.label() == Some("Find track")));
        ctx.memory_mut(|memory| memory.request_focus(id));
        run(&ctx, vec![key(egui::Key::Escape)], |ui| {
            SearchField::new(&mut text, "Find track")
                .id(id)
                .width(240.0)
                .show(ui);
        });
        assert!(text.is_empty());
    }

    #[test]
    fn one_page_view_draws_every_inline_case_and_opens_overlays_on_request() {
        let ctx = context();
        let mut specimens = Specimens::default();
        let page = |specimens: &mut Specimens| {
            run(&ctx, vec![], |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| specimens.show_all(ui, &ctx));
            })
        };
        let nodes = page(&mut specimens);
        let has = |nodes: &[Node], label: &str| nodes.iter().any(|n| n.label() == Some(label));
        for label in ["Skip first note", "Find track", "Position", "Triplet"] {
            assert!(has(&nodes, label), "missing {label}");
        }
        assert!(
            !has(&nodes, "Search commands"),
            "overlays stay closed until asked"
        );
        assert!(
            !has(&nodes, "Play / Stop"),
            "the specimen menu bar stays closed until clicked"
        );
        specimens.overlays.push("command-palette");
        let nodes = page(&mut specimens);
        assert!(has(&nodes, "Search commands"));
        // Escape closes each overlay and the page forgets it.
        for overlay in ["command-palette", "notice-box", "notice-question"] {
            if !specimens.overlays.contains(&overlay) {
                specimens.overlays.push(overlay);
                page(&mut specimens);
            }
            run(&ctx, vec![key(egui::Key::Escape)], |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| specimens.show_all(ui, &ctx));
            });
            assert!(
                !specimens.overlays.contains(&overlay),
                "{overlay} did not close"
            );
        }
    }

    #[test]
    fn segment_display_reads_its_value() {
        let ctx = context();
        let nodes = run(&ctx, vec![], |ui| {
            segment_display(ui, "Position", "12.03.0480", false, 120.0);
        });
        assert!(
            nodes
                .iter()
                .any(|n| n.label() == Some("Position") && n.value() == Some("12.03.0480")),
            "{:?}",
            nodes
                .iter()
                .map(|n| (
                    n.role(),
                    n.label().map(str::to_owned),
                    n.value().map(str::to_owned)
                ))
                .collect::<Vec<_>>()
        );
    }
}
