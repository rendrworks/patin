# Stage 7c — Adaptive Lock-Screen Input

## Why this stage exists

The first lock keyboards divided all remaining vertical space between four
rows. That made keys grow with the output instead of remaining recognisable
controls: the numeric layout looked like a grid of large panels, and the full
keyboard became especially tall. Rounded corners alone could not correct the
underlying geometry.

This stage gives `patin-lock` intrinsic keyboard dimensions. The composition
is still tested on a phone, but it contains no hardware or compositor checks.
It derives every position from the output's logical width and height, caps
keyboard width on large outputs, and reduces key size when space is limited.

## Visual bounds and touch bounds

Each internal `KeyLayout` contains two rectangles:

- `visual_bounds` is the compact rounded shape that gets drawn.
- `hit_bounds` is the logical region accepted by pointer and touch hit-testing.

Keeping those concepts separate lets the keyboard have visible gaps and a
lighter silhouette without making it unnecessarily difficult to tap.
`keyboard_numeric` builds a centered 3-by-4 group of 44–72 logical pixel
squircles. `keyboard_full` builds four lower-screen rows at 44–52 logical
pixels high and limits their combined width to 720 logical pixels. A shared
adaptive bottom margin raises either keyboard by 11% of the output height,
clamped to 48–112 logical pixels. Both functions preserve the established
QWERTY, symbol, Shift, Space, Backspace, and submit behavior.

`key_colors` assigns visual roles without changing input semantics. Character
keys use Patin's violet surface color, modifiers are quieter, Backspace has a
subtle warm tint, and the `unlock` key carries the accent color. Shift gets an
active treatment, and every key is dimmed while PAM authentication is in
progress. The submit key uses a compact checkmark rather than a word label, so
it remains legible inside both the numeric squircle and the narrower full
keyboard key.

## Password-field state

`LockUi::password_field_text` keeps the hint separate from authentication
status:

- An empty, idle field displays `Enter password`.
- The first entered character replaces the hint with masking bullets.
- Submission immediately clears the UI-owned secret and hides the hint while
  PAM verifies it.
- `Verifying…` and failure messages remain below the field.
- Editing after a failure clears the stale failure message.

The password field itself is a layered rounded rectangle. The extra outer
layer gives it a visible boundary using existing `DrawCommand::RoundedFill`
primitives, so this composition does not require a new public toolkit API.

## Changed files and important functions

- `crates/patin-lock/src/ui.rs` owns the new `KeyLayout`, adaptive keyboard
  geometry, role colors, field presentation, hit-testing, and regression
  tests.
- `README.md` describes the user-visible keyboard and placeholder behavior.
- `docs/environment.md` names xkbcommon's development files as a lock-build
  requirement.
- `docs/SUMMARY.md` links this stage from the mdBook navigation.
- This chapter records the design and verification result.

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

The automated layout tests cover 320×500, 509×1020, and 1920×1080 logical
outputs, both keyboard modes, centering, output containment, maximum width,
minimum row height, placeholder transitions, authentication status, and
touch hit-testing.

The source was also copied to the existing aarch64 phone test checkout. Its
system still had the xkbcommon runtime but no development symlink or
`xkbcommon.pc`, so `libxkbcommon-dev` was unpacked into a temporary directory
for the build rather than reinstalling any system or desktop packages:

```text
$ PKG_CONFIG_PATH=/tmp/tmp.CamNaD/usr/lib/pkgconfig \
  RUSTFLAGS="-L native=/tmp/tmp.CamNaD/usr/lib" \
  cargo test -p patin-lock
6 passed

$ PKG_CONFIG_PATH=/tmp/tmp.CamNaD/usr/lib/pkgconfig \
  RUSTFLAGS="-L native=/tmp/tmp.CamNaD/usr/lib" \
  cargo build --release --locked -p patin-lock
Finished release profile

$ install -m 0755 target/release/patin-lock ~/.local/bin/patin-lock
(exit 0)
```

The updated binary is installed and resolves `libxkbcommon.so.0` from the
phone's normal `/lib`. The visual and physical-touch check remains to be
recorded because acquiring a session lock remotely without someone ready to
unlock it is unsafe. Keep SSH recovery available and exercise both
`patin-lock` and `patin-lock --keypad=numeric`; the live check must cover failed
authentication, retry, successful unlock, and the physical power-button wake
path.
