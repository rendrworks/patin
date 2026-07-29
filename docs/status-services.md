# Status Providers

Battery, volume, brightness, and network are each an optional, opt-in
toolkit crate implementing `patin::service::Provider` (see
[Architecture](architecture.md#service-adapters)), not code inside the
`patin` library or a demo-only fixture. `examples/demo_bar/services.rs`
composes all four into one `StatusSnapshot` for its row layout — that
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

The display format is `BAT 55%`; a `State` of charging or fully charged
appends `+`.

## Volume

Linux audio systems do not expose one universal standard D-Bus volume
interface, so `crates/patin-service-volume`'s `VolumeProvider` (see
[Stage 6b](stages/stage-6b-volume-brightness.md)) shells out instead:

1. `wpctl get-volume @DEFAULT_AUDIO_SINK@`;
2. `pactl get-sink-volume @DEFAULT_SINK@` plus `get-sink-mute`.

This supports a native PipeWire default sink and the common PulseAudio
compatibility service. Failure of both commands returns `None`. The display
format is `VOL 55%`, or `VOL MUTE` when muted.

## Brightness

There is no portable D-Bus property to read the current backlight level
(systemd-logind only exposes a `SetBrightness` method, not a readable one),
so `crates/patin-service-brightness`'s `BacklightProvider` reads Linux's
documented `/sys/class/backlight` ABI directly: it discovers entries, reads
`brightness` and `max_brightness`, and returns a percentage. It does not
assume a driver or panel name. A missing or invalid backlight entry returns
`None`. The display format is `BRI n%`.

## Network

`crates/patin-service-network`'s `NetworkProvider` (see
[Stage 6c](stages/stage-6c-network.md)) reads NetworkManager over D-Bus —
`PrimaryConnection` (an object path, `"/"` when there is none) and
`PrimaryConnectionType`. A wifi primary connection additionally walks
`ActiveConnection.Devices` → `Device.Wireless.ActiveAccessPoint` →
`AccessPoint.Strength` for a signal percentage. `Provider::poll` returning
`None` means only "NetworkManager is unreachable over D-Bus" — being
reachable but disconnected is a real reading of its own.

Cellular signal detail is out of scope: `ModemManager` is a separate D-Bus
service with its own object model, so a primary connection type other than
wifi/wired (including `gsm`/`cdma`) just reports as generically connected.

The display format is `NET 55%` for wifi, `NET ETH` for wired, `NET OFF`
when disconnected, or `NET UP` for anything else connected.

## Polling

The demo's `Shell::update` polls all four providers once per platform
update. Unchanged values produce no redraw; changed values damage only
their component. A provider's snapshot appearing or disappearing changes
row membership and damages the full bar.

All four adapters are poll-based today, reusing the same once-per-second
tick. A future push-only service (notifications, media) will need a way to
wake the platform event loop from a background thread between ticks — that
plumbing does not exist yet.
