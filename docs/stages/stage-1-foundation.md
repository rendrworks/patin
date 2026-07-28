# Stage 1 — Foundation

## Concept

A reproducible foundation makes later graphics work easier to understand. The
toolchain, checks, documentation, and project rules are fixed before Wayland
introduces protocol state and platform dependencies.

This stage deliberately does not open a display or create a surface. The
`patin` binary is a valid, dependency-free Rust program that exits successfully
without output. Stage 2 will replace that temporary behavior with the first
native layer-shell surface.

## What changed

- `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, and `src/main.rs` define
  the private Rust 2024 binary and pinned toolchain.
- `AGENTS.md` makes documentation and real verification part of every code
  change.
- `book.toml`, `docs/`, and `docs/SUMMARY.md` establish the architecture,
  environment, roadmap, and stage record.
- GitHub Actions check the project and publish this book.
- `README.md`, `.gitignore`, and `LICENSE` establish the public project entry
  point and repository policy.

`main` is intentionally empty. Adding fake runtime structure now would create
interfaces without a real Wayland use case, contrary to the project's
internal-primitives-first rule.

## Verification

The following checks must pass before this stage is complete:

```text
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
mdbook build
git diff --check
```

## Actual result

Verified on 28 July 2026 with Rust 1.97.1 and mdBook 0.5.3:

- `cargo fmt --all -- --check` completed without formatting differences.
- `cargo test --all-targets` compiled Patin and passed; there are intentionally
  no behavior tests before the first behavior exists.
- `cargo clippy --all-targets --all-features -- -D warnings` completed without
  warnings.
- `mdbook build` generated the HTML book successfully.
- `git diff --check` reported no whitespace errors.
