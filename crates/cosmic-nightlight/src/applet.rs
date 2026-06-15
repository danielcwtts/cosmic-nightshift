// SPDX-License-Identifier: MPL-2.0

//! COSMIC panel applet: an icon in the status bar that opens a popup with the
//! quick controls (on/off toggle + temperature slider) and a button that opens
//! the separate settings window.
//!
//! This follows the popup pattern from libcosmic's `examples/applet`: the panel
//! button toggles a layer-shell popup via `surface::action::{app_popup,
//! destroy_popup}`, and the popup's contents are produced by the closure passed
//! to `app_popup`.

use std::path::PathBuf;

use cosmic::app::{Core, Task};
use cosmic::iced::core::window;
use cosmic::iced::window::Id;
use cosmic::iced::{Length, Rectangle};
use cosmic::surface::action::{app_popup, destroy_popup};
use cosmic::widget::{self, settings, slider, toggler};
use cosmic::Element;

use crate::backend;
use crate::config::{self, APP_ID};

/// Runs the application as a COSMIC panel applet.
pub fn run() -> cosmic::iced::Result {
    cosmic::applet::run::<NightLightApplet>(())
}

pub struct NightLightApplet {
    core: Core,
    popup: Option<Id>,
    config: Option<cosmic::cosmic_config::Config>,
    /// Whether the tint is currently on — the effective state (schedule plus
    /// any manual override), not just a raw flag. Drives the toggle and icon.
    tint_on: bool,
    /// Kelvin, kept as `f32` to feed the slider directly.
    temperature: f32,
}

#[derive(Clone, Debug)]
pub enum Message {
    PopupClosed(Id),
    Toggle(bool),
    TemperatureChanged(f32),
    TemperatureCommitted,
    OpenSettings,
    Surface(cosmic::surface::Action),
    RefreshInit,
    ConfigUpdated(config::Settings),
}

impl cosmic::Application for NightLightApplet {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let handler = config::handler();
        let settings = config::Settings::load_from(&handler);

        let app = Self {
            core,
            popup: None,
            config: handler,
            tint_on: settings.tint_on(),
            temperature: settings.temperature as f32,
        };

        let init_task = cosmic::task::future(async { Message::RefreshInit });

        (app, init_task)
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Self::Message> {
        Some(Message::PopupClosed(id))
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
            }
            Message::Toggle(on) => {
                // Interpret the toggle relative to what the schedule wants now:
                // flipping it to match the schedule just follows it (`Auto`),
                // while flipping it against the schedule sets a manual override
                // the daemon honours until the next sunset/sunrise transition.
                let settings = config::Settings::load_from(&self.config);
                let new_override = if on == settings.schedule_wants_tint() {
                    config::Override::Auto
                } else if on {
                    config::Override::On
                } else {
                    config::Override::Off
                };
                config::store_override(&self.config, new_override);
                self.tint_on = on;
                if on {
                    backend::apply_color_temperature(self.temperature as u32, 1.0);
                } else {
                    backend::reset();
                }
            }
            Message::TemperatureChanged(value) => {
                self.temperature = value;
            }
            Message::TemperatureCommitted => {
                config::store_temperature(&self.config, self.temperature as u32);
                if self.tint_on {
                    backend::apply_color_temperature(self.temperature as u32, 1.0);
                }
            }
            Message::OpenSettings => {
                spawn_settings_window();
                if let Some(id) = self.popup.take() {
                    return surface_task(destroy_popup(id));
                }
            }
            Message::Surface(action) => {
                return surface_task(action);
            }
            Message::RefreshInit => {
                // Dummy handler to trigger a redraw after the layer shell surface maps,
                // working around a bug where the panel applet appears as size 0 until moved.
            }
            Message::ConfigUpdated(settings) => {
                self.tint_on = settings.tint_on();
                self.temperature = settings.temperature as f32;
            }
        }

        Task::none()
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        config::subscription().map(Message::ConfigUpdated)
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let icon = if self.tint_on {
            "weather-clear-night-symbolic"
        } else {
            "weather-clear-symbolic"
        };

        let have_popup = self.popup;
        let button =
            self.core
                .applet
                .icon_button(icon)
                .on_press_with_rectangle(move |offset, bounds| {
                    if let Some(id) = have_popup {
                        Message::Surface(destroy_popup(id))
                    } else {
                        Message::Surface(app_popup::<NightLightApplet>(
                            move |state: &mut NightLightApplet| {
                                let new_id = Id::unique();
                                state.popup = Some(new_id);
                                let mut popup_settings = state.core.applet.get_popup_settings(
                                    state.core.main_window_id().unwrap(),
                                    new_id,
                                    None,
                                    None,
                                    None,
                                );
                                popup_settings.positioner.anchor_rect = Rectangle {
                                    x: (bounds.x - offset.x) as i32,
                                    y: (bounds.y - offset.y) as i32,
                                    width: bounds.width as i32,
                                    height: bounds.height as i32,
                                };
                                popup_settings
                            },
                            Some(Box::new(move |state: &NightLightApplet| {
                                Element::from(
                                    state.core.applet.popup_container(state.popup_content()),
                                )
                                .map(cosmic::Action::App)
                            })),
                        ))
                    }
                });

        Element::from(self.core.applet.applet_tooltip::<Message>(
            button,
            "Night Light",
            self.popup.is_some(),
            Message::Surface,
            None,
        ))
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        // Popup contents are supplied via the `app_popup` view closure above;
        // nothing else owns a window surface.
        widget::text("").into()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

impl NightLightApplet {
    /// Builds the popup body: the toggle, the temperature slider, and the
    /// button that opens the settings window.
    fn popup_content(&self) -> Element<'_, Message> {
        let settings_state = config::Settings::load_from(&self.config);

        let status_text = match settings_state.schedule {
            config::Schedule::Manual => {
                if self.tint_on {
                    "On".to_string()
                } else {
                    "Off".to_string()
                }
            }
            config::Schedule::SunsetToSunrise => {
                let military = config::is_military_time();
                if self.tint_on {
                    format!("On Until {}", config::format_hour(settings_state.sunrise_hour, military))
                } else {
                    format!("Off Until {}", config::format_hour(settings_state.sunset_hour, military))
                }
            }
        };

        let toggle = settings::item::builder("Night Light")
            .description(status_text)
            .control(toggler(self.tint_on).on_toggle(Message::Toggle));

        let temperature_row = settings::item(
            format!("Temperature: {}K", self.temperature as i32),
            slider(
                2500.0..=6500.0,
                self.temperature,
                Message::TemperatureChanged,
            )
            .step(50.0)
            .on_release(Message::TemperatureCommitted)
            .width(Length::Fixed(200.0)),
        );

        let temperature = cosmic::widget::Column::new()
            .spacing(2)
            .push(temperature_row)
            .push(widget::text::caption("Note: Screen may briefly flicker"));

        // Match the native COSMIC applets (e.g. the keyboard applet's "Keyboard
        // Settings...") — a flat, full-width `AppletMenu` row that highlights on
        // hover, sitting below a divider rather than a standalone button.
        let settings_button =
            cosmic::applet::menu_button(widget::text::body("Night Light Settings..."))
                .on_press(Message::OpenSettings);

        // No `list_column` card — native applet popups lay controls out flat,
        // each row given the standard menu padding via `padded_control`, with
        // dividers between sections. The column's vertical padding gives the
        // breathing room above the first row and below the last that the native
        // applets have.
        cosmic::widget::Column::new()
            .padding([cosmic::theme::spacing().space_s, 0])
            .push(cosmic::applet::padded_control(toggle))
            .push(cosmic::applet::padded_control(
                widget::divider::horizontal::default(),
            ))
            .push(cosmic::applet::padded_control(temperature))
            .push(cosmic::applet::padded_control(
                widget::divider::horizontal::default(),
            ))
            .push(settings_button)
            .into()
    }
}

/// Wraps a surface action as an app task (open/close popups live here).
fn surface_task(action: cosmic::surface::Action) -> Task<Message> {
    cosmic::task::message(cosmic::Action::Cosmic(cosmic::app::Action::Surface(action)))
}

/// Launches `cosmic-nightlight --settings` as a detached child process.
///
/// The settings UI is a normal top-level window, which an applet's layer-shell
/// surface can't host in-process, so we run it as a separate process.
fn spawn_settings_window() {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("cosmic-nightlight"));
    if let Err(err) = std::process::Command::new(exe).arg("--settings").spawn() {
        eprintln!("cosmic-nightlight: failed to open settings window: {err}");
    }
}
