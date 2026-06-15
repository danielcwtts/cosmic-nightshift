// SPDX-License-Identifier: MPL-2.0

//! Virtual-terminal juggling.
//!
//! cosmic-comp holds the DRM master lock while it is the foreground session,
//! so another process cannot set the CRTC gamma. Switching to a spare VT
//! makes logind revoke the compositor's DRM master; while we sit on that VT
//! a privileged process can grab master, write the gamma LUTs, and the
//! values survive the switch back because the compositor does not reset
//! them. This module performs that switch-and-restore dance.

use std::fs::OpenOptions;
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::time::Duration;

// VT ioctls from <linux/vt.h>.
const VT_OPENQRY: libc::c_ulong = 0x5600;
const VT_GETSTATE: libc::c_ulong = 0x5603;
const VT_ACTIVATE: libc::c_ulong = 0x5606;
const VT_WAITACTIVE: libc::c_ulong = 0x5607;

#[repr(C)]
struct VtStat {
    v_active: libc::c_ushort,
    v_signal: libc::c_ushort,
    v_state: libc::c_ushort,
}

fn errno() -> io::Error {
    io::Error::last_os_error()
}

fn current_vt(fd: RawFd) -> io::Result<libc::c_int> {
    let mut stat = VtStat {
        v_active: 0,
        v_signal: 0,
        v_state: 0,
    };
    // SAFETY: `fd` is a live console fd; `stat` is a valid VtStat.
    let rc = unsafe { libc::ioctl(fd, VT_GETSTATE, &mut stat as *mut VtStat) };
    if rc < 0 {
        return Err(errno());
    }
    Ok(stat.v_active as libc::c_int)
}

fn query_free_vt(fd: RawFd) -> io::Result<libc::c_int> {
    let mut vt: libc::c_int = 0;
    // SAFETY: `fd` is a live console fd; VT_OPENQRY writes an int.
    let rc = unsafe { libc::ioctl(fd, VT_OPENQRY, &mut vt as *mut libc::c_int) };
    if rc < 0 {
        return Err(errno());
    }
    Ok(vt)
}

fn activate(fd: RawFd, vt: libc::c_int) -> io::Result<()> {
    // SAFETY: `fd` is a live console fd; both ioctls take an int by value.
    let rc = unsafe { libc::ioctl(fd, VT_ACTIVATE, vt) };
    if rc < 0 {
        return Err(errno());
    }
    let rc = unsafe { libc::ioctl(fd, VT_WAITACTIVE, vt) };
    if rc < 0 {
        return Err(errno());
    }
    Ok(())
}

/// Restores the original VT when dropped, so we return the user to their
/// session even if the closure returns early or panics.
struct VtRestore {
    fd: RawFd,
    original: libc::c_int,
}

impl Drop for VtRestore {
    fn drop(&mut self) {
        let _ = activate(self.fd, self.original);
    }
}

/// Switches to a spare VT (making the compositor drop DRM master), runs
/// `f` while master is available, then switches back to the original VT.
///
/// Requires `CAP_SYS_TTY_CONFIG` (i.e. root) to open `/dev/tty0` and issue
/// the VT ioctls.
pub fn with_master_window<T>(f: impl FnOnce() -> T) -> io::Result<T> {
    let tty = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty0")?;
    let fd = tty.as_raw_fd();

    let original = current_vt(fd)?;
    let spare = query_free_vt(fd)?;

    activate(fd, spare)?;
    // Switch back to the user's session no matter how `f` exits.
    let _restore = VtRestore { fd, original };

    // Give logind a moment to revoke the compositor's DRM master after the
    // switch completes before we try to grab it.
    std::thread::sleep(Duration::from_millis(150));

    Ok(f())
}
