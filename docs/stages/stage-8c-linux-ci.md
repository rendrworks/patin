# Stage 8c — Reproducible Linux CI

This maintenance stage makes the GitHub Actions build reproduce the native
Linux prerequisites already required by the workspace.

## Concept

Cargo manages Rust packages, but some crates bind to libraries supplied by the
operating system. Patin enables `smithay-client-toolkit`'s `xkbcommon` feature,
so SCTK's build script asks `pkg-config` for `xkbcommon`. The optional
`patin-lock` workspace member also links to PAM. A fresh Ubuntu GitHub runner
does not promise the corresponding development files.

Development packages are different from runtime packages: they install the
headers, unversioned linker inputs, and `.pc` metadata needed while compiling.
On Debian and Ubuntu those packages are `libxkbcommon-dev` and `libpam0g-dev`.
Installing them before Cargo runs prevents the SCTK build script from
panicking because `xkbcommon.pc` cannot be found and prepares the complete
workspace for the later lock-screen build.

## Implementation

- `.github/workflows/ci.yml` installs the two native development packages
  immediately after checkout.
- `README.md` names the packages beside the build commands so a fresh local
  Debian/Ubuntu checkout has the same prerequisites as CI.
- `docs/environment.md` explains the `pkg-config` discovery mechanism and the
  matching environment setup.
- `docs/SUMMARY.md` links this stage into the book.

No Rust behavior, Wayland protocol handling, or runtime capability detection
changes in this stage.

## Verification

The following checks were run after the change:

- `cargo fmt --all -- --check` — passed.
- `cargo test --workspace --all-targets` — passed all 29 tests across the
  toolkit, examples, and workspace binaries/libraries.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` —
  passed with no warnings.
- `mdbook build` — passed and wrote the HTML book to `book/`.
- `git diff --check` — passed with no whitespace errors.

GitHub's hosted runner will exercise the newly added `apt-get` step on the next
push or pull request; the local machine already had the development libraries,
so no privileged package installation was needed for these checks.
