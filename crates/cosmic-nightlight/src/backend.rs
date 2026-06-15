// SPDX-License-Identifier: MPL-2.0

//! Backend bridge from the (unprivileged) GUI/daemon to the privileged
//! `cosmic-nightlight-helper`.
//!
//! Setting the DRM gamma under COSMIC requires root (to switch VTs and grab
//! the DRM master lock), so the GUI never touches DRM directly. Instead it
//! shells out to the helper through `pkexec`; the bundled polkit rule lets
//! members of the `wheel`/`sudo` group run it without a password prompt.

use std::process::Command;

/// Where the helper may live, in priority order. The `.deb` installs to
/// `/usr/bin`; the `install.sh` script uses `/usr/local/bin`.
const HELPER_CANDIDATES: &[&str] = &[
    "/usr/bin/cosmic-nightlight-helper",
    "/usr/local/bin/cosmic-nightlight-helper",
];

/// Resolves the helper path: an explicit `COSMIC_NIGHTLIGHT_HELPER` override
/// wins; otherwise the first candidate that exists on disk is used. Falls
/// back to the first candidate so pkexec produces a clear error if nothing
/// is installed.
fn helper_path() -> String {
    if let Ok(path) = std::env::var("COSMIC_NIGHTLIGHT_HELPER") {
        return path;
    }
    for candidate in HELPER_CANDIDATES {
        if std::path::Path::new(candidate).exists() {
            return candidate.to_string();
        }
    }
    HELPER_CANDIDATES[0].to_string()
}

/// The graphical-session VT, as the `--session-vt` argument to the helper, or
/// an empty vec if `XDG_VTNR` is unset (no local VT — best-effort, the helper
/// then snapshots the foreground VT). `pkexec` strips the environment, so this
/// has to be passed explicitly rather than inherited.
fn session_vt_args() -> Vec<String> {
    match std::env::var("XDG_VTNR") {
        Ok(vt) if !vt.is_empty() => vec!["--session-vt".to_string(), vt],
        _ => Vec::new(),
    }
}

/// Runs the helper via `pkexec` with the given arguments, logging the result.
/// Returns `true` only if the helper ran and exited successfully, so callers
/// (the daemon) can retry instead of assuming a failed apply took effect.
fn run_helper(args: &[String]) -> bool {
    let helper = helper_path();
    let mut command = Command::new("pkexec");
    command.arg(&helper).args(args).args(session_vt_args());

    match command.status() {
        Ok(status) if status.success() => {
            println!("backend: helper applied {args:?}");
            true
        }
        Ok(status) => {
            eprintln!("backend: helper exited with {status} (args: {args:?})");
            false
        }
        Err(err) => {
            eprintln!("backend: failed to launch pkexec for {helper} ({err})");
            false
        }
    }
}

/// Applies a color temperature (Kelvin) at the given brightness (`0.0..=1.0`).
/// Returns whether the helper succeeded.
pub fn apply_color_temperature(kelvin: u32, brightness: f32) -> bool {
    run_helper(&[
        "--temp".to_string(),
        kelvin.to_string(),
        "--brightness".to_string(),
        format!("{brightness:.3}"),
    ])
}

/// Resets all displays to a neutral, untinted ramp. Returns whether the helper
/// succeeded.
pub fn reset() -> bool {
    run_helper(&["--off".to_string()])
}
