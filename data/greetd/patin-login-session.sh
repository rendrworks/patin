#!/bin/sh
# greetd session command for Patin's greeter.
#
# greetd runs this as the unprivileged `greetd` user on the greeter VT. It
# starts a compositor whose only client is patin-login; greetd hands the
# greeter $GREETD_SOCK, and replaces this whole session with the user's own
# once the credentials are accepted.
#
# Every path is overridable so a device profile can point at its own builds
# without editing this script.
set -eu

# The compositor to host the greeter in, and the greeter itself. 0xin takes
# the client to spawn as a positional argument.
: "${PATIN_LOGIN_COMPOSITOR:=0xin}"
: "${PATIN_LOGIN_BIN:=/usr/local/bin/patin-login}"

# A compositor config with no way to launch a program: the greeter runs before
# anyone has authenticated, so a default "spawn a terminal" keybind would be an
# unauthenticated shell. See 0xin-greeter.conf.
: "${OXIN_CONFIG:=/etc/greetd/0xin-greeter.conf}"

# What greetd execs once the credentials are accepted.
: "${PATIN_LOGIN_SESSION:=0xin}"

export OXIN_CONFIG PATIN_LOGIN_SESSION

# A greetd session starts without a parent Wayland display; clear any stale
# inherited values so wlroots must choose DRM + libinput.
unset WAYLAND_DISPLAY DISPLAY
export WLR_BACKENDS="${WLR_BACKENDS:-drm,libinput}"
export LIBSEAT_BACKEND="${LIBSEAT_BACKEND:-logind}"

exec "${PATIN_LOGIN_COMPOSITOR}" "${PATIN_LOGIN_BIN}"
