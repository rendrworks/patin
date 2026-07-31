# Stage 8a — Touch Application Launcher

## Why this stage exists

0xin deliberately maps its top-edge gesture to an external command. Its phone
profile previously spawned Fuzzel on a downward swipe and killed it on an
upward swipe. That boundary means Patin can provide a replacement composition
without becoming mandatory, depending on 0xin, or moving application policy
into the compositor.

This first launcher stage is a useful touch target rather than a general app
launcher framework: it discovers applications, renders a compact floating
list, scrolls by pointer wheel or touch drag, starts one on tap, and exits.
Keyboard search remains follow-up work.

## Desktop entries and process launch

`patin-launcher` uses `freedesktop-desktop-entry` 0.8.1 with its optional
gettext feature disabled. `apps::discover` walks standard XDG application
locations, keeps the first occurrence of each desktop ID, applies localized
names and desktop visibility rules, skips hidden/non-application entries and
missing `TryExec` programs, then sorts names for a stable list.

`Application::launch` uses the library's desktop `Exec` parser instead of a
shell. The resulting first argument is passed directly to `Command` and the
remaining arguments remain distinct, so desktop field codes are handled
without introducing shell interpolation. A declared working directory is
honored. Successful spawn closes the launcher; failure remains visible in the
overlay so the user can choose another app or dismiss it.

## Floating layout, icons, scrolling, and lifecycle

The binary requests a `380×540` overlay-layer surface with no anchors, reserves
no exclusive zone, and requests no keyboard. Layer-shell compositors center an
unanchored fixed-size surface, so the launcher is a real floating window and
the rest of the output is neither covered nor dimmed. `ui::Launcher::layout`
places a single column of application rows inside the rounded dark surface.

Each row contains the localized application name and its desktop-entry icon.
The launcher searches standard XDG icon roots for PNG and SVG variants. It
decodes PNG through `image` 0.25.10's PNG-only feature and rasterizes SVG with
`resvg` 0.47.0 with all default features disabled; a neutral square is used
when no supported icon is available. The toolkit's `TextAlign::Start` provides
the left-aligned names, while `DrawCommand::Image` keeps decoded RGBA rendering
behind Patin's internal render boundary.

`Shell::scroll_by` is a defaulted vertical-scroll hook. The platform translates
pointer-axis values into it. Touch contacts now activate on release only when
they stayed within an eight-logical-pixel tap threshold; a drag instead emits
scroll deltas. `ui::Launcher::scroll_by` advances a clamped visible window and
the thin scrollbar reports its position. This avoids both page controls and
accidental launches while swiping.

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

- `crates/patin-launcher` owns desktop discovery/launch, XDG icon loading,
  scrolling list state, hit-testing, rendering, and the standalone overlay
  entrypoint.
- `src/ui.rs` and `src/render.rs` add start-aligned text and decoded RGBA image
  commands.
- `src/platform.rs` adds defaulted finite-composition and scroll hooks, pointer
  wheel translation, and tap-versus-drag touch handling.
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

Five launcher tests cover visible/hidden desktop entries, parsed launch
arguments, fixed-surface row containment, clamped scrolling, and clean close
requests. A short local smoke run found 12 launchable applications
before reporting the expected `Could not find wayland compositor`, because the
tool session does not inherit the laptop's graphical Wayland environment.

The phone-native five-test suite and optimized build passed for the final
`380×540` floating list. A timed live run discovered 33 applications, resolved
10 installed theme icons with neutral fallbacks for the remainder, and
connected to 0xin successfully:

```text
$ env -u LD_LIBRARY_PATH XDG_RUNTIME_DIR=/run/user/10000 \
  WAYLAND_DISPLAY=wayland-0 timeout 15 ~/.local/bin/patin-launcher
patin-launcher: discovered 33 launchable applications (10 resolved icons)
patin: connected; waiting for the compositor to configure the surface
```

Physical tap-to-launch and the final 0xin gesture switch remain to be recorded.
