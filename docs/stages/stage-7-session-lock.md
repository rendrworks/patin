# Stage 7 — Session Lock

## Why this stage exists

A phone lock screen cannot depend on a separate on-screen keyboard: once the
session is locked, ordinary application surfaces must not appear above it.
The lock client therefore owns both the secure Wayland surfaces and its touch
password keyboard. It remains a separate Patin composition rather than
becoming an automatically constructed toolkit feature.

## Protocol and lifecycle

`patin-lock` requests `ext-session-lock-v1`, creates a surface for every output,
and follows output hotplug and seat capability changes at runtime. The
compositor, not the client, enforces exclusivity: after acknowledging the lock,
it must not reveal the session merely because the client dies.

The outer `patin-lock` process supervises a `--worker` child. A normal child
exit means authentication succeeded and the child sent the protocol unlock
request. A panic or signal causes a delayed restart; missing globals, missing
PAM configuration, or a compositor refusal are terminal errors. `--worker` is
an internal implementation detail.

## Input, rendering, and authentication

Every output uses Patin's existing shared-memory CPU renderer. The minimal
scene contains the time, effective username, a masked password field, status
text, and a four-row QWERTY/symbol keyboard. Touch and pointer hit tests use
logical coordinates, while SCTK's XKB support supplies decoded physical
keyboard input.

The password is limited to 256 UTF-8 bytes. UI-owned password strings use
`zeroize`; submission moves the secret to a PAM worker thread and immediately
clears the UI copy. PAM's `patin-lock` service performs both authentication and
account checks. Authentication failure clears the submitted secret inside the
worker and re-enables input.

## Installation

Install the user binary:

```sh
./scripts/install-lock-user.sh
```

Then explicitly install the PAM policy matching the host. For the FP5
postmarketOS/Alpine reference target:

```sh
sudo apk add linux-pam-dev
sudo install -m 0644 data/pam/patin-lock.alpine /etc/pam.d/patin-lock
patin-lock
```

Arch and Debian policy examples live beside the Alpine file. The installer
does not modify `/etc`, and the client checks for the policy before acquiring
the lock.

Do not bind a hardware power button to this command until a live session has
confirmed touch entry and successful unlock. Keep an SSH recovery connection
available during the first test.

## Verification

Verified on 30 July 2026:

```text
$ cargo check -p patin-lock
Finished, no warnings

$ cargo test -p patin-lock
2 tests passed

$ cargo fmt --all -- --check
(no output, exit 0)

$ cargo test --workspace --all-targets
12 tests across 6 crates, all passed

$ cargo clippy --workspace --all-targets --all-features -- -D warnings
Finished, no warnings

$ mdbook build
INFO HTML book written to `/home/vdzee/proj/patin/book`

$ git diff --check
(no output, exit 0)
```

FP5 protocol and touch-authentication results are recorded after the reference
target has completed its live lock/unlock test. The first native release build
reached the final linker step and confirmed the expected missing prerequisite:

```text
$ cargo build --release --locked -p patin-lock
ld: cannot find -lpam
ld: cannot find -lpam_misc
```

Install `linux-pam-dev`, then repeat the build and live test.
