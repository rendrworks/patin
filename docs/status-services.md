# Status Services

Patin's service boundary returns optional values to the UI. A missing service
does not prevent the shell from starting and does not reserve an empty
component in the row.

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

## Polling and future replacement

Calloop polls the snapshot every two seconds. Unchanged values produce no
redraw; changed values damage only their component. Provider appearance or
disappearance changes row membership and damages the full bar.

Command polling is intentionally a small first integration, not the final
service architecture. Milestone 6 can add event-driven UPower D-Bus and native
PipeWire/PulseAudio providers behind the same optional snapshot boundary.
