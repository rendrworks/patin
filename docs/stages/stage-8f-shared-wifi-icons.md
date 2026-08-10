# Stage 8f — Shared Wi-Fi Icons and Saved Availability

## Why this stage exists

The demo bar originally owned its Wi-Fi glyph, which prevented network
settings from presenting the same signal language without copying code. The
settings model also dropped saved networks whenever they were absent from
NetworkManager's current access-point cache. A saved profile and a currently
visible radio network are different facts and should remain tracked as such.

## Crate and data boundaries

`patin-icons` is a small, opt-in workspace crate. It depends only on the Patin
draw-command API and exposes a semantic `WifiSignal` with unavailable, poor,
medium, and good states. `wifi_signal` turns that state and a consumer-supplied
palette into vector commands. Patin core does not depend on the icon crate, and
the crate does not construct a bar or settings composition.

`patin-service-network::WifiNetwork` now carries two independent flags:
`known` means NetworkManager has a saved infrastructure profile, while
`available` means the SSID occurs in its cached access-point list. The provider
merges those sources, keeps saved-but-unavailable state internally, identifies
the active network, and excludes Wi-Fi profiles whose mode is `ap`. An explicit scan adds
visible unknown networks; normal refreshes reuse the current known set.
NetworkManager can retain stale access-point objects, so availability also
requires a D-Bus `LastSeen` timestamp no older than thirty seconds. The active
connection always remains available.

## Composition behavior

The demo bar consumes the shared icon for its connected signal. Its Wi-Fi slot
disappears entirely when the radio is off. Network settings uses poor, medium,
or good states beside currently available rows; saved but unavailable entries
remain in the refresh model without rendering a row or cross. The icon carries
signal strength without a redundant numeric percentage; text marks only the
active row as connected.

The Wi-Fi page refreshes availability every two one-second shell update ticks
and requests a lightweight background scan every ten ticks. This lets an
offline access point age out without continuously scanning the radio. The Scan
button still requests immediate discovery and reveals unknown results. Unknown
scan results do not receive a Forget action. Saved rows reserve an
82-logical-pixel centered Forget button, keeping its label and bounds inside
the row on the phone-sized layout.

## Changed files and important functions

- `crates/patin-icons/src/lib.rs`: `WifiSignal::from_percentage` defines the
  shared thresholds; `wifi_signal` draws the signal arcs.
- `examples/demo_bar/scene.rs`: `DemoBar::commands` now consumes
  `patin-icons`; `wifi_palette` keeps the bar's colors local.
- `crates/patin-service-network/src/lib.rs`: `merge_wifi_profiles` combines
  saved and cached state; `parse_wifi_profile` excludes AP-mode profiles;
  `access_point_is_recent` rejects stale cache objects;
  `request_wifi_scan` uses NetworkManager D-Bus without blocking on `nmcli`;
  `refresh_wifi_networks` updates the merged state.
- `crates/patin-network-settings/src/ui.rs`: `NetworkSettings::layout` places
  icons and fitting actions; `wifi_refresh_due` and `wifi_scan_due` schedule
  state refreshes and background discovery only on the Wi-Fi page.
- Workspace manifests opt the demo and settings composition into the new icon
  crate. README, architecture, roadmap, and earlier stage notes describe the
  resulting ownership and behavior.

## Verification

Local verification on 2026-08-10:

```text
cargo fmt --all -- --check
  no output
cargo check --workspace --all-targets
  finished successfully
cargo test --workspace --all-targets
  53 passed; 0 failed
cargo clippy --workspace --all-targets --all-features -- -D warnings
  finished successfully
mdbook build
  HTML book written to book/
git diff --check
  no output
```

The FP5 native release builds used the phone's existing project-local
xkbcommon development metadata:

```text
PKG_CONFIG_PATH=~/proj/0xin/.sysroot/usr/lib/pkgconfig \
  cargo build --release --locked -p patin-network-settings
  finished successfully
PKG_CONFIG_PATH=~/proj/0xin/.sysroot/usr/lib/pkgconfig \
  cargo build --release --locked --example demo_bar
  finished successfully
```

The settings binary was installed as `~/.local/bin/patin-network-settings` and
the demo bar as `~/.local/bin/patin`; each installed SHA-256 matched its native
release artifact. A fifteen-second settings smoke test connected to the live
0xin Wayland session, mapped without a NetworkManager/profile error, crossed
the automatic ten-second D-Bus scan boundary, and was then stopped explicitly.
The already-running bar process was not restarted remotely, so it will load the
shared-icon build on its next normal launch.
