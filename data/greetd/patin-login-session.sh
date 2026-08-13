#!/bin/sh
# greetd session command for Patin's greeter.
#
# greetd runs this as the unprivileged `greetd` user on the greeter VT. It
# starts a compositor, which in turn starts patin-login as its only client;
# greetd hands the greeter $GREETD_SOCK, and replaces this whole session with
# the user's own once the credentials are accepted.
set -eu

# The compositor to host the greeter in, and the session to start on success.
: "${PATIN_LOGIN_COMPOSITOR:=0xin}"
: "${PATIN_LOGIN_SESSION:=0xin}"
export PATIN_LOGIN_SESSION

exec "${PATIN_LOGIN_COMPOSITOR}" -E "patin-login"
