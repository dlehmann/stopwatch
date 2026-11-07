//! The egui application: timer window and settings window.

use eframe::egui;
use std::time::{Duration, Instant};

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
    native_aspect_tried: bool,
    aspect: AspectState,
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

/// Tracks window resizes for `StopwatchApp::enforce_aspect_ratio`.
struct AspectState {
    last_size: egui::Vec2,
    changed_at: Instant,
    /// Last size with the correct aspect ratio.
    reference: egui::Vec2,
    /// A size correction that was requested but not applied yet.
    pending: Option<(egui::Vec2, Instant)>,
}

impl Default for AspectState {
    fn default() -> Self {
        Self {
            last_size: egui::Vec2::ZERO,
            changed_at: Instant::now(),
            reference: egui::Vec2::ZERO,
            pending: None,
        }
    }
}

/// A fixed-size dialog window, see `StopwatchApp::dialog`.
struct Dialog {
    id: &'static str,
    title: T,
    size: egui::Vec2,
    /// Frame counter of this window (for `keep_on_top`).
    frames: fn(&mut StopwatchApp) -> &mut u32,
    body: fn(&mut StopwatchApp, &mut egui::Ui),
    /// Content of the bottom bar; returns true to close the window.
    footer: fn(&mut StopwatchApp, &mut egui::Ui) -> bool,
}

/// Outer margin of the dialog windows.
const DIALOG_MARGIN: f32 = 16.0;

/// A rounded, full-width box with a title, used to group settings.
fn section(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(8.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).strong().size(15.0));
            ui.add_space(2.0);
            add_contents(ui);
        });
}

/// A labelled drop-down that fills the rest of the row. Returns true if the value changed.
fn select_row<V: PartialEq + Copy>(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    label_width: f32,
    value: &mut V,
    options: &[(V, &str)],
) -> bool {
    let before = *value;
    ui.horizontal(|ui| {
        cell(ui, label_width, 28.0, |ui| ui.label(label));
        let selected = options
            .iter()
            .find(|(v, _)| v == value)
            .map_or("", |(_, text)| *text);
        egui::ComboBox::from_id_salt(id)
            .selected_text(selected)
            .width(ui.available_width())
            .height(200.0)
            .show_ui(ui, |ui| {
                for (option, text) in options {
                    ui.selectable_value(value, *option, *text);
                }
            });
    });
    *value != before
}

/// Allocates a fixed-size, left-aligned, vertically centered cell.
fn cell<R>(ui: &mut egui::Ui, width: f32, height: f32, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.allocate_ui_with_layout(
        egui::vec2(width, height),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_size(egui::vec2(width, height));
            add_contents(ui)
        },
    )
    .inner
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
            native_aspect_tried: false,
            aspect: AspectState::default(),
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
    /// Keeps the timer window at the aspect ratio of `BASE_SIZE`.
    ///
    /// On Windows and macOS the window system enforces it while dragging (see `aspect`).
    /// Elsewhere (KWin does not apply aspect hints to XWayland windows) the window is
    /// snapped to the ratio as soon as the user stops resizing it.
    fn enforce_aspect_ratio(&mut self, ui: &egui::Ui, frame: &eframe::Frame) {
        const SETTLE: Duration = Duration::from_millis(250);
        let ctx = ui.ctx();

        if !self.native_aspect_tried && self.root_frames >= 3 {
            self.native_aspect_tried = true;
            crate::aspect::lock(frame, BASE_SIZE.x as u32, BASE_SIZE.y as u32);
        }

        let size = ctx.content_rect().size();
        let now = Instant::now();
        // After a correction, wait until the window system has applied it.
        if let Some((target, sent_at)) = self.aspect.pending {
            if (size - target).length() > 1.0 && now.duration_since(sent_at) < Duration::from_secs(1) {
                ctx.request_repaint_after(SETTLE);
                return;
            }
            self.aspect.pending = None;
        }
        if size != self.aspect.last_size {
            self.aspect.last_size = size;
            self.aspect.changed_at = now;
            ctx.request_repaint_after(SETTLE);
            return;
        }
        let ratio = BASE_SIZE.x / BASE_SIZE.y;
        if (size.x / size.y - ratio).abs() < 0.01 {
            self.aspect.reference = size;
            return;
        }
        if now.duration_since(self.aspect.changed_at) < SETTLE {
            ctx.request_repaint_after(SETTLE);
            return;
        }
        // The dimension the user changed more (relative to the last good size) wins.
        let reference = self.aspect.reference;
        let width_change = (size.x - reference.x).abs() / reference.x.max(1.0);
        let height_change = (size.y - reference.y).abs() / reference.y.max(1.0);
        let mut target = if reference == egui::Vec2::ZERO || width_change >= height_change {
            egui::vec2(size.x, size.x / ratio)
        } else {
            egui::vec2(size.y * ratio, size.y)
        };
        target = target.max(BASE_SIZE * MIN_SCALE).round();
        self.aspect.reference = target;
        self.aspect.pending = Some((target, now));
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(target));
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

    /// Shows a fixed-size dialog window that stays above the timer window.
    /// `footer` is drawn in a bar at the bottom, `body` fills the rest.
    /// Returns true if the window should be closed.
    fn dialog(&mut self, ctx: &egui::Context, dialog: Dialog) -> bool {
        let Dialog {
            id,
            title,
            size,
            frames,
            body,
            footer,
        } = dialog;
        let builder = ViewportBuilder::default()
            .with_title(format!("{} – {}", self.t(T::AppTitle), self.t(title)))
            .with_inner_size(size)
            .with_resizable(false)
            .with_maximize_button(false)
            // Otherwise it could open behind the always-on-top timer window.
            .with_window_level(WindowLevel::AlwaysOnTop);

        ctx.show_viewport_immediate(ViewportId::from_hash_of(id), builder, |ui, _class| {
            let frames = frames(self);
            if *frames < 10 {
                // Raise above the (also always-on-top) timer window, once it is mapped.
                ui.ctx().send_viewport_cmd(ViewportCommand::Focus);
            }
            keep_on_top(ui, frames);

            let mut close = ui.input(|i| i.viewport().close_requested());
            let margin = egui::Margin::symmetric(DIALOG_MARGIN as i8, 12);
            egui::Panel::bottom(format!("{id}_footer"))
                .frame(egui::Frame::side_top_panel(ui.style()).inner_margin(margin))
                .show(ui, |ui| close |= footer(self, ui));
            egui::CentralPanel::default_margins()
                .frame(egui::Frame::central_panel(ui.style()).inner_margin(DIALOG_MARGIN))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                    body(self, ui)
                });
            close
        })
    }

    fn history_window(&mut self, ctx: &egui::Context) {
        let close = self.dialog(
            ctx,
            Dialog {
                id: "history",
                title: T::History,
                size: egui::vec2(460.0, 480.0),
                frames: |app| &mut app.history_frames,
                body: Self::history_list_ui,
                footer: Self::history_footer_ui,
            },
        );
        if close {
            self.history_open = false;
            self.confirm_clear = false;
        }
    }

    fn history_list_ui(&mut self, ui: &mut egui::Ui) {
        if self.history.entries.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| ui.label(RichText::new(self.t(T::NoMeasurements)).weak()));
            return;
        }
        let date_format = match self.lang() {
            Lang::De => "%d.%m.%Y   %H:%M:%S",
            Lang::En => "%Y-%m-%d   %H:%M:%S",
        };

        // Column widths: number, date/time (takes the rest), duration, delete button.
        let row_height = 28.0;
        let gap = ui.spacing().item_spacing.x;
        let full = ui.available_width();
        let (number_w, duration_w, delete_w) = (36.0, 110.0, 28.0);
        let date_w = full - number_w - duration_w - delete_w - 3.0 * gap;
        let widths = [number_w, date_w, duration_w, delete_w];

        let header = [
            RichText::new("#"),
            RichText::new(self.t(T::DateTime)),
            RichText::new(self.t(T::Duration)),
            RichText::new(""),
        ];
        ui.horizontal(|ui| {
            for (text, width) in header.into_iter().zip(widths) {
                cell(ui, width, 20.0, |ui| ui.label(text.strong().weak()));
            }
        });
        ui.separator();

        let mut delete = None;
        let stripe = ui.visuals().faint_bg_color;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                // Newest first.
                for (row, (index, entry)) in self.history.entries.iter().enumerate().rev().enumerate() {
                    let fill = if row % 2 == 0 {
                        stripe
                    } else {
                        Color32::TRANSPARENT
                    };
                    egui::Frame::new().fill(fill).corner_radius(4.0).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            cell(ui, widths[0], row_height, |ui| {
                                ui.label(RichText::new(format!("{}", index + 1)).weak())
                            });
                            cell(ui, widths[1], row_height, |ui| {
                                ui.label(entry.finished_at.format(date_format).to_string())
                            });
                            cell(ui, widths[2], row_height, |ui| {
                                let font = egui::FontId::new(15.0, egui::FontFamily::Name(CLOCK_FONT.into()));
                                ui.label(RichText::new(format_duration(entry.duration())).font(font))
                            });
                            cell(ui, widths[3], row_height, |ui| {
                                let button = egui::Button::new("✖").frame(false);
                                if ui.add(button).on_hover_text(self.t(T::DeleteEntry)).clicked() {
                                    delete = Some(index);
                                }
                            });
                        });
                    });
                }
            });
        if let Some(index) = delete {
            self.history.entries.remove(index);
            self.save_error |= self.history.save().is_err();
        }
    }

    fn history_footer_ui(&mut self, ui: &mut egui::Ui) -> bool {
        let mut close = false;
        ui.horizontal(|ui| {
            if self.confirm_clear {
                ui.label(RichText::new(self.t(T::ConfirmClear)).color(ui.visuals().warn_fg_color));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(self.t(T::Cancel)).clicked() {
                        self.confirm_clear = false;
                    }
                    let yes =
                        egui::Button::new(RichText::new(self.t(T::ConfirmClearYes)).color(Color32::WHITE))
                            .fill(ui.visuals().error_fg_color);
                    if ui.add(yes).clicked() {
                        self.history.entries.clear();
                        self.save_error |= self.history.save().is_err();
                        self.confirm_clear = false;
                    }
                });
            } else {
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
            }
        });
        close
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        let close = self.dialog(
            ctx,
            Dialog {
                id: "settings",
                title: T::Settings,
                size: egui::vec2(500.0, 452.0),
                frames: |app| &mut app.settings_frames,
                body: Self::settings_ui,
                footer: Self::settings_footer_ui,
            },
        );
        if close {
            self.close_settings();
        }
    }

    fn settings_ui(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        self.capture_hotkey(ui, &mut changed);
        let label_w = 110.0;
        let row_h = 28.0;

        section(ui, self.t(T::Hotkeys), |ui| {
            // Same widths in every row, independent of what each row contains.
            let gap = ui.spacing().item_spacing.x;
            let row_w = ui.available_width();
            let clear_w = row_h;
            let warn_w = 16.0;
            for action in Action::ALL {
                ui.horizontal(|ui| {
                    cell(ui, label_w, row_h, |ui| ui.label(self.action_label(action)));
                    let failed = self.failed_hotkeys.contains(&action) && self.combo(action).is_some();
                    let field_w =
                        row_w - label_w - clear_w - 2.0 * gap - if failed { warn_w + gap } else { 0.0 };

                    let recording = self.recording == Some(action);
                    let text = if recording {
                        RichText::new(self.t(T::PressKey)).italics()
                    } else {
                        match self.combo(action) {
                            Some(combo) => RichText::new(combo.display()).monospace(),
                            None => RichText::new(self.t(T::NotSet)).weak(),
                        }
                    };
                    let field = egui::Button::new(text).selected(recording);
                    if ui
                        .add_sized([field_w, row_h], field)
                        .on_hover_text(self.t(T::ClickToChange))
                        .clicked()
                    {
                        self.recording = if recording { None } else { Some(action) };
                    }
                    if failed {
                        ui.label(RichText::new("⚠").color(ui.visuals().warn_fg_color))
                            .on_hover_text(self.t(T::HotkeyFailed));
                    }
                    let clear = ui
                        .add_enabled_ui(self.combo(action).is_some(), |ui| {
                            ui.add_sized([clear_w, row_h], egui::Button::new("✖"))
                        })
                        .inner
                        .on_hover_text(self.t(T::ClearHotkey));
                    if clear.clicked() {
                        self.config.hotkeys.set(action, None);
                        self.recording = None;
                        changed = true;
                    }
                });
            }
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                let hint = match self.hotkeys.backend_name() {
                    Some(name) => RichText::new(format!("{}: {name}", self.t(T::HotkeyBackend)))
                        .small()
                        .weak(),
                    None => RichText::new(self.t(T::HotkeysUnavailable))
                        .small()
                        .color(ui.visuals().warn_fg_color),
                };
                ui.label(hint);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(self.t(T::RestoreDefaults)).clicked() {
                        self.config.hotkeys = Hotkeys::default();
                        self.recording = None;
                        changed = true;
                    }
                });
            });
            ui.label(RichText::new(self.t(T::SharedPauseHint)).small().weak());
            ui.label(RichText::new(self.t(T::HotkeysSuspended)).small().weak());
        });

        ui.add_space(4.0);
        let lang = self.lang();
        section(ui, self.t(T::Appearance), |ui| {
            let themes = [
                (ThemeMode::System, tr(lang, T::ThemeSystem)),
                (ThemeMode::Light, tr(lang, T::ThemeLight)),
                (ThemeMode::Dark, tr(lang, T::ThemeDark)),
            ];
            changed |= select_row(
                ui,
                "theme",
                tr(lang, T::Theme),
                label_w,
                &mut self.config.theme,
                &themes,
            );
            let languages = [
                (LanguageMode::System, tr(lang, T::LanguageSystem)),
                (LanguageMode::De, "Deutsch"),
                (LanguageMode::En, "English"),
            ];
            changed |= select_row(
                ui,
                "language",
                tr(lang, T::Language),
                label_w,
                &mut self.config.language,
                &languages,
            );
        });

        if changed {
            self.save();
        }
    }

    fn settings_footer_ui(&mut self, ui: &mut egui::Ui) -> bool {
        let mut close = false;
        ui.horizontal(|ui| {
            if self.save_error {
                ui.label(RichText::new(self.t(T::SaveFailed)).color(ui.visuals().error_fg_color));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                close = ui.button(self.t(T::Close)).clicked();
            });
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
        self.enforce_aspect_ratio(ui, frame);
        // Maximizing (e.g. double-click on the title bar) would break the aspect ratio.
        if ui.input(|i| i.viewport().maximized == Some(true)) {
            ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(false));
        }

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
