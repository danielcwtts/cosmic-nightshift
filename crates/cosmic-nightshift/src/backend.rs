// SPDX-License-Identifier: MPL-2.0

//! Backend bridge from the (unprivileged) GUI/daemon to the privileged
//! `cosmic-nightshift-helper`.
//!
//! Setting the DRM gamma under COSMIC requires root (to switch VTs and grab
//! the DRM master lock), so the GUI never touches DRM directly. Instead it
//! shells out to the helper through `pkexec`; the bundled polkit rule lets
//! members of the `wheel`/`sudo` group run it without a password prompt.

use std::process::Command;

/// Where the helper may live, in priority order. The `.deb` installs to
/// `/usr/bin`; the `install.sh` script uses `/usr/local/bin`.
const HELPER_CANDIDATES: &[&str] = &[
    "/usr/bin/cosmic-nightshift-helper",
    "/usr/local/bin/cosmic-nightshift-helper",
];

/// Resolves the helper path: an explicit `COSMIC_NIGHTSHIFT_HELPER` override
/// wins; otherwise the first candidate that exists on disk is used. Falls
/// back to the first candidate so pkexec produces a clear error if nothing
/// is installed.
fn helper_path() -> String {
    if let Ok(path) = std::env::var("COSMIC_NIGHTSHIFT_HELPER") {
        return path;
    }
    for candidate in HELPER_CANDIDATES {
        if std::path::Path::new(candidate).exists() {
            return candidate.to_string();
        }
    }
    HELPER_CANDIDATES[0].to_string()
}

/// Runs the helper via `pkexec` with the given arguments, logging the result.
fn run_helper(args: &[String]) {
    let helper = helper_path();
    let mut command = Command::new("pkexec");
    command.arg(&helper).args(args);

    match command.status() {
        Ok(status) if status.success() => {
            println!("backend: helper applied {args:?}");
        }
        Ok(status) => {
            eprintln!("backend: helper exited with {status} (args: {args:?})");
        }
        Err(err) => {
            eprintln!("backend: failed to launch pkexec for {helper} ({err})");
        }
    }
}

/// Applies a color temperature (Kelvin) at the given brightness (`0.0..=1.0`).
pub fn apply_color_temperature(kelvin: u32, brightness: f32) {
    run_helper(&[
        "--temp".to_string(),
        kelvin.to_string(),
        "--brightness".to_string(),
        format!("{brightness:.3}"),
    ]);
}

/// Resets all displays to a neutral, untinted ramp.
pub fn reset() {
    run_helper(&["--off".to_string()]);
}
