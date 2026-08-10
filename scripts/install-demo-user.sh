#!/bin/sh
set -eu

cargo build --release --locked --example demo_bar -p patin
cargo build --release --locked -p patin-network-settings

user_bin_dir="${HOME}/.local/bin"
install -d "${user_bin_dir}"
install -m 0755 target/release/examples/demo_bar "${user_bin_dir}/patin"
install -m 0755 target/release/patin-network-settings "${user_bin_dir}/patin-network-settings"

printf 'Installed demo bar as %s\n' "${user_bin_dir}/patin"
printf 'Installed network settings as %s\n' "${user_bin_dir}/patin-network-settings"
printf 'Run it with: patin\n'
