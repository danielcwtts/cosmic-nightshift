// SPDX-License-Identifier: MPL-2.0

//! Background daemon mode.
//!
//! When the application is launched with `--daemon`, no GUI is shown.
//! Instead this module runs an indefinite loop that checks the current
//! local time against a sunset/sunrise schedule and applies (or clears)
//! the night-light tint via the backend whenever the boundary is crossed.

use std::thread;
use std::time::Duration;

use chrono::{Local, Timelike};

use crate::backend;

/// Color temperature applied while it's considered "night".
const NIGHT_TEMPERATURE: u32 = 3500;

/// Brightness used for the night tint (`0.0..=1.0`).
const NIGHT_BRIGHTNESS: f32 = 1.0;

/// Placeholder sunrise/sunset hours (local time, 24h clock).
///
/// A real implementation should compute these from the user's location
/// (e.g. via geoclue or an astronomical calculation crate) rather than
/// using fixed hours.
const SUNRISE_HOUR: u32 = 6;
const SUNSET_HOUR: u32 = 18;

const POLL_INTERVAL: Duration = Duration::from_secs(60);

/// Runs the daemon loop forever, applying the night tint after sunset and
/// clearing it after sunrise, but only when the boundary is actually
/// crossed (so we don't trigger a VT bounce every poll).
pub fn run() {
    println!("cosmic-nightshift: running in daemon mode");

    let mut current_is_night: Option<bool> = None;

    loop {
        let hour = Local::now().hour();
        let is_night = hour < SUNRISE_HOUR || hour >= SUNSET_HOUR;

        if current_is_night != Some(is_night) {
            if is_night {
                println!("cosmic-nightshift: now night -> applying {NIGHT_TEMPERATURE}K");
                backend::apply_color_temperature(NIGHT_TEMPERATURE, NIGHT_BRIGHTNESS);
            } else {
                println!("cosmic-nightshift: now day -> clearing tint");
                backend::reset();
            }

            current_is_night = Some(is_night);
        }

        thread::sleep(POLL_INTERVAL);
    }
}
