# Stage 5b — Toolkit and Example Boundary

## Why this refactor was necessary

The first visible stages placed the bar composition and its status providers in
the main crate path. That made useful demonstrations, but it incorrectly made
one shell implementation look like Patin itself.

Patin is now a library. Visible compositions are examples or downstream
projects that consume it.

## Library

`src/lib.rs` exports three focused modules:

- `platform` owns Wayland connection, layer surfaces, seats, pointer/touch
  routing, scaling, shared-memory buffers, frame callbacks, and physical damage;
- `render` executes renderer-neutral fill and text commands on the CPU backend;
- `ui` supplies logical geometry, row/column/stack layout, styling data types,
  draw commands, and hit-testing operations.

Consumers implement `platform::Shell`. The runtime asks that implementation to
resize, update, handle an activation position, produce commands, return damage,
and invalidate everything after scale changes.

Consumers also provide `LayerConfig`. Patin does not assume a top bar: layer
level, anchors, size, namespace, exclusive zone, and keyboard policy are all
explicit.

## Demo

`examples/demo_bar.rs` is an executable test consumer. Its supporting files
under `examples/demo_bar/` own:

- clock and toggle state;
- bar style and row composition;
- battery and brightness sysfs polling;
- `wpctl` and `pactl` volume polling.

None of these are exported by the library or instantiated by
`platform::run`.

Run the fixture with:

```text
cargo run --example demo_bar
```

For devices where the demo is used repeatedly, the explicit installer creates
a short user command without changing the crate boundary:

```text
./scripts/install-demo-user.sh
patin
```

The installed `patin` executable is a copy of the demo example under
`~/.local/bin`; it is not an automatically built toolkit binary.

Per-frame and raw-touch platform diagnostics are opt-in with `PATIN_TRACE=1`.
Normal demo runs retain startup/provider and error messages without continuously
printing frame submissions.

The installer was run on the FP5 and a fresh login shell resolved and launched
the short command:

```text
$ command -v patin
/home/sn3rt/.local/bin/patin
$ patin
demo_bar: status providers: battery=BAT 100%+, volume=VOL 3%, brightness=BRI 86%
patin: rendered 1222x77 buffer for 509x32 logical bar (1 damaged region)
```

## Verification

Verified on 29 July 2026:

```text
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo run --example demo_bar
mdbook build
git diff --check
```

Five toolkit tests and four demo tests passed. The local example rendered at
`3440x32`. The unchanged example was then built natively on the FP5 with:

```text
cargo build --release --locked --example demo_bar
```

After the interrupted laptop session was recovered, it reported `BAT 98%+`,
`VOL 3%`, and `BRI 54%`, then rendered `509x32` logical at `1222x77` physical.
Stopping the demo left the compositor and Wayland socket alive.
