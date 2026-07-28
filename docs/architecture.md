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
- **Mobile** composes shared components into opt-in phone navigation and
  surfaces. Desktop startup must not construct phone-specific UI.
- **Compositor integration** exposes workspace state and commands through a
  replaceable adapter. Its neutral implementation works without compositor
  IPC; a later 0xin adapter will use the documented control socket.

## Data flow

The event loop receives platform and service events, updates shell state, lays
out affected UI, collects damage, and asks the renderer to redraw damaged
regions into a buffer. Wayland buffer release and frame callbacks determine
when storage can be reused and when another frame should be submitted.

Milestone 2 implements the first narrow slice of this flow. Calloop dispatches
the Wayland event queue, SCTK owns protocol state and shared-memory buffer
slots, and the layer-shell configure event triggers a single solid-color draw.
The pure `render::fill_solid_argb` function is separate from protocol code so
pixel generation can be tested without a compositor.

## Deliberate boundaries

Rendering is kept behind a small internal interface so a GPU backend can be
added if measurements justify it. This is not a promise of a public renderer
API, and v1 will not build abstractions for hypothetical backends.

Patin reuses focused libraries for standards-heavy work. It does not reimplement
Wayland protocol machinery, font shaping, D-Bus, or complex system-service
protocols merely to remain "from scratch."
