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

The foundation has no third-party Rust or system dependencies. Later stages
will document and add each dependency when it is first exercised.

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

