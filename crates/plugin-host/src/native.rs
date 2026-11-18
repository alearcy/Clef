use crate::protocol::Result;
use eframe::egui::{Context, ViewportCommand};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::ffi::c_void;

unsafe extern "C" {
    fn clef_plugin_parent_create(parent: *mut c_void) -> *mut c_void;
    fn clef_plugin_parent_detach(surface: *mut c_void);
    fn clef_plugin_parent_destroy(surface: *mut c_void);
}

/// Owns only the plugin ABI surface inside eframe's borrowed parent view.
pub struct NativeWindow {
    context: Context,
    view: *mut c_void,
    surface: *mut c_void,
}

impl NativeWindow {
    pub fn new(context: Context, host: &impl HasWindowHandle) -> Result<Self> {
        let RawWindowHandle::AppKit(handle) = host.window_handle()?.as_raw() else {
            return Err("T02 editor embedding currently requires macOS".into());
        };
        Ok(Self {
            context,
            view: handle.ns_view.as_ptr(),
            surface: std::ptr::null_mut(),
        })
    }

    pub fn attach_parent(&mut self) -> Result<()> {
        if !self.surface.is_null() {
            return Err("plugin parent already attached".into());
        }
        self.surface = unsafe { clef_plugin_parent_create(self.view) };
        if self.surface.is_null() {
            return Err("plugin embedding surface unavailable".into());
        }
        Ok(())
    }

    pub fn view_ptr(&self) -> *mut c_void {
        self.surface
    }

    pub fn detach_parent(&self) {
        if !self.surface.is_null() {
            unsafe {
                clef_plugin_parent_detach(self.surface);
            }
        }
    }

    pub fn destroy_parent(&mut self) {
        if !self.surface.is_null() {
            unsafe {
                clef_plugin_parent_destroy(self.surface);
            }
            self.surface = std::ptr::null_mut();
        }
    }

    pub fn show(&self) {
        self.context
            .send_viewport_cmd(ViewportCommand::Visible(true));
        self.context.send_viewport_cmd(ViewportCommand::Focus);
    }

    pub fn resizable(&self, resizable: bool) {
        self.context
            .send_viewport_cmd(ViewportCommand::Resizable(resizable));
    }

    pub fn hide(&self) {
        self.context
            .send_viewport_cmd(ViewportCommand::Visible(false));
    }

    pub fn resize(&self, width: u32, height: u32) -> Result<()> {
        validate_size(width, height)?;
        self.context
            .send_viewport_cmd(ViewportCommand::InnerSize(eframe::egui::vec2(
                width as f32,
                height as f32,
            )));
        Ok(())
    }
}

impl Drop for NativeWindow {
    fn drop(&mut self) {
        self.detach_parent();
        self.destroy_parent();
    }
}

pub fn validate_size(width: u32, height: u32) -> Result<()> {
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return Err("invalid plugin editor size".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_sizes_before_forwarding_to_windowing_backend() {
        for (width, height) in [(0, 100), (100, 0), (8193, 100), (100, 8193)] {
            assert!(validate_size(width, height).is_err());
        }
        assert!(validate_size(8192, 8192).is_ok());
    }
}
