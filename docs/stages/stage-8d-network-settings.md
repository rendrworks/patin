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
drawing, shift, and symbol state initially moved from the lock to
`patin::keyboard`. Stage 8e later corrected that boundary: the special lock
keyboard returned to the lock, while normal settings fields use the session
OSK.

`patin-service-network` now distinguishes hardware availability, radio
enablement, active signal, and hotspot state. D-Bus supplies live state;
NetworkManager's `nmcli` frontend handles scans and profile mutations, keeping
profile serialization and secrets inside NetworkManager. Errors, including
PolicyKit denials, are returned to the UI.

## Composition

`patin-network-settings` was initially a separate overlay process. Its Wi-Fi
page provides radio control and scan/connect/disconnect/forget. Cellular has a
separate page for the NetworkManager mobile-data toggle and registration
state. Hotspot also has its own page for one persistent `Patin Hotspot`
profile using AP mode and IPv4 sharing; SSID, password, open or WPA-personal
security, and automatic/2.4/5 GHz band are editable. Stage 8e later changed
the process into a managed XDG toplevel without coupling it to 0xin.

The demo bar retains dim Wi-Fi and cellular slots when the relevant runtime
capability exists. Tapping a slot launches `--page=wifi` or `--page=cellular`,
with at most one child per bar. `PATIN_NETWORK_SETTINGS_PROGRAM` can replace
the executable; `--page=hotspot` is available to other launchers and all three
pages remain reachable through tabs. No 0xin-specific code is involved.

Window construction does not perform NetworkManager or `nmcli` discovery.
The first regular shell update loads the status snapshot, hotspot profile, and
the existing NetworkManager Wi-Fi cache after the compositor has had a chance
to configure and draw the XDG window. The cache is filtered to the connected
network and saved profiles that are currently visible; saved profiles that are
out of range and unknown nearby networks are not presented as available.

The Wi-Fi page's explicit `Scan for new networks` action schedules
`nmcli --rescan yes` for the next shell update. Its label changes to a scanning
state before the synchronous operation starts, and successful results replace
the filtered list with every discovered network. Thus a radio scan occurs only
after user intent and never forms part of perceived launch time.

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

Unit coverage verifies unavailable-service degradation, escaped scan parsing,
credential validation, page selection, keyboard behavior, and bar page routing
without changing host network state.

## FP5 acceptance

The NetworkManager-backed release targets were built natively and installed on
the postmarketOS FP5. The demo installer was corrected to build `demo_bar` from
the root `patin` package and `patin-network-settings` from its own package.
Before replacement, both previously installed binaries were copied to
`~/.local/share/patin/backups/pre-networkmanager-rollback/`.

```text
$ cargo build --release --locked --example demo_bar -p patin
Finished `release` profile [optimized]

$ cargo build --release --locked -p patin-network-settings
Finished `release` profile [optimized]

$ timeout 5 patin-network-settings --page=wifi
patin: connected; waiting for the compositor to configure the surface
Terminated (expected smoke-test SIGTERM)

$ systemctl is-enabled NetworkManager && systemctl is-active NetworkManager
enabled
active

$ nmcli -t -f NAME,TYPE connection show --active
Corner:802-11-wireless
Business:gsm
lo:loopback
wt0:wireguard
```

The activated bar connected to 0xin and reported Wi-Fi at 66%, cellular at
57%, both radios available and enabled, and no active hotspot. Deployment did
not alter either active connection. Mobile-data toggling, hotspot activation,
joining the hotspot from another device, and reboot persistence remain
deliberately pending because those tests disrupt the phone's live connectivity.
