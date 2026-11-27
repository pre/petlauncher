# shellcheck shell=bash disable=SC2154
# SC2154: nested-test.sh sets here, OUT, PROGRAM and the other variables.
#
# The steps of the settings window, for
#   nested-test.sh --program PATH --steps gnome/extension/test/settings-steps.sh --out OUT
# nested-test.sh starts the test shell with the R4 conflict on Super+Space
# for this file, as for program-steps.sh.
#
# The steps find a widget by its role and name through AT-SPI (a11y.py) and
# then click it or type into it with the virtual pointer and keyboard of the
# test shell. The functions of program-steps.sh are not available: this
# file has its own copies.
#
# Evidence that this file adds to OUT:
#   app-program.log        the log of the program, with "== NAME" before steps
#   acceptance.tsv         step, PASS or FAIL, check, evidence file
#   settings-*.png         screenshots of the settings window
#   launcher-*.png         screenshots of the launcher
#   a11y-*.txt             the widget tree of the settings window at a step
#   keyfile-*.ini          the GSettings key file of the test at a step
#   settings-*.toml        settings.toml at a step, shortcuts-*.toml too

app=pet.repo.Petlauncher
title='petlauncher Settings'
config=$HOME/.config/petlauncher
state_dir=$HOME/.local/state/petlauncher
program_log=$OUT/app-program.log
acceptance=$OUT/acceptance.tsv
launches=$HOME/launches.log
export PETLAUNCHER_DEBUG=1
export PETLAUNCHER_OFFLINE=1

wait_name_owned() {
    for _ in $(seq 100); do
        gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
            --method org.freedesktop.DBus.NameHasOwner "$1" | grep -q true && return 0
        sleep 0.1
    done
    echo "$1 did not start" >&2
    return 1
}

start_program() {
    "$PROGRAM" "$@" >>"$program_log" 2>&1 &
    app_pids+=($!)
    wait_name_owned $app
}

mark() { echo "== $1" >>"$program_log"; }

keys() {
    local list
    list=$(printf "'%s'," "$@")
    shell_eval "petProbe.keys(${list%,})" >/dev/null
    sleep 0.4
}

type_text() {
    shell_eval "petProbe.type($(jq -cn --arg text "$1" '$text'))" >/dev/null
    sleep 0.8
}

shot() { # name [window title]
    shell_eval "petProbe.screenshotWindow('${2:-$title}', '$OUT/$1.png')" >/dev/null ||
        echo "${step:-}: no window '${2:-$title}' for $1.png" >>"$OUT/commands.txt"
    sleep 0.5
}

# The input has this text. grep -q at the end of a pipe can stop the
# writer with SIGPIPE, and pipefail then gives a failure.
has() { grep -F -- "$1" >/dev/null; }
keyfile() { /usr/bin/cat "$HOME/.config/glib-2.0/settings/keyfile"; }
keyfile_has() { keyfile | has "$1"; }
# The binding of a custom shortcut in the key file: "scratch-notes" or
# "petlauncher".
binding_is() { # shortcut accelerator
    keyfile | grep -A3 "custom-keybindings/$1\]" | grep -x "binding='$2'" >/dev/null
}
settings_has() { grep -qF -- "$1" "$config/settings.toml"; }
shortcuts_has() { grep -qF -- "$1" "$config/shortcuts.toml"; }

# Starts a requirement step: the evidence after this point belongs to it.
begin() {
    step=$1
    mark "$step"
    launch_count=$(wc -l <"$launches")
    log_count=$(wc -l <"$program_log")
}
new_launches() { tail -n +"$((launch_count + 1))" "$launches"; }
new_log() { tail -n +"$((log_count + 1))" "$program_log"; }
logged() { new_log | has "$1"; }
launched() { # text of the launch record; waits up to 3 s for the start
    for _ in $(seq 30); do
        new_launches | has "$1" && return 0
        sleep 0.1
    done
    return 1
}
last_rows() { new_log | grep -F ": rows for " | tail -1; }
last_titles() { last_rows | sed -E 's/.*, selected (None|Some\([0-9]+\)): //'; }
rows_have() { last_titles | has "$1"; }
rows_lack() { ! rows_have "$1"; }

# description, evidence file, then a command that succeeds for PASS.
expect() {
    local description=$1 evidence=$2 result=FAIL
    shift 2
    "$@" >/dev/null 2>&1 && result=PASS
    printf '%s\t%s\t%s\t%s\n' "$step" "$result" "$description" "$evidence" >>"$acceptance"
}

windows_state() { shell_eval 'petProbe.state()'; }
window_shows() { windows_state | jq -e --arg t "$1" 'any(.windows[]; .title == $t)'; }
window_focused() { windows_state | jq -e --arg t "$1" 'any(.windows[]; .title == $t and .focus)'; }
# GNOME Shell gets its key Super+F9: the window with the focus does not
# pause the shortcuts of GNOME Shell.
shell_gets_key() {
    local before after
    before=$(shell_eval 'petProbe.shellKeyPresses()')
    keys Super_L+F9
    after=$(shell_eval 'petProbe.shellKeyPresses()')
    echo "$step: GNOME Shell got Super+F9 $before times before, $after times after" \
        >>"$OUT/commands.txt"
    [[ $after -gt $before ]]
}
window_hidden() { ! window_shows "$1"; }
frame_of() { # title -> "x y width height"
    windows_state | jq -r --arg t "$1" \
        '.windows[] | select(.title == $t) | .frame | "\(.x) \(.y) \(.width) \(.height)"'
}

a11y() { /usr/bin/python3 -I "$here/a11y.py" "$@"; }
tree() { # name [window title]
    a11y tree "${2:-$title}" >"$OUT/a11y-$1.txt" 2>&1
}
# The tree of the window has a line with this text.
tree_has() {
    local tree
    tree=$(a11y tree "$title" 2>/dev/null)
    has "$1" <<<"$tree"
}
tree_lacks() { ! tree_has "$1"; }

# A click in the middle of a box "x y width height" of the window.
click_box() { # x y width height [window title]
    local fx fy
    read -r fx fy _ _ < <(frame_of "${5:-$title}")
    shell_eval "petProbe.click($((fx + $1 + $3 / 2)), $((fy + $2 + $4 / 2)), 1)" >/dev/null
    sleep 0.8
}

# A widget that is not there is a line in commands.txt, and the checks of
# the step show the missing effect.
click() { # role name [n] [window title]
    local box
    box=$(a11y find "${4:-$title}" "$1" "$2" "${3:-0}" 2>/dev/null) || {
        echo "$step: no $1 '$2' to click" >>"$OUT/commands.txt"
        return 0
    }
    # shellcheck disable=SC2086
    click_box $box "${4:-$title}"
}

page() { # name of a page in the list at the left edge
    click 'list item' "$1"
    sleep 0.4
}

# A drag with the primary button: from the middle of a box of the window,
# then to points of the window, then the release.
drag_start() { # x y width height
    local fx fy
    read -r fx fy _ _ < <(frame_of "$title")
    shell_eval "petProbe.pointer($((fx + $1 + $3 / 2)), $((fy + $2 + $4 / 2)))" >/dev/null
    sleep 0.2
    shell_eval "petProbe.button(1, true)" >/dev/null
    sleep 0.2
}
drag_to() { # x y
    local fx fy
    read -r fx fy _ _ < <(frame_of "$title")
    shell_eval "petProbe.pointer($((fx + $1)), $((fy + $2)))" >/dev/null
    sleep 0.3
}
drag_end() {
    shell_eval "petProbe.button(1, false)" >/dev/null
    sleep 1
}

# The dialog of the global key asks GNOME Shell to pause its shortcuts.
# GNOME Shell asks first, with Allow selected: Return selects it.
allow_inhibit() {
    sleep 1
    if [[ $(shell_eval 'petProbe.modalCount()') != 0 ]]; then
        screenshot shell-inhibit-question
        keys Return
        sleep 1
    fi
}

# The text of a widget through AT-SPI.
text_of() { a11y text "$title" "$@"; }
states_of() { a11y states "$title" "$@"; }
is_sensitive() { [[ " $(states_of "$@") " == *" sensitive "* ]]; }
is_checked() { [[ " $(states_of "$@") " == *" checked "* ]]; }

# Changes one top-level key of settings.toml as an editor does: a new file
# and a rename. The key goes to the first line, before each table.
set_setting() { # key value
    local file=$config/settings.toml
    { echo "$1 = $2"; grep -v "^$1 = " "$file" || true; } >"$file.new"
    mv "$file.new" "$file"
    sleep 0.9
}

# The launcher with an empty query line on monitor 0.
open_launcher() {
    if window_shows petlauncher; then
        "$PROGRAM" toggle >>"$program_log" 2>&1
        sleep 0.5
    fi
    pointer_to_monitor 0
    sleep 0.2
    "$PROGRAM" toggle >>"$program_log" 2>&1
    sleep 1
    keys Control_L+a BackSpace
}

query() { # text
    open_launcher
    type_text "$1"
}

hide_launcher() {
    if window_shows petlauncher; then
        keys Escape
    fi
}

# The settings window from the launcher with Ctrl+comma.
open_settings() {
    open_launcher
    keys Control_L+comma
    sleep 1.5
}

# The settings window at its default size, or tall, so that the steps reach
# each row with no scroll.
size_settings() { # height
    shell_eval "global.get_window_actors().map(a => a.meta_window)
        .find(w => w.get_title() === '$title').move_resize_frame(false, 600, 50, 900, $1)" >/dev/null
    sleep 0.8
}

# Screenshots of the four pages.
page_shots() { # suffix
    local name
    for name in General 'Global Key' Search Shortcuts; do
        page "$name"
        local file=${name// /-}
        shot "settings-${file,,}-$1"
        tree "${file,,}-$1"
    done
    page General
}

: >"$program_log"
: >"$OUT/commands.txt"
: >"$acceptance"
touch "$launches"
"$PROGRAM" --version >>"$OUT/versions.txt"
echo "GTK $(pkg-config --modversion gtk4), libadwaita $(pkg-config --modversion libadwaita-1)" >>"$OUT/versions.txt"
shell_eval 'petProbe.prepareKeyboard()' >/dev/null
# A key of GNOME Shell, as gsd-media-keys takes the global key of petlauncher.
shell_eval "petProbe.grabShellKey('<Super>F9')" >/dev/null
sleep 1

# R4 writes GNOME settings. Before the first start, prove that the default
# GSettings backend of this environment is the key file of the test, not
# dconf of the user.
backend=$(gjs -c "imports.gi.versions.Gio = '2.0'; const Gio = imports.gi.Gio;
    const backend = Gio.SettingsBackend.get_default();
    print(backend.constructor.\$gtype.name + ' ' + backend.filename);")
echo "GSettings backend before the start: $backend" >>"$OUT/commands.txt"
if [[ $backend != "GKeyfileSettingsBackend $HOME/"* ]]; then
    echo "the GSettings backend is not the key file of the test: $backend" >&2
    exit 1
fi
keyfile >"$OUT/keyfile-before.ini"
cp "$config/settings.toml" "$OUT/settings-before.toml"
cp "$config/shortcuts.toml" "$OUT/shortcuts-before.toml"

begin start
start_program --hidden
sleep 1
expect "the start writes no key: two bindings have it" app-program.log \
    logged "the global key <Super>space is not set"

# The button of the notice of R4 opens the window on the Global Key page.
begin open-from-the-notice
open_launcher
shot launcher-notice petlauncher
tree launcher-notice petlauncher
expect "the notice names the key and the bindings" a11y-launcher-notice.txt \
    grep -q "have Super+Space:" "$OUT/a11y-launcher-notice.txt"
click 'push button' 'Open Global Key Settings' 0 petlauncher
sleep 1.5
tree settings-from-notice
shot settings-from-notice
expect "the button of the notice opens the settings window" app-program.log window_shows "$title"
expect "the launcher hides" app-program.log window_hidden petlauncher
expect "the window shows the Global Key page" a11y-settings-from-notice.txt \
    grep -q "list item 'Global Key' .*selected" "$OUT/a11y-settings-from-notice.txt"
close_window "$title"

# The gear at the right edge of the query line.
begin open-with-the-gear
open_launcher
click 'push button' Settings 0 petlauncher
sleep 1.5
expect "the gear opens the settings window" app-program.log window_shows "$title"
expect "the launcher hides" app-program.log window_hidden petlauncher
close_window "$title"

# The window from the launcher: Ctrl+comma.
begin open-with-ctrl-comma
open_launcher
t=$(now)
keys Control_L+comma
sleep 1.5
# analyse.sh shows the place of the window: the extension must not move it.
snapshot settings-ctrl-comma "$t"
echo settings-ctrl-comma >>"$OUT/steps.txt"
expect "Ctrl+comma opens the settings window" app-program.log window_shows "$title"
expect "the launcher hides" app-program.log window_hidden petlauncher
page_shots light
gsettings set org.gnome.desktop.interface color-scheme prefer-dark
sleep 1
page_shots dark
gsettings set org.gnome.desktop.interface color-scheme default
sleep 1

# The other steps use a tall window, so that each row shows with no scroll.
size_settings 1600
shot settings-tall-general

# The Remove button in the row of a list entry with this label.
click_remove_of() { # label
    local lx ly lw lh bx by bw bh
    read -r lx ly lw lh < <(a11y find "$title" label "$1" 2>/dev/null) || true
    read -r bx by bw bh < <(a11y find "$title" 'push button' Remove 2>/dev/null) || true
    if [[ -z ${ly:-} || -z ${bx:-} ]]; then
        echo "$step: no Remove button for '$1'" >>"$OUT/commands.txt"
        return 0
    fi
    click_box "$bx" "$((ly + lh / 2 - bh / 2))" "$bw" "$bh"
}

# Types into an entry row: a click in it, the old text away, the new text.
fill() { # role name text [n]
    click "$1" "$2" "${4:-0}"
    keys Control_L+a BackSpace
    type_text "$3"
}

autostart=$HOME/.config/autostart/$app.desktop
checked() { is_checked 'check box' "$1"; }
unchecked() { ! is_checked 'check box' "$1"; }
tray_visible() { windows_state | jq -e '.tray.exists and .tray.visible'; }
tray_hidden() { ! tray_visible; }
launcher_width() { read -r _ _ w _ < <(frame_of petlauncher); echo "$w"; }
hotkey_in_effect() { # key: the file has this key, or no line and the default
    if grep -q '^hotkey = ' "$config/settings.toml"; then
        settings_has "hotkey = \"$1\""
    else
        [[ $1 == '<Super>space' ]]
    fi
}

# R4: the page has one row. Its dialog shows the two conflicts, Remove on
# each, then Set.
begin R4-remove-and-set
page 'Global Key'
tree r4-page
shot settings-r4-page
expect "the row shows that petlauncher has no key" a11y-r4-page.txt \
    tree_has "push button 'Set shortcut…'"
expect "GNOME Shell gets its keys before the dialog" commands.txt shell_gets_key
click 'push button' 'Set shortcut…'
allow_inhibit
tree r4-conflicts
shot settings-r4-conflicts
expect "GNOME Shell pauses its shortcuts for the dialog" app-program.log \
    logged "shortcuts of GNOME Shell paused true"
expect "the custom shortcut shows as a conflict" a11y-r4-conflicts.txt \
    tree_has "list item 'Show Scratch Notes'"
expect "the input source switch shows as a conflict" a11y-r4-conflicts.txt \
    tree_has "list item 'Switch to next input source'"
expect "Set is off while a conflict is left" a11y-r4-conflicts.txt \
    eval "! is_sensitive 'push button' Set"
# GNOME Shell has Super+Space for the input source switch: the key comes to
# the dialog only while GNOME Shell pauses its shortcuts.
keys Super_L+space
expect "Super+Space comes to the dialog" app-program.log logged 'pressed "<Super>space"'
click 'push button' Remove 0
shot settings-r4-one-conflict
click 'push button' Remove 0
tree r4-no-conflict
shot settings-r4-no-conflict
keyfile >"$OUT/keyfile-after-remove.ini"
expect "the custom shortcut keeps its entry" keyfile-after-remove.ini \
    keyfile_has "custom-keybindings=['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/scratch-notes/']"
expect "the custom shortcut has an empty binding" keyfile-after-remove.ini \
    binding_is scratch-notes ''
expect "the built-in key keeps its other accelerators" keyfile-after-remove.ini \
    keyfile_has "switch-input-source=['XF86Keyboard']"
expect "no key of petlauncher before Set" keyfile-after-remove.ini \
    eval '! keyfile_has custom-keybindings/petlauncher'
expect "Set is on with no conflict" a11y-r4-no-conflict.txt is_sensitive 'push button' Set
click 'push button' Set
sleep 1
expect "the settings window has the focus after Set" app-program.log window_focused "$title"
expect "GNOME Shell gets its keys again after Set" commands.txt shell_gets_key
keyfile >"$OUT/keyfile-after-set.ini"
cp "$config/settings.toml" "$OUT/settings-after-set.toml"
tree r4-set
shot settings-r4-set
expect "the key is written to the test key file" keyfile-after-set.ini \
    binding_is petlauncher '<Super>space'
expect "the row shows the key of GNOME" a11y-r4-set.txt tree_has "push button 'Super+Space'"
expect "settings.toml has the same key" settings-after-set.toml hotkey_in_effect '<Super>space'
expect "the marker file exists" app-program.log test -e "$state_dir/global-key-installed"
open_launcher
tree launcher-after-set petlauncher
shot launcher-after-set petlauncher
expect "the launcher showed the notice before" a11y-launcher-notice.txt \
    grep -q "Show Scratch Notes" "$OUT/a11y-launcher-notice.txt"
expect "the notice in the launcher is gone" a11y-launcher-after-set.txt \
    eval "! grep -q 'Show Scratch Notes' '$OUT/a11y-launcher-after-set.txt'"
hide_launcher

# R4: a different key, pressed in the dialog.
begin R4-capture
click 'push button' 'Super+Space'
allow_inhibit
keys Control_L+Super_L+k
sleep 0.5
tree r4-captured
shot settings-r4-captured
expect "the pressed key shows in the dialog" a11y-r4-captured.txt tree_has "label 'Ctrl+Super+K'"
click 'push button' Set
sleep 1
keyfile >"$OUT/keyfile-after-capture.ini"
cp "$config/settings.toml" "$OUT/settings-after-capture.toml"
expect "the key file has the captured key" keyfile-after-capture.ini \
    binding_is petlauncher '<Control><Super>k'
expect "settings.toml has the captured key" settings-after-capture.toml \
    settings_has 'hotkey = "<Control><Super>k"'

# R4: BackSpace and Set remove the key. Then a key again.
begin R4-backspace
click 'push button' 'Ctrl+Super+K'
allow_inhibit
keys Escape
sleep 0.8
expect "Escape closes the dialog" app-program.log tree_lacks "label 'Set the Global Key'"
expect "GNOME Shell gets its keys again after Escape" commands.txt shell_gets_key
click 'push button' 'Ctrl+Super+K'
allow_inhibit
keys BackSpace
sleep 0.5
tree r4-no-key
shot settings-r4-no-key
expect "BackSpace offers no key" a11y-r4-no-key.txt tree_has "label 'No key'"
click 'push button' Set
sleep 1
keyfile >"$OUT/keyfile-after-remove-key.ini"
cp "$config/settings.toml" "$OUT/settings-after-remove-key.toml"
expect "Set with no key leaves petlauncher with an empty binding" keyfile-after-remove-key.ini \
    binding_is petlauncher ''
expect "settings.toml has no key" settings-after-remove-key.toml settings_has 'hotkey = ""'
click 'push button' 'Set shortcut…'
allow_inhibit
keys Control_L+Super_L+k
click 'push button' Set
sleep 1
keyfile >"$OUT/keyfile-after-again.ini"
expect "a pressed key is written again" keyfile-after-again.ini \
    binding_is petlauncher '<Control><Super>k'

# R9 step 8 and R3 "Run in background".
begin R9-step-8
page General
expect "the autostart file exists while the switch is on" app-program.log test -e "$autostart"
click 'check box' 'Run in background'
sleep 0.5
tree r9-off
shot settings-r9-background-off
expect "settings.toml has the change" settings.toml settings_has 'run_in_background = false'
expect "the warning of R9 shows" a11y-r9-off.txt \
    tree_has 'The query is not kept, because "Run in background" is off.'
expect "the unit list is disabled" a11y-r9-off.txt \
    eval "! is_sensitive 'combo box' 'Keep the query'"
expect "the time is disabled" a11y-r9-off.txt eval "! is_sensitive text Time"
expect "R3: the autostart file is gone" app-program.log test ! -e "$autostart"
click 'check box' 'Run in background'
sleep 0.5
tree r9-on
expect "the time setting is active again" a11y-r9-on.txt is_sensitive 'combo box' 'Keep the query'
expect "R3: the autostart file is back" app-program.log test -e "$autostart"

# R3 "Show tray icon".
begin R3-tray
expect "the tray icon shows" state tray_visible
click 'check box' 'Show tray icon'
windows_state >"$OUT/state-tray-off.json"
expect "the tray icon is gone after the switch" state-tray-off.json tray_hidden
click 'check box' 'Show tray icon'
windows_state >"$OUT/state-tray-on.json"
expect "the tray icon is back" state-tray-on.json tray_visible

# R3 "Include applications in search".
begin R3-applications
query 'Recorder Notes'
expect "an application row with the switch on" app-program.log rows_have 'Recorder Notes'
hide_launcher
click 'check box' 'Include applications in search'
begin R3-applications-off
query 'Recorder Notes'
shot launcher-no-applications petlauncher
expect "no application row with the switch off" app-program.log rows_lack 'Recorder Notes'
hide_launcher
click 'check box' 'Include applications in search'
begin R3-applications-on
query 'Recorder Notes'
expect "the application row is back" app-program.log rows_have 'Recorder Notes'
hide_launcher

# R3 "Close when losing focus".
begin R3-focus
click 'check box' 'Close when losing focus'
open_launcher
click label Behaviour
expect "the launcher stays after a click in a different window" state window_shows petlauncher
"$PROGRAM" toggle >>"$program_log" 2>&1
sleep 0.5
click 'check box' 'Close when losing focus'
open_launcher
click label Behaviour
expect "the launcher closes at a click in a different window" state window_hidden petlauncher

# R5: the width.
begin R5
fill 'spin button' '~^[0-9]+$' 900 0
keys Tab
sleep 0.5
cp "$config/settings.toml" "$OUT/settings-after-width.toml"
expect "settings.toml has the width" settings-after-width.toml settings_has 'window_width = 900'
open_launcher
windows_state >"$OUT/state-launcher-900.json"
shot launcher-width-900 petlauncher
expect "the launcher has the new width at its next show" state-launcher-900.json \
    eval "[[ \$(launcher_width) == 900 ]]"
hide_launcher

# R21 steps 1 to 4, and an entry with an error.
begin R21-step-1
query 'f palette-zoom'
sleep 0.5
shot launcher-r21-before petlauncher
expect "f palette-zoom finds the file" app-program.log rows_have 'palette-zoom.png'
hide_launcher
begin R21-step-2
page Search
cp "$config/settings.toml" "$OUT/settings-before-r21.toml"
fill text 'Add a path, a name or a pattern' '~/lolcat-evidence'
keys Return
sleep 1.5
cp "$config/settings.toml" "$OUT/settings-after-r21.toml"
diff "$OUT/settings-before-r21.toml" "$OUT/settings-after-r21.toml" >"$OUT/settings-r21.diff" || true
tree r21-added
shot settings-r21-added
expect "the window shows the new entry" a11y-r21-added.txt tree_has "label '~/lolcat-evidence'"
begin R21-step-3
query 'f palette-zoom'
sleep 0.5
shot launcher-r21-after petlauncher
expect "f palette-zoom does not find the file" app-program.log rows_lack 'palette-zoom.png'
hide_launcher
begin R21-step-4
expect "settings.toml contains the new path" settings-after-r21.toml \
    grep -qF '"~/lolcat-evidence"' "$OUT/settings-after-r21.toml"
expect "the comments of settings.toml are not changed" settings-r21.diff \
    diff <(grep '#' "$OUT/settings-before-r21.toml") <(grep '#' "$OUT/settings-after-r21.toml")
begin R21-bad-pattern
fill text 'Add a path, a name or a pattern' '[abc'
keys Return
tree r21-bad-pattern
shot settings-r21-bad-pattern
expect "a bad pattern shows its error" a11y-r21-bad-pattern.txt \
    tree_has 'The ignore pattern "[abc" is not correct'
expect "a bad pattern is not saved" settings.toml eval "! settings_has '[abc'"
keys Control_L+a BackSpace
begin R21-remove
click_remove_of '~/lolcat-evidence'
sleep 1.5
expect "Remove takes the entry from settings.toml" settings.toml \
    eval "! settings_has '~/lolcat-evidence'"
query 'f palette-zoom'
expect "f palette-zoom finds the file again" app-program.log rows_have 'palette-zoom.png'
hide_launcher

# R19: the data path and its state.
begin R19-data
tree dictionary-found
expect "the state of the data shows" a11y-dictionary-found.txt \
    eval "tree_has 'The WordNet data is found' || tree_has 'The dictionary is not installed'"
wordnet_before=$(text_of text 'Folder of the WordNet data')
fill text 'Folder of the WordNet data' "$HOME/no-wordnet"
keys Return
sleep 1
tree dictionary-missing
shot settings-dictionary-missing
expect "a missing folder shows the install command" a11y-dictionary-missing.txt \
    tree_has 'sudo apt install wordnet-base'
fill text 'Folder of the WordNet data' "$wordnet_before"
keys Return
sleep 1

# R12: a shortcut with a browser of the list.
begin R12
page Shortcuts
click 'push button' 'Add a Shortcut'
sleep 0.8
tree form-new
shot settings-form-new
fill text Name 'Docs in Edge'
fill text Keyword docsedge
click text Command
type_text 'https://docs.example.com/?q=%s'
click 'combo box' 'Open URLs with'
sleep 0.5
tree form-browser-list
shot settings-form-browser-list
click label 'Recording Edge'
sleep 0.5
tree form-r12
shot settings-form-r12
click 'push button' Save
sleep 0.8
cp "$config/shortcuts.toml" "$OUT/shortcuts-after-r12.toml"
expect "shortcuts.toml has the browser of the list" shortcuts-after-r12.toml \
    shortcuts_has 'browser = { app = "petfixture-edge.desktop" }'
query 'docsedge cats'
keys Return
expect "the recording browser gets the URL" launches.log \
    launched 'edge | https://docs.example.com/?q=cats'

# R7 with a shortcut of the window, and the checks at the fields.
begin R7
click 'push button' 'Add a Shortcut'
sleep 0.8
fill text Name 'Rektangel pulls'
fill text Keyword g
tree form-warning
shot settings-form-warning
expect "a keyword of a different shortcut is a warning" a11y-form-warning.txt \
    tree_has 'keyword "g"'
expect "Save stays active with a warning" a11y-form-warning.txt is_sensitive 'push button' Save
fill text Keyword 'c a'
tree form-error
shot settings-form-error
expect "a keyword with a space is an error" a11y-form-error.txt tree_has 'space'
expect "Save is not active with an error" a11y-form-error.txt eval "! is_sensitive 'push button' Save"
fill text Keyword rk
click text Command
type_text 'https://github.com/pre/rektangel/pull/%s'
shot settings-form-switch-off
click 'check box' 'Run without argument'
sleep 0.5
tree form-switch-on
shot settings-form-switch-on
expect "the switch shows the command without argument" a11y-form-switch-on.txt \
    tree_has "Different command without argument (optional)"
click text 'Command without argument'
type_text 'https://github.com/pre/rektangel/pulls'
shot settings-form-r7
click 'push button' Save
sleep 0.8
cp "$config/shortcuts.toml" "$OUT/shortcuts-after-r7.toml"
query 'rk 123'
keys Return
expect "R7 step 1: with an argument" launches.log \
    launched 'browser | https://github.com/pre/rektangel/pull/123'
begin R7-step-2
query rk
keys Return
expect "R7 step 2: with no argument" launches.log \
    launched 'browser | https://github.com/pre/rektangel/pulls'

# R6: reorder and remove in the list.
begin R6-order-and-remove
page Shortcuts
shot settings-shortcuts-list
# The handle of the last row to the upper half of the row before it.
read -r hx hy hw hh < <(a11y find "$title" image 'Drag to Move' 6)
read -r rx ry rw rh < <(a11y find "$title" 'list item' 'Docs in Edge')
drag_start "$hx" "$hy" "$hw" "$hh"
drag_to "$hx" "$((hy - 20))"
drag_to "$((rx + rw / 2))" "$((ry + rh / 4))"
shot settings-shortcuts-drag
drag_end
cp "$config/shortcuts.toml" "$OUT/shortcuts-after-move.toml"
expect "a drag puts the last shortcut above the one before it" shortcuts-after-move.toml \
    eval "[[ \$(grep -n '^name = ' '$OUT/shortcuts-after-move.toml' | tail -2 | cut -d'\"' -f2 | tr '\n' ,) == 'Rektangel pulls,Docs in Edge,' ]]"
read -r hx hy hw hh < <(a11y find "$title" image 'Drag to Move' 0)
click_box "$hx" "$hy" "$hw" "$hh"
tree after-handle-click
expect "a click on the handle does not open the form" a11y-after-handle-click.txt \
    tree_has "push button 'Add a Shortcut'"
click 'list item' 'Docs in Edge'
sleep 0.8
click 'push button' Back
sleep 0.8
keys Alt_L+Up
sleep 1
cp "$config/shortcuts.toml" "$OUT/shortcuts-after-alt-up.toml"
expect "Alt+Up moves the focused shortcut up" shortcuts-after-alt-up.toml \
    eval "[[ \$(grep -n '^name = ' '$OUT/shortcuts-after-alt-up.toml' | tail -2 | cut -d'\"' -f2 | tr '\n' ,) == 'Docs in Edge,Rektangel pulls,' ]]"
click 'list item' 'Docs in Edge'
sleep 0.8
click 'push button' 'Remove Shortcut'
sleep 0.8
# AdwMessageDialog has its heading as the window title.
shot settings-remove-dialog 'Remove the shortcut?'
click 'push button' Remove 0 'Remove the shortcut?'
sleep 0.8
cp "$config/shortcuts.toml" "$OUT/shortcuts-after-remove.toml"
expect "Remove takes the shortcut from shortcuts.toml" shortcuts-after-remove.toml \
    eval "! shortcuts_has 'Docs in Edge'"
expect "the other shortcuts stay" shortcuts-after-remove.toml shortcuts_has 'Rektangel pulls'

# R18: a change in an editor while the window shows, an error, and the
# correction.
begin R18-change
page General
set_setting window_width 820
tree r18-new-value
shot settings-r18-new-value
expect "the window shows the new value" a11y-r18-new-value.txt \
    eval "[[ \$(text_of 'spin button' '~^[0-9]+\$' 0) == 820 ]]"
begin R18-error
cp "$config/settings.toml" "$OUT/settings-good.toml"
{ cat "$OUT/settings-good.toml"; echo 'window_width = = 1'; } >"$config/settings.toml.new"
mv "$config/settings.toml.new" "$config/settings.toml"
sleep 1
cp "$config/settings.toml" "$OUT/settings-with-error.toml"
tree r18-error
shot settings-r18-error
expect "the window shows the error" a11y-r18-error.txt tree_has 'settings.toml has an error'
expect "the error has the file and the line" a11y-r18-error.txt \
    tree_has "settings.toml:$(wc -l <"$OUT/settings-with-error.toml")"
expect "the old values stay" a11y-r18-error.txt \
    eval "[[ \$(text_of 'spin button' '~^[0-9]+\$' 0) == 820 ]]"
click 'check box' 'Show tray icon'
expect "the window does not write the file" settings-with-error.toml \
    cmp -s "$config/settings.toml" "$OUT/settings-with-error.toml"
page Search
shot settings-r18-error-search
page General
begin R18-corrected
cp "$OUT/settings-good.toml" "$config/settings.toml.new"
mv "$config/settings.toml.new" "$config/settings.toml"
sleep 1
tree r18-corrected
shot settings-r18-corrected
expect "the error goes away" a11y-r18-corrected.txt tree_lacks 'settings.toml has an error'
expect "the switches are active again" a11y-r18-corrected.txt is_sensitive 'check box' 'Show tray icon'

# InlineShortcuts and an error in shortcuts.toml.
begin shortcuts-inline
page Shortcuts
cp "$config/shortcuts.toml" "$OUT/shortcuts-good.toml"
printf '# Inline.\nshortcut = [{ name = "Inline", keyword = "inl", cmd = "https://example.com/%%s" }]\n' \
    >"$config/shortcuts.toml.new"
mv "$config/shortcuts.toml.new" "$config/shortcuts.toml"
sleep 1
tree shortcuts-inline
shot settings-shortcuts-inline
expect "the inline list shows with its cause" a11y-shortcuts-inline.txt tree_has 'inline list'
expect "no change of an inline list" a11y-shortcuts-inline.txt \
    eval "! is_sensitive 'push button' 'Add a Shortcut'"
begin shortcuts-error
printf '[[shortcut]]\nname = "Broken\n' >"$config/shortcuts.toml.new"
mv "$config/shortcuts.toml.new" "$config/shortcuts.toml"
sleep 1
tree shortcuts-error
shot settings-shortcuts-error
expect "the window shows the error of shortcuts.toml" a11y-shortcuts-error.txt \
    tree_has 'shortcuts.toml has an error'
cp "$OUT/shortcuts-good.toml" "$config/shortcuts.toml.new"
mv "$config/shortcuts.toml.new" "$config/shortcuts.toml"
sleep 1
expect "the error of shortcuts.toml goes away" state tree_lacks 'shortcuts.toml has an error'

# The states in the dark style.
gsettings set org.gnome.desktop.interface color-scheme prefer-dark
sleep 1
page 'Global Key'
shot settings-r4-set-dark
page General
click 'check box' 'Run in background'
shot settings-r9-background-off-dark
click 'check box' 'Run in background'
page Shortcuts
click 'push button' 'Add a Shortcut'
sleep 0.8
fill text Keyword 'c a'
shot settings-form-error-dark
click 'push button' Back
gsettings set org.gnome.desktop.interface color-scheme default
sleep 1

# The stop rule: with "Run in background" off, the process stops when the
# settings window closes and no other window shows.
begin stop-rule
page General
click 'check box' 'Run in background'
close_window "$title"
sleep 2.5
expect "the process stops after the settings window closes" app-program.log \
    wait_name_gone $app
cp "$config/settings.toml" "$OUT/settings-end.toml"
cp "$config/shortcuts.toml" "$OUT/shortcuts-end.toml"
keyfile >"$OUT/keyfile-end.ini"
cp "$launches" "$OUT/launches.log"
