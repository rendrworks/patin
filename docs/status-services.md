# Demo Status Fixtures

These providers belong to `examples/demo_bar`, not the Patin library. They
exercise optional values, dynamic row membership, and component damage. A
missing provider does not prevent the example from starting.

## Battery

Battery now comes from `crates/patin-service-upower`'s `BatteryProvider`, a
toolkit-level, opt-in adapter (see [Architecture](architecture.md#service-adapters)
and [Stage 6a](stages/stage-6a-upower-battery.md)), not a demo-only fixture.
It reads UPower's synthetic `DisplayDevice` over D-Bus — the aggregate device
UPower maintains specifically for status bars — via `zbus`'s blocking API,
fetching the `Percentage` and `State` properties. It does not depend on
battery names such as `BAT0` or on a hardware model. A missing system bus or
UPower service returns `None`, same as the demo's other optional fixtures.

The display format is `BAT 55%`; a `State` of charging or fully charged
appends `+`.

## Volume

Linux audio systems do not expose one universal standard D-Bus volume
interface. The initial adapter tries:

1. `wpctl get-volume @DEFAULT_AUDIO_SINK@`;
2. `pactl get-sink-volume @DEFAULT_SINK@` plus `get-sink-mute`.

This supports a native PipeWire default sink and the common PulseAudio
compatibility service. Failure of both commands returns no component.

## Brightness

The demo discovers entries under Linux's `/sys/class/backlight`, reads
`brightness` and `max_brightness`, and displays `BRI n%`. It does not assume a
driver or panel name. A missing or invalid backlight entry removes the fixture
from the demo row.

## Polling and future replacement

The demo's `Shell::update` polls the snapshot once per platform update.
Unchanged values produce no redraw; changed values damage only their component.
Provider appearance or disappearance changes row membership and damages the
full bar.

Volume and brightness are still command/sysfs test fixtures, not toolkit
service architecture. Battery has moved to the pattern they may follow later:
an out-of-tree, opt-in crate implementing `patin::service::Provider`.
