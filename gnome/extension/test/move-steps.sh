# shellcheck shell=bash disable=SC2154
# SC2154: nested-test.sh sets here, OUT, PROGRAM and the other variables.
#
# The steps of a launcher that the owner moves with the mouse (ADR 0009), for
#   nested-test.sh --program PATH --steps gnome/extension/test/move-steps.sh --out OUT
#
# Each case runs 3 times. A show with the kept query of R9 opens at the place
# of the move; a show with no kept query opens at the place of R11.
#
# Evidence that this file adds to OUT:
#   app-program.log   the log of the program, with "== NAME" before steps
#   acceptance.tsv    step, PASS or FAIL, check, evidence file
#   frames.tsv        step, the frame "x y width height" of the launcher
#   launcher-*.png    screenshots of the launcher
#   shell.log         (nested-test.sh) the busy pointer of Mutter
#   events.json       (nested-test.sh) the map of each window and each
#                     change of an action state, with their times

app=pet.repo.Petlauncher
title=petlauncher
config=$HOME/.config/petlauncher
program_log=$OUT/app-program.log
acceptance=$OUT/acceptance.tsv
frames=$OUT/frames.tsv
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

mark() { echo "== $1" >>"$program_log"; }
begin() { step=$1; mark "$step"; }

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

shot() { shell_eval "petProbe.screenshotWindow('$title', '$OUT/$1.png')" >/dev/null; }

expect() { # description, then a command that succeeds for PASS
    local description=$1 result=FAIL
    shift
    "$@" >/dev/null 2>&1 && result=PASS
    printf '%s\t%s\t%s\t%s\n' "$step" "$result" "$description" frames.tsv >>"$acceptance"
}

frame() {
    shell_eval 'petProbe.state()' | jq -r --arg t "$title" \
        '.windows[] | select(.title == $t) | .frame | "\(.x) \(.y) \(.width) \(.height)"'
}
record() { # name: the frame of the launcher now
    printf '%s\t%s\n' "$1" "$(frame)" >>"$frames"
}
place() { frame | cut -d' ' -f1,2; }
shows() { [[ -n $(frame) ]]; }

# The toggle command of the global key, with the pointer on a monitor.
show_on() { # monitor
    pointer_to_monitor "$1"
    sleep 0.2
    "$PROGRAM" toggle >>"$program_log" 2>&1
    sleep 1.2
}

# A drag of the search icon at the left end of the query line.
move_by() { # dx dy
    local x y
    read -r x y _ _ < <(frame)
    x=$((x + 30))
    y=$((y + 32))
    shell_eval "petProbe.pointer($x, $y)" >/dev/null
    sleep 0.2
    shell_eval "petProbe.button(1, true)" >/dev/null
    sleep 0.2
    for i in 1 2 3 4 5; do
        shell_eval "petProbe.pointer($((x + $1 * i / 5)), $((y + $2 * i / 5)))" >/dev/null
        sleep 0.1
    done
    shell_eval "petProbe.button(1, false)" >/dev/null
    sleep 0.5
}

: >"$program_log"
: >"$acceptance"
: >"$frames"
shell_eval 'petProbe.prepareKeyboard()' >/dev/null
sleep 1
"$PROGRAM" --hidden >>"$program_log" 2>&1 &
app_pids+=($!)
wait_name_owned $app
sleep 1

begin default-place
show_on 0
record default-m0
default_m0=$(place)
keys Escape
sleep 0.5

for run in 1 2 3; do
    begin "moved-and-kept-$run"
    show_on 0
    keys Control_L+a BackSpace
    type_text set
    move_by 300 -200
    record "moved-$run"
    moved=$(place)
    expect "the drag moves the launcher" [ "$moved" != "$default_m0" ]
    expect "the launcher shows after the drag" shows
    keys Escape
    sleep 0.5
    show_on 1
    record "kept-$run"
    [[ $run == 1 ]] && shot launcher-kept-at-the-moved-place
    expect "a show with the kept query opens at the moved place, also with the pointer on monitor 1" \
        [ "$(place)" == "$moved" ]
    keys Escape
    sleep 0.5
    show_on 0
    record "kept-again-$run"
    expect "the next show with the kept query opens at the moved place again" [ "$(place)" == "$moved" ]

    begin "cleared-query-$run"
    keys Control_L+a BackSpace
    keys Escape
    sleep 0.5
    show_on 0
    record "cleared-$run"
    expect "a show with no kept query opens at the place of R11" [ "$(place)" == "$default_m0" ]
    keys Escape
    sleep 0.5
done

# The keep time of R9 ends.
{
    echo 'keep_query = "2s"'
    grep -v '^keep_query = ' "$config/settings.toml" || true
} >"$config/settings.toml.new"
mv "$config/settings.toml.new" "$config/settings.toml"
sleep 1
for run in 1 2 3; do
    begin "keep-time-ends-$run"
    show_on 0
    type_text set
    move_by 300 -200
    moved=$(place)
    keys Escape
    sleep 3
    show_on 0
    record "after-keep-time-$run"
    expect "a show after the keep time opens at the place of R11" [ "$(place)" == "$default_m0" ]
    keys Escape
    sleep 0.5
done

# A launcher that was not moved follows the pointer, also with a kept query.
for run in 1 2 3; do
    begin "not-moved-kept-$run"
    show_on 0
    type_text set
    keys Escape
    sleep 0.5
    show_on 1
    record "not-moved-m1-$run"
    [[ $run == 1 ]] && default_m1=$(place)
    expect "a show with a kept query and no move opens on the monitor with the pointer" \
        [ "$(place | cut -d' ' -f1)" -ge 3008 ]
    expect "the place on monitor 1 is the same each time" [ "$(place)" == "$default_m1" ]
    keys Control_L+a BackSpace Escape
    sleep 0.5
done

# Each path to a show of the launcher maps it with no activation token, so
# Mutter does not show the busy pointer.
tray_click() {
    local x y
    read -r x y < <(shell_eval 'petProbe.state()' |
        jq -r '.tray.extents | "\(.x + (.width / 2 | floor)) \(.y + (.height / 2 | floor))"')
    shell_eval "petProbe.click($x, $y, 1)" >/dev/null
    sleep 1.2
}
for run in 1 2 3; do
    begin "show-paths-$run"
    pointer_to_monitor 0
    sleep 0.2
    tray_click
    expect "the top bar icon shows the launcher" shows
    keys Escape
    sleep 0.5
    app_action $app show
    sleep 1.2
    expect "the action show shows the launcher" shows
    keys Escape
    sleep 0.5
done
begin no-busy-pointer
expect "no show of the launcher set the busy pointer" \
    eval "! grep -q 'STARTUP: Setting busy cursor' '$OUT/shell.log'"
