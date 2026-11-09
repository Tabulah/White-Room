//! The window operations the workspace needs, behind a small trait so the
//! controller can be tested without a real window.
//!
//! The Tauri implementation converts between Tauri's physical pixels and
//! *desktop coordinates* (see [`super::display::Rect`]):
//!
//! - **macOS** positions windows in logical points. Tauri reports a monitor's
//!   position as its point origin multiplied by that monitor's own scale
//!   factor, so dividing by it recovers the point origin exactly, even when
//!   displays have different scale factors.
//! - **Windows and Linux** position windows in physical pixels, which is what
//!   Tauri reports, so no conversion is needed.

use tauri::{Monitor, Runtime, WebviewWindow};

use super::display::{Display, Rect, Size};

/// Window operations used by [`super::Workspace`].
pub trait WorkspaceWindow {
    fn is_fullscreen(&self) -> tauri::Result<bool>;
    fn set_fullscreen(&self, fullscreen: bool) -> tauri::Result<()>;
    fn primary_display(&self) -> tauri::Result<Option<Display>>;
    fn current_display(&self) -> tauri::Result<Option<Display>>;
    fn outer_size(&self) -> tauri::Result<Size>;
    /// Moves the window's top-left corner, in desktop coordinates.
    fn move_to(&self, x: f64, y: f64) -> tauri::Result<()>;
    fn focus(&self) -> tauri::Result<()>;
}

impl<R: Runtime> WorkspaceWindow for WebviewWindow<R> {
    fn is_fullscreen(&self) -> tauri::Result<bool> {
        WebviewWindow::is_fullscreen(self)
    }

    fn set_fullscreen(&self, fullscreen: bool) -> tauri::Result<()> {
        WebviewWindow::set_fullscreen(self, fullscreen)
    }

    fn primary_display(&self) -> tauri::Result<Option<Display>> {
        Ok(self.primary_monitor()?.as_ref().map(to_display))
    }

    fn current_display(&self) -> tauri::Result<Option<Display>> {
        Ok(self.current_monitor()?.as_ref().map(to_display))
    }

    fn outer_size(&self) -> tauri::Result<Size> {
        let size = WebviewWindow::outer_size(self)?;
        let scale = desktop_scale(self.scale_factor()?);
        Ok(Size {
            width: f64::from(size.width) / scale,
            height: f64::from(size.height) / scale,
        })
    }

    fn move_to(&self, x: f64, y: f64) -> tauri::Result<()> {
        #[cfg(target_os = "macos")]
        let position = tauri::Position::Logical(tauri::LogicalPosition::new(x, y));
        // Rounded and range-checked by the cast; desktop coordinates are
        // always well within `i32` on these platforms.
        #[cfg(not(target_os = "macos"))]
        let position = tauri::Position::Physical(tauri::PhysicalPosition::new(
            x.round() as i32,
            y.round() as i32,
        ));
        self.set_position(position)
    }

    fn focus(&self) -> tauri::Result<()> {
        self.set_focus()
    }
}

fn to_display(monitor: &Monitor) -> Display {
    let scale = desktop_scale(monitor.scale_factor());
    let position = monitor.position();
    let size = monitor.size();
    let work_area = monitor.work_area();
    Display {
        name: monitor.name().cloned(),
        bounds: Rect {
            x: f64::from(position.x) / scale,
            y: f64::from(position.y) / scale,
            width: f64::from(size.width) / scale,
            height: f64::from(size.height) / scale,
        },
        work_area: Rect {
            x: f64::from(work_area.position.x) / scale,
            y: f64::from(work_area.position.y) / scale,
            width: f64::from(work_area.size.width) / scale,
            height: f64::from(work_area.size.height) / scale,
        },
    }
}

/// Divisor that turns Tauri's physical pixels into desktop coordinates.
#[cfg(target_os = "macos")]
fn desktop_scale(scale_factor: f64) -> f64 {
    scale_factor
}

#[cfg(not(target_os = "macos"))]
fn desktop_scale(_scale_factor: f64) -> f64 {
    1.0
}
