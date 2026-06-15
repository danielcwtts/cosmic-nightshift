// SPDX-License-Identifier: MPL-2.0

//! The settings window (`cosmic-nightshift --settings`).
//!
//! A normal libcosmic top-level window for the less-frequent configuration:
//! autostart, schedule mode, sunrise/sunset hours, and the night temperature.
//! Every change is written through to the shared `cosmic_config` store, so the
//! applet and the daemon pick it up.

use cosmic::app::{Core, Task};
use cosmic::iced::{Length, Size};
use cosmic::{widget, Element};

use crate::autostart;
use crate::config::{self, Schedule, APP_ID};

const SCHEDULE_OPTIONS: &[&str] = &["Manual", "Sunset to Sunrise"];

/// Runs the settings window.
pub fn run() -> cosmic::iced::Result {
    let settings = cosmic::app::Settings::default().size(Size::new(560.0, 480.0));
    cosmic::app::run::<SettingsWindow>(settings, ())
}

pub struct SettingsWindow {
    core: Core,
    config: Option<cosmic::cosmic_config::Config>,
    autostart: bool,
    /// Night/"on" temperature in Kelvin, kept as `f32` for the slider.
    night_temperature: f32,
    schedule: Schedule,
    sunrise_hour: u32,
    sunset_hour: u32,
    /// Pre-built `"HH:00"` labels for the hour dropdowns; owned by `self` so the
    /// dropdown's borrow outlives `view`.
    hour_labels: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum Message {
    AutostartToggled(bool),
    TemperatureChanged(f32),
    TemperatureCommitted,
    ScheduleSelected(usize),
    SunriseSelected(usize),
    SunsetSelected(usize),
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
            night_temperature: settings.temperature as f32,
            schedule: settings.schedule,
            sunrise_hour: settings.sunrise_hour,
            sunset_hour: settings.sunset_hour,
            hour_labels: (0..24).map(|h| format!("{h:02}:00")).collect(),
        };

        (app, Task::none())
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::AutostartToggled(enabled) => {
                self.autostart = enabled;
                config::store_autostart(&self.config, enabled);
                if let Err(err) = autostart::set(enabled) {
                    eprintln!("cosmic-nightshift: failed to update autostart entry: {err}");
                }
            }
            Message::TemperatureChanged(value) => {
                self.night_temperature = value;
            }
            Message::TemperatureCommitted => {
                config::store_temperature(&self.config, self.night_temperature as u32);
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
        }

        Task::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let general = widget::settings::section().title("General").add(
            widget::settings::item(
                "Start on login",
                widget::toggler(self.autostart).on_toggle(Message::AutostartToggled),
            ),
        );

        let night_light = widget::settings::section().title("Night light").add(
            widget::settings::item(
                format!("Night temperature: {}K", self.night_temperature as i32),
                widget::slider(
                    2500.0..=6500.0,
                    self.night_temperature,
                    Message::TemperatureChanged,
                )
                .step(50.0)
                .on_release(Message::TemperatureCommitted)
                .width(Length::Fixed(240.0)),
            ),
        );

        let mut schedule = widget::settings::section().title("Schedule").add(
            widget::settings::item(
                "Mode",
                widget::dropdown(
                    SCHEDULE_OPTIONS,
                    Some(self.schedule.index()),
                    Message::ScheduleSelected,
                ),
            ),
        );

        if self.schedule == Schedule::SunsetToSunrise {
            schedule = schedule
                .add(widget::settings::item(
                    "Sunset",
                    widget::dropdown(
                        &self.hour_labels,
                        Some(self.sunset_hour as usize),
                        Message::SunsetSelected,
                    ),
                ))
                .add(widget::settings::item(
                    "Sunrise",
                    widget::dropdown(
                        &self.hour_labels,
                        Some(self.sunrise_hour as usize),
                        Message::SunriseSelected,
                    ),
                ));
        }

        let content = widget::settings::view_column(vec![
            widget::text::title2("Night Shift Settings").into(),
            general.into(),
            night_light.into(),
            schedule.into(),
        ])
        .width(Length::Fill);

        let centered = widget::container(content)
            .max_width(600.0)
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
