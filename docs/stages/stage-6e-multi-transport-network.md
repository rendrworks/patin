# Stage 6e — Simultaneous Network Transports

> This chapter records the original NetworkManager implementation. Stage 8d
> replaced its Wi-Fi side with direct iwd integration.

## Why this stage exists

A single `PrimaryConnection` cannot describe a phone with registered SIM
service and wifi at the same time. VPN and loopback connections also must not
masquerade as physical signal indicators. This stage changes the optional
network adapter to report transport capabilities independently.

## Snapshot and service boundaries

`NetworkSnapshot` is now a struct with `wifi: Option<u8>`,
`cellular: Option<u8>`, and `wired: bool`. NetworkManager supplies active wifi
and ethernet state; the wifi device's active access point supplies signal
strength. ModemManager's standard ObjectManager discovers modem objects, and a
modem at least in the registered state supplies `SignalQuality`.

The two D-Bus services stay behind `NetworkProvider`. No device, interface,
modem path, compositor, or hardware name is encoded. Missing transports remain
absent fields in an available snapshot, while an unavailable system bus still
returns `None`.

## Demo composition

The demo no longer constructs `BacklightProvider`; that optional crate remains
available to toolkit consumers. Its right cluster now grows inward as wifi,
wired when present, cellular, and battery. Wifi uses concentric strength arcs,
while cellular retains ascending strength bars. Both icons can coexist and the
flexible center spacer remains clear.

## Verification

The local regression suite covers an empty transport snapshot, unavailable
system bus, simultaneous wifi/cellular layout, shape-only icons, and the
center-clearance invariant. The live phone test target reported a registered
LTE/5G modem with 57% recent signal through ModemManager while NetworkManager
reported an independent wifi connection.

Verified on 1 August 2026:

```text
$ cargo fmt --all -- --check
(no output, exit 0)

$ cargo test --workspace --all-targets
29 passed, 0 failed

$ cargo clippy --workspace --all-targets --all-features -- -D warnings
Finished, no warnings

$ mdbook build
INFO HTML book written to `/home/vdzee/proj/patin/book`

$ git diff --check
(no output, exit 0)
```

The matching source files were copied to the `aarch64` phone test tree and
their SHA-256 hashes matched the local files. Its native locked release build
completed in 11.50 seconds. After installing only the demo executable and
restarting only its transient user service, the live provider reported:

```text
NetworkSnapshot { wifi: Some(70), cellular: Some(59), wired: false }
$ systemctl --user is-active patin-bar.service
active
```

The running composition therefore exercised simultaneous NetworkManager wifi
and ModemManager cellular signal without a compositor restart.
