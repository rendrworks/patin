# Patin

Patin is a native Rust graphical shell for Wayland. It will draw its own bars,
overlays, launcher, lock screen, and optional phone interface as an independent
layer-shell client.

> **Status:** Milestone 1 establishes the project and documentation. Patin does
> not create a Wayland surface yet.

Patin is the visible surface layer intended to sit above
[0xin](https://github.com/termworks/0xin), while remaining usable with other
compositors that implement layer-shell. It is an opinionated shell first:
reusable UI and rendering primitives will be developed internally as real
features need them, rather than exposed as a general-purpose toolkit in v1.

## Direction

- Rust owns shell behavior, layout, rendering abstractions, and services.
- `smithay-client-toolkit` will provide the Wayland client foundation.
- CPU rendering with `wl_shm`, `tiny-skia`, and `cosmic-text` comes first.
- `calloop` will drive events and `zbus` will connect standard system services.
- Desktop is the default profile; phone UI is opt-in and is never constructed
  in desktop mode.
- 0xin integration will use a replaceable IPC adapter. Patin must still start
  when that socket is unavailable.
- Qt, QML, GTK, Electron, and other large GUI frameworks are out of scope.

These dependencies and modules are architectural decisions, not current Cargo
dependencies. Each will be added only when its first demonstrable stage needs
it.

## Build and verify

Patin uses ordinary stable Rust and pins the exact toolchain in
`rust-toolchain.toml`.

```sh
cargo build
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
mdbook build
```

The current binary intentionally exits without output. The first observable
runtime behavior arrives in Milestone 2 as a solid-color layer-shell bar.

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

