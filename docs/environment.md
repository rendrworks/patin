# Environment and Toolchain

Patin is ordinary Linux userspace software for compatible Wayland
environments. Current verification covers x86_64 Arch Linux and aarch64
postmarketOS, but no distribution, architecture, device, or compositor defines
the core design.

## Rust

The repository pins the development toolchain to Rust 1.97.1 with the minimal
rustup profile plus `rustfmt` and Clippy. `Cargo.toml` declares 1.97 as the
minimum supported Rust release, which includes Alpine Rust 1.97.0.
A rustup-based checkout selects the exact development pin through
`rust-toolchain.toml`.

```sh
rustc --version
cargo --version
cargo build
cargo run --example demo_bar
```

The first surface uses `smithay-client-toolkit` 0.21.1 with default features
disabled and its `calloop` and `xkbcommon` features enabled. SCTK supplies the Wayland
client bindings, layer-shell protocol bindings, shared-memory slot pool,
surface/output/seat tracking, and Calloop event source. Patin binds pointer and
touch capabilities through SCTK. The keyboard support is used by the optional
`patin-lock` consumer; the bar still deliberately requests no keyboard
interactivity.

The rendering stage adds:

- `tiny-skia` 0.12.0 for CPU raster primitives;
- `cosmic-text` 0.19.0 with fontconfig and Swash for system-font discovery,
  shaping, layout, fallback, and glyph rasterization;
- Chrono 0.4.45 for local wall-clock time.

Runtime requires a Wayland compositor that advertises `wl_compositor`, `wl_shm`,
and `wlr-layer-shell-unstable-v1`. The client uses the pure Rust Wayland backend,
so this stage does not require linking against the system `libwayland-client`.
`wp_fractional_scale_manager_v1` and `wp_viewporter` are optional; compositors
without them use the integer `wl_output` scale path.

The UI core adds no external dependency. Its logical geometry, layout,
styling, scene commands, hit-testing, and damage tracking are internal Rust
modules built around demonstrated shell components.

The optional `patin-launcher` composition uses
`freedesktop-desktop-entry` 0.8.1 with default features disabled. Desktop-entry
localization, visibility fields, XDG search paths, and `Exec` field codes are a
standard with enough edge cases that a narrow parser is safer and cheaper than
reimplementing them. The dependency belongs to the launcher crate only; the
Patin toolkit and other consumers do not inherit it.

The launcher additionally uses `image` 0.25.10 with only its PNG feature and
`resvg` 0.47.0 with all default features disabled. They decode PNG and render
SVG application icons found through standard XDG data and icon locations,
without enabling resvg's text, system-font, or raster-image features. Entries
without a usable icon receive a neutral fallback.

The toolkit does not require a battery, backlight, or audio command. The demo
optionally uses `/sys/class/power_supply`, `/sys/class/backlight`, `wpctl`, and
`pactl`; missing providers merely remove those demo labels.

```sh
echo "$WAYLAND_DISPLAY"
cargo run --example demo_bar
```

Patin reports a clear error and exits unsuccessfully when no compositor can be
found or a required global is unavailable.

## Lock-screen requirements

Building `patin-lock` requires the system PAM and xkbcommon development
packages in addition to the Rust toolchain (`linux-pam-dev` and
`libxkbcommon-dev` on postmarketOS/Alpine; package names vary elsewhere).
xkbcommon is the keyboard-state library used by SCTK's keyboard support.
Runtime requires a compositor that advertises `ext-session-lock-v1` and a
matching `/etc/pam.d/patin-lock` policy.

```sh
./scripts/install-lock-user.sh
sudo install -m 0644 data/pam/patin-lock.alpine /etc/pam.d/patin-lock
patin-lock
```

Use the `.arch` or `.debian` example instead on those PAM stacks. PAM policy is
system security configuration and is therefore never installed implicitly by
the user installer. The program checks that the policy exists before requesting
the Wayland lock, avoiding a lock screen with no configured authentication
route.

## Remote Wayland session testing

An SSH login normally does not inherit the graphical session environment.
Discover the target user's runtime directory and active Wayland socket, then
set them explicitly:

```sh
cd ~/Projects/patin
cargo build --release --locked --example demo_bar

env -u LD_LIBRARY_PATH \
  XDG_RUNTIME_DIR="/run/user/$(id -u)" \
  WAYLAND_DISPLAY=wayland-0 \
  target/release/examples/demo_bar
```

Unsetting `LD_LIBRARY_PATH` prevents a shell client from loading 0xin's private
wlroots/sysroot libraries. Keep a separate recovery connection available while
testing a standalone compositor:

```sh
ssh <host> 'pkill -TERM -x 0xin'
```

The current FP5 test checkout can be launched from its own terminal with:

```sh
env -u LD_LIBRARY_PATH \
  XDG_RUNTIME_DIR="/run/user/$(id -u)" \
  WAYLAND_DISPLAY=wayland-0 \
  /tmp/patin-fp5-test/target/release/examples/demo_bar
```

Because the checkout is under `/tmp`, rebuild or install the binary to a
persistent user path after a reboot.

## Installing the demo command

The repository includes an explicit user installer for the example:

```sh
./scripts/install-demo-user.sh
patin
```

It builds `demo_bar` in release mode and installs only that example executable
as `~/.local/bin/patin`. The library still defines no default shell binary.
Ensure the login profile contains this conventional user binary directory:

```sh
PATH="$PATH:$HOME/.local/bin"
```

Frame submission and raw touch logs are disabled during normal operation. To
debug rendering, scaling, damage, or contact delivery:

```sh
PATIN_TRACE=1 patin
```

## Documentation

The book is built with mdBook 0.5.3 in CI and Pages automation.

```sh
cargo install mdbook --version 0.5.3 --locked
mdbook build
```

Generated Cargo output (`target/`) and book output (`book/`) are ignored.

## Required checks

Every milestone runs:

```sh
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
mdbook build
git diff --check
```

Commands which require a compositor or hardware will be added to the
corresponding stage rather than pretending they can be verified in foundation
CI.
