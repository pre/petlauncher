#!/usr/bin/env bash
# Reads the evidence directory of nested-test.sh and prints one line for each
# show of a window:
#   step       the test step; m0 or m1 is the monitor with the pointer
#   monitor    the monitor that contains the pointer position
#   seq        the Mutter window number (a new number is a new window)
#   placed     the frame at the map, before the extension moves it (x,y wxh)
#   final      the frame after the map
#   center     the position of the rule in the work area of the pointer
#              monitor. For the launcher: the top of a launcher with the
#              collapsed height at the center, so that the query line has
#              the same place for each window height (launcherPosition in
#              launcher.js). For other windows: the center.
#   centered   yes when final is at that position
#   paints     the frames that a monitor painted with the window visible
#   early      those frames that a monitor painted before the map
#   positions  the different window positions in those frames (1 = no jump)
#   first      the window position in the first painted frame
#   focus      yes when the window has the keyboard focus
# Then the tray icon states.
set -euo pipefail
out=$1
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
collapsed=$(sed -nE 's/^export const COLLAPSED_HEIGHT = ([0-9]+);/\1/p' \
    "$here/../petlauncher@repo.pet/launcher.js")

{
printf 'step\tmonitor\tseq\tplaced\tfinal\tcenter\tcentered\tpaints\tearly\tpositions\tfirst\tfocus\n'
{ grep -v -e '-grown-' "$out/steps.txt" || true; } | while read -r step; do
    jq -n -r --arg step "$step" --argjson collapsed "$collapsed" --slurpfile events "$out/events.json" \
        --slurpfile state "$out/state-$step.json" '
        def rect: "\(.x),\(.y) \(.width)x\(.height)";
        def pos: "\(.x),\(.y)";
        $state[0] as $s
        | ([$s.monitors[] | .geometry as $g
            | select($s.pointer.x >= $g.x and $s.pointer.x < $g.x + $g.width and
                     $s.pointer.y >= $g.y and $s.pointer.y < $g.y + $g.height) | .index][0]) as $monitor
        | (if ($step | startswith("settings")) or $step == "tray-settings" then "petlauncher Settings"
           elif ($step | startswith("dictionary")) then "Dictionary"
           else "petlauncher" end) as $title
        | (if ($step | startswith("other")) then "org.example.Other" else "pet.repo.Petlauncher" end) as $app
        # The window of late-title has no title at its map.
        | [$events[0][] | select(.type == "map" and .t >= $s.t and .wmClass == $app and
            (.title == $title or ($step | startswith("late-title"))))][0] as $map
        | if $map == null then "\($step) no map event" else
          $map.seq as $seq
          | [$events[0][] | select(.seq == $seq)] as $own
          | $map.frame as $placed
          | ($s.windows[] | select(.seq == $seq)) as $window
          | $s.monitors[$monitor].workArea as $work
          | (if $title == "petlauncher" then $collapsed else $window.frame.height end) as $height
          | {x: ($work.x + ([0, (($work.width - $window.frame.width) / 2 | floor)] | max)),
             y: ([($work.y + ([0, (($work.height - $height) / 2 | floor)] | max)),
                 ($work.y + ([0, $work.height - $window.frame.height] | max))] | min)} as $center
          | [$own[] | select(.type == "paint")] as $paints
          | ($paints | map(.actor | pos) | unique) as $positions
          | [$step, $monitor, $seq, ($placed | rect), ($window.frame | rect), ($center | pos),
             (if $window.frame.x == $center.x and $window.frame.y == $center.y then "yes" else "no" end),
             ($paints | length), ([$paints[] | select(.msAfterMap == null)] | length),
             ($positions | length), ($paints[0].actor // {x: "-", y: "-"} | pos),
             (if $window.focus then "yes" else "no" end)]
          | @tsv
          end'
done
} | column -t -s $'\t'

# A size change of the shown launcher (the results grow): the frame before and
# after it.
for state in "$out"/state-launcher-grown-*.json; do
    [[ -e $state ]] || continue
    jq -r --slurpfile events "$out/events.json" '
        def rect: "\(.x),\(.y) \(.width)x\(.height)";
        . as $s
        | ($s.windows[] | select(.title == "petlauncher")) as $w
        | ([$events[0][] | select(.seq == $w.seq and .t < $s.t and .frame != null)] | last.frame) as $before
        | "\($s.step): before \($before | rect), after \($w.frame | rect)"' "$state"
done

if [[ -e $out/state-focus-before.json ]]; then
    jq -r '"focus before launcher-focus-m0-pointer-m1: \(.focus.title) on monitor \(.focus.monitor)"' \
        "$out/state-focus-before.json"
fi

echo
echo "tray icon:"
for state in "$out"/state-tray-*.json "$out"/state-ext-disabled.json; do
    [[ -e $state ]] || continue
    jq -r '"  \(.step): exists=\(.tray.exists) visible=\(.tray.visible) menuOpen=\(.tray.menuOpen)" +
        (if .tray.menuOpen then " items=\([.tray.items[].label] | join(","))" else "" end)' "$state"
done
echo
echo "JS errors and warnings in the shell log: $(wc -l <"$out/shell-js-errors.txt")"
