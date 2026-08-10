# Status Providers

Battery, volume, brightness, and network are each an optional, opt-in
toolkit crate implementing `patin::service::Provider` (see
[Architecture](architecture.md#service-adapters)), not code inside the
`patin` library or a demo-only fixture. `examples/demo_bar/services.rs`
composes battery, volume, and network into one `StatusSnapshot` for its row
layout — that
composition, and the row's dynamic membership/damage behavior, remains the
demo's own job. A missing provider does not prevent the example from
starting.

## Battery

`crates/patin-service-upower`'s `BatteryProvider` (see
[Stage 6a](stages/stage-6a-upower-battery.md)) reads UPower's synthetic
`DisplayDevice` over D-Bus — the aggregate device UPower maintains
specifically for status bars — via `zbus`'s blocking API, fetching the
`Percentage` and `State` properties. It does not depend on battery names
such as `BAT0` or on a hardware model. A missing system bus or UPower
service returns `None`.

The demo renders the percentage as the fill level of a battery outline. Low
battery uses a warning color and charging uses the Patin accent; no percentage
or name is drawn.

## Volume

Linux audio systems do not expose one universal standard D-Bus volume
interface, so `crates/patin-service-volume`'s `VolumeProvider` (see
[Stage 6b](stages/stage-6b-volume-brightness.md)) shells out instead:

1. `wpctl get-volume @DEFAULT_AUDIO_SINK@`;
2. `pactl get-sink-volume @DEFAULT_SINK@` plus `get-sink-mute`.

This supports a native PipeWire default sink and the common PulseAudio
compatibility service. Failure of both commands returns `None`. The demo maps
the percentage to zero through three sound bars; mute replaces them with a
warning-colored strike.

## Brightness

There is no portable D-Bus property to read the current backlight level
(systemd-logind only exposes a `SetBrightness` method, not a readable one),
so `crates/patin-service-brightness`'s `BacklightProvider` reads Linux's
documented `/sys/class/backlight` ABI directly: it discovers entries, reads
`brightness` and `max_brightness`, and returns a percentage. It does not
assume a driver or panel name. A missing or invalid backlight entry returns
`None`. The adapter remains available to other compositions, but the current
demo bar deliberately does not instantiate it.

## Network

`crates/patin-service-network`'s `NetworkProvider` originally used
NetworkManager (see the historical [Stage 6e](stages/stage-6e-multi-transport-network.md)).
It now enumerates iwd's standard D-Bus object manager, reads device/station/AP
state, and maps iwd's ordered-network signal values to a percentage. Linux
sysfs supplies independent wired carrier state.

The same provider discovers modem objects through ModemManager's standard
ObjectManager interface. A registered modem contributes its independent
`SignalQuality` percentage. This represents simultaneous wifi and SIM service
without treating VPN or loopback connections as physical transports.

The demo draws a wifi fan, a linked-node icon for wired, and cellular strength
bars. Only active/registered transport icons receive slots.

The current snapshot also reports hardware availability, radio enablement, and
the Patin hotspot state. This keeps disconnected radios visible without naming
an interface or phone model. Explicit control methods back the independent
network-settings composition. iwd owns Wi-Fi profiles and AP mode;
ModemManager owns cellular state.

## Polling

The demo's `Shell::update` polls its three providers once per platform
update. Unchanged values produce no redraw; changed values damage only
their component. A provider's snapshot appearing or disappearing changes
row membership and damages the full bar.

`examples/demo_bar/services.rs` preserves the adapters' structured snapshot
types instead of formatting strings. `examples/demo_bar/scene.rs` owns all
icon choices and builds them from existing `Fill` and `RoundedFill` commands,
so neither the toolkit nor service crates prescribe a visual style or require
an icon font.

All four adapters are poll-based today, reusing the same once-per-second
tick. A future push-only service (notifications, media) will need a way to
wake the platform event loop from a background thread between ticks — that
plumbing does not exist yet.
