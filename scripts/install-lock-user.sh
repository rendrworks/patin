#!/bin/sh
set -eu

cargo build --release --locked -p patin-lock

user_bin_dir="${HOME}/.local/bin"
install -d "${user_bin_dir}"
install -m 0755 target/release/patin-lock "${user_bin_dir}/patin-lock"

printf 'Installed lock client as %s\n' "${user_bin_dir}/patin-lock"
if [ ! -f /etc/pam.d/patin-lock ]; then
    printf '%s\n' 'PAM is not configured yet. On postmarketOS/Alpine, run:'
    printf '%s\n' '  sudo install -m 0644 data/pam/patin-lock.alpine /etc/pam.d/patin-lock'
fi
