#!/bin/sh
# Runs a command with the system libraries GPUI needs. On NixOS they come
# from desktop/shell.nix; elsewhere the system provides them
# (docs/development.md, "The desktop client").
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
if [ -e /etc/NIXOS ] && command -v nix-shell >/dev/null 2>&1; then
  exec nix-shell "$here/shell.nix" --run "$*"
fi
exec sh -c "$*"
