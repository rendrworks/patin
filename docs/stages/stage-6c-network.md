# Stage 6c — Network Service Adapter

## Why this stage exists

Stage 6 had battery, volume, and brightness done. Network is next, and
unlike volume/brightness it has a real, near-universal D-Bus service to
key off — NetworkManager — same shape as UPower for battery.

Before designing anything, the actual property names and types were
checked live against the FP5's NetworkManager over D-Bus
(`busctl --system ...`), since guessing D-Bus API details from memory is
exactly what turned out wrong for `zbus`'s feature flags in stage 6a:

- `org.freedesktop.NetworkManager` at `/org/freedesktop/NetworkManager`
  exposes `PrimaryConnection` (an object path, `"/"` when there is none)
  and `PrimaryConnectionType` (a string — `"802-11-wireless"` was the
  live value on the FP5's wifi connection).
- A wifi signal percentage takes a real walk: the `ActiveConnection`
  object's `Devices` → that `Device`'s
  `org.freedesktop.NetworkManager.Device.Wireless.ActiveAccessPoint` →
  that `AccessPoint`'s `Strength`, a `y` (byte, 0–100) — confirmed `68`
  live, no scaling needed unlike UPower's `f64` percentage.
- The FP5 also runs `ModemManager` (cellular capability exists), but
  pulling in cellular signal detail was scoped out of this stage, the same
  kind of cut stage 6b made for audio/brightness detail. Any primary
  connection type other than wifi/wired (including `gsm`/`cdma`) just
  reports as generically connected.

## New crate: `patin-service-network`

Named by domain (`network`), not mechanism, since a future cellular
addition via ModemManager would still belong under the same domain rather
than forcing a rename.

```rust
pub enum NetworkSnapshot {
    Disconnected,
    Wired,
    Wifi { percentage: u8 },
    Other,
}
```

`Provider::poll` returns `Option<NetworkSnapshot>`, but unlike battery,
`None` here means only "NetworkManager unreachable over D-Bus" — being
reachable but disconnected is a real reading
(`Some(NetworkSnapshot::Disconnected)`), not folded into `None`. This
matches how `BatterySnapshot` always carries a real `charging` reading
rather than conflating "no data" with "off".

The wifi signal walk (`wifi_strength`) is a small private helper using `?`
short-circuiting across the three D-Bus hops; if any hop fails despite the
connection type saying wireless (an edge case), it degrades to
`NetworkSnapshot::Other` rather than panicking or failing the whole poll.

`Cargo.toml` reuses the exact same `zbus = "=5.18.0"` pin and feature list
as `patin-service-upower` — already resolved in stage 6a, no new research
needed. It compiled clean on the first try, which is what checking the
live D-Bus API first was for.

## Demo integration

`examples/demo_bar/services.rs` gained a fourth provider field and a
`format_network` function (`"NET 55%"` for wifi, `"NET ETH"` for wired,
`"NET OFF"` disconnected, `"NET UP"` otherwise). `StatusSnapshot` gained a
`network: Option<String>` field.

`examples/demo_bar/scene.rs` needed the same mechanical extension the
other three fields already had — a fourth optional row slot, joining only
when `Some`, plus its damage-tracking branch in `set_status` and its `Text`
draw command — the one part of this stage that touched `scene.rs` (6a/6b
were additive only). Root `Cargo.toml` gained `patin-service-network` under
`[workspace] members` and `[dev-dependencies]`.

## Verification

Verified on 30 July 2026:

```text
$ cargo build -p patin-service-network
Finished, no errors (compiled clean on the first try)

$ cargo fmt --all -- --check
(no output after one auto-fix to a too-long line)

$ cargo test --workspace --all-targets
11 tests across 5 crates, all passed, including
  patin-service-network: 1 (poll_without_a_system_bus_returns_none)

$ cargo clippy --workspace --all-targets --all-features -- -D warnings
Finished, no warnings

$ cargo run --example demo_bar (timeout 5s, this sandbox: no NetworkManager)
demo_bar: status providers: battery=unavailable, volume=VOL MUTE, brightness=BRI 71%, network=unavailable
patin: connected; waiting for the compositor to configure the bar
```

Confirmed independently with `busctl --system list | grep -i networkmanager`
that NetworkManager is genuinely absent here, so `unavailable` is the
adapter degrading correctly.

```text
$ mdbook build
 INFO Book building has started
 INFO Running the html backend
 INFO HTML book written to `/home/vdzee/proj/patin/book`

$ git diff --check
(no output, exit 0)
```

### FP5 end-to-end confirmation

Same round-trip as 6a/6b: working tree copied over with `tar` piped over
SSH, built natively (fast — `zbus` already cached), installed, and
relaunched as a transient systemd user unit per the `fp5-test-device`
memory:

```text
demo_bar: status providers: battery=BAT 64%, volume=VOL 8%, brightness=BRI 40%, network=NET 69%
patin: connected; waiting for the compositor to configure the bar
```

`NET 69%` is close to the `68` read directly from the access point's
`Strength` property during the initial `busctl` probe (signal strength
drifts slightly between reads) — the adapter reads the FP5's real wifi
connection correctly end to end.
