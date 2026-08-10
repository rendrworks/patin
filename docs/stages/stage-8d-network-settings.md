# Stage 8d — Network Settings and Hotspot

## Why this stage exists

iwd, not Phosh or a compositor, owns Wi-Fi association and access-point mode.
ModemManager independently owns the cellular modem. Patin is a phone-oriented
frontend to those daemons; neither networking nor daemon policy moves into
Patin or 0xin.

## Toolkit and Linux mechanisms

Wayland advertises keyboard capability per seat, like pointer and touch. The
regular platform now creates those keyboards dynamically and translates xkb
press/repeat events into toolkit-owned `KeyInput`. Adaptive touch-key geometry,
drawing, shift, and symbol state moved from the lock to `patin::keyboard`.
The lock continues to own zeroized password storage, PAM, and session-lock
behavior.

`patin-service-network` distinguishes hardware availability, radio enablement,
active signal, and hotspot state by enumerating iwd's D-Bus object manager.
It calls iwd directly for scans, radio power, connection, disconnection,
forgetting known networks, device-mode changes, and AP start/stop. A temporary
credential agent answers iwd's passphrase request for a user-selected network.
ModemManager D-Bus supplies signal and modem power. There is no NetworkManager,
`nmcli`, or `iwctl` dependency.

## Composition

`patin-network-settings` is a separate overlay process. Its Wi-Fi page provides
radio control, scan/connect/disconnect/forget, and one WPA-personal hotspot.
Patin stores its SSID and password as hex-encoded UTF-8 in a mode-0600 user
configuration file, then supplies them to iwd's dynamic AP API. iwd selects the
channel automatically. Its Cellular page controls ModemManager modem power and
shows registration signal.

iwd must be configured with `EnableNetworkConfiguration=true`; it then owns
the WLAN's station DHCP client plus AP address and DHCP server. An example
`main.conf` fragment is provided. systemd-networkd may manage other links but
must leave the WLAN unmanaged. iwd does not install internet-sharing NAT, so
forwarding/masquerading remains host policy and is part of the FP5 acceptance
test rather than hidden in the graphical client.

The demo bar retains dim Wi-Fi and cellular slots when the relevant runtime
capability exists. Tapping a slot launches `--page=wifi` or `--page=cellular`,
with at most one child per bar. `PATIN_NETWORK_SETTINGS_PROGRAM` can replace
the executable; no 0xin-specific code is involved.

Enterprise enrollment, IP/DNS/routes, APN/roaming/SIM editing, multiple
hotspots, fixed AP bands, and automatic NAT policy are later work.

## Verification

Verified locally on 10 August 2026:

```text
$ cargo fmt --all -- --check
(no output, exit 0)

$ cargo test --workspace --all-targets
35 passed, 0 failed

$ cargo clippy --workspace --all-targets --all-features -- -D warnings
Finished, no warnings

$ mdbook build
INFO HTML book written to `/home/vdzee/proj/patin/book`

$ git diff --check
(no output, exit 0)
```

The sandbox has no Wayland compositor, iwd, or ModemManager service, so
the final phone acceptance pass—joining Wi-Fi, toggling modem power, enabling
the hotspot, joining it from another device, and confirming persistence after
a reboot—must be performed on the FP5 before treating device integration as
confirmed. Unit coverage verifies unavailable-service degradation, iwd signal
mapping, hotspot credential/storage behavior, page selection, keyboard
behavior, and bar page routing without changing host network state.
