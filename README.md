# Patin

Patin is a native Rust toolkit for building Wayland graphical shells. It
provides the platform, rendering, layout, input, and damage foundations from
which a consumer can compose bars, overlays, launchers, lock screens, and
other shell surfaces.

> **Status:** Patin is a library. The visible demo bar is an example/test
> consumer and is not instantiated by the toolkit. `patin-lock` is a separate,
> explicitly launched lock-screen composition.

Patin clients can run above [0xin](https://github.com/termworks/0xin) or another
compatible layer-shell compositor. Patin is focused on graphical-shell needs;
it is not intended to become a general-purpose application GUI framework.

## Direction

- Consumers own shell behavior, composition, components, and service choices.
- Patin owns reusable Wayland, layout, rendering, input, scale, and damage
  mechanisms.
- `smithay-client-toolkit` will provide the Wayland client foundation.
- CPU rendering with `wl_shm`, `tiny-skia`, and `cosmic-text` comes first.
- `calloop` drives events. Optional adapter crates use `zbus` to connect
  standard system services; the core `patin` crate never depends on `zbus`.
- The library never automatically constructs a bar, phone UI, battery reader,
  volume reader, or compositor-specific adapter.
- 0xin integration will use a replaceable IPC adapter. Patin must still start
  when that socket is unavailable.
- Qt, QML, GTK, Electron, and other large GUI frameworks are out of scope.

The toolkit uses `smithay-client-toolkit` 0.21.1 with Calloop, `tiny-skia`
0.12.0, and `cosmic-text` 0.19.0. Chrono is used by the demo only.

## Workspace

Patin is a Cargo workspace. The root package is the `patin` toolkit crate
itself; `crates/` holds optional, opt-in service-adapter crates that
implement `patin::service::Provider` against a specific system service:

- `patin-service-upower` — battery state over D-Bus/UPower.
- `patin-service-volume` — audio volume/mute via `wpctl`/`pactl`.
- `patin-service-brightness` — display backlight via `/sys/class/backlight`.
- `patin-service-network` — connectivity state over D-Bus/NetworkManager.
- `patin-launcher` — an independently launched, touch-friendly application list.
- `patin-lock` — an `ext-session-lock-v1` client with physical and touch
  keyboards and PAM authentication.
- `patin-session` — a compact, compositor-neutral session action menu.

A consumer depends on `patin` alone, or additionally on whichever adapter
crates it wants; none are pulled in automatically.

## Build and verify

Patin uses ordinary stable Rust and pins the exact toolchain in
`rust-toolchain.toml`.

```sh
cargo build --workspace
cargo run --example demo_bar
cargo fmt --all -- --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
mdbook build
```

The example connects to the compositor selected by `WAYLAND_DISPLAY`, creates
a top layer-shell bar, and demonstrates layout, rendering, scaling, and
damage. Its clock and its battery, volume, brightness, and network status
providers are fixtures for proving toolkit behavior, not built-in Patin
components. Status values are rendered as small dependency-free vector icons;
only the time remains textual. The clock occupies the inset left edge, the
battery occupies the inset right edge, and the other available status fixtures
share the space between them. The bar has no interactive element of its own
right now — pointer and touch input still reach `Shell::activate_at`, they just
have nothing to act on.

Library consumers implement `patin::platform::Shell`, choose a `LayerConfig`,
and pass both to `patin::platform::run`.

### Install and run the application launcher

The launcher is an optional composition, not toolkit startup behavior. It
discovers visible freedesktop desktop entries, displays them in a compact
floating list, and closes after successfully spawning the tapped application:

```sh
./scripts/install-launcher-user.sh
patin-launcher
```

The launcher uses a full-output transparent input surface but draws only a
centered `280×350` panel. The surrounding output remains visually unchanged;
tapping it dismisses the launcher without activating the application beneath.
The panel contains a simple vertical list with an XDG application icon and name
per row. Drag vertically on touch or use a pointer wheel to
scroll through ten compact lines; tap a row to launch it. Its deep-purple
palette matches the demo bar and lock screen rather than copying Fuzzel's
colors. On 0xin, the existing configurable shell
gestures can replace Fuzzel without adding a compositor-specific code path to
Patin:

```ini
gesture = top-down, spawn, pgrep -x patin-launcher >/dev/null || patin-launcher
gesture = to-top, spawn, pkill -x patin-launcher
```

Other compositors can start the same binary from their own key or gesture
configuration.

### Install and run the session menu

The optional session composition displays configured logout, reboot, and
power-off actions in a compact floating panel. Its full-output surface is
transparent outside the panel, and a tap there dismisses the menu without a
Cancel row:

```sh
./scripts/install-session-user.sh
PATIN_SESSION_LOGOUT_PROGRAM="$HOME/.local/bin/0xinctl" \
PATIN_SESSION_LOGOUT_ARGUMENT=quit \
PATIN_SESSION_LOGOUT_LABEL="Log out to Phrog" \
patin-session
```

Reboot and power-off use `systemctl` directly. Logout is optional and supplied
by the shell integration through environment variables, so the Patin binary
does not depend on 0xin. The phone's existing `0xin-session-menu` wrapper can
export those three variables and `exec patin-session`, leaving its 2-second
power-button mapping unchanged.

### Install and run the lock screen

The lock is security-sensitive, so its binary and PAM policy are separate,
explicit installation steps. Install the distribution-matching example policy;
do not copy one for a different PAM stack.

```sh
./scripts/install-lock-user.sh

# postmarketOS / Alpine
sudo install -m 0644 data/pam/patin-lock.alpine /etc/pam.d/patin-lock

# Then, from the graphical session:
patin-lock
```

Arch and Debian examples are available as `patin-lock.arch` and
`patin-lock.debian`. The client refuses to acquire the lock when its PAM policy
is missing. It discovers outputs and seats at runtime, covers every output, and
accepts a physical keyboard, pointer, or its built-in touch keyboard.
Successful PAM authentication is the only normal unlock path.

By default the touch keyboard is the full QWERTY/symbol layout. Pass
`--keypad=numeric` for a 3x4 digit PIN pad instead — useful when the account's
real password is itself numeric, since either mode just types into the same
password PAM checks:

```sh
patin-lock --keypad=numeric
```

Both keyboards use compact key groups with an adaptive lower-screen inset
rather than stretching to fill an output or sitting against its bottom edge.
The numeric keypad stays centered, while the full keyboard is width-limited on
larger outputs. The empty password field contains its `Enter password` hint;
typing replaces it with bullets, and authentication progress or errors remain
visible below the field.

Set `PATIN_LOCK_KEYPAD=numeric` (matching the existing `PATIN_TRACE`
convention) to make that the default without passing the flag every time —
export it from a shell profile for manual launches, or set it directly in
whatever spawns `patin-lock` (a compositor keybind, a session unit) so it
applies there too. An explicit `--keypad=` argument always overrides it.

If the compositor supports `zwlr_output_power_manager_v1`, the lock screen
also powers off the display after 1 second of no key/touch/pointer activity —
or 5 seconds once you've started typing a password, so pauses between digits
don't blank the screen mid-entry. Ordinary touch/pointer/keyboard input never
wakes it — only the power button
(`XF86PowerOff`) does, toggling the display off if it's on or back on if it's
off. This is deliberate: a phone in a pocket brushes its screen constantly,
and waking on any of that would defeat the point of blanking it.

No compositor keybind is needed for the power button: while a session is
locked, a spec-compliant compositor forwards physical keys straight to the
lock client rather than intercepting them for its own keybinds (0xin does
this explicitly to keep its keybinds from bypassing the lock), so
`patin-lock` sees `XF86PowerOff` as an ordinary keyboard event and handles it
itself. `SIGUSR1` sent to the `--worker` process does the same toggle and
remains useful for scripted testing (e.g. over SSH), but isn't required for
the physical button to work.

### Run on the FP5

For a persistent one-word user command, run this from a Patin checkout on the
FP5:

```sh
./scripts/install-demo-user.sh
patin
```

This explicitly installs the `demo_bar` example as `~/.local/bin/patin`. It
does not add a default binary to the toolkit crate. The FP5 login profile
already includes `~/.local/bin` in `PATH`; open a new terminal after the first
installation if the current shell has not loaded that profile.

Normal runs print only startup/provider information and errors. Enable
per-frame damage and raw touch diagnostics when needed:

```sh
PATIN_TRACE=1 patin
```

The current temporary native demo build can also be launched directly:

```sh
env -u LD_LIBRARY_PATH \
  XDG_RUNTIME_DIR="/run/user/$(id -u)" \
  WAYLAND_DISPLAY=wayland-0 \
  /tmp/patin-fp5-test/target/release/examples/demo_bar
```

The `/tmp` checkout is temporary and may disappear after reboot. From the
laptop, the same command can be invoked with:

```sh
ssh -t fp5 'env -u LD_LIBRARY_PATH \
  XDG_RUNTIME_DIR=/run/user/$(id -u) \
  WAYLAND_DISPLAY=wayland-0 \
  /tmp/patin-fp5-test/target/release/examples/demo_bar'
```

### Portability

Patin is built from modular shell capabilities for any compatible Wayland
environment. Outputs, scale, transforms, input capabilities, and optional
protocols are discovered at runtime. Different compositions select from shared
modules; neither hardware models nor compositor brands define the core
architecture.

## Documentation

The project book lives under [`docs/`](docs/introduction.md). Preview it with:

```sh
mdbook serve
```

## Roadmap

1. Foundation: pinned Rust project, license, checks, mdBook, and CI.
2. First surface: a solid-color top layer-shell surface with an exclusive zone.
3. Rendering: drawing primitives and a correctly scaled text clock.
4. Input: pointer and multitouch interaction without stealing application focus.
5. UI core: internal layout, styling, hit-testing, damage, and components.
6. Services: battery, network, audio, notifications, and media state.
7. Session lock: standalone multi-output lock composition with touch input and
   PAM authentication.
8. Mobile profile: phone navigation, launcher, quick settings, and keyboard.
9. Compositor integration: workspace state and commands through a replaceable
   0xin control-socket adapter.

Each completed stage records its real verification commands and results in the
book. Patin does not create commits on the user's behalf.

## License

Patin is licensed under the [MIT License](LICENSE).
