#!/bin/sh
set -eu

cargo build --release --locked --example demo_bar

user_bin_dir="${HOME}/.local/bin"
install -d "${user_bin_dir}"
install -m 0755 target/release/examples/demo_bar "${user_bin_dir}/patin"

printf 'Installed demo bar as %s\n' "${user_bin_dir}/patin"
printf 'Run it with: patin\n'
