// SPDX-License-Identifier: MPL-2.0

//! The settings window (`cosmic-nightlight --settings`).
//!
//! A normal libcosmic top-level window for the less-frequent configuration:
//! autostart, schedule mode, and sunrise/sunset hours.
//! Every change is written through to the shared `cosmic_config` store, so the
//! applet and the daemon pick it up.

use cosmic::app::{Core, Task};
use cosmic::iced::{Length, Limits, Size};
use cosmic::{widget, Element};

use crate::autostart;
use crate::backend;
use crate::config::{self, Schedule, APP_ID};

const SCHEDULE_OPTIONS: &[&str] = &["Off", "Custom Schedule"];

/// Below this, the schedule row's label and dropdown no longer fit
/// side by side and start overlapping.
const MIN_WIDTH: f32 = 400.0;
const MIN_HEIGHT: f32 = 300.0;

/// Runs the settings window.
pub fn run() -> cosmic::iced::Result {
    let settings = cosmic::app::Settings::default()
        .size(Size::new(560.0, 480.0))
        .size_limits(Limits::NONE.min_width(MIN_WIDTH).min_height(MIN_HEIGHT));
    cosmic::app::run::<SettingsWindow>(settings, ())
}

pub struct SettingsWindow {
    core: Core,
    config: Option<cosmic::cosmic_config::Config>,
    autostart: bool,
    schedule: Schedule,
    sunrise_hour: u32,
    sunset_hour: u32,
    /// Whether the tint is currently on — mirrors the applet's toggle, kept in
    /// sync via [`config::subscription`].
    tint_on: bool,
    /// Kelvin, kept as `f32` to feed the slider directly.
    temperature: f32,
    /// Pre-built `"HH:00"` labels for the hour dropdowns; owned by `self` so the
    /// dropdown's borrow outlives `view`.
    hour_labels: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum Message {
    AutostartToggled(bool),
    ScheduleSelected(usize),
    SunriseSelected(usize),
    SunsetSelected(usize),
    Toggle(bool),
    TemperatureChanged(f32),
    TemperatureCommitted,
    ConfigUpdated(config::Settings),
}

impl cosmic::Application for SettingsWindow {
    type Executor = cosmic::executor::Default;
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
        let settings = config::Settings::load();

        let app = Self {
            core,
            config: config::handler(),
            // The autostart file is the source of truth for the toggle's state.
            autostart: autostart::is_enabled(),
            schedule: settings.schedule,
            sunrise_hour: settings.sunrise_hour,
            sunset_hour: settings.sunset_hour,
            tint_on: settings.tint_on(),
            temperature: settings.temperature as f32,
            hour_labels: {
                let military = config::is_military_time();
                (0..24).map(|h| config::format_hour(h, military)).collect()
            },
        };

        (app, Task::none())
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::AutostartToggled(enabled) => {
                self.autostart = enabled;
                config::store_autostart(&self.config, enabled);
                if let Err(err) = autostart::set(enabled) {
                    eprintln!("cosmic-nightlight: failed to update autostart entry: {err}");
                }
            }
            Message::ScheduleSelected(index) => {
                self.schedule = Schedule::ALL[index];
                config::store_schedule(&self.config, self.schedule);
            }
            Message::SunriseSelected(index) => {
                self.sunrise_hour = index as u32;
                config::store_sunrise_hour(&self.config, self.sunrise_hour);
            }
            Message::SunsetSelected(index) => {
                self.sunset_hour = index as u32;
                config::store_sunset_hour(&self.config, self.sunset_hour);
            }
            Message::Toggle(on) => {
                // Mirrors the applet's toggle logic: flipping it to match the
                // schedule just follows it (`Auto`), while flipping it against
                // the schedule sets a manual override the daemon honours until
                // the next sunset/sunrise transition.
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
            Message::ConfigUpdated(settings) => {
                self.tint_on = settings.tint_on();
                self.temperature = settings.temperature as f32;
                self.schedule = settings.schedule;
                self.sunrise_hour = settings.sunrise_hour;
                self.sunset_hour = settings.sunset_hour;
            }
        }

        Task::none()
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        config::subscription().map(Message::ConfigUpdated)
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let status_text = match self.schedule {
            Schedule::Manual => {
                if self.tint_on {
                    "On".to_string()
                } else {
                    "Off".to_string()
                }
            }
            Schedule::SunsetToSunrise => {
                let military = config::is_military_time();
                if self.tint_on {
                    format!("On Until {}", config::format_hour(self.sunrise_hour, military))
                } else {
                    format!("Off Until {}", config::format_hour(self.sunset_hour, military))
                }
            }
        };

        let night_light = widget::settings::section()
            .title("Night Light")
            .add(
                widget::settings::item::builder("Night Light")
                    .description(status_text)
                    .control(widget::toggler(self.tint_on).on_toggle(Message::Toggle)),
            )
            .add(widget::settings::item(
                format!("Temperature: {}K", self.temperature as i32),
                widget::slider(
                    2500.0..=6500.0,
                    self.temperature,
                    Message::TemperatureChanged,
                )
                .step(50.0)
                .on_release(Message::TemperatureCommitted)
                .width(Length::Fixed(200.0)),
            ));

        let general = widget::settings::section()
            .title("General")
            .add(widget::settings::item(
                "Start on login",
                widget::toggler(self.autostart).on_toggle(Message::AutostartToggled),
            ));

        let mut schedule =
            widget::settings::section()
                .title("Schedule")
                .add(widget::settings::item(
                    "Schedule",
                    widget::dropdown(
                        SCHEDULE_OPTIONS,
                        Some(self.schedule.index()),
                        Message::ScheduleSelected,
                    )
                    // Wide enough for the longest option ("Custom Schedule") so the
                    // popup menu (which is sized to the longest option but anchored
                    // to this widget's left edge) doesn't extend past the window's
                    // right edge and get clipped.
                    .width(Length::Fixed(200.0)),
                ));

        if self.schedule == Schedule::SunsetToSunrise {
            schedule = schedule
                .add(widget::settings::item(
                    "From",
                    widget::dropdown(
                        &self.hour_labels,
                        Some(self.sunset_hour as usize),
                        Message::SunsetSelected,
                    ),
                ))
                .add(widget::settings::item(
                    "To",
                    widget::dropdown(
                        &self.hour_labels,
                        Some(self.sunrise_hour as usize),
                        Message::SunriseSelected,
                    ),
                ));
        }

        let content = widget::settings::view_column(vec![
            widget::text::title2("Night Light Settings").into(),
            night_light.into(),
            general.into(),
            schedule.into(),
        ])
        .width(Length::Fill);

        // `max_width` and `center_x(Fill)` must be on separate containers:
        // applying both to the same container caps its own resolved width at
        // 600, leaving it pinned to the top-left instead of centered. The
        // inner container caps the content at 600px; the outer one centers
        // that box within the full window width.
        let constrained = widget::container(content).max_width(600.0);

        let centered = widget::container(constrained)
            .padding(20)
            .center_x(Length::Fill);

        // Wrap in a vertical scrollable so a short window scrolls instead of
        // compressing the rows. Filling the height makes the scrollable
        // viewport track the window size.
        widget::scrollable(centered)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
