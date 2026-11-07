//! System-wide hotkeys that work while the window is not focused.
//!
//! * Windows, macOS, Linux/X11: the `global-hotkey` crate (RegisterHotKey,
//!   Carbon hot keys, XGrabKey).
//! * Linux with KDE Plasma (also on Wayland): KDE's KGlobalAccel service via D-Bus,
//!   because X11 key grabs do not see keys pressed in Wayland windows.

#[cfg(target_os = "linux")]
mod kde;

use eframe::egui;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender, channel},
    },
};

use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};

use crate::{keys::KeyCombo, stopwatch::Action};

trait Backend {
    fn name(&self) -> &'static str;
    /// Replaces all registered hotkeys. Returns the actions that could not be registered.
    fn apply(&mut self, bindings: &[(Action, KeyCombo)]) -> Vec<Action>;
    /// Releases the hotkeys before the app exits.
    fn shutdown(&mut self) {}
}

pub struct Hotkeys {
    backend: Option<Box<dyn Backend>>,
    events: Receiver<Action>,
}

impl Hotkeys {
    /// Must be called on the main thread (required on macOS and Windows).
    pub fn new(ctx: &egui::Context) -> Self {
        let (tx, events) = channel();
        let backend = Self::create_backend(ctx, tx);
        Self { backend, events }
    }

    fn create_backend(ctx: &egui::Context, tx: Sender<Action>) -> Option<Box<dyn Backend>> {
        #[cfg(target_os = "linux")]
        match kde::KdeBackend::new(ctx.clone(), tx.clone()) {
            Ok(backend) => return Some(Box::new(backend)),
            Err(err) => eprintln!("KGlobalAccel unavailable ({err}), falling back to X11 key grabs"),
        }
        match GenericBackend::new(ctx.clone(), tx) {
            Ok(backend) => Some(Box::new(backend)),
            Err(err) => {
                eprintln!("global hotkeys unavailable: {err}");
                None
            }
        }
    }

    pub fn backend_name(&self) -> Option<&'static str> {
        self.backend.as_ref().map(|b| b.name())
    }

    pub fn apply(&mut self, bindings: &[(Action, KeyCombo)]) -> Vec<Action> {
        match &mut self.backend {
            Some(backend) => backend.apply(bindings),
            None => bindings.iter().map(|(a, _)| *a).collect(),
        }
    }

    /// Hotkey presses received since the last call.
    pub fn poll(&self) -> impl Iterator<Item = Action> + '_ {
        self.events.try_iter()
    }

    pub fn shutdown(&mut self) {
        if let Some(backend) = &mut self.backend {
            backend.shutdown();
        }
    }
}

/// Closes the app cleanly on SIGTERM/SIGINT/SIGHUP (e.g. `kill`, Ctrl+C, logout), so that
/// `Hotkeys::shutdown` runs. Grabs of the other backends end with the process anyway,
/// but KDE keeps the keys reserved until they are explicitly deactivated.
pub fn close_on_signal(ctx: &egui::Context) {
    #[cfg(unix)]
    {
        use signal_hook::{
            consts::{SIGHUP, SIGINT, SIGTERM},
            iterator::Signals,
        };
        let Ok(mut signals) = Signals::new([SIGTERM, SIGINT, SIGHUP]) else {
            return;
        };
        let ctx = ctx.clone();
        let _ = std::thread::Builder::new().name("signals".into()).spawn(move || {
            if signals.forever().next().is_some() {
                ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
                ctx.request_repaint();
                // Fallback in case the UI does not process the close request (e.g. while hidden).
                std::thread::sleep(std::time::Duration::from_secs(3));
                #[cfg(target_os = "linux")]
                kde::release_all();
                std::process::exit(0);
            }
        });
    }
    #[cfg(not(unix))]
    let _ = ctx;
}

/// Backend based on the `global-hotkey` crate.
struct GenericBackend {
    manager: GlobalHotKeyManager,
    registered: Vec<HotKey>,
    actions: Arc<Mutex<HashMap<u32, Action>>>,
}

impl GenericBackend {
    fn new(ctx: egui::Context, tx: Sender<Action>) -> global_hotkey::Result<Self> {
        let manager = GlobalHotKeyManager::new()?;
        let actions: Arc<Mutex<HashMap<u32, Action>>> = Arc::default();
        let lookup = actions.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.state != HotKeyState::Pressed {
                return;
            }
            if let Some(action) = lookup.lock().unwrap().get(&event.id).copied() {
                let _ = tx.send(action);
                // Wake up the UI, even if the window is unfocused or minimized.
                ctx.request_repaint();
            }
        }));
        Ok(Self {
            manager,
            registered: Vec::new(),
            actions,
        })
    }
}

impl Backend for GenericBackend {
    fn name(&self) -> &'static str {
        if cfg!(target_os = "windows") {
            "Windows"
        } else if cfg!(target_os = "macos") {
            "macOS"
        } else {
            "X11"
        }
    }

    fn apply(&mut self, bindings: &[(Action, KeyCombo)]) -> Vec<Action> {
        let _ = self.manager.unregister_all(&self.registered);
        self.registered.clear();
        let mut actions = self.actions.lock().unwrap();
        actions.clear();

        let mut failed = Vec::new();
        for &(action, combo) in bindings {
            match combo.to_hotkey().map(|hk| (hk, self.manager.register(hk))) {
                Some((hotkey, Ok(()))) => {
                    self.registered.push(hotkey);
                    actions.insert(hotkey.id(), action);
                }
                _ => failed.push(action),
            }
        }
        failed
    }

    fn shutdown(&mut self) {
        let _ = self.manager.unregister_all(&self.registered);
        self.registered.clear();
    }
}
