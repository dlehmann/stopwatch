//! The egui application: timer window and settings window.

use eframe::egui;
use std::time::Duration;

use egui::{
    Align, Color32, Layout, RichText, ThemePreference, ViewportBuilder, ViewportCommand, ViewportId,
    WindowLevel,
};

use crate::{
    config::{Config, Hotkeys, LanguageMode, ThemeMode},
    history::History,
    hotkeys,
    i18n::{Lang, T, tr},
    keys::KeyCombo,
    stopwatch::{Action, State, Stopwatch, format_duration},
    theme::SystemTheme,
};

pub struct StopwatchApp {
    config: Config,
    stopwatch: Stopwatch,
    hotkeys: hotkeys::Hotkeys,
    system_theme: SystemTheme,
    /// Actions whose hotkey could not be registered.
    failed_hotkeys: Vec<Action>,
    settings_open: bool,
    /// The action whose hotkey is currently being recorded in the settings window.
    recording: Option<Action>,
    save_error: bool,
    applied_theme: Option<ThemePreference>,
    applied_lang: Option<Lang>,
    /// Frames shown since the timer / settings window was created (see `keep_on_top`).
    root_frames: u32,
    settings_frames: u32,
    aspect_frames: u32,
    history: History,
    history_open: bool,
    history_frames: u32,
    /// "Delete all" was clicked and waits for confirmation.
    confirm_clear: bool,
}

/// Size of the timer window at scale 1. Its aspect ratio is locked; when the window is
/// resized, all contents are scaled by `window width / BASE_SIZE.x`.
pub const BASE_SIZE: egui::Vec2 = egui::vec2(420.0, 170.0);
pub const MIN_SCALE: f32 = 0.6;

/// Font family used for the time display.
const CLOCK_FONT: &str = "FiraCode";

/// Embeds Fira Code (SIL Open Font License, see assets/fonts) so the binary needs no installed fonts.
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        CLOCK_FONT.to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/FiraCode-Regular.ttf"
        ))),
    );
    fonts.families.insert(
        egui::FontFamily::Name(CLOCK_FONT.into()),
        vec![CLOCK_FONT.to_owned()],
    );
    ctx.set_fonts(fonts);
}

/// Scales fonts and spacing so the whole timer window grows and shrinks with it.
fn scale_style(style: &mut egui::Style, scale: f32) {
    for font in style.text_styles.values_mut() {
        font.size *= scale;
    }
    let spacing = &mut style.spacing;
    spacing.item_spacing *= scale;
    spacing.button_padding *= scale;
    spacing.interact_size *= scale;
}

/// X11 window managers ignore the "always on top" request winit sends while the window
/// is still unmapped (eframe shows it only after the first frame), so it is repeated
/// during the first frames after the window became visible.
fn keep_on_top(ui: &egui::Ui, frames: &mut u32) {
    const REPEAT_FRAMES: u32 = 10;
    if *frames < REPEAT_FRAMES {
        *frames += 1;
        ui.ctx()
            .send_viewport_cmd(ViewportCommand::WindowLevel(WindowLevel::AlwaysOnTop));
        ui.ctx().request_repaint_after(Duration::from_millis(50));
    }
}

impl StopwatchApp {
    pub fn new(cc: &eframe::CreationContext<'_>, config: Config) -> Self {
        let ctx = &cc.egui_ctx;
        install_fonts(ctx);
        hotkeys::close_on_signal(ctx);
        let mut app = Self {
            config,
            stopwatch: Stopwatch::default(),
            hotkeys: hotkeys::Hotkeys::new(ctx),
            system_theme: SystemTheme::new(ctx),
            failed_hotkeys: Vec::new(),
            settings_open: false,
            recording: None,
            save_error: false,
            applied_theme: None,
            applied_lang: None,
            root_frames: 0,
            settings_frames: 0,
            aspect_frames: 0,
            history: History::load(),
            history_open: false,
            history_frames: 0,
            confirm_clear: false,
        };
        app.register_hotkeys();
        app
    }

    fn lang(&self) -> Lang {
        self.config.language.resolve()
    }

    fn t(&self, text: T) -> &'static str {
        tr(self.lang(), text)
    }

    fn action_label(&self, action: Action) -> &'static str {
        self.t(match action {
            Action::StartStop => T::ActionStartStop,
            Action::Pause => T::Pause,
            Action::Resume => T::Resume,
            Action::Reset => T::Reset,
        })
    }

    fn combo(&self, action: Action) -> Option<KeyCombo> {
        self.config.hotkeys.get(action).and_then(KeyCombo::parse)
    }

    /// Pause and resume may share one hotkey, which then toggles between the two.
    fn pause_resume_shared(&self) -> bool {
        self.combo(Action::Pause).is_some() && self.combo(Action::Pause) == self.combo(Action::Resume)
    }

    fn register_hotkeys(&mut self) {
        let shared = self.pause_resume_shared();
        let bindings: Vec<_> = Action::ALL
            .into_iter()
            // A shared combo can be registered only once; it is delivered as `Pause`.
            .filter(|a| !(shared && *a == Action::Resume))
            .filter_map(|a| Some((a, self.combo(a)?)))
            .collect();
        self.failed_hotkeys = self.hotkeys.apply(&bindings);
        if shared && self.failed_hotkeys.contains(&Action::Pause) {
            self.failed_hotkeys.push(Action::Resume);
        }
    }

    /// Applies an action and records finished measurements.
    fn apply(&mut self, action: Action) {
        if let Some(duration) = self.stopwatch.apply(action) {
            self.history.add(duration);
            self.save_error |= self.history.save().is_err();
        }
    }

    fn handle_hotkey(&mut self, action: Action) {
        let action = match action {
            Action::Pause if self.pause_resume_shared() && self.stopwatch.state() == State::Paused => {
                Action::Resume
            }
            other => other,
        };
        self.apply(action);
    }

    fn save(&mut self) {
        self.save_error = self.config.save().is_err();
    }

    fn open_settings(&mut self) {
        // Release the hotkeys so that the OS does not swallow them while recording new ones.
        self.hotkeys.apply(&[]);
        self.settings_open = true;
        self.settings_frames = 0;
    }

    fn close_settings(&mut self) {
        self.settings_open = false;
        self.recording = None;
        self.register_hotkeys();
        self.save();
    }

    /// Theme and language may change at any time (settings or OS), so they are synced every frame.
    fn sync_theme_and_language(&mut self, ctx: &egui::Context) {
        let theme = match self.config.theme {
            ThemeMode::System => self.system_theme.preference(),
            ThemeMode::Light => ThemePreference::Light,
            ThemeMode::Dark => ThemePreference::Dark,
        };
        if self.applied_theme != Some(theme) {
            ctx.set_theme(theme);
            self.applied_theme = Some(theme);
        }
        let lang = self.lang();
        if self.applied_lang != Some(lang) {
            ctx.send_viewport_cmd_to(
                ViewportId::ROOT,
                ViewportCommand::Title(tr(lang, T::AppTitle).into()),
            );
            self.applied_lang = Some(lang);
        }
    }

    /// Locks the window's aspect ratio once it is visible, and fixes a size restored
    /// from a previous run that does not match the ratio.
    fn lock_aspect_ratio(&mut self, ui: &egui::Ui, frame: &eframe::Frame) {
        // Runs during the first frames only (keep_on_top() keeps repainting then).
        // winit rewrites WM_NORMAL_HINTS while the window is being mapped and resized,
        // so on X11 the hint is applied a few times after the size correction.
        const LOCK_FRAMES: [u32; 3] = [4, 7, 10];
        if self.aspect_frames > 10 {
            return;
        }
        self.aspect_frames += 1;
        if self.aspect_frames == 2 {
            let size = ui.ctx().content_rect().size();
            let expected_height = size.x * BASE_SIZE.y / BASE_SIZE.x;
            if (size.y - expected_height).abs() > 1.0 {
                ui.ctx()
                    .send_viewport_cmd(ViewportCommand::InnerSize(egui::vec2(size.x, expected_height)));
            }
        }
        if LOCK_FRAMES.contains(&self.aspect_frames)
            && !crate::aspect::lock(frame, BASE_SIZE.x as u32, BASE_SIZE.y as u32)
            && self.aspect_frames == 10
        {
            eprintln!("could not lock the window aspect ratio");
        }
    }

    fn hotkey_hint(&self, action: Action) -> String {
        match self.combo(action) {
            Some(combo) => format!("{}: {}", self.action_label(action), combo.display()),
            None => format!("{}: {}", self.action_label(action), self.t(T::NotSet)),
        }
    }

    fn timer_ui(&mut self, ui: &mut egui::Ui, scale: f32) {
        let state = self.stopwatch.state();
        let text = format_duration(self.stopwatch.elapsed());

        let avail = ui.available_size();
        // Scale the digits with the window; Fira Code glyphs are 0.6 em wide.
        let size = (avail.x / (text.len() as f32 * 0.62))
            .min(avail.y * 0.42)
            .max(8.0);
        let color = match state {
            State::Running => ui.visuals().strong_text_color(),
            State::Paused => ui.visuals().warn_fg_color,
            State::Idle | State::Stopped => ui.visuals().text_color(),
        };

        ui.vertical_centered(|ui| {
            let font = egui::FontId::new(size, egui::FontFamily::Name(CLOCK_FONT.into()));
            ui.label(RichText::new(text).font(font).color(color));

            let status = self.t(match state {
                State::Idle => T::StateIdle,
                State::Running => T::StateRunning,
                State::Paused => T::StatePaused,
                State::Stopped => T::StateStopped,
            });
            ui.horizontal(|ui| {
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.label(RichText::new(status).weak());
                    if !self.failed_hotkeys.is_empty() {
                        let failed: Vec<_> =
                            self.failed_hotkeys.iter().map(|a| self.hotkey_hint(*a)).collect();
                        ui.label(
                            RichText::new(self.t(T::HotkeysFailedShort)).color(ui.visuals().warn_fg_color),
                        )
                        .on_hover_text(format!(
                            "{}\n{}",
                            self.t(T::HotkeyFailed),
                            failed.join("\n")
                        ));
                    }
                    if self.save_error {
                        ui.label(RichText::new("⚠").color(ui.visuals().error_fg_color))
                            .on_hover_text(self.t(T::SaveFailed));
                    }
                });
            });
        });

        ui.add_space(4.0);
        let start_label = self.t(match state {
            State::Idle => T::Start,
            State::Running | State::Paused => T::Stop,
            State::Stopped => T::Restart,
        });
        let (pause_label, pause_action) = match state {
            State::Paused => (self.t(T::Resume), Action::Resume),
            _ => (self.t(T::Pause), Action::Pause),
        };
        let pause_enabled = matches!(state, State::Running | State::Paused);

        let mut clicked = None;
        let mut settings = false;
        let mut history = false;
        let height = 28.0 * scale;
        let icon_width = height * 1.4;
        let spacing = ui.spacing().item_spacing.x;
        let text_width = ((ui.available_width() - 2.0 * icon_width - 4.0 * spacing) / 3.0).max(0.0);
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        ui.horizontal(|ui| {
            if ui
                .add_sized([text_width, height], egui::Button::new(start_label))
                .on_hover_text(self.hotkey_hint(Action::StartStop))
                .clicked()
            {
                clicked = Some(Action::StartStop);
            }
            if ui
                .add_enabled_ui(pause_enabled, |ui| {
                    ui.add_sized([text_width, height], egui::Button::new(pause_label))
                })
                .inner
                .on_hover_text(format!(
                    "{}\n{}",
                    self.hotkey_hint(Action::Pause),
                    self.hotkey_hint(Action::Resume)
                ))
                .clicked()
            {
                clicked = Some(pause_action);
            }
            if ui
                .add_enabled_ui(state != State::Idle, |ui| {
                    ui.add_sized([text_width, height], egui::Button::new(self.t(T::Reset)))
                })
                .inner
                .on_hover_text(self.hotkey_hint(Action::Reset))
                .clicked()
            {
                clicked = Some(Action::Reset);
            }
            history = ui
                .add_sized(
                    [icon_width, height],
                    egui::Button::new("🕘").selected(self.history_open),
                )
                .on_hover_text(self.t(T::History))
                .clicked();
            settings = ui
                .add_sized(
                    [icon_width, height],
                    egui::Button::new("⚙").selected(self.settings_open),
                )
                .on_hover_text(self.t(T::Settings))
                .clicked();
        });

        if let Some(action) = clicked {
            self.apply(action);
        }
        if history {
            self.history_open = !self.history_open;
            self.history_frames = 0;
            self.confirm_clear = false;
        }
        if settings {
            if self.settings_open {
                self.close_settings();
            } else {
                self.open_settings();
            }
        }
    }

    fn history_window(&mut self, ctx: &egui::Context) {
        let builder = ViewportBuilder::default()
            .with_title(format!("{} – {}", self.t(T::AppTitle), self.t(T::History)))
            .with_inner_size([420.0, 440.0])
            .with_min_inner_size([340.0, 240.0])
            .with_window_level(WindowLevel::AlwaysOnTop);

        let close =
            ctx.show_viewport_immediate(ViewportId::from_hash_of("history"), builder, |ui, _class| {
                if self.history_frames < 10 {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Focus);
                }
                keep_on_top(ui, &mut self.history_frames);
                let mut close = ui.input(|i| i.viewport().close_requested());
                egui::Panel::bottom("history_actions").show(ui, |ui| {
                    ui.add_space(6.0);
                    close |= self.history_actions_ui(ui);
                    ui.add_space(6.0);
                });
                egui::CentralPanel::default_margins().show(ui, |ui| self.history_list_ui(ui));
                close
            });
        if close {
            self.history_open = false;
            self.confirm_clear = false;
        }
    }

    fn history_list_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.t(T::History));
        ui.add_space(6.0);
        if self.history.entries.is_empty() {
            ui.label(RichText::new(self.t(T::NoMeasurements)).weak());
            return;
        }
        let date_format = match self.lang() {
            Lang::De => "%d.%m.%Y  %H:%M:%S",
            Lang::En => "%Y-%m-%d  %H:%M:%S",
        };
        let mut delete = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("history")
                    .num_columns(4)
                    .striped(true)
                    .spacing([16.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("#").strong());
                        ui.label(RichText::new(self.t(T::DateTime)).strong());
                        ui.label(RichText::new(self.t(T::Duration)).strong());
                        ui.label("");
                        ui.end_row();
                        // Newest first.
                        for (index, entry) in self.history.entries.iter().enumerate().rev() {
                            ui.label(RichText::new(format!("{}", index + 1)).weak());
                            ui.label(entry.finished_at.format(date_format).to_string());
                            ui.label(RichText::new(format_duration(entry.duration())).monospace());
                            if ui
                                .small_button("✖")
                                .on_hover_text(self.t(T::DeleteEntry))
                                .clicked()
                            {
                                delete = Some(index);
                            }
                            ui.end_row();
                        }
                    });
            });
        if let Some(index) = delete {
            self.history.entries.remove(index);
            self.save_error |= self.history.save().is_err();
        }
    }

    /// Returns true if the window should be closed.
    fn history_actions_ui(&mut self, ui: &mut egui::Ui) -> bool {
        let mut close = false;
        if self.confirm_clear {
            ui.label(RichText::new(self.t(T::ConfirmClear)).color(ui.visuals().warn_fg_color));
            ui.horizontal(|ui| {
                if ui.button(self.t(T::ConfirmClearYes)).clicked() {
                    self.history.entries.clear();
                    self.save_error |= self.history.save().is_err();
                    self.confirm_clear = false;
                }
                if ui.button(self.t(T::Cancel)).clicked() {
                    self.confirm_clear = false;
                }
            });
        } else {
            ui.horizontal(|ui| {
                let has_entries = !self.history.entries.is_empty();
                if ui
                    .add_enabled(has_entries, egui::Button::new(self.t(T::ClearAll)))
                    .clicked()
                {
                    self.confirm_clear = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    close = ui.button(self.t(T::Close)).clicked();
                });
            });
        }
        close
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        let builder = ViewportBuilder::default()
            .with_title(format!("{} – {}", self.t(T::AppTitle), self.t(T::Settings)))
            .with_inner_size([460.0, 420.0])
            .with_min_inner_size([380.0, 320.0])
            // Otherwise it could open behind the always-on-top timer window.
            .with_window_level(WindowLevel::AlwaysOnTop);

        let close =
            ctx.show_viewport_immediate(ViewportId::from_hash_of("settings"), builder, |ui, _class| {
                if self.settings_frames < 10 {
                    // Raise above the (also always-on-top) timer window, once it is mapped.
                    ui.ctx().send_viewport_cmd(ViewportCommand::Focus);
                }
                keep_on_top(ui, &mut self.settings_frames);
                let mut close = ui.input(|i| i.viewport().close_requested());
                egui::CentralPanel::default_margins().show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        close |= self.settings_ui(ui);
                    });
                });
                close
            });
        if close {
            self.close_settings();
        }
    }

    /// Returns true if the window should be closed.
    fn settings_ui(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        self.capture_hotkey(ui, &mut changed);

        ui.heading(self.t(T::Hotkeys));
        ui.add_space(4.0);
        egui::Grid::new("hotkeys")
            .num_columns(3)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                for action in Action::ALL {
                    ui.label(self.action_label(action));
                    let text = if self.recording == Some(action) {
                        RichText::new(self.t(T::PressKey)).italics()
                    } else {
                        match self.combo(action) {
                            Some(combo) => RichText::new(combo.display()).monospace(),
                            None => RichText::new(self.t(T::NotSet)).weak(),
                        }
                    };
                    let failed = self.failed_hotkeys.contains(&action) && self.combo(action).is_some();
                    let button = egui::Button::new(text)
                        .selected(self.recording == Some(action))
                        .min_size([220.0, 0.0].into());
                    let response = ui.add(button).on_hover_text(self.t(T::ClickToChange));
                    if response.clicked() {
                        self.recording = if self.recording == Some(action) {
                            None
                        } else {
                            Some(action)
                        };
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(self.combo(action).is_some(), egui::Button::new("✖"))
                            .on_hover_text(self.t(T::ClearHotkey))
                            .clicked()
                        {
                            self.config.hotkeys.set(action, None);
                            self.recording = None;
                            changed = true;
                        }
                        if failed {
                            ui.label(RichText::new("⚠").color(ui.visuals().warn_fg_color))
                                .on_hover_text(self.t(T::HotkeyFailed));
                        }
                    });
                    ui.end_row();
                }
            });
        ui.add_space(4.0);
        if ui.button(self.t(T::RestoreDefaults)).clicked() {
            self.config.hotkeys = Hotkeys::default();
            self.recording = None;
            changed = true;
        }
        ui.add_space(2.0);
        ui.label(RichText::new(self.t(T::SharedPauseHint)).small().weak());
        ui.label(RichText::new(self.t(T::HotkeysSuspended)).small().weak());
        match self.hotkeys.backend_name() {
            Some(name) => ui.label(
                RichText::new(format!("{}: {name}", self.t(T::HotkeyBackend)))
                    .small()
                    .weak(),
            ),
            None => ui.label(
                RichText::new(self.t(T::HotkeysUnavailable))
                    .small()
                    .color(ui.visuals().warn_fg_color),
            ),
        };

        ui.add_space(12.0);
        ui.heading(self.t(T::Appearance));
        ui.add_space(4.0);
        egui::Grid::new("appearance")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                let lang = self.lang();
                ui.label(self.t(T::Theme));
                ui.horizontal(|ui| {
                    for (mode, text) in [
                        (ThemeMode::System, T::ThemeSystem),
                        (ThemeMode::Light, T::ThemeLight),
                        (ThemeMode::Dark, T::ThemeDark),
                    ] {
                        changed |= ui
                            .radio_value(&mut self.config.theme, mode, tr(lang, text))
                            .changed();
                    }
                });
                ui.end_row();

                ui.label(self.t(T::Language));
                ui.horizontal(|ui| {
                    let system = tr(lang, T::LanguageSystem);
                    for (mode, text) in [
                        (LanguageMode::System, system),
                        (LanguageMode::De, "Deutsch"),
                        (LanguageMode::En, "English"),
                    ] {
                        changed |= ui.radio_value(&mut self.config.language, mode, text).changed();
                    }
                });
                ui.end_row();
            });

        if changed {
            self.save();
        }

        ui.add_space(12.0);
        let mut close = false;
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            close = ui.button(self.t(T::Close)).clicked();
            if self.save_error {
                ui.label(RichText::new(self.t(T::SaveFailed)).color(Color32::RED));
            }
        });
        close
    }

    /// While recording, the next key press (with modifiers) becomes the hotkey.
    fn capture_hotkey(&mut self, ui: &mut egui::Ui, changed: &mut bool) {
        let Some(action) = self.recording else { return };
        let pressed = ui.input_mut(|input| {
            let mut found = None;
            input.events.retain(|event| match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if found.is_none() => {
                    found = Some((*key, *modifiers));
                    false
                }
                _ => true,
            });
            found
        });
        let Some((key, modifiers)) = pressed else { return };
        if key == egui::Key::Escape && modifiers.is_none() {
            self.recording = None;
            return;
        }
        let Some(combo) = KeyCombo::from_event(key, modifiers) else {
            return;
        };
        let value = combo.to_config_string();
        // A combo can only trigger one action (except pause + resume, which may share one):
        // take it away from any other action.
        let may_share = |a: Action, b: Action| {
            matches!(
                (a, b),
                (Action::Pause, Action::Resume) | (Action::Resume, Action::Pause)
            )
        };
        for other in Action::ALL {
            if other != action
                && !may_share(action, other)
                && self.config.hotkeys.get(other) == Some(value.as_str())
            {
                self.config.hotkeys.set(other, None);
            }
        }
        self.config.hotkeys.set(action, Some(value));
        self.recording = None;
        *changed = true;
    }
}

impl eframe::App for StopwatchApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Runs even while the window is hidden/minimized, so hotkeys keep working.
        let actions: Vec<Action> = self.hotkeys.poll().collect();
        for action in actions {
            self.handle_hotkey(action);
        }
        self.sync_theme_and_language(ctx);
        if self.stopwatch.state() == State::Running {
            ctx.request_repaint_after(Duration::from_millis(30));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        keep_on_top(ui, &mut self.root_frames);
        self.lock_aspect_ratio(ui, frame);

        let scale = (ui.ctx().content_rect().width() / BASE_SIZE.x).max(MIN_SCALE);
        let panel = egui::Frame::central_panel(ui.style()).inner_margin(8.0 * scale);
        egui::CentralPanel::default_margins().frame(panel).show(ui, |ui| {
            scale_style(ui.style_mut(), scale);
            self.timer_ui(ui, scale)
        });
        let ctx = ui.ctx().clone();
        if self.history_open {
            self.history_window(&ctx);
        }
        if self.settings_open {
            self.settings_window(&ctx);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.hotkeys.shutdown();
    }
}
