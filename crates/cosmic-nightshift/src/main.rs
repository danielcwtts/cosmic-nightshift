// SPDX-License-Identifier: MPL-2.0

//! `cosmic-nightshift` is a single binary with three run modes, selected by
//! command-line flag:
//!
//! - (no args)    → run as a COSMIC panel applet (the status-bar icon + popup)
//! - `--settings` → open the settings window
//! - `--daemon`   → run the headless sunset/sunrise scheduler
//!
//! All three share state through [`config`] (`cosmic_config`).

mod applet;
mod autostart;
mod backend;
mod config;
mod daemon;
mod settings_window;

fn main() -> cosmic::iced::Result {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|arg| arg == "--daemon") {
        daemon::run();
        return Ok(());
    }

    if args.iter().any(|arg| arg == "--settings") {
        return settings_window::run();
    }

    applet::run()
}
