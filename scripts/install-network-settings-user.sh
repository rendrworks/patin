#!/bin/sh
set -eu

cargo build --release --locked -p patin-network-settings

user_bin_dir="${HOME}/.local/bin"
install -d "${user_bin_dir}"
install -m 0755 target/release/patin-network-settings "${user_bin_dir}/patin-network-settings"

printf 'Installed network settings as %s\n' "${user_bin_dir}/patin-network-settings"
