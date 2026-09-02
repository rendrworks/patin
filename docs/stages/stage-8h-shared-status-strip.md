# Stage 8h — Shared Status Strip on the Lock Screen

## Why this stage exists

The greeter grew a status strip in Stage 8g's wake: the time, the battery, and
whether a radio is actually up, drawn from `patin-icons` and the provider
crates. The lock screen — the surface a phone user sees many times a day,
against the greeter's once per boot — still showed only a clock, a username,
and a password field. Someone picking up a locked phone wants the same facts:
how much charge is left, whether Wi-Fi dropped, whether the SIM registered.

The greeter's implementation was self-contained, and the demo bar already held
a second copy of the same provider-to-icon mapping. Adding a third to the lock
would have meant three places to fix a visibility rule. So the strip moved into
an opt-in workspace crate that both the greeter and the lock consume.

## What the crate owns, and what it does not

`patin-status` owns the strip: a clock on the left at a fixed inset, icons laid
out from the right edge inwards so the battery keeps the corner however many
radios are present, and the polling that feeds them. It sits one level above
`patin-icons`, which still owns every glyph and no layout at all.

It does not own colour. A composition passes an `IconPalette`, and the
palette's `background` must be the fill drawn behind the strip — several glyphs
punch holes in that colour rather than being transparent, so a mismatch shows
up as a halo around the Wi-Fi arcs and inside the battery.

Two parts are optional, because the consumers genuinely differ:

- **The clock.** The greeter has no other one and keeps it. The lock already
  draws a 64pt clock of its own and asks for icons only, so it does not get a
  second, smaller time in the corner.
- **The volume icon.** The lock shows it; the greeter does not. `with_volume`
  gates construction of the provider, not just its drawing, because polling
  volume spawns `wpctl` or `pactl` where every other provider reads D-Bus.
  A composition that does not show volume must not pay for it — the same rule
  `AGENTS.md` states for compositions generally.

The demo bar deliberately did not move. It lays its row out with
`patin::ui::row`, tracks per-icon damage rectangles, and makes its Wi-Fi and
cellular icons hit targets that launch network settings. Generalizing the strip
to cover that would produce something neither consumer wants.

## Refresh, and why it is measured in seconds

The greeter's original strip counted ticks: ten of them, at its 200ms poll
interval, for a refresh every two seconds. That constant is only correct for
one consumer. `patin-lock` drives its own Calloop loop with a 50ms dispatch
timeout, so the same tick count would have refreshed four times as fast and
quadrupled the D-Bus load.

The shared crate measures a `Duration` against `Instant::now()` instead, so a
consumer's tick rate and the provider load are independent. Construction still
performs no I/O: the first D-Bus round trip would delay the first frame, and on
a machine with no reachable system bus it would delay it for nothing.

## Blanking

The lock powers its outputs off after one second of inactivity, or five once
the user has started typing. `draw_pending` already skipped a blanked screen,
but the poll behind it would not have: without a gate the lock would have hit
D-Bus and spawned `wpctl` every two seconds at a display nobody can see. The
refresh is therefore conditional on `!blanked`, and waking already redraws
every view, so the strip is current the moment the screen comes back.

## Layout collision

The lock's big clock sits at `height * 0.09`, which is 32.4 on a 360px output —
inside the strip, whose bottom edge is at 42.0. `clock_top` clamps it the same
way the greeter's `header_top` clamps its hostname, and a test walks a range of
real output heights. The lock's own colour literals became named constants in
the same change, so `BACKGROUND` has one definition shared by the background
fill and the strip palette rather than two that could drift.

## Verification

Verified on 25 August 2026:

```text
$ cargo fmt --all -- --check
(no output, exit 0)

$ cargo test --workspace --all-targets
117 tests across 17 crates, all passed

$ cargo clippy --workspace --all-targets --all-features -- -D warnings
Finished `dev` profile, no warnings

$ mdbook build
INFO Book building has started
INFO Running the html backend
INFO HTML book written to `/home/vdzee/proj/patin/book`
```

The greeter's four strip tests moved into `patin-status` and were joined by
four more covering the clock and volume switches; `patin-lock` gained the
clock-collision test. The greeter's own suite went from 33 to 29 tests
accordingly, and its appearance is unchanged — the palette it passes is the one
its module previously hardcoded.

Run the greeter's preview mode to confirm that no-op without greetd, then the
lock on a device with a real battery and modem:

```sh
patin-login          # preview: renders, reports sign-in unavailable
patin-lock           # icons top-right, battery in the corner, one clock
```
