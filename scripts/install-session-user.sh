#!/bin/sh
set -eu

cargo build --release --locked -p patin-session

user_bin_dir="${HOME}/.local/bin"
install -d "${user_bin_dir}"
install -m 0755 target/release/patin-session "${user_bin_dir}/patin-session"

printf 'Installed session menu as %s\n' "${user_bin_dir}/patin-session"
