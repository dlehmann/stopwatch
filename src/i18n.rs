//! German and English UI texts.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    De,
    En,
}

impl Lang {
    /// German if the OS locale is German, English otherwise.
    pub fn from_system() -> Self {
        match sys_locale::get_locale() {
            Some(locale) if locale.to_lowercase().starts_with("de") => Lang::De,
            _ => Lang::En,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum T {
    AppTitle,
    Start,
    Stop,
    Restart,
    Pause,
    Resume,
    Reset,
    Settings,
    StateIdle,
    StateRunning,
    StatePaused,
    StateStopped,
    Hotkeys,
    ActionStartStop,
    PressKey,
    NotSet,
    ClearHotkey,
    ClickToChange,
    RestoreDefaults,
    HotkeysSuspended,
    SharedPauseHint,
    HotkeyFailed,
    HotkeysFailedShort,
    HotkeysUnavailable,
    HotkeyBackend,
    Appearance,
    Theme,
    ThemeSystem,
    ThemeLight,
    ThemeDark,
    Language,
    LanguageSystem,
    Close,
    SaveFailed,
    History,
    DateTime,
    Duration,
    NoMeasurements,
    ClearAll,
    ConfirmClear,
    ConfirmClearYes,
    Cancel,
    DeleteEntry,
}

pub fn tr(lang: Lang, text: T) -> &'static str {
    use T::*;
    let (de, en) = match text {
        AppTitle => ("Stoppuhr", "Stopwatch"),
        Start => ("Start", "Start"),
        Stop => ("Stopp", "Stop"),
        Restart => ("Neustart", "Restart"),
        Pause => ("Pause", "Pause"),
        Resume => ("Fortsetzen", "Resume"),
        Reset => ("Zurücksetzen", "Reset"),
        Settings => ("Einstellungen", "Settings"),
        StateIdle => ("Bereit", "Ready"),
        StateRunning => ("Läuft", "Running"),
        StatePaused => ("Pausiert", "Paused"),
        StateStopped => ("Gestoppt", "Stopped"),
        Hotkeys => ("Globale Tastenkürzel", "Global hotkeys"),
        ActionStartStop => ("Start / Stopp", "Start / Stop"),
        PressKey => ("Taste drücken … (Esc = Abbruch)", "Press a key … (Esc = cancel)"),
        NotSet => ("— nicht belegt —", "— not set —"),
        ClearHotkey => ("Tastenkürzel entfernen", "Remove hotkey"),
        ClickToChange => (
            "Klicken, um ein neues Tastenkürzel festzulegen",
            "Click to set a new hotkey",
        ),
        RestoreDefaults => ("Standardwerte", "Defaults"),
        SharedPauseHint => (
            "Pause und Fortsetzen dürfen dieselbe Taste haben – sie schaltet dann zwischen beiden um.",
            "Pause and resume may use the same key – it then toggles between the two.",
        ),
        HotkeysSuspended => (
            "Solange dieses Fenster offen ist, sind die globalen Tastenkürzel deaktiviert.",
            "Global hotkeys are disabled while this window is open.",
        ),
        HotkeyFailed => (
            "Konnte nicht registriert werden – vermutlich von einer anderen Anwendung belegt.",
            "Could not be registered – probably used by another application.",
        ),
        HotkeysFailedShort => ("⚠ Tastenkürzel belegt", "⚠ Hotkey unavailable"),
        HotkeysUnavailable => (
            "Globale Tastenkürzel werden in dieser Umgebung nicht unterstützt.",
            "Global hotkeys are not supported in this environment.",
        ),
        HotkeyBackend => ("Registriert über", "Registered via"),
        Appearance => ("Darstellung", "Appearance"),
        Theme => ("Farbschema", "Color scheme"),
        ThemeSystem => ("Wie System", "Follow system"),
        ThemeLight => ("Hell", "Light"),
        ThemeDark => ("Dunkel", "Dark"),
        Language => ("Sprache", "Language"),
        LanguageSystem => ("Wie System", "Follow system"),
        Close => ("Schließen", "Close"),
        SaveFailed => (
            "Einstellungen konnten nicht gespeichert werden",
            "Could not save settings",
        ),
        History => ("Letzte Messungen", "Recent measurements"),
        DateTime => ("Datum / Uhrzeit", "Date / time"),
        Duration => ("Dauer", "Duration"),
        NoMeasurements => ("Noch keine Messungen.", "No measurements yet."),
        ClearAll => ("🗑 Alle löschen", "🗑 Delete all"),
        ConfirmClear => (
            "Alle Messungen unwiderruflich löschen?",
            "Delete all measurements permanently?",
        ),
        ConfirmClearYes => ("Ja, alle löschen", "Yes, delete all"),
        Cancel => ("Abbrechen", "Cancel"),
        DeleteEntry => ("Diese Messung löschen", "Delete this measurement"),
    };
    match lang {
        Lang::De => de,
        Lang::En => en,
    }
}
