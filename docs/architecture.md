# Architecture

Patin is a library organized around narrow shell-toolkit boundaries:

- **Platform** owns Wayland connections, globals, outputs, seats, input events,
  layer surfaces, shared-memory buffers, and the event loop.
- **Render** turns a UI scene into pixels. The first backend uses `wl_shm` and
  `tiny-skia`; text is shaped and rasterized with `cosmic-text`.
- **UI** owns internal geometry, row/column/stack layout, style resolution,
  hit-testing, and damage collection.
- **Service adapters** are optional, out-of-tree crates that implement
  `patin::service::Provider` against one system service (D-Bus or otherwise).
  Patin never constructs one; a consumer depends on and instantiates the
  ones it wants.
- **Consumers** own components, services, and compositions. The demo bar is one
  consumer used to verify the library.
- **Compositor integration** exposes workspace state and commands through a
  replaceable adapter. Its neutral implementation works without compositor
  IPC; a later 0xin adapter will use the documented control socket.

## Data flow

The platform event loop receives Wayland and timer events and calls a consumer
implementation of `Shell`. The consumer updates its state, returns logical draw
commands and damage, and decides how input positions affect its composition.
Wayland buffer release and frame callbacks determine when storage can be reused
and when another frame should be submitted.

Calloop dispatches the Wayland event queue and a general consumer update tick.
SCTK owns protocol state and shared-memory slots. Configure, scale, input, or
consumer updates can mark a surface for redraw; frame callbacks ensure Patin
does not submit another frame while one is pending.

`CpuRenderer` owns tiny-skia, the cosmic-text font system, and its glyph cache.
It receives only a byte canvas, physical dimensions, scale, and a list of
renderer-neutral fill/text commands produced by the UI scene. Layout,
component state, hit-testing, and style stay outside the renderer. Wayland
protocol objects also stay outside it. Tiny-skia produces premultiplied RGBA
pixels internally, which the renderer converts to little-endian Wayland
ARGB8888 when copying into `wl_shm`.

Logical surface size and physical buffer size are separate. Fractional scale is
represented in protocol-native 120ths, physical dimensions are rounded upward,
and `wp_viewport` maps that buffer back to the compositor-provided logical
surface size.

SCTK's seat state discovers pointer and touch capabilities at runtime. Patin
creates one protocol object per advertised capability and releases it when the
capability or seat disappears. Both input paths receive surface-local logical
coordinates and call the same pure rectangle hit test. Successful activations
change component state and enter the existing frame-coalesced redraw path.
Every touch contact is handled independently. Active contacts are keyed by
their touch protocol object and contact ID, so overlapping contacts remain
distinct across seats.

The consumer supplies `LayerConfig`: namespace, layer level, anchors, logical
size, exclusive zone, and keyboard policy. The demo chooses a top exclusive
bar with keyboard policy `None`; the toolkit does not choose those values.

## Internal UI core

Logical `Point`, `Size`, and `Rect` types are shared by layout, hit-testing,
draw commands, and damage. Row and column distribute fixed and weighted fill
lengths along one axis; stack assigns the same bounds to layered children.
When fixed children cannot fit, they shrink proportionally instead of
generating negative or overflowing rectangles.

`examples/demo_bar/scene.rs` is a retained test composition:

```text
Row
├── Toggle (fixed preference)
├── Spacer (weighted fill)
├── Optional status fixtures
└── Clock (fixed preference)
```

The example owns component state and styling. Optional battery, volume, and
brightness fixtures join its row only when their providers return values. It
emits a small command list and records logical damaged rectangles when state
changes.
Resize and scale changes damage the full bar; toggle and clock changes damage
only their component bounds. The Wayland boundary converts those rectangles to
outward-rounded physical buffer coordinates.

The status adapters and Chrono dependency are example implementation details.
They are not exported by `src/lib.rs` and are never constructed by
`platform::run`.

## Service adapters

`patin::service::Provider` is a minimal, dependency-free trait: `poll(&mut
self) -> Self::Snapshot`. It is the only thing the core crate contributes to
service integration. Construction is left to each adapter, since opening a
D-Bus connection or similar can fail in ways only that adapter understands.

Concrete adapters live in their own workspace crates under `crates/`, never
in `src/`, so their dependencies (`zbus`, and later whatever a network or
media adapter needs) never reach a consumer that only wants the toolkit.
`crates/patin-service-upower` is the first one: it polls UPower's
`DisplayDevice` — the synthetic aggregate battery device UPower maintains
for shells — over `zbus`'s blocking API, and degrades to `None` when no
system bus or UPower service is reachable, the same failure behavior as the
demo's other optional status fixtures.

This first adapter is intentionally poll-based, reusing the same
`Shell::update` tick the demo already had. A push-only service such as
notifications will need a way to wake the platform event loop from a
background thread between ticks; that plumbing does not exist yet and is
scoped to whichever future stage first needs it.

## Deliberate boundaries

Rendering is kept behind a small interface so a GPU backend can be added if
measurements justify it. Patin exposes only shell-focused primitives and does
not build abstractions for hypothetical application GUI use.

Core behavior must not branch on hardware models, connector names, fixed
resolutions, compositor brands, or assumed scales. Outputs, transforms, input
capabilities, and protocol support come from Wayland at runtime. Different
shell compositions are selected explicitly and built from the same platform,
rendering, UI, component, and service modules.

Patin reuses focused libraries for standards-heavy work. It does not reimplement
Wayland protocol machinery, font shaping, D-Bus, or complex system-service
protocols merely to remain "from scratch."
