#!/bin/sh
set -eu

cargo build --release --locked -p patin-launcher

user_bin_dir="${HOME}/.local/bin"
install -d "${user_bin_dir}"
install -m 0755 target/release/patin-launcher "${user_bin_dir}/patin-launcher"

printf 'Installed launcher as %s\n' "${user_bin_dir}/patin-launcher"
