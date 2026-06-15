// SPDX-License-Identifier: MPL-2.0

//! Background daemon mode (`cosmic-nightlight --daemon`).
//!
//! Runs an indefinite loop that reads the shared [`config`] every minute and
//! applies (or clears) the night tint through the backend whenever the desired
//! state changes.
//!
//! The desired state comes from [`config::Settings::tint_on`], which combines
//! the schedule (Manual / Sunset-to-Sunrise) with the manual override. When a
//! manual override has been "caught up" by the schedule (e.g. a force-on lasts
//! until sunset arrives), the daemon clears it back to `Auto` so automatic
//! scheduling resumes.

use std::thread;
use std::time::{Duration, Instant};

use crate::backend;
use crate::config::{self, Override};

const POLL_INTERVAL: Duration = Duration::from_secs(60);

/// While an apply is failing (e.g. displays/pkexec not ready right after
/// login), retry on this shorter interval instead of waiting a full poll.
const RETRY_INTERVAL: Duration = Duration::from_secs(5);

/// Cap on consecutive fast retries before backing off to [`POLL_INTERVAL`], so
/// a persistent failure doesn't spin (or flicker) forever.
const MAX_FAST_RETRIES: u32 = 12;

/// How long to wait for the graphical session to become the foreground VT
/// before giving up and proceeding anyway.
const SESSION_READY_TIMEOUT: Duration = Duration::from_secs(60);
const SESSION_READY_POLL: Duration = Duration::from_millis(500);

/// Runs the daemon loop forever. Applies a change only when the desired tint
/// actually differs from what's already applied, so we don't trigger a VT
/// bounce on every poll.
pub fn run() {
    println!("cosmic-nightlight: running in daemon mode");

    // At login the daemon can start before the compositor owns its VT. Doing a
    // VT bounce during that handoff can strand the user on a spare TTY, so wait
    // until the graphical session is actually foreground before touching DRM.
    wait_for_session_foreground();

    let handler = config::handler();
    // A fresh login starts with an identity gamma ramp (the compositor sets it
    // at modeset and we never persist hardware state across logout). So treat
    // the screen as already neutral: a startup state of "tint off" then matches
    // and fires nothing — avoiding a pointless reset bounce at login — while a
    // real tint still differs and applies.
    let mut applied: Option<Option<u32>> = Some(None);
    let mut failures: u32 = 0;

    loop {
        let settings = config::Settings::load_from(&handler);
        expire_override(&handler, &settings);
        let desired = settings.tint_on().then_some(settings.temperature);

        let sleep_for = if applied == Some(desired) {
            POLL_INTERVAL
        } else {
            let ok = match desired {
                Some(kelvin) => {
                    println!("cosmic-nightlight: applying {kelvin}K");
                    backend::apply_color_temperature(kelvin, settings.brightness as f32)
                }
                None => {
                    println!("cosmic-nightlight: clearing tint");
                    backend::reset()
                }
            };

            if ok {
                applied = Some(desired);
                failures = 0;
                POLL_INTERVAL
            } else {
                // Leave `applied` unchanged so we try again rather than
                // assuming a failed apply took effect (the bug where the tint
                // never came up at login). Retry quickly at first — the
                // displays or pkexec may just not be ready yet — then back off.
                failures += 1;
                eprintln!("cosmic-nightlight: apply failed (attempt {failures}); will retry");
                if failures >= MAX_FAST_RETRIES {
                    POLL_INTERVAL
                } else {
                    RETRY_INTERVAL
                }
            }
        };

        thread::sleep(sleep_for);
    }
}

/// Clears a manual override back to `Auto` once the schedule has caught up to
/// it, so a force-on/force-off lasts only until the next sunset/sunrise
/// transition and automatic scheduling then resumes. A no-op when the override
/// is already `Auto` or still differs from the schedule.
fn expire_override(handler: &Option<cosmic::cosmic_config::Config>, settings: &config::Settings) {
    let want = settings.schedule_wants_tint();
    let caught_up = match settings.tint_override {
        Override::On => want,
        Override::Off => !want,
        Override::Auto => false,
    };
    if caught_up {
        config::store_override(handler, Override::Auto);
    }
}

/// Blocks until our graphical session is the foreground VT (or a timeout), so
/// the first apply doesn't VT-bounce during the greeter→session handoff.
///
/// The session VT is `XDG_VTNR`; the foreground VT is the world-readable
/// `/sys/class/tty/tty0/active` (e.g. `"tty2"`). If `XDG_VTNR` is unset (no
/// local VT) there is nothing to wait for, so we return immediately.
fn wait_for_session_foreground() {
    let Some(session_vt) = std::env::var("XDG_VTNR")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
    else {
        return;
    };

    let deadline = Instant::now() + SESSION_READY_TIMEOUT;
    loop {
        if foreground_vt() == Some(session_vt) {
            return;
        }
        if Instant::now() >= deadline {
            eprintln!(
                "cosmic-nightlight: timed out waiting for session VT {session_vt} to be foreground; proceeding anyway"
            );
            return;
        }
        thread::sleep(SESSION_READY_POLL);
    }
}

/// The currently-active VT number, parsed from `/sys/class/tty/tty0/active`
/// (contents like `"tty2\n"`). `None` if it can't be read or parsed.
fn foreground_vt() -> Option<u32> {
    let active = std::fs::read_to_string("/sys/class/tty/tty0/active").ok()?;
    active.trim().strip_prefix("tty")?.parse().ok()
}
