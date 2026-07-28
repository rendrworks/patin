# Stage 2 — First Surface

## Concept

A Wayland surface is only pixel storage until a shell protocol gives it a role.
Patin uses `wlr-layer-shell-unstable-v1`, which lets desktop components select a
z-order layer, anchor themselves to output edges, and reserve space that normal
application windows should not occupy.

Patin creates one surface on the top layer, anchors it to the top, left, and
right edges, requests a logical height of 32 pixels, and sets a matching
exclusive zone. Keyboard interactivity is explicitly disabled, so the bar
cannot take keyboard focus from applications.

The initial empty commit asks the compositor to configure the surface. Patin
does not guess the output width: it waits for that configure event, allocates an
ARGB8888 `wl_shm` buffer of the returned size, fills every pixel purple, damages
the full buffer, attaches it, and commits the visible frame.

## What changed

- `Cargo.toml` pins `smithay-client-toolkit` 0.21.1 with only Calloop support.
  SCTK provides the protocol bindings, registry handling, surface/output state,
  and safe shared-memory slot pool.
- `src/main.rs` connects to Wayland, binds required globals, configures the
  layer surface, dispatches events with Calloop, and submits the first buffer.
- `src/render.rs` contains the compositor-independent solid ARGB fill and its
  unit tests.
- The README and architecture/environment chapters now describe actual first
  surface behavior and runtime requirements.

The first surface targets the compositor-selected default output. Per-output
bars, scale-aware buffer sizing, output hotplug behavior, and buffer reuse are
deliberately deferred to the stages that can demonstrate them properly.

## Important functions

- `run` owns startup: connection, registry enumeration, global binding, layer
  configuration, state construction, and the Calloop dispatch loop.
- `Patin::configure` accepts the compositor's first size and initiates drawing.
- `Patin::draw` creates and attaches the shared-memory buffer.
- `fill_solid_argb` writes one little-endian ARGB value into every pixel and is
  independent of Wayland.

## Verification

Verified on 28 July 2026 with Rust 1.97.1, SCTK 0.21.1, and Hyprland:

```text
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo run
hyprctl layers
mdbook build
git diff --check
```

Two renderer tests passed. The live run reported:

```text
patin: connected; waiting for the compositor to configure the bar
patin: rendered 1920x32 top bar with a 32px exclusive zone
```

The host compositor independently listed a `1920x32` surface in layer level 2
(`top`) with namespace `patin`. Other shell panels were active during this
test, so Hyprland placed Patin below their already-reserved top area rather
than at output coordinate zero.

The integration check then launched current local 0xin nested at `1280x720`
with Patin as its child client:

```text
0xin: socket ready — WAYLAND_DISPLAY=wayland-0
0xin: spawned client `/home/vdzee/proj/patin/target/debug/patin`
patin: connected; waiting for the compositor to configure the bar
patin: rendered 1280x32 top bar with a 32px exclusive zone
```

The purple bar was visible in the nested 0xin output. Stopping 0xin shut the
compositor down cleanly after the client test.
