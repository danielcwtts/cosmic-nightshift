// SPDX-License-Identifier: MPL-2.0

//! Persistent settings shared by every run mode (applet, settings window,
//! daemon). They are stored through libcosmic's `cosmic_config` so the three
//! processes — all running as the same user — read and write the same state at
//! `~/.config/cosmic/io.github.cosmic_nightshift/v1/<key>`.
//!
//! Reads fall back to [`Settings::default`] when a key is missing or the config
//! directory is unavailable; writes are best-effort (a missing config handle
//! just means nothing is persisted).

use cosmic::cosmic_config::{Config, ConfigGet, ConfigSet};

/// Config namespace; also the application/desktop id.
pub const APP_ID: &str = "io.github.cosmic_nightshift";

/// Bumped if the on-disk schema ever changes incompatibly.
const CONFIG_VERSION: u64 = 1;

const KEY_ENABLED: &str = "enabled";
const KEY_TEMPERATURE: &str = "temperature";
const KEY_BRIGHTNESS: &str = "brightness";
const KEY_SCHEDULE: &str = "schedule";
const KEY_SUNRISE_HOUR: &str = "sunrise_hour";
const KEY_SUNSET_HOUR: &str = "sunset_hour";
const KEY_AUTOSTART: &str = "autostart";

/// How the night tint is driven.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Schedule {
    /// Tint follows the manual on/off toggle only.
    Manual,
    /// Tint turns on after sunset and off after sunrise.
    SunsetToSunrise,
}

impl Schedule {
    pub const ALL: [Schedule; 2] = [Schedule::Manual, Schedule::SunsetToSunrise];

    /// Index into [`Schedule::ALL`], for the settings dropdown.
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    fn as_key(self) -> &'static str {
        match self {
            Schedule::Manual => "manual",
            Schedule::SunsetToSunrise => "sunset",
        }
    }

    fn from_key(key: &str) -> Self {
        match key {
            "sunset" => Schedule::SunsetToSunrise,
            _ => Schedule::Manual,
        }
    }
}

/// A snapshot of all persisted settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub enabled: bool,
    pub temperature: u32,
    pub brightness: f64,
    pub schedule: Schedule,
    pub sunrise_hour: u32,
    pub sunset_hour: u32,
    pub autostart: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            temperature: 4500,
            brightness: 1.0,
            schedule: Schedule::Manual,
            sunrise_hour: 6,
            sunset_hour: 18,
            autostart: false,
        }
    }
}

impl Settings {
    /// Reads every key from the store, falling back to defaults.
    pub fn load() -> Self {
        let handler = handler();
        Self::load_from(&handler)
    }

    /// Reads every key from an already-opened handle, falling back to defaults.
    pub fn load_from(handler: &Option<Config>) -> Self {
        let mut settings = Settings::default();
        let Some(config) = handler else {
            return settings;
        };

        if let Ok(v) = config.get::<bool>(KEY_ENABLED) {
            settings.enabled = v;
        }
        if let Ok(v) = config.get::<u32>(KEY_TEMPERATURE) {
            settings.temperature = v;
        }
        if let Ok(v) = config.get::<f64>(KEY_BRIGHTNESS) {
            settings.brightness = v;
        }
        if let Ok(v) = config.get::<String>(KEY_SCHEDULE) {
            settings.schedule = Schedule::from_key(&v);
        }
        if let Ok(v) = config.get::<u32>(KEY_SUNRISE_HOUR) {
            settings.sunrise_hour = v;
        }
        if let Ok(v) = config.get::<u32>(KEY_SUNSET_HOUR) {
            settings.sunset_hour = v;
        }
        if let Ok(v) = config.get::<bool>(KEY_AUTOSTART) {
            settings.autostart = v;
        }

        settings
    }
}

/// Opens (or creates) the config store; `None` if no config directory exists.
pub fn handler() -> Option<Config> {
    Config::new(APP_ID, CONFIG_VERSION).ok()
}

/// Best-effort write helpers. Each silently no-ops without a config handle.
/// `value` types are concrete so the `Serialize` bound on [`ConfigSet::set`] is
/// satisfied by inference (avoids a direct `serde` dependency).
pub fn store_enabled(handler: &Option<Config>, value: bool) {
    if let Some(config) = handler {
        report(KEY_ENABLED, config.set(KEY_ENABLED, value));
    }
}

pub fn store_temperature(handler: &Option<Config>, value: u32) {
    if let Some(config) = handler {
        report(KEY_TEMPERATURE, config.set(KEY_TEMPERATURE, value));
    }
}

pub fn store_schedule(handler: &Option<Config>, value: Schedule) {
    if let Some(config) = handler {
        report(KEY_SCHEDULE, config.set(KEY_SCHEDULE, value.as_key()));
    }
}

pub fn store_sunrise_hour(handler: &Option<Config>, value: u32) {
    if let Some(config) = handler {
        report(KEY_SUNRISE_HOUR, config.set(KEY_SUNRISE_HOUR, value));
    }
}

pub fn store_sunset_hour(handler: &Option<Config>, value: u32) {
    if let Some(config) = handler {
        report(KEY_SUNSET_HOUR, config.set(KEY_SUNSET_HOUR, value));
    }
}

pub fn store_autostart(handler: &Option<Config>, value: bool) {
    if let Some(config) = handler {
        report(KEY_AUTOSTART, config.set(KEY_AUTOSTART, value));
    }
}

fn report(key: &str, result: Result<(), cosmic::cosmic_config::Error>) {
    if let Err(err) = result {
        eprintln!("cosmic-nightshift: failed to persist {key}: {err}");
    }
}
