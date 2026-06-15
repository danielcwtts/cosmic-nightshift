<!-- SPDX-License-Identifier: MPL-2.0 -->
# Packaging cosmic-nightshift as a `.deb`

This tool needs root (DRM master + VT switching), so its natural distribution
channel is a **native `.deb`**, not a Flatpak — a Flatpak sandbox cannot get
the capabilities the gamma workaround requires. A `.deb` shows up in the COSMIC
Store as the "System" version of the app once it's in a repo (a PPA or the
Pop!_OS repos).

The `debian/` directory here produces a single binary package,
`cosmic-nightshift`, that installs:

| Path | What |
| --- | --- |
| `/usr/bin/cosmic-nightshift-helper` | privileged DRM/VT helper (run via pkexec) |
| `/usr/bin/cosmic-nightshift` | libcosmic GUI + `--daemon` scheduler |
| `/usr/share/polkit-1/rules.d/49-cosmic-nightshift.rules` | passwordless pkexec for `wheel`/`sudo` |
| `/usr/share/applications/io.github.cosmic_nightshift.desktop` | launcher entry |
| `/usr/lib/systemd/user/cosmic-nightshift.service` | per-user scheduler unit |

## Build dependencies

```sh
sudo apt install build-essential debhelper cargo rustc pkg-config \
    libdrm-dev libxkbcommon-dev libwayland-dev libfontconfig-dev libexpat1-dev
```

The GUI links libcosmic/wgpu; if the build fails on a missing `-dev` library,
install it and add it to `Build-Depends` in [`debian/control`](debian/control).

## Quick local build (network available)

libcosmic is a **git dependency**, so cargo must fetch it. On your own machine
(with network) this just works:

```sh
dpkg-buildpackage -b -us -uc
# -> ../cosmic-nightshift_0.1.0-1_amd64.deb
sudo apt install ../cosmic-nightshift_0.1.0-1_amd64.deb
```

## Clean-room / offline build (PPA, sbuild, Launchpad)

Official build environments have **no network**, and cargo cannot fetch the
libcosmic git dependency there. Vendor the dependencies once and commit them:

```sh
mkdir -p .cargo
cargo vendor --locked vendor > .cargo/config.toml.fragment
# Merge the printed [source.*] stanzas into .cargo/config.toml, e.g.:
cat .cargo/config.toml.fragment >> .cargo/config.toml
git add vendor .cargo/config.toml Cargo.lock
```

With `vendor/` committed and `.cargo/config.toml` redirecting crates to it, the
`--locked` build in [`debian/rules`](debian/rules) runs fully offline.

> Note: `vendor/` is large. For a real upstream you'd typically keep it out of
> the main branch and generate it in the packaging branch / orig tarball
> instead.

## Getting it into the COSMIC Store

The Store reads the system's apt repos (Ubuntu, Pop!_OS, and any you add) plus
Flathub. To make this installable there as a System package:

1. Host the `.deb` in an **apt repository** — a Launchpad PPA is the easiest
   (`debuild -S` then `dput ppa:you/cosmic-nightshift`), or self-host with
   `reprepro`.
2. Users add the PPA (`add-apt-repository`); the package then appears in the
   COSMIC Store and `apt`.
3. For inclusion in the first-party Pop!_OS repos, that's a System76 decision —
   open it upstream once the PPA is proven.

A Flatpak/Flathub submission only becomes appropriate **after** COSMIC ships its
native gamma protocol, at which point the DRM/VT helper is replaced by a plain
Wayland client that needs no special privileges.
