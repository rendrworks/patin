# Stage 8b — Session Action Menu

## Why this stage exists

The tested phone profile maps a two-second power-button hold to an external
`0xin-session-menu` script. That script previously piped four text choices into
Fuzzel's dmenu mode. Session policy already lived outside the compositor, so a
separate Patin composition can replace only its visible menu without coupling
the toolkit to 0xin.

`patin-session` is an optional consumer and not toolkit startup behavior. It
does not construct the launcher, lock screen, bar, or phone-only modules.

## Actions and compositor boundary

`actions::configured` always provides `systemctl reboot` and `systemctl
poweroff`. Logout is compositor-specific and appears only when the launching
integration sets `PATIN_SESSION_LOGOUT_PROGRAM`. An optional argument and label
come from `PATIN_SESSION_LOGOUT_ARGUMENT` and `PATIN_SESSION_LOGOUT_LABEL`.

`Action::launch` passes the program and arguments directly to `Command`; it
does not invoke a shell. For the tested 0xin session, the existing wrapper uses:

```sh
export PATIN_SESSION_LOGOUT_PROGRAM="$HOME/.local/bin/0xinctl"
export PATIN_SESSION_LOGOUT_ARGUMENT=quit
export PATIN_SESSION_LOGOUT_LABEL="Log out to Phrog"
exec "$HOME/.local/bin/patin-session"
```

The existing hold mapping can remain:

```ini
hold = , XF86PowerOff, 2000, spawn, ~/.local/bin/0xin-session-menu
```

Another compositor can supply its own logout command or omit that row.

## Floating panel and outside dismissal

The binary creates an overlay-layer surface anchored to the complete output,
with no exclusive zone and no keyboard request. The buffer remains transparent
except for a centered deep-purple panel. With all three actions configured,
`SessionMenu::layout` makes that panel `240×144` logical pixels and lays out
three compact rows: Log out to Phrog, Reboot, and Shut down.

The transparent area stays in the Wayland surface's default input region. A tap
there calls `SessionMenu::activate_at`, finds no action row, and requests clean
exit. The tap is consumed, so it does not activate the application underneath.
There is deliberately no Cancel row.

If spawning an action fails, the menu stays open, logs the error, and adds a
small error line to the panel. A successful spawn closes the menu immediately.

## Changed files and important functions

- `crates/patin-session/src/actions.rs` owns configured action policy and direct
  process spawning.
- `crates/patin-session/src/ui.rs` owns centered layout, hit-testing, rendering,
  outside dismissal, and finite lifecycle state.
- `crates/patin-session/src/main.rs` configures the transparent full-output
  layer surface and starts the composition.
- `scripts/install-session-user.sh` installs only the standalone user binary.
- Workspace, README, architecture, environment, and mdBook navigation changes
  document the new optional consumer.

## Verification

Local verification on 1 August 2026:

```text
$ cargo test -p patin-session --offline
3 passed

$ cargo clippy -p patin-session --all-targets --offline -- -D warnings
Finished, no warnings
```

Full verification also passed:

```text
$ cargo fmt --all -- --check
(no output, exit 0)

$ cargo test --workspace --all-targets --offline
all passed

$ cargo clippy --workspace --all-targets --all-features --offline -- -D warnings
Finished, no warnings

$ mdbook build
INFO HTML book written to `/home/vdzee/proj/patin/book`

$ git diff --check
(no output, exit 0)
```

The phone-native three-test suite and optimized build passed. A safe live test
placed a harmless `systemctl` shim first in `PATH` and used `/bin/true` for the
logout row. The menu connected to 0xin and exited before its 30-second timeout
when the transparent area was tapped. No logout, reboot, or power-off action was
executed during verification.

The installed `~/.local/bin/0xin-session-menu` now exports the tested 0xin
logout program, argument, and label before executing `patin-session`. The
existing two-second power-button hold mapping was left unchanged.
