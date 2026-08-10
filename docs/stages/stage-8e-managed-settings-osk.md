# Stage 8e — Managed Settings and System OSK

## Why this stage exists

A lock-screen keyboard and a session keyboard have different trust and
composition boundaries. The lock edits one private authentication secret and
must work while ordinary clients are inaccessible. A session OSK instead types
into whichever normal application owns keyboard focus. Sharing the lock's
embedded layout with network settings blurred those roles.

Network settings also behaves like an application: users expect the compositor
to tile, move, resize, focus, and close it. A full-output layer-shell overlay
prevented those normal window-management operations.

## Wayland mechanisms

The platform now supports two explicit surface roles. Existing shell
compositions use `LayerConfig` and `run`; normal settings use `WindowConfig`
and `run_window`, backed by `xdg_toplevel`. Rendering, fractional scaling,
damage, pointer, touch, physical keyboard, and seat discovery remain shared.

`Shell::text_input` exposes either normal, password, or inactive intent. When
the compositor advertises `zwp_text_input_manager_v3`, Patin enables a text
input for the focused surface, supplies the content purpose, and disables it
when editing ends. Missing protocol support is non-fatal: injected
`wl_keyboard` events and physical keyboards continue to work.

The adaptive touch keyboard is private to `patin-lock` again. Network settings
contains no OSK layout or provider command. This keeps Patin usable without
0xin and allows wvkbd today—or a future `patin-keyboard`—to satisfy the same
session-level request.

## Composition behavior

`patin-network-settings` is an XDG toplevel with app ID
`patin-network-settings`. Wi-Fi, Cellular, and Hotspot are independent tabs;
the optional `--page=` argument can open any one directly. Selecting a Wi-Fi
password or a hotspot SSID/password field advertises the matching text-input
purpose and visibly marks the edited value. Enter submits, Escape cancels, and
page changes or close requests end text input.

Launcher search is deliberately not part of this stage. It will become the
next consumer of the same window/text-input boundary without importing lock
keyboard code.

## Verification

Local verification on 2026-08-10:

```text
cargo fmt --all -- --check
cargo test --workspace --all-targets
  36 passed; 0 failed
cargo clippy --workspace --all-targets --all-features -- -D warnings
  finished successfully
mdbook build
  HTML book written to book/
git diff --check
  no output
```

The FP5 native release build used the phone's existing project-local
xkbcommon development metadata:

```text
PKG_CONFIG_PATH=~/proj/0xin/.sysroot/usr/lib/pkgconfig \
  cargo build --release --locked -p patin-network-settings -p patin-lock
  finished successfully
```

Both binaries were installed in `~/.local/bin`. The installed network settings
client connected to the running 0xin session and remained alive as an XDG
toplevel until the five-second smoke-test timeout. Against an isolated build of
the updated 0xin on the same FP5, its Wayland trace discovered and bound both
`xdg_wm_base` version 6 and `zwp_text_input_manager_v3` version 1.

The running graphical login deliberately was not terminated remotely. The
tap-to-show, typing, and edit-end-to-hide acceptance sequence therefore remains
to be checked after the next normal logout/login loads the rebuilt compositor.
