# Stage 8i — Lua Configuration

## Why this stage exists

Everything a Patin composition looks like was a Rust constant. The lock
screen's purple, the greeter's teal, the pill geometry in the workspaces
strip, the two rows in the session menu — all of it compiled in, changeable
only by editing the crate and rebuilding. What configuration existed was a
scattering of `PATIN_*` environment variables and `--flag=` arguments, added
one at a time wherever a compositor integration needed to inject something.
That is a reasonable way to pass one value to one program and a poor way to
describe a shell: there is no way to say "this accent, everywhere", no way to
add a row to the session menu at all, and no way for the description to adapt
to the machine it lands on.

This stage adds a configuration *language* rather than another dozen
variables, and puts it where it cannot contaminate the toolkit: a new opt-in
crate, `patin-lua`, that the core `patin` crate knows nothing about. A
consumer that wants no config file never compiles a Lua VM, exactly as a
consumer that wants no battery never compiles `zbus`.

## Why Lua, and why luna

A shell's configuration is not a document. It has to ask questions — is this
the phone or the laptop, is `0xinctl` installed, is there a hardware keyboard
— and answer them differently on each machine. A TOML or JSON file cannot ask
anything; it can only be written twice and kept in sync by hand. Lua is the
smallest well-known language that can, and the one people arriving from
neovim, wezterm, or awesome already know.

The interpreter is [luna](https://github.com/onix-os/luna), a stackless Lua VM
in pure Rust. Two properties earned it the dependency:

**It is pure Rust.** The alternative, `mlua`, links against a C Lua and would
have made Patin the first thing in this workspace to need a C build beyond the
PAM and xkbcommon system libraries. On postmarketOS, cross-compiled to a
phone, that cost is real.

**It is stackless, so a runaway config is survivable.** Lua and Rust never
nest on the call stack: control returns to Patin between slices of VM work.
That turns "the config has an infinite loop" from a hung lock screen into a
bounded failure — `patin-lua` steps the executor itself, counts the fuel it
spends, and stops it:

```rust
let mut fuel = Fuel::with(FUEL_PER_SLICE);
let finished = lua.enter(|ctx| ctx.fetch(&executor).step(ctx, &mut fuel));
spent += i64::from(FUEL_PER_SLICE - fuel.remaining());
if spent > FUEL_BUDGET {
    lua.enter(|ctx| ctx.fetch(&executor).stop(&ctx));
    return Err(ConfigError::Budget { path: path.to_path_buf() });
}
```

A conventional interpreter would be inside its own loop with nothing to ask.
`Lua::finish`, luna's own convenience runner, has the same problem — it runs
to completion however long that takes — which is why it is not used here.

## Assign the settings, register the behaviour, return nothing

The config file is a list of statements, not a table handed back to Patin:

```lua
local patin = require("patin")

patin.theme.accent = "#7c3aed"
patin.bar.pill = { width = 32, height = 10, gap = 12, radius = 5 }

if patin.which("0xinctl") then
  patin.actions["logout"] = { label = "Log out", run = { "0xinctl", "exit" } }
end
```

Three consequences follow from that shape, and each of them is the reason for
it.

**A setting the file never mentions is left alone.** There is no defaults
table to keep in sync and no way to blank a setting by forgetting to list it.
Every accessor returns an `Option`, and *missing means unset, never zero* —
which is what lets `--keypad=` and `PATIN_LOCK_KEYPAD` keep overriding the
file rather than fighting it.

**The file can adapt to the machine.** The `if` above is the whole point: the
same `init.lua` works on the phone and the laptop because it asked. A returned
table cannot ask.

**Nested data stays nested.** `patin.bar.pill` is assigned as a table, not
flattened into four statements. The style is about how a setting is
*delivered*, not about pulling trees apart; Patin's config is mostly values —
colours, geometry, timings — so it keeps them as values.

Behaviour is registered instead, and the session menu is the case Patin
actually has. Rows are a **keyed** registrar rather than a list, because a row
has an identity:

```lua
patin.actions["reboot"] = { label = "Restart", run = { "loginctl", "reboot" } }
patin.actions["shutdown"] = false
```

Keyed registration is idempotent — naming `reboot` twice replaces it rather
than drawing two Reboot buttons — and it is what makes overriding one of
Patin's own rows possible at all. `false` removes a row, and it has to be
`false` rather than `nil`: in Lua a key assigned `nil` is a key that was never
there, so `nil` cannot distinguish "take this away" from "I did not mention
it", and those must mean different things.

There is deliberately no `patin.on.<event>` yet. Patin has real events — a
workspace changing, an unlock failing — but none of them has a caller that
would run a config's handler today, and inventing a taxonomy before there is
something to fire is how an API acquires names nobody can use.

## What the config may reach, and why that is small

The VM is started with `Lua::core()` — base, string, table, math, coroutine,
utf8 — and not `Lua::full()`, which would add `io`, `os`, `package`, and
`debug`. The probes a config genuinely needs are handed in as host functions
instead:

| function | answers |
|---|---|
| `patin.env(name)` | the value of an environment variable, or nil |
| `patin.exists(path)` | whether a path is there |
| `patin.which(cmd)` | the `$PATH` entry for a program, or nil |
| `patin.log(message)` | prints to stderr, for debugging a config |

This is not a security boundary and is not claimed as one: whoever writes
`init.lua` already owns the session. It is a *blocking* boundary. The obvious
next thing a config author wants is a widget that shows the battery, and the
obvious wrong way to write one is `io.open("/sys/class/power_supply/…")`
inside the draw path, where it blocks the next frame on a syscall. Patin
already has `service::Provider` for polling; when widgets arrive they will be
handed values that are already read, and they will work the same whether `io`
exists or not. Withholding it now means nobody writes the blocking version
first.

`require` is provided as a host function too, since `Lua::core()` has no
`package` library. It serves the module table and refuses everything else by
name, so the familiar first line of a config works and a mistyped module gets
a real message instead of a nil index three lines later.

## Failure, and the two compositions that must not have any

A raise while loading `init.lua` is **fatal**, and the error names the file and
the line. Carrying on with defaults would silently apply a shell the user did
not ask for, and "my config is being ignored" is a much harder thing to debug
than a refusal.

`patin-lock` and `patin-login` are the exception, and it is the only place
Patin deviates from that rule. Those two stand between a person and their own
machine. A greeter that refuses to start is a computer nobody can sign into; a
lock screen that refuses to start is a phone nobody can unlock. Both call
`Config::load_or_report`, which prints the same error and continues with the
built-in palette:

```text
$ PATIN_CONFIG=broken.lua patin-login
patin-login: broken.lua: parse error at line 2: unexpected end of token stream
patin-login: continuing with built-in defaults
```

Nothing a config sets can weaken authentication. PAM's service name and policy
path, the password length limit, greetd's socket, and the supervisor that
restarts a dead lock worker are not settings.

The other cost of assigning settings straight onto the module table is that a
typo lands in the same namespace as everything else, where the host cannot
tell it from a helper the config stashed. Patin buys that back by having each
composition declare the namespaces it owns and the keys it knows:

```text
$ patin-workspaces-bar
patin config: ~/.config/patin/init.lua: unknown setting bar.pil.width
patin config: ~/.config/patin/init.lua: bar.height expects a number, found string — using the default
```

A namespace Patin parked and the config never wrote to is not reported — it is
not a setting anybody typed.

## Where the file lives, and what still outranks it

`$XDG_CONFIG_HOME/patin/init.lua`, else `~/.config/patin/init.lua`. A missing
file is not an error; a path the user named explicitly and got wrong is.

The precedence rule is unchanged at the top and merely gains a rung:

```text
--flag=value  >  PATIN_* environment  >  init.lua  >  built-in default
```

An explicit argument has always had the last word in Patin, and a config file
arriving does not change that. The environment keeps its place above Lua for a
concrete reason: `PATIN_SESSION_LOGOUT_PROGRAM` and friends are how a
compositor ships a working logout row, and an integration that works today
must not break because somebody wrote an `init.lua`. `PATIN_NO_CONFIG=1`
ignores the file entirely, and `--config=path` names another one — the first
question when a shell misbehaves is "is it me or my config?", and a tool with
no way to start without one makes that unanswerable.

## Changed files and important functions

- `crates/patin-lua/src/host.rs` — the VM. `evaluate` parks the module table
  and its namespaces, installs the four host functions and `require`, loads
  the chunk under a name so errors carry it, steps the executor against a fuel
  budget, and harvests what the chunk assigned. `harvest`/`convert` walk the
  Lua tables into owned Rust values, bounded in depth, which is also what stops
  `patin.self = patin` recursing forever.
- `crates/patin-lua/src/value.rs` — the owned tree the VM leaves behind, so
  nothing borrows from a garbage collector at draw time. `Table::path` resolves
  dotted keys, `Value::as_color` accepts `"#rrggbb"`, `"#rrggbbaa"` and
  `{ r, g, b }`, and `leaf_paths` is what unknown-key reporting walks.
- `crates/patin-lua/src/lib.rs` — `Config::load` (discovery and precedence),
  `Config::load_or_report` (the lock/greeter path), the typed accessors, and
  `warn_unknown`. Every accessor takes a *list* of keys and returns the first
  the config set, which is how `&["lock.accent", "theme.accent"]` lets one
  shared colour reach every composition while any one of them keeps the last
  word.
- `crates/patin-workspaces-bar/src/ui.rs` — `Style`, whose `Default` is the
  constants the strip shipped with. `main.rs` reads `style.height` for both the
  surface size and the exclusive zone, so a configured bar reserves the space
  it actually occupies.
- `crates/patin-session/src/actions.rs` — `configured` builds Patin's own rows,
  applies `patin.actions` over them by key, and sorts by `order` then name so
  the menu cannot reshuffle itself between launches.
- `crates/patin-lock/src/settings.rs`, `crates/patin-login/src/settings.rs` —
  each composition's own mapping from config to values, including the keypad,
  session, and username precedence chains moved out of `main.rs`.
- `crates/patin-lock/src/ui.rs`, `crates/patin-login/src/ui.rs` — `Theme`, with
  `status_palette` moving onto it. The greeter derives its resting accent from
  its focused one, so a config that names one colour gets the relationship the
  greeter already had rather than being asked to reproduce it.
- `data/patin/init.lua.example` — a complete config to copy.

## Verification

Verified on 2 September 2026:

```text
$ cargo fmt --all -- --check
(no output, exit 0)

$ cargo test --workspace --all-targets
144 tests across 18 crates, all passed

$ cargo clippy --workspace --all-targets --all-features -- -D warnings
Finished `dev` profile, no warnings

$ mdbook build
INFO HTML book written to `/home/vdzee/proj/patin/book`

$ git diff --check
(no output, exit 0)
```

The config layer was then exercised through the real binaries, with
`WAYLAND_DISPLAY` pointed at nothing so each stops at the compositor connect
*after* configuration has been read:

```text
$ PATIN_CONFIG=data/patin/init.lua.example patin-workspaces-bar
patin-workspaces-bar: Could not find wayland compositor

$ printf 'patin.bar.pil = { width = 40 }\npatin.bar.height = "tall"\n' > typo.lua
$ PATIN_CONFIG=typo.lua patin-workspaces-bar
patin config: typo.lua: unknown setting bar.pil.width
patin config: typo.lua: bar.height expects a number, found string — using the default
patin-workspaces-bar: Could not find wayland compositor

$ printf 'patin.bar.height =\n' > broken.lua
$ PATIN_CONFIG=broken.lua patin-workspaces-bar
patin-workspaces-bar: broken.lua: parse error at line 2: unexpected end of token stream
(exit 1, before touching Wayland)

$ PATIN_CONFIG=broken.lua patin-login
patin-login: broken.lua: parse error at line 2: unexpected end of token stream
patin-login: continuing with built-in defaults
patin-login: GREETD_SOCK is not set; running in preview mode
patin-login: Could not find wayland compositor

$ PATIN_NO_CONFIG=1 PATIN_CONFIG=broken.lua patin-workspaces-bar
patin-workspaces-bar: Could not find wayland compositor
```

The last two are the stage's two rules in one line each: the greeter reports
and carries on where the bar refuses, and the escape hatch skips a file that
would otherwise be fatal.

To try it:

```sh
mkdir -p ~/.config/patin
cp data/patin/init.lua.example ~/.config/patin/init.lua
patin-workspaces-bar          # geometry and colours from the file
patin-session                 # the rows it registered, in the order it gave
PATIN_NO_CONFIG=1 patin-lock  # the built-in palette, config ignored
```
