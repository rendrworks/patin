# Stage 4 — Pointer and Touch Input

## Concept

Wayland seats advertise capabilities rather than hardware identities. A seat
can gain or lose a pointer or touchscreen while Patin is running. Patin binds
each advertised pointer and touch capability and releases the corresponding
protocol object when it disappears. No device, connector, or compositor name
is involved.

Pointer and touch positions arrive in surface-local logical coordinates. The
toggle target is therefore a logical rectangle shared by both input paths; it
does not change when the renderer creates a larger physical buffer for a
fractionally scaled output.

The bar retains `KeyboardInteractivity::None`. Pointer or touch interaction can
change Patin state without asking the compositor to move keyboard focus away
from an application.

## Visible behavior

The leftmost 112 logical pixels form a visible `SHELL OFF` target. A primary
pointer press or a touch-down inside it toggles the state. The target becomes
green and reads `SHELL ON`; another activation restores the initial state.
Every touch-down is processed independently, including contacts delivered
together in a multitouch frame.

The toggle is deliberately local demonstration state. It proves input,
hit-testing, component state, and redraw flow without inventing a shell action
before the UI-core stage.

## Important functions

- `input::Rect::contains` performs the half-open logical-coordinate hit test.
- `input::toggle_target` defines target geometry independently of rendering.
- `Patin::activate_at` applies the shared hit test, changes state, and requests
  a redraw.
- `SeatHandler::new_capability` and `remove_capability` follow runtime pointer
  and touch availability for every seat.
- `PointerHandler::pointer_frame` accepts only primary-button presses on
  Patin's layer surface.
- `TouchHandler::down` handles each contact delivered on Patin's surface.
- `CpuRenderer::draw_toggle` draws the visible state at the current physical
  scale.

## Verification

Verified on 29 July 2026 with Rust 1.97.1:

```text
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo run
grim /tmp/patin-stage4-initial.png
mdbook build
git diff --check
```

The local screenshot showed the `SHELL OFF` target at the left, the clock at
the right, and the bar's accent and exclusive zone unchanged.

The same source was built natively and run against the FP5's active Wayland
compositor. The compositor supplied a 2.4× preferred scale:

```text
patin: rendered 509x32 buffer for 509x32 logical bar
patin: rendered 1222x77 buffer for 509x32 logical bar
patin: toggle activated; state is on
patin: rendered 1222x77 buffer for 509x32 logical bar
patin: toggle activated; state is off
patin: rendered 1222x77 buffer for 509x32 logical bar
```

Two real touchscreen activations changed the visible state and redrew the
scaled buffer. Stopping Patin left the compositor process and Wayland socket
alive. A simultaneous two-finger activation was not observed during this test,
so that live check remains pending; the handler itself processes every
touch-down event independently.
