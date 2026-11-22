//! Composer Dark design tokens: the Rust side of `docs/UI/foundations.md`.
//!
//! Every colour role, text style and metric named in the foundations lives here, and the parity
//! test at the bottom fails when this file and the document drift. The token tables are complete
//! by contract, so some entries have no caller yet.
#![allow(dead_code)]

use eframe::egui::{self, Color32, Context, FontFamily, FontId, TextStyle};

macro_rules! colours {
    ($($name:ident = $hex:literal;)*) => {
        $(pub const $name: Color32 = Color32::from_rgb(
            (($hex >> 16) & 0xff) as u8,
            (($hex >> 8) & 0xff) as u8,
            ($hex & 0xff) as u8,
        );)*
        const COLOUR_TOKENS: &[(&str, Color32)] = &[$((stringify!($name), $name)),*];
    };
}

colours! {
    BACKGROUND = 0x101A1C;
    PANEL = 0x172326;
    CANVAS = 0x111C1F;
    FIELD = 0x1F2F32;
    TEXT = 0xE3E8E5;
    SECONDARY = 0xA4B5AE;
    DISABLED = 0x74847D;
    ACCENT = 0x78B5AA;
    PEACOCK = 0x336A64;
    OXBLOOD = 0x882820;
    DANGER = 0xC7847C;
    LOOP_ACTIVE = 0xE5C34F;
    SOLO = 0xD8B23C;
    DIVIDER = 0x2B3B3D;
    GRID = 0x243236;
    BAR_LINE = 0x3A4D50;
    CONTROL_EDGE = 0x3A4D50;
    CAP = 0xCCD3D1;
    CAP_LIGHT = 0xEDF1EF;
    CAP_DARK = 0x8C9994;
    GLOBAL_HEADER = 0x131E21;
    GLOBAL_LANE = 0x14221F;
    AUTOMATION_HEADER = 0x070D0F;
    AUTOMATION_LANE = 0x0C1416;
    AUTOMATION_OUTSIDE = 0x070D0F;
    OUT_OF_SCALE = 0xC75A50;
    SIDECHAIN = 0xA98BD0;
    MIDI = 0xC08A4E;
    CREATE = 0xE0913A;
}

/// A stored track colour as an egui colour.
pub fn track_colour(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// Label colour on a fill: the light or dark foreground with the higher contrast.
pub fn foreground_on(fill: Color32) -> Color32 {
    if contrast(TEXT, fill) >= contrast(BACKGROUND, fill) {
        TEXT
    } else {
        BACKGROUND
    }
}

/// WCAG 2 contrast ratio of two opaque colours (1 to 21).
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// What a foreground/background pair must reach (foundations, contrast).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Requirement {
    /// Normal text: 4.5:1.
    Text,
    /// Focus edges, glyphs and other non-text cues that identify a control: 3:1.
    NonText,
    /// Not required by WCAG; the reason is recorded in foundations.
    Exempt(&'static str),
}

impl Requirement {
    pub fn minimum(self) -> f32 {
        match self {
            Self::Text => 4.5,
            Self::NonText => 3.0,
            Self::Exempt(_) => 1.0,
        }
    }
}

/// The token pairs the surfaces draw, mirrored by the contrast table in foundations. Labels on
/// lit fills and track hues use `foreground_on`, so they are listed by their fill.
pub const CONTRAST: &[(&str, &str, Requirement)] = &[
    ("TEXT", "BACKGROUND", Requirement::Text),
    ("TEXT", "PANEL", Requirement::Text),
    ("TEXT", "CANVAS", Requirement::Text),
    ("TEXT", "FIELD", Requirement::Text),
    ("SECONDARY", "BACKGROUND", Requirement::Text),
    ("SECONDARY", "PANEL", Requirement::Text),
    ("SECONDARY", "CANVAS", Requirement::Text),
    ("SECONDARY", "FIELD", Requirement::Text),
    ("ACCENT", "BACKGROUND", Requirement::Text),
    ("ACCENT", "PANEL", Requirement::Text),
    ("ACCENT", "FIELD", Requirement::Text),
    ("DANGER", "PANEL", Requirement::Text),
    ("DANGER", "FIELD", Requirement::Text),
    ("foreground_on", "PEACOCK", Requirement::Text),
    ("foreground_on", "OXBLOOD", Requirement::Text),
    ("foreground_on", "SOLO", Requirement::Text),
    ("foreground_on", "ACCENT", Requirement::Text),
    ("foreground_on", "track hues", Requirement::Text),
    ("ACCENT", "FIELD (focus edge)", Requirement::NonText),
    ("DISABLED", "PANEL", Requirement::Exempt("inactive control")),
    ("DISABLED", "FIELD", Requirement::Exempt("inactive control")),
    (
        "CONTROL_EDGE",
        "FIELD",
        Requirement::Exempt("decorative; controls are identified by label or glyph"),
    ),
];

/// A colour token by its Rust name.
pub fn token(name: &str) -> Option<Color32> {
    COLOUR_TOKENS
        .iter()
        .find_map(|(token, colour)| (*token == name).then_some(*colour))
}

fn luminance(colour: Color32) -> f32 {
    let channel = |value: u8| {
        let value = f32::from(value) / 255.0;
        if value <= 0.040_45 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(colour.r()) + 0.7152 * channel(colour.g()) + 0.0722 * channel(colour.b())
}

/// The six text styles. Surfaces use a style, never an ad-hoc size.
pub mod text {
    use super::{FontFamily, FontId};

    pub const SF_PRO: &str = "sf-pro";
    pub const HEADING: f32 = 16.0;
    pub const BODY: f32 = 13.0;
    pub const SMALL: f32 = 11.0;
    pub const CAPTION: f32 = 10.0;
    pub const TINY: f32 = 9.0;
    pub const MENU: f32 = 14.0;

    pub fn font(size: f32) -> FontId {
        FontId::new(size, FontFamily::Name(SF_PRO.into()))
    }

    #[cfg(test)]
    pub(super) const STYLES: &[(&str, f32)] = &[
        ("HEADING", HEADING),
        ("BODY", BODY),
        ("SMALL", SMALL),
        ("CAPTION", CAPTION),
        ("TINY", TINY),
        ("MENU", MENU),
    ];
}

/// Loads the system SF Pro as `sf-pro` and the first proportional face, and maps egui's text
/// styles to the foundation sizes. egui's bundled face stays only as the glyph fallback.
pub fn install_fonts(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();
    if let Ok(data) = std::fs::read("/System/Library/Fonts/SFNS.ttf") {
        fonts
            .font_data
            .insert(text::SF_PRO.into(), egui::FontData::from_owned(data).into());
        if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
            family.insert(0, text::SF_PRO.into());
        }
    }
    let family = fonts.families[&FontFamily::Proportional].clone();
    fonts
        .families
        .insert(FontFamily::Name(text::SF_PRO.into()), family);
    ctx.set_fonts(fonts);
    ctx.all_styles_mut(|style| {
        for (role, size) in [
            (TextStyle::Heading, text::HEADING),
            (TextStyle::Body, text::BODY),
            (TextStyle::Button, text::BODY),
            (TextStyle::Small, text::SMALL),
        ] {
            style.text_styles.insert(role, text::font(size));
        }
    });
}

/// Window and chrome metrics (`theme::metric`).
pub mod metric {
    pub const TOOL_TARGET: f32 = 28.0;
    pub const TOOL_GLYPH: f32 = 16.0;
    pub const TOOL_STROKE: f32 = 1.5;
    pub const CONTROL_HEIGHT: f32 = 24.0;
    pub const NESTING: f32 = 16.0;
    pub const STRIPE: f32 = 3.0;
    pub const CHANNEL_WIDTH: f32 = 92.0;
    pub const CHANNEL_GAP: f32 = 1.0;
    pub const MINIMUM_HEADER_WIDTH: f32 = 160.0;
    pub const MAXIMUM_HEADER_WIDTH: f32 = 400.0;
    pub const FOLDER_HEIGHT: f32 = 24.0;
    pub const TRANSPORT_HEIGHT: f32 = 40.0;
    pub const MENU_BAR_HEIGHT: f32 = 28.0;
    pub const INSPECTOR_WIDTH: f32 = 208.0;
    pub const STATUS_HEIGHT: f32 = 24.0;
    pub const TAB_HEIGHT: f32 = 28.0;
    pub const WORKSPACE_PADDING: f32 = 12.0;
    pub const MINIMUM_WINDOW_WIDTH: f32 = 1024.0;
    pub const MINIMUM_WINDOW_HEIGHT: f32 = 640.0;
    pub const RAIL_WIDTH: f32 = 2.0;
    pub const CORNER_CONTROL: u8 = 2;
    /// Shared body inset of raised and recessed controls (foundations, control depth).
    pub const CONTROL_BODY_INSET: f32 = 2.0;
    pub const CONTROL_PADDING: f32 = 8.0;
    pub const CHIP_HEIGHT: f32 = 20.0;
    pub const SELECTION_EDGE: f32 = 1.5;
    pub const SCROLL_BAR: f32 = 12.0;
    pub const MENU_ROW: f32 = 26.0;
    pub const MENU_SEPARATOR: f32 = 9.0;
    pub const MENU_CAPTION: f32 = 22.0;
    pub const MENU_TICK: f32 = 16.0;
    pub const MENU_PADDING: f32 = 4.0;

    #[cfg(test)]
    pub(super) const TOKENS: &[(&str, f32)] = &[
        ("TOOL_TARGET", TOOL_TARGET),
        ("TOOL_GLYPH", TOOL_GLYPH),
        ("TOOL_STROKE", TOOL_STROKE),
        ("CONTROL_HEIGHT", CONTROL_HEIGHT),
        ("NESTING", NESTING),
        ("STRIPE", STRIPE),
        ("CHANNEL_WIDTH", CHANNEL_WIDTH),
        ("CHANNEL_GAP", CHANNEL_GAP),
        ("MINIMUM_HEADER_WIDTH", MINIMUM_HEADER_WIDTH),
        ("MAXIMUM_HEADER_WIDTH", MAXIMUM_HEADER_WIDTH),
        ("FOLDER_HEIGHT", FOLDER_HEIGHT),
        ("TRANSPORT_HEIGHT", TRANSPORT_HEIGHT),
        ("MENU_BAR_HEIGHT", MENU_BAR_HEIGHT),
        ("INSPECTOR_WIDTH", INSPECTOR_WIDTH),
        ("STATUS_HEIGHT", STATUS_HEIGHT),
        ("TAB_HEIGHT", TAB_HEIGHT),
        ("WORKSPACE_PADDING", WORKSPACE_PADDING),
        ("MINIMUM_WINDOW_WIDTH", MINIMUM_WINDOW_WIDTH),
        ("MINIMUM_WINDOW_HEIGHT", MINIMUM_WINDOW_HEIGHT),
        ("RAIL_WIDTH", RAIL_WIDTH),
        ("CONTROL_BODY_INSET", CONTROL_BODY_INSET),
        ("CONTROL_PADDING", CONTROL_PADDING),
        ("CHIP_HEIGHT", CHIP_HEIGHT),
        ("SELECTION_EDGE", SELECTION_EDGE),
        ("SCROLL_BAR", SCROLL_BAR),
        ("MENU_ROW", MENU_ROW),
        ("MENU_SEPARATOR", MENU_SEPARATOR),
        ("MENU_CAPTION", MENU_CAPTION),
        ("MENU_TICK", MENU_TICK),
        ("MENU_PADDING", MENU_PADDING),
    ];
}

/// Piano-roll metrics (`theme::roll`).
pub mod roll {
    pub const KEYBED_WIDTH: f32 = 56.0;
    pub const RULER_HEIGHT: f32 = 24.0;
    pub const PITCH_ROW_DEFAULT: f32 = 12.0;
    pub const PITCH_ROW_MINIMUM: f32 = 6.0;
    pub const PITCH_ROW_MAXIMUM: f32 = 24.0;
    pub const NOTE_RADIUS: f32 = 2.0;
    pub const HANDLE_WIDTH: f32 = 3.0;
    pub const PLAYHEAD_WIDTH: f32 = 1.0;
    pub const VELOCITY_LANE_HEIGHT: f32 = 72.0;
    pub const CONTROLLER_LANE_HEIGHT: f32 = 64.0;
    pub const LANE_MINIMUM_HEIGHT: f32 = 40.0;
    pub const LANE_MAXIMUM_HEIGHT: f32 = 240.0;
    pub const LANE_GUTTER_HEIGHT: f32 = 4.0;
    pub const LANE_HEADER_BUTTON: f32 = 14.0;
    pub const VELOCITY_BAR_WIDTH: f32 = 3.0;
    pub const MINIMUM_GRID_PIXELS: f32 = 7.0;

    #[cfg(test)]
    pub(super) const TOKENS: &[(&str, f32)] = &[
        ("KEYBED_WIDTH", KEYBED_WIDTH),
        ("RULER_HEIGHT", RULER_HEIGHT),
        ("PITCH_ROW_DEFAULT", PITCH_ROW_DEFAULT),
        ("PITCH_ROW_MINIMUM", PITCH_ROW_MINIMUM),
        ("PITCH_ROW_MAXIMUM", PITCH_ROW_MAXIMUM),
        ("NOTE_RADIUS", NOTE_RADIUS),
        ("HANDLE_WIDTH", HANDLE_WIDTH),
        ("PLAYHEAD_WIDTH", PLAYHEAD_WIDTH),
        ("VELOCITY_LANE_HEIGHT", VELOCITY_LANE_HEIGHT),
        ("CONTROLLER_LANE_HEIGHT", CONTROLLER_LANE_HEIGHT),
        ("LANE_MINIMUM_HEIGHT", LANE_MINIMUM_HEIGHT),
        ("LANE_MAXIMUM_HEIGHT", LANE_MAXIMUM_HEIGHT),
        ("LANE_GUTTER_HEIGHT", LANE_GUTTER_HEIGHT),
        ("LANE_HEADER_BUTTON", LANE_HEADER_BUTTON),
        ("VELOCITY_BAR_WIDTH", VELOCITY_BAR_WIDTH),
        ("MINIMUM_GRID_PIXELS", MINIMUM_GRID_PIXELS),
    ];
}

/// The nine track hues (foundations, track colours), owned by the model so new tracks and the
/// theme can never disagree.
pub mod track {
    use super::Color32;
    use clef_model::project::TRACK_COLOURS;

    pub const COLOURS: [Color32; 9] = {
        let mut colours = [Color32::BLACK; 9];
        let mut index = 0;
        while index < 9 {
            let [r, g, b] = TRACK_COLOURS[index];
            colours[index] = Color32::from_rgb(r, g, b);
            index += 1;
        }
        colours
    };
}

// Surface metrics that predate the foundations tables. They stay surface-owned until the
// surface that uses them is migrated (T40 for the roll and arrangement).
pub const ROLL_KEYBED: f32 = roll::KEYBED_WIDTH;
pub const ROLL_LINE: f32 = 1.0;
pub const ROLL_CAPTION: f32 = text::CAPTION;
pub const ROLL_NOTE_INSET: f32 = 1.0;
pub const ROLL_NOTE_MIN: f32 = 3.0;
pub const ROLL_RADIUS: f32 = roll::NOTE_RADIUS;
pub const ROLL_SELECTION: f32 = 1.5;
pub const ROLL_HANDLE: f32 = roll::HANDLE_WIDTH;
pub const ROLL_CONTROLLER_HEIGHT: f32 = roll::CONTROLLER_LANE_HEIGHT;
pub const ROLL_POINT: f32 = 3.0;
/// T41 round 3 (S7): radius of the velocity lollipop head.
pub const ROLL_VELOCITY_HEAD: f32 = 4.5;
/// T41 round 3 (S8): the white keys' front-edge shade (black at 45 %, applied in steps).
pub const KEY_FRONT_SHADE: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 115);
pub const ROLL_HIT: f32 = 8.0;
pub const ROLL_CURVE_STEPS: u8 = 16;
pub const ROLL_LANE_HEIGHT: f32 = 144.0;
pub const ROLL_MULTI_LANE_HEIGHT: f32 = 88.0;
pub const ROLL_GUTTER: f32 = 7.0;

pub const MIX_CHANNEL: f32 = metric::CHANNEL_WIDTH;
pub const MIX_GAP: f32 = metric::CHANNEL_GAP;
pub const MIX_KNOB: f32 = 30.0;
pub const MIX_SEND_KNOB: f32 = 20.0;
pub const MIX_FADER: f32 = 38.0;
pub const MIX_FADER_HEIGHT: f32 = 210.0;
pub const MIX_CAP: [f32; 2] = [22.0, 40.0];
pub const MIX_METER: f32 = 14.0;
pub const HEADER_METER: f32 = 9.0;
pub const ARRANGEMENT_TRACK_HEIGHT: f32 = 44.0;
pub const TOOL_TARGET: f32 = metric::TOOL_TARGET;
pub const TOOL_GLYPH: f32 = metric::TOOL_GLYPH;
pub const TOOL_STROKE: f32 = metric::TOOL_STROKE;
pub const MIX_SILENCE: i16 = -600;
pub const MIX_GROUP: f32 = 12.0;
pub const MIX_ROW: f32 = 3.0;
pub const MIX_PADDING: [i8; 4] = [5, 5, 8, 6];
pub const MIX_SMALL: f32 = text::SMALL;
pub const MIX_SCALE_TEXT: f32 = text::CAPTION;

#[cfg(test)]
mod tests {
    use super::*;

    const FOUNDATIONS: &str = include_str!("../../../docs/UI/foundations.md");

    /// The rows of the first Markdown table after `heading`, as trimmed cells.
    fn table(heading: &str) -> Vec<Vec<String>> {
        let start = FOUNDATIONS
            .find(heading)
            .unwrap_or_else(|| panic!("foundations has no {heading}"));
        FOUNDATIONS[start..]
            .lines()
            .skip_while(|line| !line.starts_with('|'))
            .take_while(|line| line.starts_with('|'))
            .skip(2)
            .map(|line| {
                line.trim_matches('|')
                    .split('|')
                    .map(|cell| cell.trim().trim_matches('`').to_owned())
                    .collect()
            })
            .collect()
    }

    fn screaming(camel: &str) -> String {
        let mut out = String::new();
        for (index, character) in camel.chars().enumerate() {
            if character.is_uppercase() && index > 0 {
                out.push('_');
            }
            out.push(character.to_ascii_uppercase());
        }
        out
    }

    /// `name / Other / Third` with values `a / b / c`: later names extend the first one's stem.
    fn metric_rows(heading: &str) -> Vec<(String, f32)> {
        let mut rows = Vec::new();
        for row in table(heading) {
            let names: Vec<_> = row[0].split(" / ").map(str::trim).collect();
            let values: Vec<_> = row[1].split(" / ").map(str::trim).collect();
            if names.len() != values.len() || names.iter().any(|name| name.contains(' ')) {
                continue;
            }
            let stem = names[0]
                .rfind(|c: char| c.is_uppercase())
                .map_or(names[0], |index| &names[0][..index]);
            for (name, value) in names.iter().zip(values) {
                let name = if name.starts_with(char::is_uppercase) {
                    format!("{stem}{name}")
                } else {
                    (*name).to_owned()
                };
                rows.push((screaming(&name), value.parse().expect("numeric metric")));
            }
        }
        rows
    }

    fn assert_same(kind: &str, document: Vec<(String, f32)>, code: &[(&str, f32)]) {
        let mut document = document;
        document.sort_by(|a, b| a.0.cmp(&b.0));
        let mut code: Vec<_> = code.iter().map(|(n, v)| ((*n).to_owned(), *v)).collect();
        code.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(document, code, "{kind} tokens drifted from foundations.md");
    }

    #[test]
    fn colour_roles_match_the_foundations_table() {
        let document = table("## Colour roles")
            .into_iter()
            .map(|row| {
                let hex = u32::from_str_radix(row[2].trim_start_matches('#'), 16).expect("hex");
                (row[0].clone(), hex)
            })
            .collect::<Vec<_>>();
        let code = COLOUR_TOKENS
            .iter()
            .map(|(name, colour)| {
                let [r, g, b, _] = colour.to_array();
                (
                    (*name).to_owned(),
                    u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b),
                )
            })
            .collect::<Vec<_>>();
        let (mut document, mut code) = (document, code);
        document.sort();
        code.sort();
        assert_eq!(document, code, "colour tokens drifted from foundations.md");
    }

    #[test]
    fn text_styles_match_the_foundations_table() {
        let document = table("## Typography")
            .into_iter()
            .map(|row| (row[0].clone(), row[1].parse().expect("size")))
            .collect();
        assert_same("text style", document, text::STYLES);
    }

    #[test]
    fn metrics_match_the_foundations_tables() {
        assert_same(
            "window metric",
            metric_rows("Window and chrome"),
            metric::TOKENS,
        );
        assert_same(
            "roll metric",
            metric_rows("Piano roll (`theme::roll`)"),
            roll::TOKENS,
        );
    }

    #[test]
    fn colour_literals_live_only_in_the_theme() {
        fn visit(dir: &std::path::Path, offenders: &mut Vec<String>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    visit(&path, offenders);
                } else if path.extension().is_some_and(|e| e == "rs")
                    && path.file_name().is_some_and(|n| n != "theme.rs")
                {
                    let source = std::fs::read_to_string(&path).unwrap();
                    for (number, line) in source.lines().enumerate() {
                        if line.contains("Color32::from_rgb") || line.contains("Color32::from_gray")
                        {
                            offenders.push(format!("{}:{}", path.display(), number + 1));
                        }
                    }
                }
            }
        }
        let mut offenders = Vec::new();
        visit(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut offenders,
        );
        assert!(
            offenders.is_empty(),
            "colour literals outside theme.rs: {offenders:?}"
        );
    }

    #[test]
    fn foreground_on_picks_the_readable_label() {
        assert_eq!(foreground_on(PEACOCK), TEXT);
        assert_eq!(foreground_on(SOLO), BACKGROUND);
        assert_eq!(foreground_on(CAP_LIGHT), BACKGROUND);
    }

    /// The colours a contrast row names: one pair, or one per track hue.
    fn resolve(foreground: &str, background: &str) -> Vec<(Color32, Color32)> {
        let background = background.split(" (").next().unwrap();
        let fills = if background == "track hues" {
            track::COLOURS.to_vec()
        } else {
            vec![token(background).unwrap_or_else(|| panic!("no token {background}"))]
        };
        fills
            .into_iter()
            .map(|fill| {
                let label = if foreground == "foreground_on" {
                    foreground_on(fill)
                } else {
                    token(foreground).unwrap_or_else(|| panic!("no token {foreground}"))
                };
                (label, fill)
            })
            .collect()
    }

    fn weakest(foreground: &str, background: &str) -> f32 {
        resolve(foreground, background)
            .into_iter()
            .map(|(a, b)| contrast(a, b))
            .fold(f32::INFINITY, f32::min)
    }

    #[test]
    fn every_drawn_pair_meets_wcag_aa() {
        for (foreground, background, requirement) in CONTRAST {
            let ratio = weakest(foreground, background);
            assert!(
                ratio >= requirement.minimum(),
                "{foreground} on {background}: {ratio:.2}:1 is below {requirement:?}"
            );
        }
        // A token pushed below the threshold fails the same check.
        let dimmed = TEXT.gamma_multiply(0.38);
        assert!(contrast(Color32::from_rgb(dimmed.r(), dimmed.g(), dimmed.b()), PANEL) < 4.5);
    }

    #[test]
    fn contrast_table_matches_the_foundations_table() {
        let document: Vec<_> = table("### Contrast")
            .into_iter()
            .map(|row| {
                (
                    row[0].clone(),
                    row[1].clone(),
                    row[2].clone(),
                    row[3].clone(),
                )
            })
            .collect();
        let code: Vec<_> = CONTRAST
            .iter()
            .map(|(foreground, background, requirement)| {
                let kind = match requirement {
                    Requirement::Text => "text 4.5".to_owned(),
                    Requirement::NonText => "non-text 3".to_owned(),
                    Requirement::Exempt(reason) => format!("exempt: {reason}"),
                };
                (
                    (*foreground).to_owned(),
                    (*background).to_owned(),
                    kind,
                    format!("{:.2}", weakest(foreground, background)),
                )
            })
            .collect();
        assert_eq!(document, code, "contrast table drifted from foundations.md");
    }

    #[test]
    fn track_hues_stay_distinct() {
        fn lab(colour: Color32) -> [f32; 3] {
            let linear = |v: u8| {
                let v = f32::from(v) / 255.0;
                if v <= 0.040_45 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            let [r, g, b] = [colour.r(), colour.g(), colour.b()].map(linear);
            let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.950_47;
            let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.088_83;
            let f = |t: f32| {
                if t > 0.008_856 {
                    t.cbrt()
                } else {
                    7.787 * t + 16.0 / 116.0
                }
            };
            [
                116.0 * f(y) - 16.0,
                500.0 * (f(x) - f(y)),
                200.0 * (f(y) - f(z)),
            ]
        }
        let colours = track::COLOURS.map(lab);
        let mut minimum = f32::INFINITY;
        for (index, a) in colours.iter().enumerate() {
            for b in &colours[index + 1..] {
                let distance = a
                    .iter()
                    .zip(b)
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f32>()
                    .sqrt();
                minimum = minimum.min(distance);
            }
        }
        assert!(minimum >= 19.0, "closest track hues are {minimum:.1} apart");
    }
}
