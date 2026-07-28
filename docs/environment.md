# Environment and Toolchain

Patin is ordinary Linux userspace software. Development begins on Arch Linux
and x86_64, with later on-device verification on the aarch64 Fairphone 5.

## Rust

The repository pins Rust 1.97.1 with the minimal rustup profile plus `rustfmt`
and Clippy. A fresh checkout selects it through `rust-toolchain.toml`.

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
