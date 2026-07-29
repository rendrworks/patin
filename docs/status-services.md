# Demo Status Fixtures

These providers belong to `examples/demo_bar`, not the Patin library. They
exercise optional values, dynamic row membership, and component damage. A
missing provider does not prevent the example from starting.

## Battery

The first battery provider reads Linux's documented power-supply sysfs ABI. It
discovers entries whose `type` is `Battery`, prefers `scope` `System`, and reads
`capacity` and `status`. It does not depend on battery names such as `BAT0` or
on a hardware model.

The display format is `BAT 55%`; charging or full state appends `+`.

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

Command polling is intentionally a test fixture, not the toolkit's service
architecture. Optional reusable provider crates may be designed later from
demonstrated consumer needs.
