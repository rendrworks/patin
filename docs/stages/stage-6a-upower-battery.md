# Stage 6a — UPower Battery Service Adapter

## Why this stage exists

`docs/status-services.md` already noted that command/sysfs polling in the
demo was "intentionally a test fixture, not the toolkit's service
architecture," and that "optional reusable provider crates may be designed
later from demonstrated consumer needs." This stage makes that real for one
service: battery.

It also settles how service adapters are packaged going forward. Rather than
feature-gating D-Bus support inside the core `patin` crate, Patin became a
Cargo workspace: each adapter is its own crate under `crates/`, so a
consumer that wants only the toolkit never compiles `zbus` or any other
adapter dependency.

## Workspace conversion

The root `Cargo.toml` gained `[workspace]` and `[workspace.package]` sections
alongside its existing `[package]` table; the root package remains an
implicit workspace member, so `src/`, `examples/`, and `Cargo.lock` did not
move. `edition`, `rust-version`, `license`, and `publish` are now inherited
via `.workspace = true` on both crates to avoid drift.

`cargo build`/`test`/`clippy` only operate on the current directory's
package unless `--workspace` is passed, so `.github/workflows/ci.yml` and
the README's verification commands now pass `--workspace` to `test` and
`clippy` (`fmt --all` already covered every member).

## Core crate: `patin::service`

`src/service.rs` adds one trait, exported from `src/lib.rs`:

```rust
pub trait Provider {
    type Snapshot: Clone + PartialEq;
    fn poll(&mut self) -> Self::Snapshot;
}
```

Construction is deliberately left out of the trait: opening a D-Bus
connection (or whatever a future adapter needs) can fail in ways specific to
that adapter, so each one exposes its own fallible `new()`.

## New crate: `patin-service-upower`

`crates/patin-service-upower` implements `Provider` for `BatteryProvider`
against UPower's synthetic `DisplayDevice` — the aggregate device UPower
maintains specifically for status bars and shells, which avoids
reimplementing the sysfs version's "pick the best battery" logic. It reads
the `Percentage` and `State` D-Bus properties over `zbus`'s blocking API on
the existing once-per-second `Shell::update` tick; no calloop or
`platform.rs` changes were needed for this poll-based adapter.

`zbus = "=5.18.0"` is used with `default-features = false` and an explicit
feature list (`async-executor`, `async-fs`, `async-io`, `async-lock`,
`async-process`, `async-task`, `blocking`, `blocking-api`) — resolved by
trial build against the crate's actual feature graph, since `blocking-api`
alone does not transitively pull the executor it needs. This set excludes
`tokio`; `zbus` implements the D-Bus wire protocol itself, so no `libdbus`
system package is required.

A missing system bus, missing UPower service, or missing device all degrade
to `None` via `.ok()?` short-circuiting, the same failure philosophy as the
demo's sysfs and `wpctl`/`pactl` fixtures. This also means unit tests (run
without a real system bus reachable, or with one reachable but no UPower
registered on it) see `None` deterministically rather than failing.

## Demo integration

`examples/demo_bar/services.rs`'s `SystemStatus` now holds a
`BatteryProvider` field instead of a `power_supply_root` path; the sysfs
`read_battery` function and its formatting are gone, replaced by
`format_battery` over `BatterySnapshot`. `SystemStatus::poll` became
`&mut self` (both call sites were already in `&mut self` contexts). Volume
and brightness are unchanged. Root `Cargo.toml` depends on
`patin-service-upower` under `[dev-dependencies]`, since only the example
uses it.

## Verification

Verified on 29 July 2026:

```text
$ cargo fmt --all -- --check
(no output)

$ cargo test --workspace --all-targets
running 5 tests (patin)
... all ok
running 4 tests (examples/demo_bar)
... all ok
running 1 test (patin-service-upower)
test tests::poll_without_a_system_bus_returns_none ... ok

$ cargo clippy --workspace --all-targets --all-features -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s), no warnings

$ git diff --check
(no output, exit 0)
```

`cargo run --example demo_bar` (timeout 5s) in this sandbox, which has a
Wayland compositor and a session/system D-Bus socket but no `upowerd`
registered:

```text
demo_bar: status providers: battery=unavailable, volume=VOL MUTE, brightness=BRI 71%
patin: connected; waiting for the compositor to configure the bar
```

Confirmed independently with `dbus-send --system ... org.freedesktop.UPower
...` that the service is genuinely `ServiceUnknown` here, so `unavailable` is
the adapter degrading correctly, not a bug.

```text
$ mdbook build
 INFO Book building has started
 INFO Running the html backend
 INFO HTML book written to `/home/vdzee/proj/patin/book`
```

### FP5 end-to-end confirmation

Verified on the FP5 the same day (postmarketOS edge, `aarch64`, `upowerd`
genuinely active). The working tree was copied over with `tar` piped over
SSH (no `rsync` on-device), built natively with
`cargo build --release --locked --example demo_bar` (~4 minutes cold,
fetching `zbus` and its transitive dependencies), then installed and
launched via the existing `scripts/install-demo-user.sh`:

```text
demo_bar: status providers: battery=BAT 69%, volume=VOL 8%, brightness=BRI 70%
patin: connected; waiting for the compositor to configure the bar
```

A real percentage from D-Bus/UPower, not `unavailable` — the adapter works
end to end against the target device's actual UPower.

Backgrounding it as a plain `setsid nohup ... &` SSH command was not enough
to keep it alive: the phone's PAM session tears down its whole cgroup on SSH
disconnect regardless of `setsid`. It stayed running only once launched as a
transient systemd user unit instead:

```sh
systemd-run --user --unit=patin-demo --collect \
  --setenv=XDG_RUNTIME_DIR=/run/user/10000 \
  --setenv=WAYLAND_DISPLAY=wayland-0 \
  -- /home/sn3rt/.local/bin/patin
```
