# Stage 6d — Vector Status Icons

## Why this stage exists

The first demo bar proved its service adapters by printing values such as
`BAT 64%`, `VOL 8%`, and `NET 69%`. Those labels were useful diagnostics, but
they made a 32-logical-pixel shell bar feel like a test fixture rather than a
compact status surface.

This stage originally changed only the demo composition. Stage 8f later moved
the Wi-Fi glyph into the optional `patin-icons` crate so multiple consumers can
reuse it. Patin still does not ship a bar, and service adapters still return
data rather than presentation.

## Structured state through the scene

`examples/demo_bar/services.rs::StatusSnapshot` now retains
`BatterySnapshot`, `VolumeSnapshot`, and `NetworkSnapshot` directly. Removing
the demo's formatting functions avoids
discarding useful state such as charging, mute, and transport strengths before
the scene renders it.

`examples/demo_bar/scene.rs` converts that state into compact icons:

- battery fill represents charge, with warning and charging colors;
- zero through three bars represent volume, with a mute strike;
- concentric signal arcs represent wifi, linked nodes represent wired, and
  ascending bars represent cellular strength.

The helpers use only existing `DrawCommand::Fill` and
`DrawCommand::RoundedFill` primitives. This makes the icons scale with the
Wayland surface and avoids emoji rendering, private-use glyphs, bundled assets,
or an icon-font dependency. The clock deliberately remains text.

The demo row keeps the textual clock and optional volume icon in fixed slots
growing inward from the inset left edge. Active wifi, wired, cellular, and
battery indicators use fixed slots growing inward from the inset right edge. A flexible spacer between
the clusters keeps the output center empty, avoiding centered obstructions
without branching on a hardware or compositor name. The 12-logical-pixel outer
inset is scale independent: the Wayland backend applies the output scale later
when it creates the buffer. A value change alters its icon commands and damages
only that status slot.

## Changed files and important functions

- `examples/demo_bar/services.rs` preserves provider snapshots instead of
  converting them to labels.
- `examples/demo_bar/scene.rs` renders the battery, volume, wired, and cellular
  glyphs locally. Its original Wi-Fi helper moved to `patin-icons` in Stage 8f.
- `README.md` and `docs/status-services.md` describe the visible behavior and
  retain the toolkit/example boundary.
- `docs/SUMMARY.md` links this chapter.

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

The automated demo test confirms that all four status components emit shape
commands rather than text and that battery charging and volume mute produce
different command sets. A live visual check on the phone test target remains
to be recorded.

The inset end layout was verified on 1 August 2026:

```text
$ cargo fmt --all -- --check
(no output, exit 0)

$ cargo test --workspace --all-targets
28 passed, 0 failed

$ cargo clippy --workspace --all-targets --all-features -- -D warnings
Finished, no warnings

$ mdbook build
INFO HTML book written to `/home/vdzee/proj/patin/book`

$ git diff --check
(no output, exit 0)
```

The new regression test uses the phone's 509-by-32 logical bar size and checks
that the clock starts at the left inset, the battery slot ends at the right
inset, and a 64-logical-pixel area around the output center contains no status
slot.

The same revision was then built natively on the `aarch64` phone test target
with its existing xkbcommon development path:

```text
$ cargo build --release --locked --example demo_bar
Finished `release` profile; produced target/release/examples/demo_bar

$ systemctl --user is-active patin-bar.service
active
```

The installed `~/.local/bin/patin` remained active as a Wayland client after a
fresh SSH connection. The transient user service was used only to preserve the
live test process after SSH disconnected; normal session startup remains the
documented `exec_once = ~/.local/bin/patin` compositor configuration.
