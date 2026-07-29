# Patin

Patin is a native Rust toolkit for building Wayland graphical shells. It
provides the platform, rendering, layout, input, and damage foundations from
which a consumer can compose bars, overlays, launchers, lock screens, and
other shell surfaces.

> **Status:** Patin is a library. The visible demo bar is an example/test
> consumer and is not instantiated by the toolkit.

Patin clients can run above [0xin](https://github.com/termworks/0xin) or another
compatible layer-shell compositor. Patin is focused on graphical-shell needs;
it is not intended to become a general-purpose application GUI framework.

## Direction

- Consumers own shell behavior, composition, components, and service choices.
- Patin owns reusable Wayland, layout, rendering, input, scale, and damage
  mechanisms.
- `smithay-client-toolkit` will provide the Wayland client foundation.
- CPU rendering with `wl_shm`, `tiny-skia`, and `cosmic-text` comes first.
- `calloop` will drive events and `zbus` will connect standard system services.
- The library never automatically constructs a bar, phone UI, battery reader,
  volume reader, or compositor-specific adapter.
- 0xin integration will use a replaceable IPC adapter. Patin must still start
  when that socket is unavailable.
- Qt, QML, GTK, Electron, and other large GUI frameworks are out of scope.

The toolkit uses `smithay-client-toolkit` 0.21.1 with Calloop, `tiny-skia`
0.12.0, and `cosmic-text` 0.19.0. Chrono and the provisional battery, audio,
and brightness providers are used by the demo only.

## Build and verify

Patin uses ordinary stable Rust and pins the exact toolchain in
`rust-toolchain.toml`.

```sh
cargo build
cargo run --example demo_bar
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
mdbook build
```

The example connects to the compositor selected by `WAYLAND_DISPLAY`, creates
a top layer-shell bar, and demonstrates layout, rendering, input, scaling, and
damage. Its clock, toggle, battery, and volume are fixtures for proving toolkit
behavior, not built-in Patin components.

Library consumers implement `patin::platform::Shell`, choose a `LayerConfig`,
and pass both to `patin::platform::run`.

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
7. Mobile profile: phone navigation, launcher, quick settings, and keyboard.
8. Compositor integration: workspace state and commands through a replaceable
   0xin control-socket adapter.

Each completed stage records its real verification commands and results in the
book. Patin does not create commits on the user's behalf.

## License

Patin is licensed under the [MIT License](LICENSE).
