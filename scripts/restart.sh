#!/usr/bin/env bash
# Stops the running petlauncher and starts DIR/bin/petlauncher with no
# window, as at login. Use it after scripts/install.sh, so that the new
# program runs.
#
# Usage: scripts/restart.sh [--prefix DIR]      (default: ~/.local)
#
# The new program writes its log to the journal with the name
# pet.repo.Petlauncher.desktop, as at login. With "Run in background" off,
# the new program stops at once, as at login.
set -euo pipefail

app=pet.repo.Petlauncher
prefix=$HOME/.local

usage() {
    sed -n '2,10p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
    case "$1" in
    --prefix) prefix=$2; shift 2 ;;
    -h | --help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
    esac
done
program=$(realpath -m "$prefix")/bin/petlauncher
[[ -x $program ]] || { echo "no program at $program" >&2; exit 2; }

runs() {
    [[ $(gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
        --method org.freedesktop.DBus.NameHasOwner "$app") == "(true,)" ]]
}

wait_for() { # yes or no: waits up to 5 s until the program runs, or does not run
    local state
    for _ in $(seq 50); do
        state=no
        runs && state=yes
        [[ $state == "$1" ]] && return 0
        sleep 0.1
    done
    return 1
}

if runs; then
    gapplication action "$app" quit
    wait_for no || { echo "petlauncher did not stop in 5 s" >&2; exit 1; }
    echo "Stopped the running petlauncher."
fi
setsid -f systemd-cat -t "$app.desktop" "$program" --hidden
wait_for yes || {
    echo "petlauncher did not start. See: journalctl --user -t $app.desktop" >&2
    exit 1
}
echo "Started $program --hidden."
