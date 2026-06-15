// SPDX-License-Identifier: MPL-2.0

//! Privileged backend for cosmic-nightshift.
//!
//! This is the only part of the project that runs as root (via `pkexec`).
//! It keeps a deliberately tiny surface: parse a target temperature and
//! brightness, then ask `nightshift-core` to write the gamma LUTs to DRM,
//! performing the VT bounce required under COSMIC.
//!
//! Usage:
//!   cosmic-nightshift-helper --temp <kelvin> [--brightness <0.0-1.0>]
//!   cosmic-nightshift-helper --off          (reset to a neutral ramp)

use std::process::ExitCode;

struct Args {
    kelvin: u32,
    brightness: f64,
}

fn parse_args() -> Result<Args, String> {
    let mut kelvin: Option<u32> = None;
    let mut brightness: f64 = 1.0;
    let mut off = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--off" => off = true,
            "--temp" | "-t" => {
                let v = args.next().ok_or("--temp requires a value")?;
                kelvin = Some(v.parse().map_err(|_| format!("invalid temperature: {v}"))?);
            }
            "--brightness" | "-b" => {
                let v = args.next().ok_or("--brightness requires a value")?;
                brightness = v.parse().map_err(|_| format!("invalid brightness: {v}"))?;
            }
            "--help" | "-h" => {
                return Err("help".to_string());
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    let kelvin = if off {
        nightshift_core::NEUTRAL_KELVIN
    } else {
        kelvin.ok_or("missing required --temp <kelvin> (or use --off)")?
    };

    if !(0.0..=1.0).contains(&brightness) {
        return Err(format!("brightness must be between 0.0 and 1.0 (got {brightness})"));
    }

    Ok(Args { kelvin, brightness })
}

fn usage() {
    eprintln!(
        "usage: cosmic-nightshift-helper --temp <kelvin> [--brightness <0.0-1.0>]\n       cosmic-nightshift-helper --off"
    );
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(msg) => {
            if msg != "help" {
                eprintln!("cosmic-nightshift-helper: {msg}");
            }
            usage();
            return ExitCode::FAILURE;
        }
    };

    if !nightshift_core::is_root() {
        eprintln!(
            "cosmic-nightshift-helper: must run as root (it switches VTs and grabs DRM master).\n\
             Invoke it via pkexec, not directly."
        );
        return ExitCode::FAILURE;
    }

    match nightshift_core::apply(args.kelvin, args.brightness) {
        Ok(applied) => {
            println!(
                "cosmic-nightshift-helper: applied {}K @ {:.0}% to {} CRTC(s)",
                args.kelvin,
                args.brightness * 100.0,
                applied.crtcs
            );
            if applied.crtcs == 0 {
                eprintln!(
                    "cosmic-nightshift-helper: warning: no CRTCs were updated (no active displays with gamma support?)"
                );
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("cosmic-nightshift-helper: failed to apply gamma: {err}");
            ExitCode::FAILURE
        }
    }
}
