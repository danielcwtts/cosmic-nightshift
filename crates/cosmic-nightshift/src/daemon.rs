// SPDX-License-Identifier: MPL-2.0

//! Background daemon mode (`cosmic-nightshift --daemon`).
//!
//! Runs an indefinite loop that reads the shared [`config`] every minute and
//! applies (or clears) the night tint through the backend whenever the desired
//! state changes. On startup it applies the saved state immediately, so a
//! daemon launched at login restores the user's tint right away.
//!
//! Two schedule modes:
//! - **Manual**: the tint follows the on/off toggle only.
//! - **Sunset to Sunrise**: the tint is on (when enabled) between the
//!   configured sunset and sunrise hours.

use std::thread;
use std::time::Duration;

use chrono::{Local, Timelike};

use crate::backend;
use crate::config::{self, Schedule};

const POLL_INTERVAL: Duration = Duration::from_secs(60);

/// Runs the daemon loop forever. Applies a change only when the desired tint
/// actually differs from what's already applied, so we don't trigger a VT
/// bounce on every poll.
pub fn run() {
    println!("cosmic-nightshift: running in daemon mode");

    let handler = config::handler();
    // `None` = nothing applied yet (forces the first computed state to apply).
    let mut applied: Option<Option<u32>> = None;

    loop {
        let settings = config::Settings::load_from(&handler);
        let desired = desired_temperature(&settings);

        if applied != Some(desired) {
            match desired {
                Some(kelvin) => {
                    println!("cosmic-nightshift: applying {kelvin}K");
                    backend::apply_color_temperature(kelvin, settings.brightness as f32);
                }
                None => {
                    println!("cosmic-nightshift: clearing tint");
                    backend::reset();
                }
            }
            applied = Some(desired);
        }

        thread::sleep(POLL_INTERVAL);
    }
}

/// The tint that should currently be applied: `Some(kelvin)` for a warm tint,
/// or `None` for a neutral screen.
fn desired_temperature(settings: &config::Settings) -> Option<u32> {
    match settings.schedule {
        Schedule::Manual => settings.enabled.then_some(settings.temperature),
        Schedule::SunsetToSunrise => {
            let night = is_night(settings.sunrise_hour, settings.sunset_hour);
            (settings.enabled && night).then_some(settings.temperature)
        }
    }
}

/// Whether the current local hour is within the night window.
fn is_night(sunrise_hour: u32, sunset_hour: u32) -> bool {
    let hour = Local::now().hour();
    hour < sunrise_hour || hour >= sunset_hour
}
