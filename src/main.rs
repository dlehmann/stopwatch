// No console window on Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod aspect;
mod config;
mod history;
mod hotkeys;
mod i18n;
mod keys;
mod stopwatch;
mod theme;

use eframe::egui::{ViewportBuilder, WindowLevel};

fn main() -> eframe::Result {
    let config = config::Config::load();
    let title = i18n::tr(config.language.resolve(), i18n::T::AppTitle);

    // On Linux only the X11 backend of winit is compiled in: Wayland does not let
    // applications keep their window on top, X11 (and XWayland) does.
    let viewport = ViewportBuilder::default()
        .with_title(title)
        .with_app_id("stopwatch")
        .with_icon(std::sync::Arc::new(
            eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).expect("valid icon"),
        ))
        .with_inner_size(app::BASE_SIZE)
        .with_min_inner_size(app::BASE_SIZE * app::MIN_SCALE)
        // Maximizing would break the fixed aspect ratio.
        .with_maximize_button(false)
        .with_window_level(WindowLevel::AlwaysOnTop);

    let options = eframe::NativeOptions {
        viewport,
        // Remember window position and size between runs.
        persist_window: true,
        ..Default::default()
    };

    eframe::run_native(
        "stopwatch",
        options,
        Box::new(move |cc| Ok(Box::new(app::StopwatchApp::new(cc, config)))),
    )
}
