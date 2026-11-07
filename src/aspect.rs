//! Locks the aspect ratio of the timer window with the native window system
//! (Windows, macOS), so the ratio is kept while the user drags a window edge.
//! winit has no API for this. On Linux the app snaps the size after resizing instead,
//! because KWin ignores X11 aspect hints for XWayland windows.

use raw_window_handle::HasWindowHandle;

/// Returns true once the aspect ratio has been applied.
pub fn lock(window: &impl HasWindowHandle, width: u32, height: u32) -> bool {
    let Ok(handle) = window.window_handle() else {
        return false;
    };
    match handle.as_raw() {
        #[cfg(windows)]
        raw_window_handle::RawWindowHandle::Win32(h) => {
            win32::lock(h.hwnd.get() as _, width as f32 / height as f32)
        }
        #[cfg(target_os = "macos")]
        raw_window_handle::RawWindowHandle::AppKit(h) => unsafe {
            appkit::lock(h.ns_view.as_ptr(), width, height)
        },
        _ => {
            let _ = (width, height);
            false
        }
    }
}

#[cfg(windows)]
mod win32 {
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        UI::{
            Shell::{DefSubclassProc, SetWindowSubclass},
            WindowsAndMessaging::{
                GetClientRect, GetWindowRect, WM_SIZING, WMSZ_BOTTOM, WMSZ_TOP, WMSZ_TOPLEFT, WMSZ_TOPRIGHT,
            },
        },
    };

    const SUBCLASS_ID: usize = 0x5354_4f50;

    /// Hooks WM_SIZING and corrects the rectangle while the user drags.
    pub fn lock(hwnd: HWND, ratio: f32) -> bool {
        unsafe { SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, ratio.to_bits() as usize) != 0 }
    }

    unsafe extern "system" fn subclass_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        data: usize,
    ) -> LRESULT {
        if msg == WM_SIZING {
            let ratio = f32::from_bits(data as u32);
            unsafe {
                let rect = &mut *(lparam as *mut RECT);
                // The ratio applies to the client area, so account for the window frame.
                let (mut window, mut client) = (std::mem::zeroed::<RECT>(), std::mem::zeroed::<RECT>());
                GetWindowRect(hwnd, &mut window);
                GetClientRect(hwnd, &mut client);
                let extra_w = (window.right - window.left) - (client.right - client.left);
                let extra_h = (window.bottom - window.top) - (client.bottom - client.top);
                let client_w = (rect.right - rect.left - extra_w) as f32;
                let client_h = (rect.bottom - rect.top - extra_h) as f32;

                match wparam as u32 {
                    // Dragging the top/bottom edge: height leads.
                    WMSZ_TOP | WMSZ_BOTTOM => {
                        rect.right = rect.left + (client_h * ratio).round() as i32 + extra_w
                    }
                    // Top corners: width leads, the top edge moves.
                    WMSZ_TOPLEFT | WMSZ_TOPRIGHT => {
                        rect.top = rect.bottom - (client_w / ratio).round() as i32 - extra_h
                    }
                    // Left/right edges and bottom corners: width leads, the bottom edge moves.
                    _ => rect.bottom = rect.top + (client_w / ratio).round() as i32 + extra_h,
                }
            }
            return 1;
        }
        unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
    }
}

#[cfg(target_os = "macos")]
mod appkit {
    use objc2::{Encode, Encoding, msg_send, runtime::AnyObject};

    #[repr(C)]
    struct CGSize {
        width: f64,
        height: f64,
    }

    unsafe impl Encode for CGSize {
        const ENCODING: Encoding = Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
    }

    /// `[[view window] setContentAspectRatio:]`
    pub unsafe fn lock(ns_view: *mut std::ffi::c_void, width: u32, height: u32) -> bool {
        let view = ns_view.cast::<AnyObject>();
        let window: *mut AnyObject = unsafe { msg_send![view, window] };
        if window.is_null() {
            return false;
        }
        let size = CGSize {
            width: width as f64,
            height: height as f64,
        };
        let _: () = unsafe { msg_send![window, setContentAspectRatio: size] };
        true
    }
}
