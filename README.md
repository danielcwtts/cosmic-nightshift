<!-- SPDX-License-Identifier: MPL-2.0 -->
# cosmic-nightlight

A night-light / red-tint utility for the **COSMIC** desktop (Pop!_OS). It warms
your screen's color temperature on a schedule, working around the fact that
COSMIC's compositor does not yet expose a color/gamma protocol.

## Install

Requires a Rust toolchain and `libdrm` headers (`libdrm-dev`).

```sh
# builds the root helper + installs it and the polkit rule
./scripts/install.sh

# also build & install the libcosmic GUI (slow: pulls libcosmic from git)
./scripts/install.sh --gui
```

This installs:
- `/usr/local/bin/cosmic-nightlight-helper`
- `/etc/polkit-1/rules.d/49-cosmic-nightlight.rules` — lets `wheel`/`sudo`
  members run the helper via `pkexec` without a password prompt
- with `--gui`: `/usr/local/bin/cosmic-nightlight` plus the applet and settings
  desktop entries

To remove everything again:

```sh
./scripts/uninstall.sh
```

For a proper system package (and getting it into the COSMIC Store as a
"System" app), build the `.deb` instead — see [PACKAGING.md](PACKAGING.md).
Flatpak is **not** an option for this tool: the sandbox cannot grant the
root / DRM-master / VT-switch capabilities the workaround needs.

## Usage

### Panel applet (the normal way)

`cosmic-nightlight` runs as a **COSMIC panel applet**. After installing the
desktop file, add it from **COSMIC Settings → Panel** (or **Dock**) **→ Applets**.
It puts an icon in the status bar; clicking it opens a popup with the on/off
toggle and the temperature slider, plus a **Settings…** button.

### Settings window

The settings window (autostart, schedule mode, sunset/sunrise hours,
temperature) opens from that button, from the **Night Light Settings** launcher
entry, or directly:

```sh
cosmic-nightlight --settings
```

Flipping the tint on or off against the schedule sets a manual override that
lasts until the next sunset/sunrise transition, after which automatic
scheduling resumes. Settings are stored with `cosmic_config` under
`~/.config/cosmic/io.github.cosmic_nightlight/` and shared live by the applet,
the settings window, and the daemon — a change in one shows up in the others
immediately.

### Background scheduler

Warm after sunset, neutral after sunrise. Toggling **Autostart** in the settings
window writes an XDG autostart entry that launches `cosmic-nightlight --daemon`
on login (and re-applies the saved tint). You can also run it under systemd:

```sh
mkdir -p ~/.config/systemd/user
cp systemd/cosmic-nightlight.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now cosmic-nightlight.service
```

### Command line (advanced)

The privileged helper can be driven directly. Each call flickers the screen
briefly:

```sh
pkexec /usr/local/bin/cosmic-nightlight-helper --temp 3500            # warm tint
pkexec /usr/local/bin/cosmic-nightlight-helper --temp 4000 --brightness 0.9
pkexec /usr/local/bin/cosmic-nightlight-helper --off                  # reset
```

## Known limitations

- **Flicker on every change** — inherent to the VT-bounce workaround.
- **A modeset can clear the tint** — resolution/monitor-hotplug/DPMS-wake events
  make the compositor reprogram the CRTC, dropping the LUT. Re-apply (the daemon
  re-applies on the next schedule boundary).
- The sunset/sunrise schedule is currently fixed hours (06:00/18:00); wiring it
  to your location (geoclio / an astronomical calc) is a clear next step — see
  the TODO in [`daemon.rs`](crates/cosmic-nightlight/src/daemon.rs).
- Requires `pkexec`/polkit and membership in `wheel` or `sudo`.

---

## How it works

COSMIC's `cosmic-comp` does **not** implement
`wlr-gamma-control-unstable-v1` ([cosmic-comp#764]), so `wlsunset`,
`gammastep`, and `redshift` cannot adjust the screen through Wayland. Native
Night Light is only planned for COSMIC **Epoch 3** ([cosmic-comp#2059],
[cosmic-epoch#2498]) and has not shipped.

So we go around Wayland and write the gamma ramp straight to the kernel's
DRM/KMS layer — the same thing `redshift` does on a bare TTY. There is **one
real obstacle**:

> While COSMIC is the foreground session it holds the **DRM master** lock, so
> any other process that calls `drmModeCrtcSetGamma` gets `EACCES`.

The workaround (proven by [jjo/drm-colortemp]): when the session switches to a
spare virtual terminal, `logind` revokes the compositor's DRM master. During
that window a root process can grab master, write the gamma LUTs, and — because
the compositor doesn't reset them — **the tint persists after switching back.**

This project automates that VT bounce so it happens on a schedule. The cost is
a brief (~1–2 s) screen flicker each time the tint changes. This is inherent to
the workaround; it goes away once COSMIC ships a real gamma protocol.

## Architecture

A Cargo workspace with three crates so the privileged, security-sensitive code
stays tiny and independent of the heavy GUI:

| Crate | Runs as | Responsibility |
| --- | --- | --- |
| [`nightlight-core`](crates/nightlight-core) | library | Gamma math ([`gamma.rs`](crates/nightlight-core/src/gamma.rs)), DRM apply ([`drm.rs`](crates/nightlight-core/src/drm.rs)), VT bounce ([`vt.rs`](crates/nightlight-core/src/vt.rs)) |
| [`nightlight-helper`](crates/nightlight-helper) | **root** (via `pkexec`) | Thin CLI: parse `--temp`/`--brightness`, call core |
| [`cosmic-nightlight`](crates/cosmic-nightlight) | your user | libcosmic panel applet + `--settings` window + `--daemon` scheduler; shells out to the helper |

Flow on a tint change:

```
daemon/GUI ──pkexec──▶ cosmic-nightlight-helper (root)
                          │ 1. VT_ACTIVATE a spare VT  (compositor drops DRM master)
                          │ 2. drmSetMaster + drmModeCrtcSetGamma on every active CRTC
                          │ 3. drmDropMaster
                          └ 4. VT_ACTIVATE back to your session  (tint persists)
```

The gamma curve is Tanner Helland's black-body white-point fit: 6500 K is an
identity ramp (no tint); lower temperatures cut green/blue to warm the image —
far finer than the 3 coarse presets a DDC/CI approach can offer, and it works
on laptop internal panels (which usually have no DDC/CI).

## The real fix

This whole approach is a stopgap. The proper solution is COSMIC implementing a
gamma-control protocol; track [cosmic-comp#764] and [cosmic-comp#2059]. Once
that lands, the DRM/VT machinery here can be replaced with a normal Wayland
client.

[cosmic-comp#764]: https://github.com/pop-os/cosmic-comp/issues/764
[cosmic-comp#2059]: https://github.com/pop-os/cosmic-comp/issues/2059
[cosmic-epoch#2498]: https://github.com/pop-os/cosmic-epoch/issues/2498
[jjo/drm-colortemp]: https://github.com/jjo/drm-colortemp
