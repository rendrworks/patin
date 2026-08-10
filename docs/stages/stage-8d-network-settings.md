# Stage 8d — Network Settings and Hotspot

## Why this stage exists

NetworkManager, not Phosh or a compositor, owns Linux connection profiles,
radio state, and shared-hotspot routing. Patin already displayed its state but
could not change it. This milestone adds a phone-oriented frontend without
moving system networking into Patin or 0xin.

## Toolkit and Linux mechanisms

Wayland advertises keyboard capability per seat, like pointer and touch. The
regular platform now creates those keyboards dynamically and translates xkb
press/repeat events into toolkit-owned `KeyInput`. Adaptive touch-key geometry,
drawing, shift, and symbol state moved from the lock to `patin::keyboard`.
The lock continues to own zeroized password storage, PAM, and session-lock
behavior.

`patin-service-network` now distinguishes hardware availability, radio
enablement, active signal, and hotspot state. D-Bus supplies live state;
NetworkManager's `nmcli` frontend handles scans and profile mutations, keeping
profile serialization and secrets inside NetworkManager. Errors, including
PolicyKit denials, are returned to the UI.

## Composition

`patin-network-settings` is a separate overlay process. Its Wi-Fi page provides
radio control, scan/connect/disconnect/forget, and one persistent `Patin
Hotspot` profile using AP mode and IPv4 sharing. SSID, password, open or
WPA-personal security, and automatic/2.4/5 GHz band are editable. Its Cellular
page provides the NetworkManager mobile-data toggle and registration state.

The demo bar retains dim Wi-Fi and cellular slots when the relevant runtime
capability exists. Tapping a slot launches `--page=wifi` or `--page=cellular`,
with at most one child per bar. `PATIN_NETWORK_SETTINGS_PROGRAM` can replace
the executable; no 0xin-specific code is involved.

Enterprise enrollment, IP/DNS/routes, APN/roaming/SIM editing, multiple
hotspots, and a PolicyKit agent are later work.

## Verification

Verified locally on 10 August 2026:

```text
$ cargo fmt --all -- --check
(no output, exit 0)

$ cargo test --workspace --all-targets
34 passed, 0 failed

$ cargo clippy --workspace --all-targets --all-features -- -D warnings
Finished, no warnings

$ mdbook build
INFO HTML book written to `/home/vdzee/proj/patin/book`

$ git diff --check
(no output, exit 0)
```

The sandbox has no Wayland compositor, NetworkManager service, or `nmcli`, so
the final phone acceptance pass—joining Wi-Fi, changing mobile data, enabling
the hotspot, joining it from another device, and confirming persistence after
a reboot—must be performed on the FP5 before treating device integration as
confirmed. Unit coverage verifies unavailable-service degradation, escaped
scan parsing, credential validation, page selection, keyboard behavior, and
bar page routing without changing host network state.
