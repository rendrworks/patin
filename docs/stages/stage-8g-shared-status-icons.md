# Stage 8g — Shared Status Icons and Explicit Audio Off State

## Why this stage exists

The original bar volume glyph used zero active level bars for 0%. That looked
like a weak or incomplete meter rather than clearly communicating that audio
was fully off. Battery, volume, wired, and cellular drawing also still belonged
to the demo bar after Wi-Fi had moved to the shared icon crate. A composition
should arrange icons, not own their drawing implementations.

## State and rendering

`patin-icons::VolumeLevel` represents off, low, medium, and high independently
of the service provider. `VolumeLevel::from_percentage` selects off whenever
the reported percentage is zero or the sink is muted. The off glyph retains
the speaker body and places a small foreground-colored vector cross beside it;
low volume retains one active level bar, so the two states cannot be confused.

The demo bar converts its existing `VolumeSnapshot` into this semantic state
and supplies the same palette used for shared Wi-Fi icons. Battery, cellular,
and wired glyphs now use the same crate and palette. Service adapters continue
to own only status data, while the reusable icon crate owns every status-glyph
representation. The bar retains only layout, hit targets, status-to-icon state
mapping, and its palette.

## Changed files and important functions

- `crates/patin-icons/src/lib.rs`: `VolumeLevel::from_percentage` maps service
  values; `volume` draws the speaker, level bars, and off cross; `cross` is the
  shared vector helper also used by unavailable Wi-Fi. `battery`,
  `cellular_signal`, and `wired` contain the remaining status drawings.
- `examples/demo_bar/scene.rs`: `DemoBar::commands` consumes every shared
  status icon; all former bar-local icon and shape helpers are removed.
- README, architecture, Stage 6d, and `docs/SUMMARY.md` record the shared
  ownership and off-state behavior.

## Verification

Local verification on 2026-08-10:

```text
cargo fmt --all -- --check
  no output
cargo check --workspace --all-targets
  finished successfully
cargo test --workspace --all-targets
  56 passed; 0 failed
cargo clippy --workspace --all-targets --all-features -- -D warnings
  finished successfully
mdbook build
  HTML book written to book/
git diff --check
  no output
```

The FP5 native release build compiled both consumers with the phone's existing
project-local xkbcommon metadata:

```text
PKG_CONFIG_PATH=~/proj/0xin/.sysroot/usr/lib/pkgconfig \
  cargo build --release --locked --example demo_bar
  finished successfully
PKG_CONFIG_PATH=~/proj/0xin/.sysroot/usr/lib/pkgconfig \
  cargo build --release --locked -p patin-network-settings
  finished successfully
```

The demo bar was installed as `~/.local/bin/patin` and network settings as
`~/.local/bin/patin-network-settings`. Both installed SHA-256 values matched
their native release artifacts. The running bar was not killed remotely; it
will load the shared status-icon build on its next normal restart.
