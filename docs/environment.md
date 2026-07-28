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
```

The first surface uses `smithay-client-toolkit` 0.21.1 with default features
disabled and only its `calloop` feature enabled. SCTK supplies the Wayland
client bindings, layer-shell protocol bindings, shared-memory slot pool,
surface/output tracking, and Calloop event source. Keyboard support remains
disabled until the input stage.

Runtime requires a Wayland compositor that advertises `wl_compositor`, `wl_shm`,
and `wlr-layer-shell-unstable-v1`. The client uses the pure Rust Wayland backend,
so this stage does not require linking against the system `libwayland-client`.

```sh
echo "$WAYLAND_DISPLAY"
cargo run
```

Patin reports a clear error and exits unsuccessfully when no compositor can be
found or a required global is unavailable.

## Remote Wayland session testing

An SSH login normally does not inherit the graphical session environment.
Discover the target user's runtime directory and active Wayland socket, then
set them explicitly:

```sh
cd ~/Projects/patin
cargo build --release --locked

env -u LD_LIBRARY_PATH \
  XDG_RUNTIME_DIR="/run/user/$(id -u)" \
  WAYLAND_DISPLAY=wayland-0 \
  target/release/patin
```

Unsetting `LD_LIBRARY_PATH` prevents a shell client from loading 0xin's private
wlroots/sysroot libraries. Keep a separate recovery connection available while
testing a standalone compositor:

```sh
ssh <host> 'pkill -TERM -x 0xin'
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
