// SPDX-License-Identifier: MPL-2.0

//! Persistent settings shared by every run mode (applet, settings window,
//! daemon). They are stored through libcosmic's `cosmic_config` so the three
//! processes — all running as the same user — read and write the same state at
//! `~/.config/cosmic/io.github.cosmic_nightlight/v1/<key>`.
//!
//! Reads fall back to [`Settings::default`] when a key is missing or the config
//! directory is unavailable; writes are best-effort (a missing config handle
//! just means nothing is persisted).

use chrono::{Local, Timelike};
use cosmic::cosmic_config::{Config, ConfigGet, ConfigSet};

/// Config namespace; also the application/desktop id.
pub const APP_ID: &str = "io.github.cosmic_nightlight";

/// Bumped if the on-disk schema ever changes incompatibly.
const CONFIG_VERSION: u64 = 1;

const KEY_OVERRIDE: &str = "override";
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

/// Manual override of the current scheduled tint state.
///
/// `Auto` follows the schedule. `On`/`Off` force the tint regardless of the
/// schedule (e.g. warm the screen at noon); the daemon auto-clears the override
/// back to `Auto` once the schedule next agrees with it, so a manual choice
/// lasts only until the next sunset/sunrise transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Override {
    Auto,
    On,
    Off,
}

impl Override {
    fn as_key(self) -> &'static str {
        match self {
            Override::Auto => "auto",
            Override::On => "on",
            Override::Off => "off",
        }
    }

    fn from_key(key: &str) -> Self {
        match key {
            "on" => Override::On,
            "off" => Override::Off,
            _ => Override::Auto,
        }
    }
}

/// A snapshot of all persisted settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub tint_override: Override,
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
            tint_override: Override::Auto,
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

        if let Ok(v) = config.get::<String>(KEY_OVERRIDE) {
            settings.tint_override = Override::from_key(&v);
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

    /// Whether the *schedule alone* (ignoring any manual override) wants the
    /// tint on right now. Manual mode has no time schedule, so its baseline is
    /// always off — the tint there is driven purely by the override.
    pub fn schedule_wants_tint(&self) -> bool {
        match self.schedule {
            Schedule::Manual => false,
            Schedule::SunsetToSunrise => is_night(self.sunrise_hour, self.sunset_hour),
        }
    }

    /// Whether the tint should be on right now, accounting for the override.
    pub fn tint_on(&self) -> bool {
        match self.tint_override {
            Override::Auto => self.schedule_wants_tint(),
            Override::On => true,
            Override::Off => false,
        }
    }
}

/// Watches the config store on disk and emits a fresh [`Settings`] snapshot on
/// startup and again whenever any key changes.
///
/// This lets the applet and settings window mirror each other's toggle and
/// temperature changes live: each writes through `cosmic_config`, and the
/// resulting file change wakes up the other process's subscription.
pub fn subscription() -> cosmic::iced::Subscription<Settings> {
    cosmic::iced::Subscription::run(|| {
        cosmic::iced::stream::channel(10, |mut output: cosmic::iced::futures::channel::mpsc::Sender<Settings>| async move {
            use cosmic::iced::futures::{SinkExt, StreamExt};

            let Some(config) = handler() else {
                std::future::pending::<()>().await;
                unreachable!();
            };

            let _ = output.send(Settings::load_from(&Some(config.clone()))).await;

            let (tx, mut rx) = cosmic::iced::futures::channel::mpsc::channel(10);
            let Ok(_watcher) = config.watch(move |_, _keys| {
                let _ = tx.clone().try_send(());
            }) else {
                std::future::pending::<()>().await;
                unreachable!();
            };

            while rx.next().await.is_some() {
                let _ = output.send(Settings::load_from(&Some(config.clone()))).await;
            }
        })
    })
}

/// Whether the current local hour is within the night window
/// (`[sunset_hour, sunrise_hour)`, wrapping past midnight).
fn is_night(sunrise_hour: u32, sunset_hour: u32) -> bool {
    let hour = Local::now().hour();
    hour < sunrise_hour || hour >= sunset_hour
}

/// Opens (or creates) the config store; `None` if no config directory exists.
pub fn handler() -> Option<Config> {
    Config::new(APP_ID, CONFIG_VERSION).ok()
}

/// Best-effort write helpers. Each silently no-ops without a config handle.
/// `value` types are concrete so the `Serialize` bound on [`ConfigSet::set`] is
/// satisfied by inference (avoids a direct `serde` dependency).
pub fn store_override(handler: &Option<Config>, value: Override) {
    if let Some(config) = handler {
        report(KEY_OVERRIDE, config.set(KEY_OVERRIDE, value.as_key()));
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
        eprintln!("cosmic-nightlight: failed to persist {key}: {err}");
    }
}

/// Returns whether the system is configured to use 24-hour time.
pub fn is_military_time() -> bool {
    cosmic::cosmic_config::Config::new("com.system76.CosmicAppletTime", 1)
        .ok()
        .and_then(|c| c.get::<bool>("military_time").ok())
        .unwrap_or(false) // Default to 12-hour if unknown
}

/// Formats an hour (0..23) according to the system's 24-hour time setting.
pub fn format_hour(hour: u32, military: bool) -> String {
    if military {
        format!("{hour:02}:00")
    } else {
        let h12 = if hour % 12 == 0 { 12 } else { hour % 12 };
        let ampm = if hour < 12 { "AM" } else { "PM" };
        format!("{h12}:00{ampm}") // e.g. "10:00PM"
    }
}
