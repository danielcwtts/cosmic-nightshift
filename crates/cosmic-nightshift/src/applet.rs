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
use cosmic::widget::{self, list_column, settings, slider, toggler};
use cosmic::Element;

use crate::backend;
use crate::config::{self, APP_ID};

/// Runs the application as a COSMIC panel applet.
pub fn run() -> cosmic::iced::Result {
    cosmic::applet::run::<NightShiftApplet>(())
}

pub struct NightShiftApplet {
    core: Core,
    popup: Option<Id>,
    config: Option<cosmic::cosmic_config::Config>,
    enabled: bool,
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
}

impl cosmic::Application for NightShiftApplet {
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
            enabled: settings.enabled,
            temperature: settings.temperature as f32,
        };

        (app, Task::none())
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
            Message::Toggle(enabled) => {
                self.enabled = enabled;
                config::store_enabled(&self.config, enabled);
                if enabled {
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
                if self.enabled {
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
        }

        Task::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let icon = if self.enabled {
            "weather-clear-night-symbolic"
        } else {
            "weather-clear-symbolic"
        };

        let have_popup = self.popup;
        let button = self
            .core
            .applet
            .icon_button(icon)
            .on_press_with_rectangle(move |offset, bounds| {
                if let Some(id) = have_popup {
                    Message::Surface(destroy_popup(id))
                } else {
                    Message::Surface(app_popup::<NightShiftApplet>(
                        move |state: &mut NightShiftApplet| {
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
                        Some(Box::new(move |state: &NightShiftApplet| {
                            Element::from(state.core.applet.popup_container(state.popup_content()))
                                .map(cosmic::Action::App)
                        })),
                    ))
                }
            });

        Element::from(self.core.applet.applet_tooltip::<Message>(
            button,
            "Night Shift",
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

impl NightShiftApplet {
    /// Builds the popup body: the toggle, the temperature slider, and the
    /// button that opens the settings window.
    fn popup_content(&self) -> Element<'_, Message> {
        let toggle = settings::item(
            "Night Shift",
            toggler(self.enabled).on_toggle(Message::Toggle),
        );

        let temperature = settings::item(
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

        let settings_button = widget::button::text("Settings…")
            .on_press(Message::OpenSettings)
            .width(Length::Fill);

        list_column()
            .add(toggle)
            .add(temperature)
            .add(settings_button)
            .into()
    }
}

/// Wraps a surface action as an app task (open/close popups live here).
fn surface_task(action: cosmic::surface::Action) -> Task<Message> {
    cosmic::task::message(cosmic::Action::Cosmic(cosmic::app::Action::Surface(action)))
}

/// Launches `cosmic-nightshift --settings` as a detached child process.
///
/// The settings UI is a normal top-level window, which an applet's layer-shell
/// surface can't host in-process, so we run it as a separate process.
fn spawn_settings_window() {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("cosmic-nightshift"));
    if let Err(err) = std::process::Command::new(exe).arg("--settings").spawn() {
        eprintln!("cosmic-nightshift: failed to open settings window: {err}");
    }
}
