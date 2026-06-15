<!-- SPDX-License-Identifier: MPL-2.0 -->
# Night Light for COSMIC

**Night Light** is an easy-to-use applet for the **COSMIC** desktop (Pop!_OS)
that warms your screen's color temperature to cut blue light. It lives as an
icon on your **panel or dock**: click it for a simple popup with an on/off
toggle and a temperature slider, and open **Settings** for a schedule
(sunset → sunrise), the night temperature, and start-on-login.

It exists because COSMIC's compositor does not yet expose a color/gamma
protocol, so the usual tools (`redshift`, `gammastep`, `wlsunset`) can't adjust
the screen — see [How it works](#how-it-works).

## Install

The easy way — install the `.deb` from the COSMIC Store:

1. Download the latest **`cosmic-nightlight_*.deb`** from the
   [**Releases**](https://github.com/danielcwtts/cosmic-nightlight/releases)
   page.
2. Open the downloaded file with the **COSMIC Store** and click **Install**.
   (Or from a terminal: `sudo apt install ./cosmic-nightlight_*.deb`.)
3. Add the applet to your bar: **COSMIC Settings → Desktop → Panel** (or
   **Dock**) **→ Configure applets → Add applet → Night Light**.

That's it — click the Night Light icon to toggle the tint or open its settings.

> Flatpak is **not** an option for this tool: the sandbox cannot grant the
> root / DRM-master / VT-switch capabilities the workaround needs. That is also
> why the `.deb` installs a small `pkexec` helper and a polkit rule (so the
> tint can be applied without a password prompt for `wheel`/`sudo` members).

### Build from source (for development)

The `scripts/install.sh` / `scripts/uninstall.sh` helpers build and install
locally from a checkout — handy for hacking on the app, and usable as an
alternative to the `.deb`. They need a Rust toolchain and `libdrm` headers
(`libdrm-dev`):

```sh
./scripts/install.sh --gui     # build + install the helper, polkit rule, and GUI
./scripts/uninstall.sh         # remove everything install.sh added
```

To build the `.deb` yourself, see [PACKAGING.md](PACKAGING.md).

## Using it

**The applet.** The Night Light icon opens a popup with the on/off toggle, the
temperature slider, and a **Night Light Settings…** button.

**Settings.** The settings window covers start-on-login, schedule mode,
sunrise/sunset hours, and the night temperature. Open it from the popup, from
the **Night Light Settings** launcher entry, or with `cosmic-nightlight --settings`.

Toggling the tint against the schedule sets a manual override that lasts until
the next sunset/sunrise transition, after which automatic scheduling resumes.
Settings live in `~/.config/cosmic/io.github.cosmic_nightlight/` and sync live
across the applet, the settings window, and the background scheduler.

**Start on login.** Enabling it in Settings runs the background scheduler at
login (warm after sunset, neutral after sunrise) and re-applies your saved tint.

<details>
<summary>Advanced: drive the helper directly</summary>

The privileged helper can be called by hand. Each call briefly flickers the
screen:

```sh
pkexec /usr/bin/cosmic-nightlight-helper --temp 3500            # warm tint
pkexec /usr/bin/cosmic-nightlight-helper --temp 4000 --brightness 0.9
pkexec /usr/bin/cosmic-nightlight-helper --off                 # reset
```

(Use `/usr/local/bin/...` if you installed via `scripts/install.sh`.)
</details>

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
