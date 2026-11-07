//! Resolving the "follow system" color scheme.
//!
//! On Windows and macOS winit reports the OS theme and egui follows it.
//! On Linux (X11 / XWayland) winit cannot, so we ask the XDG desktop portal,
//! which is implemented by KDE, GNOME and most other desktops.

use eframe::egui;
use egui::ThemePreference;

pub struct SystemTheme {
    #[cfg(target_os = "linux")]
    scheme: std::sync::Arc<std::sync::atomic::AtomicU8>,
}

impl SystemTheme {
    pub fn new(ctx: &egui::Context) -> Self {
        #[cfg(target_os = "linux")]
        {
            Self {
                scheme: linux::spawn_watcher(ctx.clone()),
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = ctx;
            Self {}
        }
    }

    pub fn preference(&self) -> ThemePreference {
        #[cfg(target_os = "linux")]
        match self.scheme.load(std::sync::atomic::Ordering::Relaxed) {
            linux::DARK => return ThemePreference::Dark,
            linux::LIGHT => return ThemePreference::Light,
            _ => {}
        }
        ThemePreference::System
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicU8, Ordering},
        },
        thread,
        time::Duration,
    };

    use zbus::{
        blocking::{Connection, Proxy},
        zvariant::{OwnedValue, Value},
    };

    // Values of `org.freedesktop.appearance color-scheme`.
    pub const DARK: u8 = 1;
    pub const LIGHT: u8 = 2;

    /// Reads the color scheme once synchronously, then polls it in the background
    /// so that changes of the system setting are picked up while running.
    pub fn spawn_watcher(ctx: eframe::egui::Context) -> Arc<AtomicU8> {
        let scheme = Arc::new(AtomicU8::new(0));
        let Ok(conn) = Connection::session() else {
            return scheme;
        };
        scheme.store(read(&conn).unwrap_or(0), Ordering::Relaxed);

        let shared = scheme.clone();
        let _ = thread::Builder::new().name("color-scheme".into()).spawn(move || {
            loop {
                thread::sleep(Duration::from_secs(2));
                let value = read(&conn).unwrap_or(0);
                if shared.swap(value, Ordering::Relaxed) != value {
                    ctx.request_repaint();
                }
            }
        });
        scheme
    }

    fn read(conn: &Connection) -> Option<u8> {
        let proxy = Proxy::new(
            conn,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
        )
        .ok()?;
        let args = ("org.freedesktop.appearance", "color-scheme");
        let value: OwnedValue = match proxy.call("ReadOne", &args) {
            Ok(v) => v,
            // Older portals only have the deprecated `Read`, which wraps the value twice.
            Err(_) => proxy.call("Read", &args).ok()?,
        };
        let number = match &*value {
            Value::Value(inner) => u32::try_from(&**inner).ok()?,
            other => u32::try_from(other).ok()?,
        };
        u8::try_from(number).ok()
    }
}
