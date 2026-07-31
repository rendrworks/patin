# Stage 6d — Vector Status Icons

## Why this stage exists

The first demo bar proved its service adapters by printing values such as
`BAT 64%`, `VOL 8%`, and `NET 69%`. Those labels were useful diagnostics, but
they made a 32-logical-pixel shell bar feel like a test fixture rather than a
compact status surface.

This stage changes only the demo composition. Patin still does not ship a bar
or prescribe icons, and the optional service adapters still return reusable
data rather than presentation.

## Structured state through the scene

`examples/demo_bar/services.rs::StatusSnapshot` now retains
`BatterySnapshot`, `VolumeSnapshot`, `BrightnessSnapshot`, and
`NetworkSnapshot` directly. Removing the demo's formatting functions avoids
discarding useful state such as charging, mute, wired, and disconnected before
the scene renders it.

`examples/demo_bar/scene.rs` converts that state into four compact icons:

- battery fill represents charge, with warning and charging colors;
- zero through three bars represent volume, with a mute strike;
- the center of a four-ray sun represents brightness;
- strength bars, linked nodes, dim bars, or a ring represent wifi, wired,
  disconnected, or other networking.

The helpers use only existing `DrawCommand::Fill` and
`DrawCommand::RoundedFill` primitives. This makes the icons scale with the
Wayland surface and avoids emoji rendering, private-use glyphs, bundled assets,
or an icon-font dependency. The clock deliberately remains text.

The existing layout, optional membership, and per-component damage behavior
are unchanged. A value change alters its icon commands and damages only that
status slot.

## Changed files and important functions

- `examples/demo_bar/services.rs` preserves provider snapshots instead of
  converting them to labels.
- `examples/demo_bar/scene.rs` renders `battery_icon`, `volume_icon`,
  `brightness_icon`, and `network_icon`, with shared centering and shape
  helpers plus state-regression tests.
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
