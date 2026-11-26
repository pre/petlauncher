#!/usr/bin/env bash
# Tests the petlauncher GNOME Shell extension in a separate GNOME Shell 46.
#
# The test shell is headless: it has virtual monitors of different sizes and
# shows nothing on the desktop of the user. It has its own HOME, XDG
# directories, session bus, fake system bus and keyfile GSettings, so it does
# not touch the running GNOME session. docs/gnome-extension.md gives the
# reasons and the results.
#
# Usage: nested-test.sh [--out DIR] [--owner-layout] [--no-animations]
#                       [--no-extension] [--program PATH [--steps FILE]]
#   --out DIR         Evidence directory (logs, JSON, screenshots).
#   --program PATH    Test the real petlauncher binary at PATH with the steps
#                     of program-steps.sh, in place of fake-petlauncher.js.
#                     The program gets its own settings directory and a tree
#                     of test files in the test HOME.
#   --steps FILE      With --program: the steps of FILE in place of
#                     program-steps.sh, for example dictionary-steps.sh.
#                     The variable PETLAUNCHER_WORDNET_PATH goes to the steps.
#   --owner-layout    The monitors of the owner: 6016x3384 at scale 2, and
#                     2880x1800 at scale 1.5 to the right of it, 234 pixels
#                     lower. Default: 3008x1692 and 1920x1200 at scale 1,
#                     which are the logical sizes of the same monitors.
#   --no-animations   Set the GNOME setting enable-animations to false.
#   --no-extension    Baseline: run the same steps with no extension.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
uuid=petlauncher@repo.pet

if [[ "${1:-}" != --inner ]]; then
    out="$HOME/daybook-evidence/petlauncher/ext/run-$(date +%Y%m%d-%H%M%S)"
    monitors="3008x1692 1920x1200"
    layout=""
    extension=on
    animations=true
    program=""
    steps="$here/program-steps.sh"
    while [[ $# -gt 0 ]]; do
        case "$1" in
        --out) out=$2; shift 2 ;;
        --owner-layout)
            monitors="6016x3384 2880x1800"
            layout='[{"x": 0, "y": 0, "scale": 2}, {"x": 3008, "y": 234, "scale": 1.5}]'
            shift ;;
        --no-animations) animations=false; shift ;;
        --no-extension) extension=off; shift ;;
        --program) program=$(realpath "$2"); shift 2 ;;
        --steps) steps=$(realpath "$2"); shift 2 ;;
        *) echo "unknown argument $1" >&2; exit 2 ;;
        esac
    done
    mkdir -p "$out" "$HOME/tmp"
    root=$(mktemp -d "$HOME/tmp/petext.XXXXXX")
    mkdir -p "$root/home/.config/glib-2.0/settings" "$root/home/.local/share" "$root/run"
    chmod 700 "$root/run"
    keyfile="$root/home/.config/glib-2.0/settings/keyfile"
    printf "[org/gnome/desktop/interface]\nenable-animations=%s\n" "$animations" >"$keyfile"
    # The live session of the owner uses the logical layout with fractional
    # scales. The test shell starts in the physical layout without this.
    [[ -n $layout ]] &&
        printf "[org/gnome/mutter]\nexperimental-features=['scale-monitor-framebuffer']\n" >>"$keyfile"
    if [[ $extension == on ]]; then
        mkdir -p "$root/home/.local/share/gnome-shell/extensions"
        cp -r "$here/../$uuid" "$root/home/.local/share/gnome-shell/extensions/"
        printf "[org/gnome/shell]\nenabled-extensions=['%s']\n" "$uuid" >>"$keyfile"
    fi
    if [[ -n $program ]]; then
        # The steps of program-steps.sh begin with the R4 conflict.
        conflict=""
        [[ $steps == "$here/program-steps.sh" ]] && conflict=--key-conflict
        "$here/program-files.sh" "$root/home" ${conflict:+"$conflict"}
    fi
    # A fake system bus: the test shell must not reach logind, UPower or
    # NetworkManager of the real system.
    system_bus_pid=$(dbus-daemon --session --address="unix:path=$root/run/system_bus" \
        --fork --print-pid)
    status=0
    env -i PATH="$PATH" LANG=C.UTF-8 USER="$USER" HOME="$root/home" \
        XDG_RUNTIME_DIR="$root/run" XDG_CONFIG_HOME="$root/home/.config" \
        XDG_DATA_HOME="$root/home/.local/share" XDG_CACHE_HOME="$root/home/.cache" \
        XDG_STATE_HOME="$root/home/.local/state" GSETTINGS_BACKEND=keyfile \
        DBUS_SYSTEM_BUS_ADDRESS="unix:path=$root/run/system_bus" \
        OUT="$out" MONITORS="$monitors" LAYOUT="$layout" EXTENSION="$extension" \
        PROGRAM="$program" STEPS="$steps" \
        PETLAUNCHER_WORDNET_PATH="${PETLAUNCHER_WORDNET_PATH:-}" \
        timeout 1200 dbus-run-session -- "${BASH_SOURCE[0]}" --inner 2>"$out/bus.log" || status=$?
    kill "$system_bus_pid" 2>/dev/null || true
    # Services of the private bus can still write for a moment after it stops.
    for _ in 1 2 3 4 5; do
        rm -rf "$root" 2>/dev/null && break
        sleep 1
    done
    echo "evidence: $out"
    exit "$status"
fi

# Inner part: runs on the private session bus.
export WAYLAND_DISPLAY=wayland-pet GDK_BACKEND=wayland XDG_CURRENT_DESKTOP=GNOME
# The services that the bus starts, for example the portal backends, use the
# display of the test shell. libadwaita reads the light or dark style through
# the Settings portal of xdg-desktop-portal-gnome, which needs a display.
dbus-update-activation-environment WAYLAND_DISPLAY XDG_CURRENT_DESKTOP GDK_BACKEND
shell_pid=""
app_pids=()

cleanup() {
    for pid in "${app_pids[@]}"; do kill "$pid" 2>/dev/null || true; done
    if [[ -n $shell_pid ]]; then
        kill "$shell_pid" 2>/dev/null || true
        wait "$shell_pid" 2>/dev/null || true
    fi
}
trap cleanup EXIT

# Runs JavaScript in the test shell and prints the result as JSON.
shell_eval() {
    busctl --user --json=short call org.gnome.Shell /org/gnome/Shell org.gnome.Shell Eval s "$1" |
        jq -e -r 'if .data[0] then .data[1] else error("Eval failed: \(.data[1])") end'
}

app_action() { # bus name, action, [parameter as GVariant text, for example <true>]
    local dest=$1 action=$2 parameter=${3:-}
    gdbus call --session --dest "$dest" --object-path "/${dest//.//}" \
        --method org.freedesktop.Application.ActivateAction "$action" "[$parameter]" '{}' >/dev/null
}

start_app() { # name, then arguments of fake-petlauncher.js
    local name=$1 dest=pet.repo.Petlauncher
    shift
    [[ "$*" == *--app-id* ]] && dest=$(sed -E 's/.*--app-id ([^ ]+).*/\1/' <<<"$*")
    gjs -m "$here/fake-petlauncher.js" "$@" >>"$OUT/app-$name.log" 2>&1 &
    app_pids+=($!)
    for _ in $(seq 100); do
        gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
            --method org.freedesktop.DBus.NameHasOwner "$dest" | grep -q true && return 0
        sleep 0.1
    done
    echo "the test application $name did not start" >&2
    return 1
}

wait_name_gone() {
    for _ in $(seq 50); do
        gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
            --method org.freedesktop.DBus.NameHasOwner "$1" | grep -q false && return 0
        sleep 0.1
    done
    return 1
}

monitor_center() { # index -> "x y"
    jq -r --argjson i "$1" '.monitors[$i].geometry | "\(.x + (.width / 2 | floor)) \(.y + (.height / 2 | floor))"' \
        "$OUT/state-start.json"
}

pointer_to_monitor() {
    read -r x y < <(monitor_center "$1")
    shell_eval "petProbe.pointer($x, $y)" >/dev/null
}

# Saves the state of the shell under a step name, with the time of the step.
snapshot() {
    shell_eval 'petProbe.state()' | jq --arg step "$1" --argjson t "$2" '. + {step: $step, t: $t}' \
        >"$OUT/state-$1.json"
}

now() { shell_eval 'petProbe.now()'; }

screenshot() { shell_eval "petProbe.screenshot('$OUT/$1.png')" >/dev/null; }

close_window() { # title
    shell_eval "global.get_window_actors().map(a => a.meta_window)
        .filter(w => w.get_title() === '$1').forEach(w => w.delete(global.get_current_time()))" >/dev/null
    sleep 0.5
}

# One show of a window: pointer to a monitor, the action, wait for the paint
# record of the probe (2 s), snapshot.
show_step() { # step monitor dest action
    local step=$1 monitor=$2 dest=$3 action=$4 t
    pointer_to_monitor "$monitor"
    sleep 0.2
    t=$(now)
    app_action "$dest" "$action"
    sleep 2.3
    snapshot "$step" "$t"
    echo "$step" >>"$OUT/steps.txt"
}

# Writes the events, stops the shell and makes the result table.
finish_test() {
    shell_eval 'petProbe.events()' >"$OUT/events.json"
    shell_eval 'petProbe.finish()' >/dev/null || true
    wait "$shell_pid" 2>/dev/null || true
    shell_pid=""
    grep -E "JS ERROR|JS WARNING|$uuid" "$OUT/shell.log" >"$OUT/shell-js-errors.txt" || true
    "$here/analyse.sh" "$OUT" | tee "$OUT/summary.txt"
    if [[ -n $PROGRAM ]]; then
        cp "$HOME/launches.log" "$OUT/launches.log" 2>/dev/null || true
        "$here/analyse-program.sh" "$OUT" | tee -a "$OUT/summary.txt"
    fi
}

gnome-shell --headless --wayland --no-x11 --wayland-display="$WAYLAND_DISPLAY" \
    $(for m in $MONITORS; do echo --virtual-monitor "$m"; done) \
    --automation-script="$here/shell-probe.js" >"$OUT/shell.log" 2>&1 &
shell_pid=$!
for _ in $(seq 150); do
    shell_eval 'petProbe.state()' >/dev/null 2>&1 && break
    sleep 0.2
done
sleep 1
shell_eval 'petProbe.displayState()' >"$OUT/display-before.json"
if [[ -n $LAYOUT ]]; then
    shell_eval "petProbe.applyLayout($LAYOUT)" >"$OUT/display-after.json"
    sleep 2
fi
gnome-shell --version >"$OUT/versions.txt"
gjs --version >>"$OUT/versions.txt"
snapshot start 0
: >"$OUT/steps.txt"

if [[ -n $PROGRAM ]]; then
    # shellcheck source=program-steps.sh
    source "$STEPS"
    finish_test
    exit 0
fi

app=pet.repo.Petlauncher
start_app main
sleep 0.5
snapshot app-started "$(now)"

# Launcher: the first show, then shows again on both monitors.
show_step launcher-first-m1 1 $app show
screenshot launcher-first-m1
app_action $app hide; sleep 0.5
show_step launcher-m0 0 $app show
screenshot launcher-m0
app_action $app hide; sleep 0.5
show_step launcher-again-m1 1 $app show
app_action $app hide; sleep 0.5
show_step launcher-again-m0 0 $app show
app_action $app hide; sleep 0.5

# Results grow below the query line, and the next show is tall.
show_step launcher-grow-m1 1 $app show
t=$(now); app_action $app grow; sleep 1
snapshot launcher-grown-m1 "$t"
echo launcher-grown-m1 >>"$OUT/steps.txt"
app_action $app hide; sleep 0.5
show_step launcher-tall-m0 0 $app show
app_action $app hide; app_action $app shrink; sleep 0.5

# Other windows of petlauncher stay where GNOME puts them.
show_step settings-m1 1 $app settings
screenshot settings-m1
close_window 'petlauncher Settings'
show_step dictionary-m0 0 $app dictionary
close_window 'Dictionary'

# The pointer decides the monitor, not the focused window.
pointer_to_monitor 0
app_action $app dictionary; sleep 1
snapshot focus-before "$(now)"
show_step launcher-focus-m0-pointer-m1 1 $app show
app_action $app hide; sleep 0.5
close_window 'Dictionary'

# A launcher that gets its title after the map stays where GNOME puts it.
show_step late-title-m1 1 $app show-late-title
app_action $app hide; sleep 0.5

# A different application with the title of the launcher stays.
start_app other --app-id org.example.Other
show_step other-app-m1 1 org.example.Other show
app_action org.example.Other quit
wait_name_gone org.example.Other

# Tray icon.
snapshot tray-running "$(now)"
if [[ $EXTENSION == on ]]; then
    # disable() must remove the icon and stop the moves; enable() again must
    # restore both. GNOME disables the extensions while the screen is locked.
    shell_eval "Main.extensionManager.disableExtension('$uuid')" >/dev/null; sleep 0.5
    snapshot ext-disabled "$(now)"
    show_step ext-disabled-m1 1 $app show
    app_action $app hide; sleep 0.5
    shell_eval "Main.extensionManager.enableExtension('$uuid')" >/dev/null; sleep 1
    show_step ext-enabled-again-m1 1 $app show
    app_action $app hide; sleep 0.5
    snapshot tray-enabled-again "$(now)"

    read -r ix iy < <(jq -r '.tray.extents | "\(.x + (.width / 2 | floor)) \(.y + (.height / 2 | floor))"' \
        "$OUT/state-tray-enabled-again.json")
    t=$(now); shell_eval "petProbe.click($ix, $iy, 1)" >/dev/null; sleep 2.3
    snapshot tray-left-click "$t"; echo tray-left-click >>"$OUT/steps.txt"
    screenshot tray-left-click
    app_action $app hide; sleep 0.5

    shell_eval "petProbe.click($ix, $iy, 3)" >/dev/null; sleep 0.5
    snapshot tray-menu "$(now)"
    screenshot tray-menu
    read -r mx my < <(jq -r '.tray.items[] | select(.label == "Show petlauncher") | .extents |
        "\(.x + (.width / 2 | floor)) \(.y + (.height / 2 | floor))"' "$OUT/state-tray-menu.json")
    t=$(now); shell_eval "petProbe.click($mx, $my, 1)" >/dev/null; sleep 2.3
    snapshot tray-menu-show "$t"; echo tray-menu-show >>"$OUT/steps.txt"
    app_action $app hide; sleep 0.5

    shell_eval "petProbe.click($ix, $iy, 3)" >/dev/null; sleep 0.5
    snapshot tray-menu-1 "$(now)"
    read -r sx sy < <(jq -r '.tray.items[] | select(.label == "Settings") | .extents |
        "\(.x + (.width / 2 | floor)) \(.y + (.height / 2 | floor))"' "$OUT/state-tray-menu-1.json")
    t=$(now); shell_eval "petProbe.click($sx, $sy, 1)" >/dev/null; sleep 2.3
    snapshot tray-settings "$t"; echo tray-settings >>"$OUT/steps.txt"
    close_window 'petlauncher Settings'

    shell_eval "petProbe.click($ix, $iy, 3)" >/dev/null; sleep 0.5
    snapshot tray-menu-2 "$(now)"
    read -r qx qy < <(jq -r '.tray.items[] | select(.label == "Quit") | .extents |
        "\(.x + (.width / 2 | floor)) \(.y + (.height / 2 | floor))"' "$OUT/state-tray-menu-2.json")
    shell_eval "petProbe.click($qx, $qy, 1)" >/dev/null
    wait_name_gone $app && echo "quit: the program left the bus" >>"$OUT/app-main.log"
    sleep 0.5
    snapshot tray-after-quit "$(now)"

    start_app no-tray --no-tray-icon
    sleep 0.5
    snapshot tray-setting-off "$(now)"
    app_action $app set-tray-icon '<true>'; sleep 0.5
    snapshot tray-setting-on "$(now)"
    app_action $app set-tray-icon '<false>'; sleep 0.5
    snapshot tray-setting-off-again "$(now)"
    app_action $app quit
    wait_name_gone $app
fi

finish_test
