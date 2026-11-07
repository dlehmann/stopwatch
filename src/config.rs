//! Persistent user settings, stored as JSON in the OS config directory.

use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::{i18n::Lang, stopwatch::Action};

/// Directory for all files of the app (settings and measurement history).
pub fn config_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "stopwatch").map(|dirs| dirs.config_dir().to_path_buf())
}

/// Writes via a temporary file so a crash never leaves a truncated file behind.
pub fn write_atomic(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, contents)?;
    fs::rename(tmp, path)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguageMode {
    #[default]
    System,
    De,
    En,
}

impl LanguageMode {
    pub fn resolve(self) -> Lang {
        match self {
            LanguageMode::System => Lang::from_system(),
            LanguageMode::De => Lang::De,
            LanguageMode::En => Lang::En,
        }
    }
}

/// Hotkeys as portable strings like `Ctrl+Shift+F9`; `None` means unassigned.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hotkeys {
    pub start_stop: Option<String>,
    pub pause: Option<String>,
    pub resume: Option<String>,
    pub reset: Option<String>,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            start_stop: Some("Ctrl+Shift+F9".into()),
            pause: Some("Ctrl+Shift+F10".into()),
            resume: Some("Ctrl+Shift+F11".into()),
            reset: Some("Ctrl+Shift+F12".into()),
        }
    }
}

impl Hotkeys {
    pub fn get(&self, action: Action) -> Option<&str> {
        self.slot(action).as_deref()
    }

    pub fn set(&mut self, action: Action, value: Option<String>) {
        *self.slot_mut(action) = value;
    }

    fn slot(&self, action: Action) -> &Option<String> {
        match action {
            Action::StartStop => &self.start_stop,
            Action::Pause => &self.pause,
            Action::Resume => &self.resume,
            Action::Reset => &self.reset,
        }
    }

    fn slot_mut(&mut self, action: Action) -> &mut Option<String> {
        match action {
            Action::StartStop => &mut self.start_stop,
            Action::Pause => &mut self.pause,
            Action::Resume => &mut self.resume,
            Action::Reset => &mut self.reset,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: ThemeMode,
    pub language: LanguageMode,
    pub hotkeys: Hotkeys,
}

impl Config {
    pub fn path() -> Option<PathBuf> {
        config_dir().map(|dir| dir.join("config.json"))
    }

    /// Loads the config; missing or broken files fall back to defaults.
    pub fn load() -> Self {
        Self::path()
            .and_then(|p| fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::path().ok_or_else(|| std::io::Error::other("no config directory"))?;
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        write_atomic(&path, &json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_partial_files() {
        let mut cfg = Config::default();
        cfg.theme = ThemeMode::Dark;
        cfg.hotkeys.set(Action::Pause, None);
        let json = serde_json::to_string(&cfg).unwrap();
        assert_eq!(serde_json::from_str::<Config>(&json).unwrap(), cfg);

        let partial: Config = serde_json::from_str(r#"{"language":"en"}"#).unwrap();
        assert_eq!(partial.language, LanguageMode::En);
        assert_eq!(partial.hotkeys, Hotkeys::default());
    }
}
