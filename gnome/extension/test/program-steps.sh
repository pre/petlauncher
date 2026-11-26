# shellcheck shell=bash disable=SC2088,SC2154
# SC2088: each ~ is text that the test types. SC2154: nested-test.sh sets
# here, uuid and the other variables.
# The steps of `nested-test.sh --program PATH`: the real petlauncher binary
# with the real extension. nested-test.sh sources this file in its inner
# part, so the functions of that part are available. analyse-program.sh
# reads the evidence.
#
# Evidence that this file adds to OUT:
#   app-program.log   the log of the program, with a line "== NAME" before
#                     each step that gives an action at the seam
#   toggle-times.txt  the time of each `petlauncher toggle`, in microseconds
#   checks.txt        the names of the states that are not a show
#   clipboard-*.txt   the text on the clipboard, read in the shell
#   commands.txt      the exit status and time of the other commands

app=pet.repo.Petlauncher
config=$HOME/.config/petlauncher
collapsed=$(sed -nE 's/^export const COLLAPSED_HEIGHT = ([0-9]+);/\1/p' "$here/../$uuid/launcher.js")
program_log=$OUT/app-program.log
export PETLAUNCHER_DEBUG=1

wait_name_owned() {
    for _ in $(seq 100); do
        gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
            --method org.freedesktop.DBus.NameHasOwner "$1" | grep -q true && return 0
        sleep 0.1
    done
    echo "$1 did not start" >&2
    return 1
}

# Starts the program in the background and waits for its bus name.
start_program() {
    "$PROGRAM" "$@" >>"$program_log" 2>&1 &
    app_pids+=($!)
    wait_name_owned $app
}

mark() { echo "== $1" >>"$program_log"; }

# `petlauncher toggle`, the command of the global key.
toggle() { # name
    local start end
    start=$(date +%s%N)
    "$PROGRAM" toggle >>"$program_log" 2>&1
    end=$(date +%s%N)
    echo "$1 $(((end - start) / 1000))" >>"$OUT/toggle-times.txt"
}

# One show with the toggle command: pointer to a monitor, the command, wait
# for the paint records of the probe (2 s), snapshot.
toggle_step() { # step monitor
    local step=$1 t
    pointer_to_monitor "$2"
    sleep 0.2
    t=$(now)
    toggle "$step"
    sleep 2.3
    snapshot "$step" "$t"
    echo "$step" >>"$OUT/steps.txt"
}

hide_with_toggle() {
    toggle "hide-$1"
    sleep 0.5
}

# A state that is not a show of the launcher.
check() {
    snapshot "$1" "$(now)"
    echo "$1" >>"$OUT/checks.txt"
}

keys() {
    local list
    list=$(printf "'%s'," "$@")
    shell_eval "petProbe.keys(${list%,})" >/dev/null
    sleep 0.4
}

# Types the text and waits for the search.
type_text() {
    shell_eval "petProbe.type($(jq -cn --arg text "$1" '$text'))" >/dev/null
    sleep 0.8
}

shot() {
    shell_eval "petProbe.screenshotWindow('${2:-petlauncher}', '$OUT/$1.png')" >/dev/null
    sleep 0.5
}

# Changes one key of settings.toml as an editor does: a new file and a
# rename. The program reads the file again after 0.1 s.
set_setting() { # key value
    local file=$config/settings.toml
    { grep -v "^$1 = " "$file" || true; echo "$1 = $2"; } >"$file.new"
    mv "$file.new" "$file"
    sleep 0.7
}

read_clipboard() { # name
    shell_eval 'petProbe.readClipboard()' >/dev/null
    sleep 0.5
    shell_eval 'petProbe.clipboard()' | jq -r '. // "(no text)"' >"$OUT/clipboard-$1.txt"
}

center_of_tray_item() { # state file, label or empty for the icon
    if [[ -z $2 ]]; then
        jq -r '.tray.extents | "\(.x + (.width / 2 | floor)) \(.y + (.height / 2 | floor))"' "$1"
    else
        jq -r --arg label "$2" '.tray.items[] | select(.label == $label) | .extents |
            "\(.x + (.width / 2 | floor)) \(.y + (.height / 2 | floor))"' "$1"
    fi
}

: >"$program_log"
: >"$OUT/toggle-times.txt"
: >"$OUT/checks.txt"
: >"$OUT/commands.txt"
"$PROGRAM" --version >>"$OUT/versions.txt"
echo "GTK $(pkg-config --modversion gtk4), libadwaita $(pkg-config --modversion libadwaita-1)" >>"$OUT/versions.txt"
shell_eval 'petProbe.prepareKeyboard()' >/dev/null
sleep 1

# Start at login: no window. The tray icon shows.
start_program --hidden
sleep 1
check app-started

# R11: the first show of the process, with no rows, on monitor 1, then a
# show on monitor 0.
toggle_step launcher-first-m1 1
shot launcher-first-m1
hide_with_toggle launcher-first-m1
toggle_step launcher-empty-m0 0
shot launcher-empty-m0

# R1 step 1: the contents of a typed directory, no selected row. Enter opens
# the directory and hides the window.
type_text '~/pathtest/'
check typed-directory-m0
shot typed-directory-m0
mark enter-on-typed-directory
keys Return
check after-enter-on-directory

# R1 step 3: Down selects the newest file. Enter opens it.
toggle_step launcher-after-activation-m0 0
type_text '~/pathtest/'
keys Down
shot down-selected-m0
mark down-enter
keys Return
check after-down-enter

# R9 and R11: Escape keeps the query. The next show has the rows at its
# first frame, and the query line at the place of the empty launcher.
toggle_step launcher-before-escape-m0 0
type_text '~/pathtest/'
keys Escape
check after-escape
toggle_step launcher-kept-escape-m0 0
shot launcher-kept-escape-m0
keys Escape
toggle_step launcher-kept-escape-m1 1
shot launcher-kept-escape-m1

# R3 and R9: a different window takes the focus. The launcher hides and
# keeps the query.
gjs -m "$here/fake-petlauncher.js" --app-id org.example.Other >>"$OUT/app-other.log" 2>&1 &
app_pids+=($!)
wait_name_owned org.example.Other
app_action org.example.Other dictionary
sleep 1
check after-focus-loss
toggle_step launcher-kept-focus-m1 1

# R9: a typed character replaces the selected kept query. A shortcut row:
# Enter gives OpenUrl.
type_text 'g rust'
check typed-shortcut-m1
mark shortcut-enter
keys Return
check after-shortcut-enter
close_window Dictionary
app_action org.example.Other quit
wait_name_gone org.example.Other

# A system command row: Enter gives the action at the seam only.
toggle_step launcher-lock-m0 0
type_text 'lock'
shot lock-row-m0
mark lock-enter
keys Return

# Tab completes the selected directory row. Enter then opens the directory.
toggle_step launcher-tab-m0 0
type_text '~/pathtest/alp'
keys Tab
sleep 0.5
shot tab-completed-m0
mark tab-then-enter
keys Return

# R2: Ctrl+Enter on a file row shows the file in its folder.
toggle_step launcher-ctrl-enter-m0 0
type_text '~/pathtest/'
keys Down
mark ctrl-enter
keys Control_L+Return

# R13: Shift+Enter copies the path. The text stays on the clipboard after the
# window hides, because the process stays.
toggle_step launcher-shift-enter-m0 0
type_text '~/pathtest/'
keys Down
mark shift-enter
keys Shift_L+Return
sleep 0.5
check after-shift-enter
read_clipboard background-on

# The pointer activates a row: a click on the first row.
toggle_step launcher-click-m0 0
type_text '~/pathtest/'
read -r cx cy < <(shell_eval 'petProbe.state()' | jq -r --argjson collapsed "$collapsed" \
    '.windows[] | select(.title == "petlauncher") | .frame | "\(.x + 100) \(.y + $collapsed + 30)"')
mark click-first-row
shell_eval "petProbe.click($cx, $cy, 1)" >/dev/null
sleep 0.7
check after-click

# R17: the dark style of the system. The same rows as typed-directory-m0.
toggle_step launcher-dark-m0 0
type_text '~/pathtest/'
keys Down
gsettings set org.gnome.desktop.interface color-scheme prefer-dark
sleep 1
shot down-selected-m0-dark
gsettings set org.gnome.desktop.interface color-scheme default
sleep 1
shot down-selected-m0-light-again
keys Escape

# R5: a new window width in settings.toml.
set_setting window_width 900
toggle_step launcher-width-900-m0 0
hide_with_toggle launcher-width-900-m0
set_setting window_width 750

# R3: the tray icon follows show_tray_icon in settings.toml.
set_setting show_tray_icon false
check tray-setting-off
set_setting show_tray_icon true
check tray-setting-on

# The actions of the extension: a click on the icon shows the launcher, the
# menu items Settings and Quit.
read -r ix iy < <(center_of_tray_item "$OUT/state-tray-setting-on.json" "")
t=$(now)
shell_eval "petProbe.click($ix, $iy, 1)" >/dev/null
sleep 2.3
snapshot tray-left-click "$t"
echo tray-left-click >>"$OUT/steps.txt"
keys Escape

shell_eval "petProbe.click($ix, $iy, 3)" >/dev/null
sleep 0.5
snapshot tray-menu "$(now)"
read -r sx sy < <(center_of_tray_item "$OUT/state-tray-menu.json" Settings)
t=$(now)
shell_eval "petProbe.click($sx, $sy, 1)" >/dev/null
sleep 2.3
snapshot tray-settings "$t"
echo tray-settings >>"$OUT/steps.txt"
shot tray-settings 'petlauncher Settings'
close_window 'petlauncher Settings'

shell_eval "petProbe.click($ix, $iy, 3)" >/dev/null
sleep 0.5
snapshot tray-menu-2 "$(now)"
read -r qx qy < <(center_of_tray_item "$OUT/state-tray-menu-2.json" Quit)
shell_eval "petProbe.click($qx, $qy, 1)" >/dev/null
if wait_name_gone $app; then
    echo "quit from the tray menu: the program left the bus" >>"$OUT/commands.txt"
fi
sleep 0.5
check tray-after-quit

# R4: `petlauncher toggle` with no running program starts it and shows the
# launcher. This process stays as the program.
pointer_to_monitor 1
sleep 0.2
t=$(now)
"$PROGRAM" toggle >>"$program_log" 2>&1 &
app_pids+=($!)
sleep 2.5
snapshot launcher-cold-toggle-m1 "$t"
echo launcher-cold-toggle-m1 >>"$OUT/steps.txt"
app_action $app quit
wait_name_gone $app

# R3 "Run in background" off: the process stops after the window hides. The
# copied text stays on the clipboard: GNOME Shell keeps a copy when the owner
# of the clipboard stops.
set_setting run_in_background false
pointer_to_monitor 0
sleep 0.2
t=$(now)
start_program
sleep 2.3
snapshot launcher-no-background-m0 "$t"
echo launcher-no-background-m0 >>"$OUT/steps.txt"
type_text '~/pathtest/'
keys Down Down
mark shift-enter-no-background
keys Shift_L+Return
if wait_name_gone $app; then
    echo "run_in_background false: the program stopped after the window hid" >>"$OUT/commands.txt"
fi
read_clipboard background-off
start=$(date +%s%N)
status=0
"$PROGRAM" --hidden >>"$program_log" 2>&1 || status=$?
end=$(date +%s%N)
echo "run_in_background false: --hidden stopped with status $status after $(((end - start) / 1000000)) ms" \
    >>"$OUT/commands.txt"
set_setting run_in_background true
