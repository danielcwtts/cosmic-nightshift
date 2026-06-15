// SPDX-License-Identifier: MPL-2.0

//! Core night-light engine for COSMIC.
//!
//! COSMIC's compositor does not yet implement `wlr-gamma-control-unstable-v1`
//! (pop-os/cosmic-comp#764), so there is no Wayland path to adjust the screen
//! color temperature. This crate works around that by writing the gamma LUTs
//! straight to the kernel's DRM/KMS layer.
//!
//! Because the running compositor owns the DRM master lock, the write only
//! succeeds during the brief window after a VT switch, when logind has
//! revoked the compositor's master. [`apply`] performs that VT bounce around
//! the gamma write; the values then persist after switching back.
//!
//! All of this requires root, so the intended entry point is the
//! `nightshift-helper` binary invoked via `pkexec`.

use std::io;

pub mod drm;
pub mod gamma;
pub mod vt;

pub use gamma::{MAX_KELVIN, MIN_KELVIN, NEUTRAL_KELVIN};

/// Outcome of an [`apply`] call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Applied {
    /// Number of CRTCs (display pipes) whose gamma was updated.
    pub crtcs: usize,
}

/// Applies a color temperature (Kelvin) and brightness (`0.0..=1.0`) to all
/// active displays.
///
/// Performs the VT bounce internally, so the caller must be running as root.
/// Passing [`NEUTRAL_KELVIN`] with full brightness resets displays to an
/// identity ramp (i.e. turns the tint off).
pub fn apply(kelvin: u32, brightness: f64) -> io::Result<Applied> {
    let kelvin = kelvin.clamp(MIN_KELVIN, MAX_KELVIN);
    let crtcs = vt::with_master_window(|| drm::apply_all(kelvin, brightness))??;
    Ok(Applied { crtcs })
}

/// Resets all displays to a neutral (untinted) ramp.
pub fn reset() -> io::Result<Applied> {
    apply(NEUTRAL_KELVIN, 1.0)
}

/// Returns `true` if the current process is running as root, which every
/// real apply path requires (VT ioctls + DRM master).
pub fn is_root() -> bool {
    // SAFETY: geteuid is always safe to call.
    unsafe { libc::geteuid() == 0 }
}
