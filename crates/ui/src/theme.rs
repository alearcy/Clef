//! Piano-roll metrics shared by the surface and its editing primitives.

pub const ROLL_KEYBED: f32 = 56.0;
pub const ROLL_LINE: f32 = 1.0;
pub const ROLL_CAPTION: f32 = 10.0;
pub const ROLL_NOTE_INSET: f32 = 1.0;
pub const ROLL_NOTE_MIN: f32 = 3.0;
pub const ROLL_RADIUS: f32 = 2.0;
pub const ROLL_SELECTION: f32 = 1.5;
pub const ROLL_HANDLE: f32 = 3.0;
pub const ROLL_CONTROLLER_HEIGHT: f32 = 64.0;
pub const ROLL_POINT: f32 = 3.0;
pub const ROLL_HIT: f32 = 8.0;
pub const ROLL_CURVE_STEPS: u8 = 16;
pub const ROLL_LANE_HEIGHT: f32 = 144.0;
pub const ROLL_MULTI_LANE_HEIGHT: f32 = 88.0;
pub const ROLL_GUTTER: f32 = 7.0;

pub const TOOL_TARGET: f32 = 28.0;
pub const TOOL_GLYPH: f32 = 16.0;
pub const TOOL_STROKE: f32 = 1.5;
pub const LOOP_ACTIVE: eframe::egui::Color32 = eframe::egui::Color32::from_rgb(0xe5, 0xc3, 0x4f);
pub const CAP_LIGHT: eframe::egui::Color32 = eframe::egui::Color32::from_rgb(0xed, 0xf1, 0xef);
