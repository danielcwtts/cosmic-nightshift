// SPDX-License-Identifier: MPL-2.0

//! XDG autostart integration.
//!
//! Toggling autostart writes (or removes) a desktop entry under
//! `~/.config/autostart/`. The entry launches `cosmic-nightlight --daemon`, so
//! on the next login the background scheduler starts and — because the daemon
//! re-applies the saved tint on startup — the user's night-light state is
//! restored automatically.

use std::fs;
use std::io;
use std::path::PathBuf;

use crate::config::APP_ID;

/// File name of the autostart entry (matches the app id, as is conventional).
fn entry_path() -> Option<PathBuf> {
    Some(autostart_dir()?.join(format!("{APP_ID}.desktop")))
}

/// `$XDG_CONFIG_HOME/autostart`, falling back to `$HOME/.config/autostart`.
fn autostart_dir() -> Option<PathBuf> {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(config_home.join("autostart"))
}

/// Resolves the running binary's path so the autostart entry points at the
/// same executable that is installed (handles `/usr/bin` vs `/usr/local/bin`).
fn exec_path() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(str::to_owned))
        .unwrap_or_else(|| "cosmic-nightlight".to_string())
}

/// Whether the autostart entry currently exists on disk.
pub fn is_enabled() -> bool {
    entry_path().is_some_and(|p| p.exists())
}

/// Applies the desired autostart state, creating or removing the entry.
pub fn set(enabled: bool) -> io::Result<()> {
    if enabled {
        enable()
    } else {
        disable()
    }
}

fn enable() -> io::Result<()> {
    let path =
        entry_path().ok_or_else(|| io::Error::other("no XDG config directory for autostart"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let contents = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Night Light (daemon)\n\
         Comment=Apply the night-light tint on a schedule\n\
         Exec={exec} --daemon\n\
         Icon=weather-clear-night-symbolic\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n",
        exec = exec_path(),
    );

    fs::write(path, contents)
}

fn disable() -> io::Result<()> {
    if let Some(path) = entry_path() {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
    }
    Ok(())
}
