# Stage 8a — Touch Application Launcher

## Why this stage exists

0xin deliberately maps its top-edge gesture to an external command. Its phone
profile previously spawned Fuzzel on a downward swipe and killed it on an
upward swipe. That boundary means Patin can provide a replacement composition
without becoming mandatory, depending on 0xin, or moving application policy
into the compositor.

This first launcher stage is a useful touch target rather than a general app
launcher framework: it discovers applications, renders a simple paged list,
starts one on tap, and exits. Search and touch scrolling remain follow-up work.

## Desktop entries and process launch

`patin-launcher` uses `freedesktop-desktop-entry` 0.8.1 with its optional
gettext feature disabled. `apps::discover` walks standard XDG application
locations, keeps the first occurrence of each desktop ID, applies localized
names and desktop visibility rules, skips hidden/non-application entries and
missing `TryExec` programs, then sorts names for stable pages.

`Application::launch` uses the library's desktop `Exec` parser instead of a
shell. The resulting first argument is passed directly to `Command` and the
remaining arguments remain distinct, so desktop field codes are handled
without introducing shell interpolation. A declared working directory is
honored. Successful spawn closes the launcher; failure remains visible in the
overlay so the user can choose another app or dismiss it.

## Overlay layout and lifecycle

The binary requests a full-output overlay layer, anchors all four edges,
reserves no exclusive zone, and requests no keyboard. `ui::Launcher::layout`
places one compact column of fixed-height application lines directly beneath
each other. Excess applications become explicit pages controlled by the left
and right halves of a plain text footer. The list uses the available width on
narrow outputs and caps itself at 640 logical pixels on wider outputs.

The result deliberately resembles a terminal running `fzf`: near-black
background, compact monospace names, and no cards, row backgrounds, icons,
title bar, or close button. The toolkit's `TextAlign` gains a general `Start`
option, rendered as left alignment for the current left-to-right text path.

The toolkit gains one general lifecycle hook: `Shell::close_requested` defaults
to `false`, preserving every existing consumer. The platform checks it after
updates and activations and exits its Calloop loop when a finite composition
returns `true`. The launcher requests this after a successful spawn or a tap on
empty background.

The shared startup diagnostic now says it is waiting for the compositor to
configure a `surface` rather than a `bar`, since the same platform path also
runs overlays.

## Installation and compositor mapping

Install the standalone user binary:

```sh
./scripts/install-launcher-user.sh
patin-launcher
```

0xin can replace only its swipe launcher while leaving other Fuzzel uses alone:

```ini
gesture = top-down, spawn, pgrep -x patin-launcher >/dev/null || patin-launcher
gesture = to-top, spawn, pkill -x patin-launcher
```

These are ordinary spawn mappings. Patin contains no 0xin branch, and another
layer-shell compositor may bind the same executable however it prefers.

## Changed files and important functions

- `crates/patin-launcher` owns desktop discovery/launch, paged-list state,
  hit-testing, rendering, and the standalone overlay entrypoint.
- `src/ui.rs` and `src/render.rs` add and render start-aligned text for simple
  list rows.
- `src/platform.rs` adds the defaulted finite-composition lifecycle hook.
- `scripts/install-launcher-user.sh` builds the locked release package and
  installs only its executable under `~/.local/bin`.
- The workspace manifest and lockfile pin the new consumer and parser; README,
  architecture, environment, and mdBook navigation document the boundary.

## Verification

Verified on 31 July 2026:

```text
$ cargo fmt --all -- --check
(no output, exit 0)

$ cargo test --workspace --all-targets
all passed

$ cargo clippy --workspace --all-targets --all-features -- -D warnings
Finished, no warnings

$ mdbook build
INFO HTML book written to `/home/vdzee/proj/patin/book`

$ git diff --check
(no output, exit 0)
```

Six launcher tests cover visible/hidden desktop entries, parsed launch
arguments, phone-sized pagination and output containment, centered and
width-limited desktop layout, page navigation, and clean close requests. A
short local smoke run found 12 launchable applications
before reporting the expected `Could not find wayland compositor`, because the
tool session does not inherit the laptop's graphical Wayland environment.

The phone-native six-test suite and release build also passed. A timed live
run of the terminal-style list discovered 33 applications and connected to
0xin successfully:

```text
$ env -u LD_LIBRARY_PATH XDG_RUNTIME_DIR=/run/user/10000 \
  WAYLAND_DISPLAY=wayland-0 timeout 10 ~/.local/bin/patin-launcher
patin-launcher: discovered 33 launchable applications
patin: connected; waiting for the compositor to configure the surface
```

Physical tap-to-launch and the final 0xin gesture switch remain to be recorded.
