//! KDE Plasma global shortcuts via the KGlobalAccel D-Bus service.
//! Works on X11 and Wayland sessions.

use eframe::egui;
use std::{sync::mpsc::Sender, thread};

use zbus::{
    MatchRule,
    blocking::{Connection, MessageIterator, Proxy},
    message::Type,
    zvariant::OwnedObjectPath,
};

use super::Backend;
use crate::{keys::KeyCombo, stopwatch::Action};

const SERVICE: &str = "org.kde.kglobalaccel";
const COMPONENT: &str = "stopwatch";
const COMPONENT_FRIENDLY: &str = "Stopwatch";

// Flags of KGlobalAccel::setShortcut.
const SET_PRESENT: u32 = 2;
const NO_AUTOLOADING: u32 = 4;

pub struct KdeBackend {
    proxy: Proxy<'static>,
    active: Vec<(Action, Vec<i32>)>,
}

fn action_id(action: Action) -> Vec<&'static str> {
    let friendly = match action {
        Action::StartStop => "Start / Stop",
        Action::Pause => "Pause",
        Action::Resume => "Resume",
        Action::Reset => "Reset",
    };
    vec![COMPONENT, action.id(), COMPONENT_FRIENDLY, friendly]
}

impl KdeBackend {
    pub fn new(ctx: egui::Context, tx: Sender<Action>) -> zbus::Result<Self> {
        let conn = Connection::session()?;
        let proxy = Proxy::new_owned(conn.clone(), SERVICE, "/kglobalaccel", "org.kde.KGlobalAccel")?;
        for action in Action::ALL {
            proxy.call_method("doRegister", &(action_id(action),))?;
        }
        let component: OwnedObjectPath = proxy.call("getComponent", &(COMPONENT,))?;

        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .interface("org.kde.kglobalaccel.Component")?
            .member("globalShortcutPressed")?
            .path(component)?
            .build();
        let messages = MessageIterator::for_match_rule(rule, &conn, None)?;
        thread::Builder::new()
            .name("kglobalaccel".into())
            .spawn(move || {
                for msg in messages.flatten() {
                    let Ok((comp, name, _timestamp)) = msg.body().deserialize::<(String, String, i64)>()
                    else {
                        continue;
                    };
                    if let (COMPONENT, Some(action)) = (comp.as_str(), Action::from_id(&name)) {
                        let _ = tx.send(action);
                        ctx.request_repaint();
                    }
                }
            })?;

        Ok(Self {
            proxy,
            active: Vec::new(),
        })
    }

    fn set_shortcut(&self, action: Action, keys: &[i32], flags: u32) -> zbus::Result<Vec<i32>> {
        self.proxy.call("setShortcut", &(action_id(action), keys, flags))
    }
}

impl Backend for KdeBackend {
    fn name(&self) -> &'static str {
        "KDE KGlobalAccel"
    }

    fn apply(&mut self, bindings: &[(Action, KeyCombo)]) -> Vec<Action> {
        self.active.clear();
        let mut failed = Vec::new();
        for action in Action::ALL {
            let combo = bindings.iter().find(|(a, _)| *a == action).map(|(_, c)| *c);
            let keys: Vec<i32> = combo.and_then(KeyCombo::to_qt).into_iter().collect();
            if combo.is_some() && keys.is_empty() {
                failed.push(action);
            }
            // NO_AUTOLOADING: our config file is the source of truth, not kglobalaccel's.
            match self.set_shortcut(action, &keys, SET_PRESENT | NO_AUTOLOADING) {
                // KGlobalAccel returns the keys it actually assigned; an empty result
                // means the key is already taken by another component.
                Ok(assigned) if assigned == keys => {
                    if !keys.is_empty() {
                        self.active.push((action, keys));
                    }
                }
                _ if keys.is_empty() => {}
                _ => failed.push(action),
            }
        }
        failed
    }

    fn shutdown(&mut self) {
        // Mark the shortcuts as "not present" so KDE releases the keys while we are not running.
        for (action, keys) in std::mem::take(&mut self.active) {
            let _ = self.set_shortcut(action, &keys, NO_AUTOLOADING);
        }
    }
}
