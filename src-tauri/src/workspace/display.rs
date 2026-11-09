//! Which display the workspace opens on.
//!
//! Policy (see `docs/workspace.md`): the workspace always opens on the
//! **primary display**. If the window is on another display, it is first moved
//! to the centre of the primary display's work area, then made fullscreen.
//! Other displays are never touched. If the primary display cannot be
//! determined, the workspace opens on whichever display holds the window.
//!
//! This module is pure: it decides, [`super::window`] applies.

/// A rectangle in desktop coordinates.
///
/// Desktop coordinates are the unit each platform positions windows in:
/// logical points on macOS, physical pixels on Windows and Linux. The
/// conversion happens in [`super::window`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    /// Whether two rectangles describe the same area, allowing for rounding.
    fn same_as(&self, other: &Rect) -> bool {
        const TOLERANCE: f64 = 0.5;
        (self.x - other.x).abs() <= TOLERANCE
            && (self.y - other.y).abs() <= TOLERANCE
            && (self.width - other.width).abs() <= TOLERANCE
            && (self.height - other.height).abs() <= TOLERANCE
    }
}

/// Window dimensions in desktop coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

/// A connected display.
#[derive(Debug, Clone, PartialEq)]
pub struct Display {
    /// Name reported by the operating system. Not unique: two monitors of the
    /// same model usually share a name, so displays are compared by bounds.
    pub name: Option<String>,
    /// Full area of the display.
    pub bounds: Rect,
    /// Area not covered by the menu bar, Dock, or taskbar.
    pub work_area: Rect,
}

impl Display {
    fn is_same_display(&self, other: &Display) -> bool {
        self.bounds.same_as(&other.bounds)
    }
}

/// What to do with the window before making it fullscreen.
#[derive(Debug, Clone, PartialEq)]
pub enum Placement {
    /// The window is already on the primary display.
    StayOnPrimary,
    /// Move the window's top-left corner to `(x, y)` on the primary display.
    MoveToPrimary { x: f64, y: f64 },
    /// The primary display is unknown; stay on the current display.
    StayOnCurrent,
}

/// Decides where the workspace opens.
///
/// `window` is the window's current outer size, if known. It is used to centre
/// the window so that it is back in a sensible place after leaving fullscreen.
pub fn plan(
    primary: Option<&Display>,
    current: Option<&Display>,
    window: Option<Size>,
) -> Placement {
    let Some(primary) = primary else {
        return Placement::StayOnCurrent;
    };
    if current.is_some_and(|current| current.is_same_display(primary)) {
        return Placement::StayOnPrimary;
    }
    let (x, y) = centred_origin(&primary.work_area, window);
    Placement::MoveToPrimary { x, y }
}

/// Top-left position that centres a window of `size` in `area`, never placing
/// it above or to the left of the area. Without a size, uses the area's corner.
fn centred_origin(area: &Rect, size: Option<Size>) -> (f64, f64) {
    let Some(size) = size else {
        return (area.x, area.y);
    };
    let x = area.x + ((area.width - size.width) / 2.0).max(0.0);
    let y = area.y + ((area.height - size.height) / 2.0).max(0.0);
    (x.round(), y.round())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(name: &str, x: f64, y: f64, width: f64, height: f64) -> Display {
        let bounds = Rect {
            x,
            y,
            width,
            height,
        };
        // A 25-unit menu bar at the top, like macOS.
        let work_area = Rect {
            y: y + 25.0,
            height: height - 25.0,
            ..bounds
        };
        Display {
            name: Some(name.into()),
            bounds,
            work_area,
        }
    }

    const WINDOW: Size = Size {
        width: 1200.0,
        height: 800.0,
    };

    #[test]
    fn stays_put_when_already_on_primary() {
        let primary = display("Built-in", 0.0, 0.0, 1512.0, 982.0);
        let current = primary.clone();
        assert_eq!(
            plan(Some(&primary), Some(&current), Some(WINDOW)),
            Placement::StayOnPrimary
        );
    }

    #[test]
    fn moves_from_secondary_to_centre_of_primary() {
        let primary = display("Built-in", 0.0, 0.0, 1512.0, 982.0);
        let secondary = display("External", 1512.0, -200.0, 2560.0, 1440.0);
        assert_eq!(
            plan(Some(&primary), Some(&secondary), Some(WINDOW)),
            Placement::MoveToPrimary {
                x: 156.0, // (1512 - 1200) / 2
                y: 104.0, // 25 + (957 - 800) / 2 = 103.5, rounded
            }
        );
    }

    #[test]
    fn moves_from_display_left_of_primary() {
        // Displays arranged to the left of the primary have negative x on
        // both macOS and Windows; the primary is always at the origin.
        let primary = display("Primary", 0.0, 0.0, 1920.0, 1080.0);
        let left = display("Left", -2560.0, -180.0, 2560.0, 1440.0);
        assert_eq!(
            plan(Some(&primary), Some(&left), Some(WINDOW)),
            Placement::MoveToPrimary {
                x: 360.0, // (1920 - 1200) / 2
                y: 153.0, // 25 + (1055 - 800) / 2 = 152.5, rounded
            }
        );
    }

    #[test]
    fn identical_model_names_are_not_confused() {
        let primary = display("DELL U2720Q", 0.0, 0.0, 2560.0, 1440.0);
        let twin = display("DELL U2720Q", 2560.0, 0.0, 2560.0, 1440.0);
        assert!(matches!(
            plan(Some(&primary), Some(&twin), Some(WINDOW)),
            Placement::MoveToPrimary { .. }
        ));
    }

    #[test]
    fn unknown_current_display_moves_to_primary() {
        let primary = display("Built-in", 0.0, 0.0, 1512.0, 982.0);
        assert!(matches!(
            plan(Some(&primary), None, Some(WINDOW)),
            Placement::MoveToPrimary { .. }
        ));
    }

    #[test]
    fn unknown_primary_stays_on_current() {
        let current = display("Only", 0.0, 0.0, 1920.0, 1080.0);
        assert_eq!(
            plan(None, Some(&current), Some(WINDOW)),
            Placement::StayOnCurrent
        );
        assert_eq!(plan(None, None, None), Placement::StayOnCurrent);
    }

    #[test]
    fn window_larger_than_display_is_aligned_to_work_area_corner() {
        let primary = display("Small", 0.0, 0.0, 1024.0, 768.0);
        let secondary = display("Large", 1024.0, 0.0, 3840.0, 2160.0);
        let huge = Size {
            width: 2000.0,
            height: 1500.0,
        };
        assert_eq!(
            plan(Some(&primary), Some(&secondary), Some(huge)),
            Placement::MoveToPrimary { x: 0.0, y: 25.0 }
        );
    }

    #[test]
    fn unknown_window_size_uses_work_area_corner() {
        let primary = display("Built-in", 0.0, 0.0, 1512.0, 982.0);
        let secondary = display("External", 1512.0, 0.0, 2560.0, 1440.0);
        assert_eq!(
            plan(Some(&primary), Some(&secondary), None),
            Placement::MoveToPrimary { x: 0.0, y: 25.0 }
        );
    }

    #[test]
    fn rounding_differences_still_match_the_same_display() {
        let primary = display("Built-in", 0.0, 0.0, 1512.0, 982.0);
        let mut current = primary.clone();
        current.bounds.width += 0.25;
        assert_eq!(
            plan(Some(&primary), Some(&current), Some(WINDOW)),
            Placement::StayOnPrimary
        );
    }
}
