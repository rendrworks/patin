# Stage 5 — Internal UI Core

## Concept

Previous stages drew and hit-tested the toggle and clock directly from platform
and renderer code. That works for one bar, but it cannot scale into launchers,
quick settings, or alternative compositions without duplicating geometry.

Stage 5 introduces a small retained UI scene. It is deliberately an internal
shell implementation, not a public general-purpose toolkit.

## Geometry and layout

`Point`, `Size`, and `Rect` use logical floating-point coordinates. The same
rectangles drive layout, drawing, hit-testing, and damage, avoiding separate
scale-dependent definitions.

Row and column accept fixed and weighted-fill lengths. Stack gives multiple
children the same bounds for overlays. Gaps are reserved first; remaining space
is assigned to fill children. On a very narrow surface, fixed children shrink
proportionally rather than overflowing or receiving negative sizes.

The current bar is a row:

```text
Toggle (180) | Spacer (fill) | [Battery] | [Volume] | [Brightness] | Clock (72)
```

Bracketed status components participate only when their providers are
available.

## Scene and styling

The demo `DemoBar` owns the clock string, toggle state, component bounds, and
its style.
It generates renderer-neutral `Fill` and `Text` commands. The CPU renderer maps
logical bounds to physical pixels and executes those commands with tiny-skia
and cosmic-text. It no longer knows what a toggle, clock, or bar layout is.

The scene also owns hit-testing. Pointer and touch handlers ask the scene for an
action at a logical position instead of testing a hard-coded rectangle.

## Damage

State mutations record logical damage:

- resize and output-scale changes invalidate the full scene;
- toggling invalidates only the toggle bounds;
- a new minute invalidates only the clock bounds.

Before attaching the next buffer, the platform converts every logical damage
rectangle to physical coordinates, flooring its origin and ceiling its far
edge. Outward rounding ensures fractional pixels are never omitted.

The current shared-memory backend still draws a complete fresh buffer. Damage
describes which parts differ from the previously committed surface content and
is now ready for later buffer reuse and partial raster work.

## Important functions

- `row`, `column`, `stack`, and `linear_layout` implement internal layout.
- `Rect::contains` and `Rect::inset` provide shared geometry operations.
- `DemoBar::resize` computes the example component bounds.
- `DemoBar::action_at` maps an input position to an example action.
- `DemoBar::activate_at`, `update`, and `damage_all` mutate demo state and
  record appropriate damage through the toolkit's `Shell` trait.
- `DemoBar::commands` builds the renderer-neutral example scene.
- `CpuRenderer::render_bar` executes generic draw commands.
- `Patin::draw` converts logical damage to physical Wayland buffer damage.

## Verification

Verified on 29 July 2026 with Rust 1.97.1:

```text
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo run --example demo_bar
grim /tmp/patin-stage5.png
mdbook build
git diff --check
```

After the toolkit split, five library tests cover rendering and generic layout.
Four demo-only tests cover clock formatting, volume parsing, brightness
formatting, and example hit-testing/damage.

On the local `3440x32` logical output, the screenshot confirmed the visible
layout was preserved. Real pointer presses toggled the state in both directions
and each redraw reported one damaged component region.

The unchanged demo consumer was built natively and launched on the FP5. The scene
first rendered at the integer fallback and then at the compositor's 2.4×
preferred scale:

```text
patin: rendered 509x32 buffer for 509x32 logical bar (1 damaged region)
patin: rendered 1222x77 buffer for 509x32 logical bar (1 damaged region)
```

Repeated single-touch and overlapping two-finger input toggled the
scene-generated target correctly. Every state change reported one damage
region. Stopping Patin left the compositor and Wayland socket alive.

## Demo status follow-up

The same example scene was used for optional battery, volume, and brightness
fixtures. These are not Patin library components. On the laptop, a screenshot showed
`BAT 100%+` and `VOL MUTE`. On the FP5, the demo reported and rendered:

```text
demo_bar: status providers: battery=BAT 55%+, volume=VOL 65%
```

The FP5 had no native PipeWire default sink at test time, so the volume adapter
correctly fell back to its PulseAudio-compatible default sink. No device name
or hardware branch was added.
