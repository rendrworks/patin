# Stage 7b — Selectable Keypad and Idle Blank/Wake

## Why this stage exists

Stage 7 shipped `patin-lock` with a single fixed QWERTY/symbol keyboard and a
display that stays on for as long as the session is locked. Two gaps followed
from real use on the FP5: some accounts use a numeric PIN rather than an
alphanumeric password, and an always-on lock screen wastes battery and is a
privacy signal in itself (anyone can see the clock/username glowing in a
pocket or on a table). Both are consumer-level lock-composition choices, not
toolkit behavior, so both live entirely in `crates/patin-lock`.

## Selectable keypad

`ui::KeyboardMode` (`Full` or `Numeric`) is chosen once, at startup, via
`--keypad=full|numeric` (default `full`, so existing installs are unaffected),
or `PATIN_LOCK_KEYPAD=full|numeric` as a persistent default matching the
existing `PATIN_TRACE` env-var convention — an explicit `--keypad=` flag
always wins over the env var. The supervisor forwards whichever `--keypad=`
flag it was given to its `--worker` child exactly like it already does with
`--worker` itself; the env var needs no forwarding since child processes
inherit it automatically. `Numeric` renders a fixed 3x4 digit grid (1-9,
backspace, 0, enter) instead of the QWERTY/symbol pages; `Full` is the
unchanged stage 7 keyboard. Both modes feed the same
`LockUi::press`/`take_password` path — PAM checks whatever the account's real
password is, so a numeric keypad only makes sense for accounts whose password
is itself a PIN.

## Idle blank and power-button wake

`patin-lock` binds `zwlr_output_power_manager_v1` if the compositor
advertises it (logged and skipped otherwise — the lock still works, it just
never blanks). After a period without a real key/touch/pointer press, it
calls `set_mode(Off)` on every output's power object and stops drawing; this
is a real DPMS-off, not a rendered black frame. The idle threshold is 1
second before the display has ever been woken (`App::ever_woken`), and 5
seconds from the moment of any wake onward, reset by each keystroke exactly
like the 1-second case. Two earlier passes were both too aggressive: an
unconditional 1 second blanked the screen mid-entry on any pause longer than
a second between digits, and switching to 5 seconds only once the password
buffer already had a character in it still left just 1 second between waking
the display and typing the first digit — `ever_woken` fixes that by starting
the 5-second grace at the wake itself, not at the first keystroke.
Ordinary touch, pointer, and keyboard input are ignored while blanked rather
than treated as a wake — a phone in a pocket brushes its screen constantly,
and waking on any of that would defeat the point of blanking it.

Two independent triggers toggle the blank state instead (off if currently
on, back on if currently off), so a single press or signal always does the
right thing and responds within roughly one event-loop tick:

- A `SIGUSR1` sent to the `--worker` process, checked once per dispatch loop
  iteration. Useful for scripted/SSH testing, e.g.:
  ```sh
  pkill -USR1 -f -- '--worker'
  ```
  (`pkill -f` matches on the full command line, so this only ever reaches the
  worker — its argv contains `--worker`, the supervisor's does not.)
- The physical power button (`XF86PowerOff`), handled directly in
  `press_key` as an ordinary keyboard event. This needed no compositor
  config at all, once we understood why an earlier attempt (a compositor
  keybind spawning the `SIGUSR1` command above) didn't work: 0xin's
  `handle_keybinding` short-circuits with `if server.locked { return false; }`
  before checking any bind table, deliberately forwarding every key straight
  to the locked client instead of running its own keybinds — so keybinds
  can never be used to bypass a lock. That forwarding is exactly what
  delivers `XF86PowerOff` to `patin-lock` as a normal `wl_keyboard` event
  while locked, so it's handled there directly instead of round-tripping
  through an external signal. Any compositor that forwards keys to the lock
  client the same way (which `ext-session-lock-v1` more or less implies) gets
  this for free, with zero configuration.

## Verification

Verified on 30 July 2026:

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

Manual, on the FP5 (physical touch and the physical power button can't be
driven over SSH, so this needs hands on the device):

- `--keypad=full` and `--keypad=numeric` both unlock correctly.
- The display visibly powers off after 1 second idle.
- `ssh fp5 "pkill -USR1 -f -- '--worker'"` sent directly to an already-locked
  worker wakes it and redraws the lock UI — confirmed live, which is what
  surfaced the `server.locked` keybind-gating behavior above (a compositor
  bind wired to the same command never fired on a physical power press,
  while the identical command sent directly over SSH worked every time).

Still to confirm live: the direct `XF86_PowerOff` keysym handling in
`press_key`, and the 5-second entering-password threshold, are both new code
added after the findings above and have not yet been exercised on the
device (only the unconditional 1-second timeout and the `SIGUSR1` path have).
