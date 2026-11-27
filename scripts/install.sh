#!/usr/bin/env bash
# Builds petlauncher and installs it for the current user, with no root.
#
# Usage: scripts/install.sh [--prefix DIR]      (default: ~/.local)
#
# It installs:
#   DIR/bin/petlauncher                                    the program
#   DIR/share/applications/pet.repo.Petlauncher.desktop    the desktop entry
#   DIR/share/gnome-shell/extensions/petlauncher@repo.pet  the GNOME Shell extension
#
# It changes no settings: it does not start petlauncher, does not enable the
# extension and writes no GNOME shortcut. At the end it prints the steps
# that you do by hand. docs/install.md describes them.
set -euo pipefail

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
uuid=petlauncher@repo.pet
prefix=$HOME/.local

usage() {
    sed -n '2,13p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
    case "$1" in
    --prefix) prefix=$2; shift 2 ;;
    -h | --help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
    esac
done
prefix=$(realpath -m "$prefix")
# The path goes into the Exec key of the desktop entry with no quotes.
if [[ $prefix =~ [^A-Za-z0-9_./+-] ]]; then
    echo "use a prefix with no spaces or special characters: $prefix" >&2
    exit 2
fi
command -v jq >/dev/null || { echo "install jq first: sudo apt install jq" >&2; exit 2; }

bin=$prefix/bin
applications=$prefix/share/applications
extensions=$prefix/share/gnome-shell/extensions

cargo build --release -p petlauncher --manifest-path "$repo/Cargo.toml"

install -D -m 755 "$repo/target/release/petlauncher" "$bin/petlauncher"

mkdir -p "$applications"
cat >"$applications/pet.repo.Petlauncher.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=petlauncher
Comment=Find and start applications, files, calculations and shortcuts
Exec=$bin/petlauncher
Icon=system-search
Terminal=false
Categories=Utility;
StartupNotify=false
EOF

mkdir -p "$extensions"
rm -rf "${extensions:?}/$uuid"
cp -r "$repo/gnome/extension/$uuid" "$extensions/"

# The release workflow writes the release version to version-name. An install
# from a git checkout gets git-<commit>, so the extension apps show the build.
if commit=$(git -C "$repo" rev-parse --short HEAD 2>/dev/null); then
    metadata=$extensions/$uuid/metadata.json
    jq --arg v "git-$commit" '. + {"version-name": $v}' "$metadata" >"$metadata.tmp"
    mv "$metadata.tmp" "$metadata"
fi

echo
echo "Installed:"
echo "  $bin/petlauncher"
echo "  $applications/pet.repo.Petlauncher.desktop"
echo "  $extensions/$uuid"
echo
if [[ ":$PATH:" != *":$bin:"* ]]; then
    echo "Note: $bin is not in PATH. The global key runs \"petlauncher toggle\","
    echo "so the GNOME session must have $bin in its PATH."
    echo
fi
cat <<EOF
Do these steps by hand:
  1. Log out and log in, so that GNOME Shell finds the new extension.
  2. Enable the extension:
       gnome-extensions enable $uuid
  3. Start petlauncher one time:
       $bin/petlauncher
     At its first start petlauncher writes the GNOME custom shortcut
     "Show petlauncher" with Super+Space, when no other GNOME key binding
     has that key. If one has it, the launcher shows which binding has the
     key and where GNOME Settings shows it. Remove the key there, then open
     the launcher again.
  4. With "Run in background" on, petlauncher starts at each login.
EOF
