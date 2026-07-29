# Stage 6b — Volume and Brightness Service Adapters

## Why this stage exists

Stage 6a proved the `patin::service::Provider` + opt-in-crate pattern with
battery, which polls a real D-Bus service (UPower). Volume and brightness
were the two demo fixtures still left as subprocess/sysfs code inline in
`examples/demo_bar/services.rs`.

Neither has a real D-Bus service to key a crate name off the way UPower did
for battery: Linux audio has no universal standard D-Bus volume interface,
and systemd-logind only exposes a `SetBrightness` method, not a readable
one. So this stage names by domain concept instead of mechanism —
`patin-service-volume` and `patin-service-brightness` — and is otherwise a
mechanical port: the existing, already-working subprocess/sysfs logic moved
verbatim into two new crates implementing `Provider`, restructuring their
return values from pre-formatted strings into typed snapshots (matching
`BatterySnapshot`'s shape), with formatting left to the demo. No new
external dependencies, no calloop changes — same poll-once-per-second model
as battery.

## New crate: `patin-service-volume`

Ports `read_volume`/`read_wpctl_volume`/`read_pactl_volume`/
`parse_wpctl_volume` from the demo, returning `VolumeSnapshot { percentage,
muted }` instead of a pre-formatted string. The percentage is no longer
discarded when muted — the demo's formatter chooses to still show
`"VOL MUTE"` for now, but the data is available. Depends only on `patin`
(for `Provider`); `Command` handling is all `std`.

## New crate: `patin-service-brightness`

Ports `read_brightness`/`brightness_label`/`read_trimmed` verbatim,
returning `BrightnessSnapshot { percentage }`. Same no-external-dependency
shape as the volume crate.

## Demo integration

`examples/demo_bar/services.rs` is now composition-and-formatting only:
`SystemStatus` holds all three providers (`BatteryProvider`,
`VolumeProvider`, `BacklightProvider`); `poll` calls each `.poll()` and maps
through `format_battery`/`format_volume`/`format_brightness` to rebuild the
same `"BAT n%[+]"` / `"VOL n%"` / `"VOL MUTE"` / `"BRI n%"` strings
`scene.rs` already expected — `scene.rs` itself is unchanged. Root
`Cargo.toml` gained both crates under `[workspace] members` and
`[dev-dependencies]`.

## Documentation

`docs/status-services.md` was retitled from "Demo Status Fixtures" to
"Status Providers" and rewritten: none of the three are demo-only fixtures
anymore, only their composition into `StatusSnapshot` is. `README.md` and
`docs/architecture.md` were updated to list all three adapter crates and
drop the now-inaccurate "provisional audio and brightness providers are
used by the demo only" framing.

## Verification

Verified on 30 July 2026:

```text
$ cargo fmt --all -- --check
(no output)

$ cargo test --workspace --all-targets
10 tests across 4 crates, all passed:
  patin: 5
  examples/demo_bar: 2
  patin-service-brightness: 1 (computes_brightness_and_rejects_zero_maximum)
  patin-service-upower: 1 (poll_without_a_system_bus_returns_none)
  patin-service-volume: 1 (parses_wpctl_volume_and_mute_state)

$ cargo clippy --workspace --all-targets --all-features -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s), no warnings

$ cargo run --example demo_bar (timeout 5s, this sandbox: no upowerd)
demo_bar: status providers: battery=unavailable, volume=VOL MUTE, brightness=BRI 71%
patin: connected; waiting for the compositor to configure the bar

$ mdbook build
 INFO Book building has started
 INFO Running the html backend
 INFO HTML book written to `/home/vdzee/proj/patin/book`

$ git diff --check
(no output, exit 0)
```

### FP5 end-to-end confirmation

Verified on the FP5 the same day, same round-trip as stage 6a: working
tree copied over with `tar` piped over SSH, built natively
(`cargo build --release --locked --example demo_bar`, ~6s — fast, since
`zbus` and its transitive deps were already cached from stage 6a and
neither new crate adds an external dependency), then installed and
relaunched as a transient systemd user unit (plain backgrounded SSH
processes don't survive the PAM session tearing down, per stage 6a):

```sh
systemd-run --user --unit=patin-demo --collect \
  --setenv=XDG_RUNTIME_DIR=/run/user/10000 \
  --setenv=WAYLAND_DISPLAY=wayland-0 \
  -- /home/sn3rt/.local/bin/patin
```

```text
demo_bar: status providers: battery=BAT 66%, volume=VOL 8%, brightness=BRI 40%
patin: connected; waiting for the compositor to configure the bar
```

All three readings are real, not `unavailable`/degraded — `patin-service-volume`
and `patin-service-brightness` work end to end against the FP5's real
`wpctl`/`sysfs`, alongside the already-proven UPower battery adapter.
