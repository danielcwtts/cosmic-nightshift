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

/// App ids this binary has shipped under in the past. Their autostart entries
/// must be cleaned up on disable (and before enable), otherwise an entry written
/// by an older build keeps launching on login regardless of the toggle. See the
/// Night Shift → Night Light rename.
const LEGACY_APP_IDS: &[&str] = &["io.github.cosmic_nightshift"];

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

/// Whether an autostart entry currently exists on disk — under the current app
/// id or any legacy one, so the toggle reflects reality immediately after an
/// upgrade (toggling either way then rewrites to the current app id).
pub fn is_enabled() -> bool {
    if entry_path().is_some_and(|p| p.exists()) {
        return true;
    }
    autostart_dir().is_some_and(|dir| {
        LEGACY_APP_IDS
            .iter()
            .any(|app_id| dir.join(format!("{app_id}.desktop")).exists())
    })
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
    // Drop any entries from older app ids so we don't launch twice on login.
    remove_legacy_entries();

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
    remove_legacy_entries();
    if let Some(path) = entry_path() {
        remove_if_present(&path)?;
    }
    Ok(())
}

/// Removes autostart entries written under any [`LEGACY_APP_IDS`], ignoring ones
/// that aren't there. A failure to remove one is logged but not fatal, so the
/// current-app-id entry is still handled.
fn remove_legacy_entries() {
    let Some(dir) = autostart_dir() else {
        return;
    };
    for app_id in LEGACY_APP_IDS {
        let path = dir.join(format!("{app_id}.desktop"));
        if let Err(err) = remove_if_present(&path) {
            eprintln!("cosmic-nightlight: failed to remove legacy autostart entry {path:?}: {err}");
        }
    }
}

/// Deletes `path`, treating "already gone" as success.
fn remove_if_present(path: &PathBuf) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}
