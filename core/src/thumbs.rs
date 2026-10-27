//! DWM's live thumbnails: one window drawn inside another, kept up to date by DWM itself.
//!
//! A thumbnail draws on top of everything in the window it's registered on (a WebView included),
//! and takes no input: clicks and keys reach the window underneath as usual. Thumbnails registered
//! later draw above earlier ones. Everything here is in physical pixels.

use crate::ffi::*;

/// Where a window is: what's seen of it on the screen, and that part of the window (its
/// thumbnail's source rect, from its top-left corner). A minimized window has only where it would
/// be restored to, from its placement, and no source rect.
pub fn bounds(hwnd: isize) -> Option<(RECT, Option<RECT>)> {
    unsafe {
        if IsIconic(hwnd) != 0 {
            let mut wp = WINDOWPLACEMENT { length: std::mem::size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
            return (GetWindowPlacement(hwnd, &mut wp) != 0).then_some((wp.rcNormalPosition, None));
        }
        let mut whole = RECT::default();
        if GetWindowRect(hwnd, &mut whole) == 0 {
            return None;
        }
        // The extended frame bounds leave out the invisible borders Windows 10 and 11 put around
        // a window to grab for resizing; a thumbnail would show them as a clear margin.
        let mut seen = RECT::default();
        if DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, (&mut seen as *mut RECT).cast(), 16) != 0 {
            seen = whole;
        }
        let crop = RECT { left: seen.left - whole.left, top: seen.top - whole.top, right: seen.right - whole.left, bottom: seen.bottom - whole.top };
        Some((seen, Some(crop)))
    }
}

/// Draw `source` on `dest` (hidden until placed). None if DWM won't: the window has gone, or
/// isn't a top-level window.
pub fn register(dest: isize, source: isize) -> Option<isize> {
    let mut id = 0isize;
    (unsafe { DwmRegisterThumbnail(dest, source, &mut id) } >= 0).then_some(id)
}

pub fn unregister(id: isize) {
    unsafe { DwmUnregisterThumbnail(id) };
}

/// Show a thumbnail at `at` (in `dest`'s client area), `opacity` of 255; `crop` is the part of
/// the source to show, or None for all of it.
pub fn place(id: isize, crop: Option<RECT>, at: RECT, opacity: u8) -> bool {
    let props = DWM_THUMBNAIL_PROPERTIES {
        dwFlags: DWM_TNP_RECTDESTINATION | DWM_TNP_OPACITY | DWM_TNP_VISIBLE | if crop.is_some() { DWM_TNP_RECTSOURCE } else { 0 },
        rcDestination: at,
        rcSource: crop.unwrap_or_default(),
        opacity,
        fVisible: 1,
        fSourceClientAreaOnly: 0,
    };
    unsafe { DwmUpdateThumbnailProperties(id, &props) >= 0 }
}

/// Wait for DWM's next frame.
pub fn next_frame() {
    unsafe { DwmFlush() };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnail_properties_are_packed() {
        assert_eq!(std::mem::size_of::<DWM_THUMBNAIL_PROPERTIES>(), 45);
    }

    #[test]
    fn a_window_that_is_not_there_has_no_thumbnail() {
        assert!(register(0, 0).is_none());
        assert!(bounds(0).is_none());
    }
}
