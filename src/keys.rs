//! Key combinations: parsing, display and conversion to the OS hotkey formats.

use eframe::egui;
use egui::Key;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyCombo {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// Cmd on macOS, Super/Windows key elsewhere.
    pub meta: bool,
    pub key: Key,
}

impl KeyCombo {
    /// Builds a combo from a key press inside the app. Returns `None` for keys
    /// that cannot be used as global hotkeys.
    pub fn from_event(key: Key, mods: egui::Modifiers) -> Option<Self> {
        let combo = Self {
            ctrl: mods.ctrl,
            alt: mods.alt,
            shift: mods.shift,
            meta: mods.mac_cmd,
            key,
        };
        combo.to_code().map(|_| combo)
    }

    /// Parses the portable format, e.g. `Ctrl+Shift+F9`.
    pub fn parse(s: &str) -> Option<Self> {
        let mut parts: Vec<&str> = s.split('+').map(str::trim).collect();
        let key = Key::from_name(parts.pop()?)?;
        let mut combo = Self {
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
            key,
        };
        for part in parts {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => combo.ctrl = true,
                "alt" | "option" => combo.alt = true,
                "shift" => combo.shift = true,
                "meta" | "cmd" | "super" | "win" => combo.meta = true,
                _ => return None,
            }
        }
        combo.to_code().map(|_| combo)
    }

    /// Portable format stored in the config file.
    pub fn to_config_string(self) -> String {
        self.join(["Ctrl", "Alt", "Shift", "Meta"])
    }

    /// Human readable, using the platform's names for modifier keys.
    pub fn display(self) -> String {
        if cfg!(target_os = "macos") {
            self.join(["Ctrl", "Option", "Shift", "Cmd"])
        } else {
            self.join(["Ctrl", "Alt", "Shift", "Super"])
        }
    }

    fn join(self, names: [&str; 4]) -> String {
        let flags = [self.ctrl, self.alt, self.shift, self.meta];
        let mut parts: Vec<&str> = names
            .iter()
            .zip(flags)
            .filter(|(_, on)| *on)
            .map(|(n, _)| *n)
            .collect();
        parts.push(self.key.name());
        parts.join("+")
    }

    pub fn to_hotkey(self) -> Option<HotKey> {
        let mut mods = Modifiers::empty();
        mods.set(Modifiers::CONTROL, self.ctrl);
        mods.set(Modifiers::ALT, self.alt);
        mods.set(Modifiers::SHIFT, self.shift);
        mods.set(Modifiers::META, self.meta);
        Some(HotKey::new(Some(mods), self.to_code()?))
    }

    fn to_code(self) -> Option<Code> {
        use Code as C;
        Some(match self.key {
            Key::ArrowDown => C::ArrowDown,
            Key::ArrowLeft => C::ArrowLeft,
            Key::ArrowRight => C::ArrowRight,
            Key::ArrowUp => C::ArrowUp,
            Key::Escape => C::Escape,
            Key::Tab => C::Tab,
            Key::Backspace => C::Backspace,
            Key::Enter => C::Enter,
            Key::Space => C::Space,
            Key::Insert => C::Insert,
            Key::Delete => C::Delete,
            Key::Home => C::Home,
            Key::End => C::End,
            Key::PageUp => C::PageUp,
            Key::PageDown => C::PageDown,
            Key::Comma => C::Comma,
            Key::Period => C::Period,
            Key::Minus => C::Minus,
            Key::Equals => C::Equal,
            Key::Semicolon => C::Semicolon,
            Key::Quote => C::Quote,
            Key::Slash => C::Slash,
            Key::Backslash => C::Backslash,
            Key::OpenBracket => C::BracketLeft,
            Key::CloseBracket => C::BracketRight,
            Key::Backtick => C::Backquote,
            Key::Num0 => C::Digit0,
            Key::Num1 => C::Digit1,
            Key::Num2 => C::Digit2,
            Key::Num3 => C::Digit3,
            Key::Num4 => C::Digit4,
            Key::Num5 => C::Digit5,
            Key::Num6 => C::Digit6,
            Key::Num7 => C::Digit7,
            Key::Num8 => C::Digit8,
            Key::Num9 => C::Digit9,
            Key::A => C::KeyA,
            Key::B => C::KeyB,
            Key::C => C::KeyC,
            Key::D => C::KeyD,
            Key::E => C::KeyE,
            Key::F => C::KeyF,
            Key::G => C::KeyG,
            Key::H => C::KeyH,
            Key::I => C::KeyI,
            Key::J => C::KeyJ,
            Key::K => C::KeyK,
            Key::L => C::KeyL,
            Key::M => C::KeyM,
            Key::N => C::KeyN,
            Key::O => C::KeyO,
            Key::P => C::KeyP,
            Key::Q => C::KeyQ,
            Key::R => C::KeyR,
            Key::S => C::KeyS,
            Key::T => C::KeyT,
            Key::U => C::KeyU,
            Key::V => C::KeyV,
            Key::W => C::KeyW,
            Key::X => C::KeyX,
            Key::Y => C::KeyY,
            Key::Z => C::KeyZ,
            Key::F1 => C::F1,
            Key::F2 => C::F2,
            Key::F3 => C::F3,
            Key::F4 => C::F4,
            Key::F5 => C::F5,
            Key::F6 => C::F6,
            Key::F7 => C::F7,
            Key::F8 => C::F8,
            Key::F9 => C::F9,
            Key::F10 => C::F10,
            Key::F11 => C::F11,
            Key::F12 => C::F12,
            Key::F13 => C::F13,
            Key::F14 => C::F14,
            Key::F15 => C::F15,
            Key::F16 => C::F16,
            Key::F17 => C::F17,
            Key::F18 => C::F18,
            Key::F19 => C::F19,
            Key::F20 => C::F20,
            Key::F21 => C::F21,
            Key::F22 => C::F22,
            Key::F23 => C::F23,
            Key::F24 => C::F24,
            _ => return None,
        })
    }

    /// Qt key code (key | modifiers) as expected by KDE's KGlobalAccel.
    #[cfg(target_os = "linux")]
    pub fn to_qt(self) -> Option<i32> {
        const SHIFT: i32 = 0x0200_0000;
        const CTRL: i32 = 0x0400_0000;
        const ALT: i32 = 0x0800_0000;
        const META: i32 = 0x1000_0000;

        let name = self.key.name();
        let key = match self.key {
            Key::Escape => 0x0100_0000,
            Key::Tab => 0x0100_0001,
            Key::Backspace => 0x0100_0003,
            Key::Enter => 0x0100_0004,
            Key::Insert => 0x0100_0006,
            Key::Delete => 0x0100_0007,
            Key::Home => 0x0100_0010,
            Key::End => 0x0100_0011,
            Key::ArrowLeft => 0x0100_0012,
            Key::ArrowUp => 0x0100_0013,
            Key::ArrowRight => 0x0100_0014,
            Key::ArrowDown => 0x0100_0015,
            Key::PageUp => 0x0100_0016,
            Key::PageDown => 0x0100_0017,
            Key::Space => 0x20,
            Key::Quote => 0x27,
            Key::Comma => 0x2c,
            Key::Minus => 0x2d,
            Key::Period => 0x2e,
            Key::Slash => 0x2f,
            Key::Semicolon => 0x3b,
            Key::Equals => 0x3d,
            Key::OpenBracket => 0x5b,
            Key::Backslash => 0x5c,
            Key::CloseBracket => 0x5d,
            Key::Backtick => 0x60,
            // Letters and digits map to their ASCII code.
            _ if name.len() == 1 => name.as_bytes()[0] as i32,
            // F1..F35 are consecutive in Qt.
            _ if name.starts_with('F') => 0x0100_0030 + name[1..].parse::<i32>().ok()? - 1,
            _ => return None,
        };
        let mut code = key;
        if self.shift {
            code |= SHIFT;
        }
        if self.ctrl {
            code |= CTRL;
        }
        if self.alt {
            code |= ALT;
        }
        if self.meta {
            code |= META;
        }
        Some(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_format() {
        let c = KeyCombo::parse("Ctrl+Shift+F9").unwrap();
        assert!(c.ctrl && c.shift && !c.alt && !c.meta);
        assert_eq!(c.key, Key::F9);
        assert_eq!(c.to_config_string(), "Ctrl+Shift+F9");
        assert_eq!(
            KeyCombo::parse("Meta+Alt+1").unwrap().to_config_string(),
            "Alt+Meta+1"
        );
        assert!(KeyCombo::parse("Hyper+A").is_none());
        assert!(KeyCombo::parse("").is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn qt_codes() {
        assert_eq!(
            KeyCombo::parse("Ctrl+Shift+F9").unwrap().to_qt(),
            Some(0x0700_0038)
        );
        assert_eq!(KeyCombo::parse("Alt+S").unwrap().to_qt(), Some(0x0800_0053));
        assert_eq!(KeyCombo::parse("Space").unwrap().to_qt(), Some(0x20));
    }
}
