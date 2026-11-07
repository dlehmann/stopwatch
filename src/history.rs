//! Persistent list of finished measurements.

use std::{fs, path::PathBuf, time::Duration};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::config::{config_dir, write_atomic};

/// Oldest entries are dropped beyond this many measurements.
const MAX_ENTRIES: usize = 1000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    /// When the measurement ended.
    pub finished_at: DateTime<Local>,
    pub duration_ms: u64,
}

impl Measurement {
    pub fn duration(&self) -> Duration {
        Duration::from_millis(self.duration_ms)
    }
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct History {
    /// Oldest first.
    pub entries: Vec<Measurement>,
}

impl History {
    fn path() -> Option<PathBuf> {
        config_dir().map(|dir| dir.join("history.json"))
    }

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

    pub fn add(&mut self, duration: Duration) {
        self.entries.push(Measurement {
            finished_at: Local::now(),
            duration_ms: duration.as_millis().try_into().unwrap_or(u64::MAX),
        });
        let excess = self.entries.len().saturating_sub(MAX_ENTRIES);
        self.entries.drain(..excess);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_caps_and_roundtrips() {
        let mut history = History::default();
        for i in 0..MAX_ENTRIES + 5 {
            history.add(Duration::from_millis(i as u64));
        }
        assert_eq!(history.entries.len(), MAX_ENTRIES);
        assert_eq!(history.entries[0].duration_ms, 5);

        let json = serde_json::to_string(&history).unwrap();
        assert_eq!(serde_json::from_str::<History>(&json).unwrap(), history);
    }
}
