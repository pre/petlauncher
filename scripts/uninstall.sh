#!/usr/bin/env bash
# Removes the files that scripts/install.sh installed, and the autostart file
# that petlauncher writes. It keeps the settings, the cache and the state.
#
# Usage: scripts/uninstall.sh [--prefix DIR] [--config-home DIR]
#   --prefix DIR        the prefix of install.sh (default: ~/.local)
#   --config-home DIR   the directory of the autostart file
#                       (default: $XDG_CONFIG_HOME, else ~/.config)
#
# It changes no GNOME settings. At the end it prints the steps that you do
# by hand. docs/install.md describes them.
set -euo pipefail

uuid=petlauncher@repo.pet
prefix=$HOME/.local
config_home=${XDG_CONFIG_HOME:-$HOME/.config}

usage() {
    sed -n '2,11p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
    case "$1" in
    --prefix) prefix=$2; shift 2 ;;
    --config-home) config_home=$2; shift 2 ;;
    -h | --help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
    esac
done
prefix=$(realpath -m "$prefix")

for path in \
    "$prefix/bin/petlauncher" \
    "$prefix/share/applications/pet.repo.Petlauncher.desktop" \
    "$prefix/share/gnome-shell/extensions/$uuid" \
    "$config_home/autostart/pet.repo.Petlauncher.desktop"; do
    if [[ -e $path ]]; then
        rm -rf "$path"
        echo "Removed $path"
    fi
done

cat <<EOF

Do these steps by hand:
  1. Stop petlauncher: its top bar menu, Quit. Or:
       gdbus call --session --dest pet.repo.Petlauncher --object-path /pet/repo/Petlauncher \\
           --method org.freedesktop.Application.ActivateAction quit '[]' '{}'
  2. Disable the extension, then log out and log in:
       gnome-extensions disable $uuid
  3. Remove the custom shortcut "Show petlauncher" in GNOME Settings >
     Keyboard > View and Customize Shortcuts > Custom Shortcuts.
  4. The settings, the cache and the state stay. Remove them if you want:
       ${XDG_CONFIG_HOME:-~/.config}/petlauncher
       ${XDG_CACHE_HOME:-~/.cache}/petlauncher
       ${XDG_STATE_HOME:-~/.local/state}/petlauncher
EOF
