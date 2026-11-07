//! Stopwatch state machine, independent of any UI.

use std::time::{Duration, Instant};

/// Everything the user can trigger, via buttons or global hotkeys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    StartStop,
    Pause,
    Resume,
    Reset,
}

impl Action {
    pub const ALL: [Action; 4] = [Action::StartStop, Action::Pause, Action::Resume, Action::Reset];

    /// Stable identifier, used for config keys and OS hotkey registration.
    pub fn id(self) -> &'static str {
        match self {
            Action::StartStop => "start_stop",
            Action::Pause => "pause",
            Action::Resume => "resume",
            Action::Reset => "reset",
        }
    }

    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.id() == id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Showing zero, nothing measured yet.
    Idle,
    Running,
    /// Temporarily halted, can be resumed.
    Paused,
    /// Measurement finished; the final time stays visible until reset or restart.
    Stopped,
}

#[derive(Debug)]
pub struct Stopwatch {
    state: State,
    accumulated: Duration,
    running_since: Option<Instant>,
}

impl Default for Stopwatch {
    fn default() -> Self {
        Self {
            state: State::Idle,
            accumulated: Duration::ZERO,
            running_since: None,
        }
    }
}

impl Stopwatch {
    pub fn state(&self) -> State {
        self.state
    }

    pub fn elapsed(&self) -> Duration {
        self.accumulated + self.running_since.map(|t| t.elapsed()).unwrap_or_default()
    }

    /// Applies an action. Actions that make no sense in the current state are ignored.
    ///
    /// Returns the measured time when a measurement ends, i.e. on stop, or on reset
    /// while running or paused.
    pub fn apply(&mut self, action: Action) -> Option<Duration> {
        let before = self.state;
        let elapsed = self.elapsed();
        match (action, self.state) {
            (Action::StartStop, State::Idle) => self.run(),
            (Action::StartStop, State::Running | State::Paused) => self.halt(State::Stopped),
            (Action::StartStop, State::Stopped) => {
                // Starting again after a stop begins a fresh measurement.
                self.reset();
                self.run();
            }
            (Action::Pause, State::Running) => self.halt(State::Paused),
            (Action::Resume, State::Paused) => self.run(),
            (Action::Reset, _) => self.reset(),
            _ => {}
        }
        let ended = matches!(before, State::Running | State::Paused)
            && matches!(self.state, State::Stopped | State::Idle);
        match self.state {
            State::Stopped if ended => Some(self.elapsed()),
            State::Idle if ended => Some(elapsed),
            _ => None,
        }
    }

    fn run(&mut self) {
        self.running_since = Some(Instant::now());
        self.state = State::Running;
    }

    fn halt(&mut self, new_state: State) {
        if let Some(since) = self.running_since.take() {
            self.accumulated += since.elapsed();
        }
        self.state = new_state;
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Formats as `HH:MM:SS.cc` (fixed width so the display does not jump).
pub fn format_duration(d: Duration) -> String {
    let centis = d.as_millis() / 10;
    let (h, rest) = (centis / 360_000, centis % 360_000);
    let (m, rest) = (rest / 6_000, rest % 6_000);
    let (s, c) = (rest / 100, rest % 100);
    format!("{h:02}:{m:02}:{s:02}.{c:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_durations() {
        assert_eq!(format_duration(Duration::ZERO), "00:00:00.00");
        assert_eq!(format_duration(Duration::from_millis(61_234)), "00:01:01.23");
        assert_eq!(format_duration(Duration::from_secs(3 * 3600 + 5)), "03:00:05.00");
    }

    #[test]
    fn state_transitions() {
        let mut sw = Stopwatch::default();
        sw.apply(Action::Pause);
        assert_eq!(sw.state(), State::Idle);
        sw.apply(Action::StartStop);
        assert_eq!(sw.state(), State::Running);
        sw.apply(Action::Resume);
        assert_eq!(sw.state(), State::Running);
        sw.apply(Action::Pause);
        assert_eq!(sw.state(), State::Paused);
        let paused_at = sw.elapsed();
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(sw.elapsed(), paused_at, "time must not advance while paused");
        sw.apply(Action::Resume);
        assert_eq!(sw.state(), State::Running);
        sw.apply(Action::StartStop);
        assert_eq!(sw.state(), State::Stopped);
        sw.apply(Action::StartStop);
        assert_eq!(sw.state(), State::Running);
        assert!(
            sw.elapsed() < paused_at + Duration::from_millis(15),
            "restart begins at zero"
        );
        sw.apply(Action::Reset);
        assert_eq!(sw.state(), State::Idle);
        assert_eq!(sw.elapsed(), Duration::ZERO);
    }

    #[test]
    fn reports_finished_measurements() {
        let mut sw = Stopwatch::default();
        assert_eq!(sw.apply(Action::StartStop), None);
        std::thread::sleep(Duration::from_millis(10));
        assert_eq!(sw.apply(Action::Pause), None);
        let stopped = sw.apply(Action::StartStop).expect("stop ends a measurement");
        assert!(stopped >= Duration::from_millis(10));
        assert_eq!(sw.apply(Action::Reset), None, "already recorded on stop");
        sw.apply(Action::StartStop);
        assert!(
            sw.apply(Action::Reset).is_some(),
            "reset while running ends a measurement"
        );
        assert_eq!(sw.apply(Action::Reset), None);
    }
}
