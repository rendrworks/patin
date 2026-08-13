#!/bin/sh
set -eu

cargo build --release --locked -p patin-login

user_bin_dir="${HOME}/.local/bin"
install -d "${user_bin_dir}"
install -m 0755 target/release/patin-login "${user_bin_dir}/patin-login"

printf 'Installed greeter as %s\n' "${user_bin_dir}/patin-login"
printf '%s\n' 'Run it without GREETD_SOCK to preview the UI in the current session.'
printf '%s\n' 'Wiring it in as the real greeter is a separate, deliberate step:'
printf '%s\n' '  see "Install and run the greeter" in README.md'
