# Stage 8j — Nix Flake and NixOS Module

This stage adds a Nix flake that builds the workspace reproducibly and a NixOS
module that declares Patin in `configuration.nix`. It also fixes the greeter's
session discovery, which had assumed sessions always live under `/usr`.

## Concept

Until now the only way to install Patin was one of the `scripts/install-*-user.sh`
scripts: `cargo build --release` into `$HOME/.local/bin`, with the distribution
expected to have supplied xkbcommon and PAM. That is a reasonable contract on
Arch or Alpine, but it gives a NixOS machine nothing to declare, and it gives
nobody a build that is byte-reproducible from a lockfile.

A flake is a directory with a `flake.nix` describing pinned inputs and named
outputs — packages, apps, checks, development shells, and NixOS modules. The
pinning is the point: `flake.lock` records the exact revision of every input,
so the same commit builds the same closure on any machine. Flakes are a feature
of `nix` itself rather than a separate program, gated behind the
`nix-command flakes` experimental setting.

Two properties shape this particular flake.

**It is additive.** `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`,
`scripts/`, `data/`, and the CI workflow are all untouched. A checkout on Arch
builds exactly as it did before; Nix is an optional second path, and CI keeps
exercising the non-Nix one.

**The toolchain has one home.** `rust-toolchain.toml` already pins Rust 1.97.1,
rustup obeys it locally, and CI obeys it through `rustup show`. The flake reads
the same file with `rust-bin.fromRustupToolchainFile` rather than repeating the
version, and builds the packages with that toolchain through `makeRustPlatform`
instead of whichever `rustc` nixpkgs happens to ship. There is no second number
to keep in sync.

### Pinning nixpkgs

Oslo hardcodes a nixpkgs revision in the input URL. Patin names the
`nixpkgs-unstable` branch and lets `flake.lock` hold the revision instead,
which is equally reproducible — the lock file is what a build reads — while
leaving `nix flake update` a branch to follow.

Copying oslo's revision was tried first and does not work: that April 2026
nixpkgs fetches crates from `https://crates.io/api/v1/crates/...`, and
crates.io now answers any request whose User-Agent contains `curl/` with a 403.
`fetchurl` sends exactly `curl/<version> Nixpkgs/<version>`, so every crate
download fails. Later nixpkgs revisions fetch from `static.crates.io`, which
has no such rule. The issue is
[rust-lang/crates.io#13482](https://github.com/rust-lang/crates.io/issues/13482).

### The native dependency set

Patin's `buildInputs` are `libxkbcommon` and `pam`, and nothing else — the same
two packages CI installs on Ubuntu. This is worth stating explicitly, because
a Wayland shell that renders text and talks to system services would normally
be expected to link against considerably more. It does not, because the crates
involved are pure Rust:

| Would normally be a C library | What Patin uses instead |
| --- | --- |
| `libwayland-client` | `wayland-backend`'s Rust implementation |
| `libfontconfig`, `freetype` | `fontconfig-parser`, `fontdb`, `swash` |
| `libdbus` | `zbus`, speaking the socket directly |
| `liblua` | `luna`, a stackless Lua VM in Rust |

`pkg-config` remains a native build input because SCTK's `xkbcommon` feature
runs a build script that asks it for `xkbcommon.pc`.

### Why the binaries stay dynamically linked

The sibling `termworks/oslo` flake, whose structure this one follows, builds
fully static musl binaries and asserts in its install check that the result
requests no dynamic loader and lists no `NEEDED` entries. Patin cannot do that.
PAM `dlopen`s its authentication modules at runtime, so a statically linked
`libpam` would be a PAM stack with no modules in it — `patin-lock` could not
authenticate anyone. Patin therefore links dynamically against glibc, and its
install check asserts the opposite of oslo's: that `patin-lock` really does
carry `libpam` and `libxkbcommon` as dynamic dependencies. That check is what
catches an accidentally emptied `buildInputs`, which would otherwise produce a
package that builds and then fails at the lock screen.

The install check also cannot borrow oslo's `--version` smoke test: no Patin
binary parses `--version` or `--help`, so binary existence plus linkage is what
there is to assert.

### The one git dependency

`crates/patin-lua` depends on `luna` by git revision. A git dependency cannot be
fetched from a plain `cargoHash`, so the derivation uses `cargoLock.lockFile`
with an explicit `outputHashes."luna-0.5.1"`. When the pinned revision changes,
that hash changes with it, and the first build after the bump reports the value
to paste back.

### Selecting what to build

Examples are not built. `examples/demo_bar` demonstrates the toolkit; it is not
a program Patin ships, and building it on every `nix build` would be wrong.
`packages.patin-demo-bar` builds it opt-in and installs it as `patin`, matching
`scripts/install-demo-user.sh`. It also builds `patin-network-settings`, which
the demo bar spawns when a status icon is tapped.

`--example` is a cargo *target selector*: passing it alone narrows the build to
that example and nothing else, so a `-p patin-network-settings --example demo_bar`
invocation silently produces no `patin-network-settings` binary. The derivation
adds `--bins` alongside it. The install check is what caught this — the first
build of the demo-bar package failed with `missing patin-network-settings`
rather than shipping a package whose demo could not open its own settings.

The package function is wrapped in `lib.makeOverridable` with a `crates`
argument, so a machine can build only the binaries it needs:

```nix
patin.packages.${system}.patin.override { crates = [ "patin-login" ]; }
```

A greeter host has no reason to compile the launcher, the lock screen, and the
on-screen keyboard.

### Session discovery

`patin-login` previously read sessions from two hard-coded directories,
`/usr/local/share/wayland-sessions` and `/usr/share/wayland-sessions`. NixOS
installs them under `/run/current-system/sw/share`, so the greeter would have
found nothing and fallen back to its single configured command.

The fix belongs in the Rust, not in a Nix patch. `sessions.rs` now derives its
search path from `XDG_DATA_DIRS` and appends the two previous paths as
defaults, which is what the base directory specification prescribes and what
`patin-launcher` already does for icon themes in `apps.rs`. The behaviour on a
system that never sets the variable is unchanged; a system that does set it —
NixOS, or an Arch user with a session in a non-standard data root — now works.

`session_dirs` takes the variable as an argument rather than reading the
environment, so it is testable without a process-wide environment change.

### The NixOS module

`nix/module.nix` wires up only what a system has to own, and asserts nothing it
cannot know:

- `lock.enable` declares `security.pam.services.patin-lock`. This is not
  optional decoration: `patin-lock` refuses to start unless
  `/etc/pam.d/patin-lock` exists, and the generated stack is equivalent to
  `data/pam/patin-lock.arch`. No component is setuid; the locker runs as the
  user and calls PAM directly.
- `greeter.enable` configures greetd with a session script that exports
  `PATIN_LOGIN_COMPOSITOR`, `PATIN_LOGIN_BIN`, `PATIN_LOGIN_SESSION`,
  `OXIN_CONFIG`, and `PATIN_LOGIN_STATE` and then execs the packaged
  `patin-login-session`. Every one of those is already an overridable
  `: "${VAR:=default}"` in `data/greetd/patin-login-session.sh`, so the module
  reuses that script's `WLR_BACKENDS`, `LIBSEAT_BACKEND`, and stale-display
  handling rather than reimplementing it. There is no VT option to mirror the
  `vt = 7` in `data/greetd/config.toml.example`: NixOS removed
  `services.greetd.vt` and fixes the greeter to VT1.
- `config` exports `PATIN_CONFIG`. Patin has no system-wide configuration path
  of its own — the resolution order is `--config=`, then `PATIN_CONFIG`, then
  `$XDG_CONFIG_HOME/patin/init.lua` — so an environment variable is how a
  declarative machine supplies one.
- `session.enable` installs a `wayland-sessions` entry, which the session
  discovery fix above makes visible.

`compositor` is a package option with no default. 0xin is not in nixpkgs, and
`AGENTS.md` requires Patin to stay usable without it, so coupling the flake to
a specific compositor would be the wrong trade.

Commands Patin spawns by name — `nmcli`, `wpctl`, `pactl`, `systemctl`,
`loginctl` — are deliberately absent from the closure. On NixOS they are in the
system profile as soon as the matching service is enabled, which is the same
expectation Patin has everywhere else. Wrapping the binaries to inject them
would also break `patin-lock`, which re-execs `current_exe()` with `--worker`.

## Implementation

- `flake.nix` pins nixpkgs, `flake-utils`, and `rust-overlay`; derives the
  toolchain from `rust-toolchain.toml`; and exposes `packages`, `apps`,
  `checks`, `devShells`, `nixosModules`, and an overlay.
- `nix/module.nix` defines `services.patin` with `package`, `compositor`,
  `config`, `lock`, `greeter`, and `session` options.
- `crates/patin-login/src/sessions.rs` replaces the `SESSION_DIRS` constant
  with a `session_dirs` function reading `XDG_DATA_DIRS`, plus two tests.
- `README.md` and `docs/environment.md` document the Nix path beside the
  existing per-distribution instructions.
- `.gitignore` ignores the `result` symlinks a Nix build leaves behind.
- `flake.lock` is committed.

No Wayland protocol handling, rendering, or runtime capability detection
changes in this stage. The one behavioural change is that the greeter now finds
sessions in every XDG data root rather than only the two under `/usr`.

## Verification

The Cargo path was checked first, since it is the one that must not regress:

- `cargo fmt --all -- --check` — passed.
- `cargo test --workspace --all-targets` — passed all 148 tests, including the
  two new `sessions.rs` cases.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` —
  passed with no warnings.
- `mdbook build` — passed and wrote the HTML book to `book/`.
- `git diff --check` — passed with no whitespace errors.

Then the Nix path, on x86_64 Arch Linux with Nix 2.35.2:

- `nix flake check` — `all checks passed!`. It builds `patin`,
  `patin-demo-bar`, and the `fmt` check. `aarch64-linux` was reported as
  omitted; the flake declares it, but this machine cannot build it.
- `nix build .#patin` — installed the seven binaries plus
  `bin/patin-login-session`, and `share/patin/` with the PAM policies, the
  greetd examples, and `init.lua.example`.
- `readelf -d result/bin/patin-lock | grep NEEDED` — listed `libpam.so.0` and
  `libxkbcommon.so.0` alongside libgcc, libm, and libc, which is what the
  install check asserts.
- `nix build .#patin-demo-bar` — installed `bin/patin` and
  `bin/patin-network-settings`, and no `patin-login-session`, since the greeter
  it would point at is not part of that package.
- `nix develop -c cargo --version` — `cargo 1.97.1`; `rustc --version` in the
  same shell reported `rustc 1.97.1`, matching `rust-toolchain.toml`.
- Evaluating `patin.override { crates = [ "patin-login" ]; }` produced
  `cargoBuildFlags = [ "-p" "patin-login" ]`, confirming the narrowed build.

The NixOS module was evaluated against a throwaway configuration with
`eval-config.nix`, which produced the expected `/etc/pam.d/patin-lock` stack
(`auth`/`account` through `pam_unix.so`, the equivalent of
`data/pam/patin-lock.arch`), a greetd `default_session.command` pointing at the
generated wrapper, the `d /var/lib/greetd 0755 greeter greeter -` tmpfiles
rule, and `patin.desktop` in `environment.systemPackages`. The generated
wrapper exports each `PATIN_LOGIN_*` and `OXIN_CONFIG` value as a store path
and execs `patin-login-session`.

**Evaluation is as far as this went.** The greeter, the lock screen, and the
session entry were not exercised on a running NixOS machine — that needs a
NixOS host or the FP5, and it is the obvious next thing to confirm.
