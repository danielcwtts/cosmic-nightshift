// SPDX-License-Identifier: MPL-2.0

//! DRM/KMS gamma application.
//!
//! Enumerates the DRM cards under `/dev/dri`, grabs the DRM master lock on
//! each (which only succeeds while no compositor holds it — see
//! [`crate::vt`]), and writes per-channel gamma LUTs to every active CRTC.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::path::{Path, PathBuf};
use std::time::Duration;

use drm::control::Device as ControlDevice;
use drm::Device as BasicDevice;

use crate::gamma;

/// Thin wrapper that lets us implement the `drm` crate's device traits over
/// an owned file handle to a `/dev/dri/cardN` node.
struct Card(File);

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl drm::Device for Card {}
impl ControlDevice for Card {}

impl Card {
    fn open(path: &Path) -> io::Result<Self> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map(Card)
    }
}

/// Lists the primary DRM nodes (`/dev/dri/card*`), skipping render nodes.
fn card_paths() -> io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir("/dev/dri")? {
        let path = entry?.path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with("card") {
                paths.push(path);
            }
        }
    }
    paths.sort();
    Ok(paths)
}

/// Tries to acquire the DRM master lock, retrying briefly to absorb the
/// race between switching VTs and logind actually revoking the
/// compositor's master.
fn acquire_master(card: &Card) -> io::Result<()> {
    let mut last_err = None;
    for _ in 0..20 {
        match card.acquire_master_lock() {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_err = Some(err);
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| io::Error::other("could not acquire DRM master")))
}

/// Writes the gamma ramp for `kelvin`/`brightness` to every active CRTC on
/// a single card. Returns the number of CRTCs updated.
fn apply_card(path: &Path, kelvin: u32, brightness: f64) -> io::Result<usize> {
    let card = Card::open(path)?;
    acquire_master(&card)?;

    let mut updated = 0;
    let resources = card.resource_handles()?;

    for &crtc_handle in resources.crtcs() {
        let info = card.get_crtc(crtc_handle)?;

        // A CRTC with no current mode is not driving a display; skip it.
        if info.mode().is_none() {
            continue;
        }

        let size = info.gamma_length() as usize;
        if size == 0 {
            // Driver does not expose a legacy gamma LUT for this CRTC.
            continue;
        }

        let [red, green, blue] = gamma::ramp(kelvin, brightness, size);
        card.set_gamma(crtc_handle, &red, &green, &blue)?;
        updated += 1;
    }

    // Release master so the compositor can reclaim it on VT switch-back.
    card.release_master_lock()?;
    Ok(updated)
}

/// Applies the gamma ramp to every active CRTC on every card.
///
/// Cards that cannot be updated (no master, no displays) are logged and
/// skipped rather than aborting the whole operation. Returns the total
/// number of CRTCs updated across all cards.
pub fn apply_all(kelvin: u32, brightness: f64) -> io::Result<usize> {
    let paths = card_paths()?;
    if paths.is_empty() {
        return Err(io::Error::other("no DRM cards found under /dev/dri"));
    }

    let mut total = 0;
    for path in paths {
        match apply_card(&path, kelvin, brightness) {
            Ok(n) => total += n,
            Err(err) => eprintln!("nightshift: {}: {err}", path.display()),
        }
    }
    Ok(total)
}
