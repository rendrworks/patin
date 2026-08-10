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
  consumer used to verify the library. `patin-lock` is another consumer and is
  built/launched independently. `patin-launcher` is a third: an ordinary
  overlay-layer consumer with no compositor-specific integration.
  `patin-session` is another finite overlay consumer; compositor-specific
  logout commands are injected by its launching environment.
- **Compositor integration** exposes workspace state and commands through a
  replaceable adapter. Its neutral implementation works without compositor
  IPC; a later 0xin adapter will use the documented control socket.

## Data flow

The platform event loop receives Wayland and timer events and calls a consumer
implementation of `Shell`. The consumer updates its state, returns logical draw
commands and damage, and decides how input positions affect its composition.
Wayland buffer release and frame callbacks determine when storage can be reused
and when another frame should be submitted.

Finite and scrollable compositions use defaulted `Shell` lifecycle and
vertical-scroll hooks. The platform translates pointer-axis input and touch
drags into that capability; consumers that do not implement it are unchanged.

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
Consumers may return `true` from `Shell::close_requested` when their lifecycle
is complete. The platform then leaves its event loop cleanly; persistent
compositions inherit the default `false` implementation.

## Session-lock composition

`crates/patin-lock` uses SCTK's `ext-session-lock-v1` support directly because
a lock surface has stricter lifecycle rules than an ordinary layer surface.
It creates one lock surface for every output discovered at runtime and adds or
removes surfaces as outputs change. All surfaces share one `LockUi` state and
the toolkit CPU renderer.

Seat capabilities are also discovered dynamically. Physical keyboard, pointer,
and touch events all feed the same password model and hit-testable keyboard,
which is either the QWERTY/symbol layout or a numeric PIN grid depending on
`--keypad=full|numeric` (default `full`, `PATIN_LOCK_KEYPAD` sets the default
without passing the flag, and an explicit flag always wins). Password storage
is bounded and zeroized when cleared or dropped. PAM authentication runs on a
worker thread so the Wayland event loop continues to redraw and service
compositor events.

If the compositor advertises `zwlr_output_power_manager_v1`, `patin-lock`
binds it and requests every output be powered off and stops drawing after a
period without a real key/touch/pointer press — 1 second before the display
has ever been woken, 5 seconds from the moment of any wake onward (reset by
each keystroke, same as the 1-second case), so a pause between digits doesn't
blank the screen mid-entry. Otherwise this is skipped and the lock behaves as
it always has. Ordinary
touch, pointer, and keyboard
input are ignored while blanked rather than treated as a wake, since a phone
in a pocket brushes its screen constantly.

Two independent triggers toggle the blank state (off if on, on if off,
responding within one event-loop iteration): a `SIGUSR1` sent to the
`--worker` process, useful for scripted/SSH testing; and the physical power
button (`XF86PowerOff`), handled directly as an ordinary keyboard event in
`press_key`. The latter needs no compositor keybind — while a session is
locked, a spec-compliant compositor forwards physical keys straight to the
lock client instead of consuming them for its own keybinds (0xin's own
`handle_keybinding` does this via a `server.locked` check specifically so its
keybinds can't be used to bypass a lock), so `patin-lock` already receives
`XF86PowerOff` as ordinary input while locked and can act on it itself.

The public process supervises an internal worker. A successful PAM result makes
the worker send the protocol unlock request and exit successfully. A crash is
restarted after a delay while the compositor's session-lock protocol remains
fail-closed; configuration/protocol errors are terminal instead of entering a
restart loop. This is generic Wayland/PAM behavior and contains no 0xin or FP5
branch.

## Internal UI core

Logical `Point`, `Size`, and `Rect` types are shared by layout, hit-testing,
draw commands, and damage. Row and column distribute fixed and weighted fill
lengths along one axis; stack assigns the same bounds to layered children.
When fixed children cannot fit, they shrink proportionally instead of
generating negative or overflowing rectangles.

`DrawCommand::RoundedFill { bounds, color, radius }` sits alongside the plain
`Fill` variant for consumers that want rounded corners (button-like elements,
key backgrounds). `CpuRenderer` builds the rounded rectangle as a `tiny-skia`
path from four cubic-bezier corners rather than relying on a library-provided
rounded-rect helper, since `tiny-skia` 0.12 doesn't have one; the radius is
clamped to half the smaller side so it degrades to a normal rectangle instead
of self-intersecting on very small or very radius-heavy bounds.

`examples/demo_bar/scene.rs` is a retained test composition:

```text
Row
├── Clock (fixed left preference)
├── Optional volume fixture (fixed left preference)
├── Empty center (weighted fill)
├── Optional wifi, wired, and cellular fixtures (fixed right preferences)
└── Optional battery fixture (fixed right edge preference)
```

The row sits inside a logical horizontal inset, so its two end components do
not touch an output edge at any output scale. Fixed-width components grow
inward from both edges while a flexible spacer consumes the center. This keeps
content away from centered output obstructions without naming or detecting a
specific device. The example owns component state and styling. Optional
battery, volume, and individual network-transport fixtures join its row only when their
providers return values. It emits a small command list and records logical
damaged rectangles when state changes.
Resize and scale changes damage the full bar; clock and status-fixture
changes damage only their component bounds. The Wayland boundary converts
those rectangles to outward-rounded physical buffer coordinates. The bar has
capability-backed Wi-Fi and cellular hit targets. They launch the independent
network-settings process on the corresponding page, including while a radio is
disabled or disconnected.

Battery, volume, brightness, and network themselves are toolkit-level
provider crates (see "Service adapters" below), not example implementation
details — only their composition into one `StatusSnapshot` in
`examples/demo_bar/services.rs` is. The Chrono dependency is a genuine
example-only detail. None of this is exported by `src/lib.rs` or ever
constructed by `platform::run`.

Reusable presentation assets follow the same opt-in boundary. The
`patin-icons` workspace crate converts semantic states such as unavailable,
poor, medium, and good Wi-Fi into ordinary `DrawCommand` values. The core
toolkit does not depend on it, and neither a bar nor any other composition owns
the shared glyph. The demo bar and network settings independently choose the
crate and supply palettes appropriate to their own surfaces.

## Service adapters

`patin::service::Provider` is a minimal, dependency-free trait: `poll(&mut
self) -> Self::Snapshot`. It is the only thing the core crate contributes to
service integration. Construction is left to each adapter, since opening a
D-Bus connection or similar can fail in ways only that adapter understands.

Concrete adapters live in their own workspace crates under `crates/`, never
in `src/`, so their dependencies (`zbus`, and later whatever a media adapter
needs) never reach a consumer that only wants the toolkit. Four exist so
far, named by domain rather than mechanism except where one real service
owns the domain outright:

- `crates/patin-service-upower` polls UPower's `DisplayDevice` — the
  synthetic aggregate battery device UPower maintains for shells — over
  `zbus`'s blocking API.
- `crates/patin-service-network` polls NetworkManager's active connections
  plus access-point strength for wifi and wired state. It independently reads
  registered modem signal from ModemManager, allowing wifi and cellular fields
  to be populated simultaneously. Missing transports remain `None`/`false`
  inside a real snapshot; an unavailable system bus returns `None`.
  It also exposes explicit essential controls. NetworkManager's `nmcli`
  frontend performs profile-heavy mutations so persistent settings and secrets
  remain owned by NetworkManager. Saved infrastructure profiles are merged
  with cached access points so unavailable profiles remain visible; hotspot
  profiles in AP mode are excluded. The adapter uses each access point's D-Bus
  `LastSeen` timestamp rather than assuming every cached object is still in
  range.

- `crates/patin-service-volume` and `crates/patin-service-brightness` have
  no equivalent standard D-Bus interface to poll (noted in
  [Status Providers](status-services.md)), so they shell out to
  `wpctl`/`pactl` and read `/sys/class/backlight` respectively.

All four degrade their `Provider::poll` result when their underlying
service or file isn't reachable, the same failure behavior the demo's
status fixtures already had before they moved into these crates.

## Text input and lock input

Consumers receive toolkit-owned `KeyInput` values from physical or virtual
keyboards discovered per Wayland seat. A normal text field exposes only a
`TextInputPurpose`; the platform announces that intent with optional
text-input-v3 and the compositor chooses the session OSK. Network settings is
an XDG toplevel, while bars and transient shell compositions continue to use
layer-shell.

Seat globals can already exist when Smithay Client Toolkit constructs
`SeatState`, so the platform idempotently creates each seat's text-input-v3
object from both `new_seat` and the later capability callbacks. Relying only on
`new_seat` would leave startup-time seats with physical input but no text-input
object, preventing automatic OSK activation.

The lock is intentionally different. Its touch keyboard, password buffer, and
PAM flow are private to `patin-lock`, so authentication never depends on a
session OSK or a focused normal application.

All four adapters are intentionally poll-based, reusing the same
`Shell::update` tick the demo already had. A push-only service such as
notifications will need a way to wake the platform event loop from a
background thread between ticks; that plumbing does not exist yet and is
scoped to whichever future stage first needs it.

## Deliberate boundaries

Rendering is kept behind a small interface so a GPU backend can be added if
measurements justify it. Patin supports the two demonstrated Wayland roles—
layer-shell compositions and small XDG toplevels—without becoming a general
application GUI framework.

Core behavior must not branch on hardware models, connector names, fixed
resolutions, compositor brands, or assumed scales. Outputs, transforms, input
capabilities, and protocol support come from Wayland at runtime. Different
shell compositions are selected explicitly and built from the same platform,
rendering, UI, component, and service modules.

Patin reuses focused libraries for standards-heavy work. It does not reimplement
Wayland protocol machinery, font shaping, D-Bus, or complex system-service
protocols merely to remain "from scratch."
