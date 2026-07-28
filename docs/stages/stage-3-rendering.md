# Stage 3 — Scale-aware Rendering

## Concept

Wayland configures surfaces in logical coordinates. A 32-pixel logical bar may
need a 32, 48, or 77-pixel physical buffer depending on output scale. Drawing
text directly at the logical size and asking the compositor to enlarge it makes
glyphs blurry.

Patin requests `wp_fractional_scale_v1` and `wp_viewport` when available. The
preferred scale is expressed in 120ths: 120 is 1× and 180 is 1.5×. Physical
dimensions are the logical dimensions multiplied by that scale and rounded
upward. The viewport destination remains the logical size and `wl_surface`
buffer scale stays 1, as required by the fractional-scale protocol. Without
both optional protocols, Patin uses the integer `wl_output` scale.

## Rendering boundary

`CpuRenderer` is an internal backend with no Wayland knowledge. It:

1. creates a physical tiny-skia pixmap;
2. fills the background and draws a two-logical-pixel accent rectangle;
3. asks cosmic-text to shape and rasterize a right-aligned monospace clock;
4. scales font metrics and padding with the physical scale;
5. converts tiny-skia RGBA storage to Wayland's little-endian ARGB8888 canvas.

Shared-memory pool slots may contain alignment padding after the visible pixel
payload. The conversion therefore writes exactly the rendered bytes into any
slot large enough to hold them and leaves trailing allocation bytes untouched.

The font system and Swash glyph cache live for the process lifetime. This keeps
font discovery and glyph rasterization state out of the frame loop.

## Event and frame flow

A Calloop timer checks local time once per second, but requests a redraw only
when the displayed `HH:MM` string changes. Configure and scale changes also
request redraws. Each submitted buffer requests a Wayland frame callback; if
state changes while a frame is pending, one redraw is retained and submitted
after the callback. The SCTK slot pool can recycle shared-memory storage after
the compositor releases its buffers.

## Important functions

- `Scale::physical` converts logical lengths using protocol-native 120ths and
  ceiling division.
- `CpuRenderer::render_bar` is the internal CPU renderer entry point.
- `CpuRenderer::draw_clock` owns cosmic-text layout and tiny-skia glyph
  compositing.
- `Patin::request_redraw` and `Patin::frame` coalesce updates around compositor
  frame callbacks.
- The fractional-scale dispatch handler updates scale without depending on an
  output, device, or compositor name.

## Verification

Verified on 29 July 2026 with Rust 1.97.1:

```text
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo run
grim /tmp/patin-stage3.png
mdbook build
git diff --check
```

Three unit tests passed: fractional physical-size rounding, RGBA-to-ARGB
conversion into a padded shared-memory slot, and zero-padded clock formatting.
On the active 1× output, Patin rendered a `1920x32` buffer for a `1920x32`
logical bar. The screenshot confirmed that the clock was visible, right-aligned,
vertically centered, and separated from the desktop by the accent.

The timer crossed two minute boundaries during the live test and produced
exactly one logged render for each new minute.

## Fractional-scale verification

A nested compositor output was configured to 1.5×. Patin first submitted its
1× fallback while protocol events were arriving, then reacted to the preferred
scale:

```text
patin: rendered 853x32 buffer for 853x32 logical bar
patin: rendered 1280x48 buffer for 853x32 logical bar
```

The second dimensions are `ceil(853 × 1.5)` by `32 × 1.5`. A raw screencopy of
the nested `1280x720` output confirmed the clock was sharp and correctly
right-aligned in the scaled bar.

The same source was then built natively on an aarch64 Wayland system and
launched against its running compositor. It received a 2.4× preferred scale:

```text
patin: rendered 509x32 buffer for 509x32 logical bar
patin: rendered 1222x77 buffer for 509x32 logical bar
```

This test exposed the valid shared-memory slot padding described above. After
adding the regression test and general renderer fix, Patin stayed running at
the fractional scale. Terminating Patin left both the compositor process and
its Wayland socket alive.
