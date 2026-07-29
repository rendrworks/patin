# Architecture

Patin will be organized around narrow internal boundaries. These are intended
directions rather than empty modules in the foundation:

- **Platform** owns Wayland connections, globals, outputs, seats, input events,
  layer surfaces, shared-memory buffers, and the event loop.
- **Render** turns a UI scene into pixels. The first backend uses `wl_shm` and
  `tiny-skia`; text is shaped and rasterized with `cosmic-text`.
- **UI** owns internal geometry, row/column/stack layout, style resolution,
  hit-testing, and damage collection.
- **Components** combine UI primitives into bars, clocks, launchers, overlays,
  quick settings, and notification views.
- **Services** expose typed state from standard system interfaces such as
  D-Bus, without coupling UI components to transport details.
- **Compositions** select and arrange shared components for a use case. A
  composition instantiates only the modules it enables.
- **Compositor integration** exposes workspace state and commands through a
  replaceable adapter. Its neutral implementation works without compositor
  IPC; a later 0xin adapter will use the documented control socket.

## Data flow

The event loop receives platform and service events, updates shell state, lays
out affected UI, collects damage, and asks the renderer to redraw damaged
regions into a buffer. Wayland buffer release and frame callbacks determine
when storage can be reused and when another frame should be submitted.

Calloop dispatches the Wayland event queue and clock timer. SCTK owns protocol
state and shared-memory slots. Configure, scale, and minute changes mark the
bar for redraw; Wayland frame callbacks ensure Patin does not submit another
frame while one is pending.

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

The bar uses layer-shell keyboard interactivity `None`. Clicking or touching it
therefore does not request keyboard focus from the compositor.

## Internal UI core

Logical `Point`, `Size`, and `Rect` types are shared by layout, hit-testing,
draw commands, and damage. Row and column distribute fixed and weighted fill
lengths along one axis; stack assigns the same bounds to layered children.
When fixed children cannot fit, they shrink proportionally instead of
generating negative or overflowing rectangles.

`BarScene` is a retained internal component composition:

```text
Row
├── Toggle (fixed preference)
├── Spacer (weighted fill)
└── Clock (fixed preference)
```

The scene owns component state and styling. It emits a small command list for
the current frame and records logical damaged rectangles when state changes.
Resize and scale changes damage the full bar; toggle and clock changes damage
only their component bounds. The Wayland boundary converts those rectangles to
outward-rounded physical buffer coordinates.

## Deliberate boundaries

Rendering is kept behind a small internal interface so a GPU backend can be
added if measurements justify it. This is not a promise of a public renderer
API, and v1 will not build abstractions for hypothetical backends.

Core behavior must not branch on hardware models, connector names, fixed
resolutions, compositor brands, or assumed scales. Outputs, transforms, input
capabilities, and protocol support come from Wayland at runtime. Different
shell compositions are selected explicitly and built from the same platform,
rendering, UI, component, and service modules.

Patin reuses focused libraries for standards-heavy work. It does not reimplement
Wayland protocol machinery, font shaping, D-Bus, or complex system-service
protocols merely to remain "from scratch."
