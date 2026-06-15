// SPDX-License-Identifier: MPL-2.0

mod backend;
mod daemon;

use cosmic::app::{Settings, Task};
use cosmic::iced::{Length, Size};
use cosmic::{executor, widget, Core, Element};

const APP_ID: &str = "io.github.cosmic_nightshift";

const SCHEDULE_OPTIONS: &[&str] = &["Manual", "Sunset to Sunrise"];

fn main() -> cosmic::iced::Result {
    if std::env::args().any(|arg| arg == "--daemon") {
        daemon::run();
        return Ok(());
    }

    let settings = Settings::default().size(Size::new(600.0, 400.0));
    cosmic::app::run::<NightShiftApp>(settings, ())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Schedule {
    Manual,
    SunsetToSunrise,
}

impl Schedule {
    const ALL: [Schedule; 2] = [Schedule::Manual, Schedule::SunsetToSunrise];

    fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    NightShiftToggled(bool),
    TemperatureChanged(f32),
    TemperatureCommitted,
    ScheduleSelected(usize),
}

pub struct NightShiftApp {
    core: Core,
    night_shift_enabled: bool,
    color_temperature: f32,
    schedule: Schedule,
}

impl cosmic::Application for NightShiftApp {
    type Executor = executor::Default;
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
        let app = Self {
            core,
            night_shift_enabled: false,
            color_temperature: 4500.0,
            schedule: Schedule::Manual,
        };

        (app, Task::none())
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::NightShiftToggled(enabled) => {
                self.night_shift_enabled = enabled;
                println!("Night Shift enabled: {enabled}");
                if enabled {
                    backend::apply_color_temperature(self.color_temperature as u32, 1.0);
                } else {
                    backend::reset();
                }
            }
            Message::TemperatureChanged(value) => {
                self.color_temperature = value;
                println!("Color temperature: {}K", value as i32);
            }
            Message::TemperatureCommitted => {
                if self.night_shift_enabled {
                    backend::apply_color_temperature(self.color_temperature as u32, 1.0);
                }
            }
            Message::ScheduleSelected(index) => {
                self.schedule = Schedule::ALL[index];
                println!("Schedule: {:?}", self.schedule);
            }
        }

        Task::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let night_shift_toggle = widget::settings::item(
            "Night Shift",
            widget::toggler(self.night_shift_enabled).on_toggle(Message::NightShiftToggled),
        );

        let temperature_slider = widget::settings::item(
            format!("Color Temperature: {}K", self.color_temperature as i32),
            widget::slider(
                2500.0..=6500.0,
                self.color_temperature,
                Message::TemperatureChanged,
            )
            .step(50.0)
            .on_release(Message::TemperatureCommitted)
            .width(Length::Fixed(240.0)),
        );

        let schedule_dropdown = widget::settings::item(
            "Schedule",
            widget::dropdown(
                SCHEDULE_OPTIONS,
                Some(self.schedule.index()),
                Message::ScheduleSelected,
            ),
        );

        let content = widget::settings::view_column(vec![
            widget::text::title2("Night Shift Settings").into(),
            widget::settings::section()
                .title("General")
                .add(night_shift_toggle)
                .add(temperature_slider)
                .add(schedule_dropdown)
                .into(),
        ])
        .width(Length::Fill);

        widget::container(content)
            .max_width(600.0)
            .padding(20)
            .center_x(Length::Fill)
            .into()
    }
}
