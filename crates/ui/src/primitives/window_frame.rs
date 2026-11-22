//! Window frame: the in-app frame of Clef's floating tool windows (settings, commands, history,
//! picker). Never a repaint of the OS title bar.
use crate::theme::{self, text};
use eframe::egui::{self, Context, Stroke, Ui};

/// The frame every Clef tool window uses: `panel` fill, 1-point `divider` edge, no shadow.
pub fn frame() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::PANEL)
        .stroke(Stroke::new(1.0_f32, theme::DIVIDER))
        .corner_radius(4)
        .inner_margin(12)
}

/// A titled, closable tool window. Returns the content's value while the window is open.
#[must_use]
pub struct ToolWindow<'a> {
    title: &'a str,
    width: f32,
    position: Option<egui::Pos2>,
}

impl<'a> ToolWindow<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            width: 360.0,
            position: None,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn position(mut self, position: egui::Pos2) -> Self {
        self.position = Some(position);
        self
    }

    pub fn show<R>(
        self,
        ctx: &Context,
        open: &mut bool,
        draw: impl FnOnce(&mut Ui) -> R,
    ) -> Option<R> {
        let mut window = egui::Window::new(
            egui::RichText::new(self.title)
                .font(text::font(text::BODY))
                .strong()
                .color(theme::TEXT),
        )
        .open(open)
        .collapsible(false)
        .frame(frame())
        .default_width(self.width);
        // T41 round 4 (Q1): a window opens centred on Clef's window unless its owner places
        // it; once moved, egui keeps its place for the session.
        window = match self.position {
            Some(position) => window.default_pos(position),
            None => window
                .pivot(egui::Align2::CENTER_CENTER)
                .default_pos(ctx.content_rect().center()),
        };
        window.show(ctx, draw).and_then(|inner| inner.inner)
    }
}
