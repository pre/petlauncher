#!/usr/bin/env bash
# Reads the evidence of `nested-test.sh --program` and prints:
#   - the place of the query line for each show of the launcher,
#   - the time from the command to the window,
#   - the actions at the seam of the program for each marked step,
#   - the states of the checks, the clipboard and the other commands.
# analyse.sh prints the position table of the same run.
set -euo pipefail
out=$1
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
collapsed=$(sed -nE 's/^export const COLLAPSED_HEIGHT = ([0-9]+);/\1/p' \
    "$here/../petlauncher@repo.pet/launcher.js")

launcher_steps() {
    grep -E '^(launcher|tray-left-click)' "$out/steps.txt" || true
}

echo
echo "query line: the top of the launcher frame at the map, for each show"
echo "  (the query line is the top $collapsed pixels of the frame)"
{
printf 'step\tmonitor\theight\ttop\tempty top\tsame place\n'
launcher_steps | while read -r step; do
    jq -n -r --arg step "$step" --argjson collapsed "$collapsed" \
        --slurpfile events "$out/events.json" --slurpfile state "$out/state-$step.json" '
        $state[0] as $s
        | ([$s.monitors[] | .geometry as $g
            | select($s.pointer.x >= $g.x and $s.pointer.x < $g.x + $g.width and
                     $s.pointer.y >= $g.y and $s.pointer.y < $g.y + $g.height) | .index][0]) as $monitor
        | [$events[0][] | select(.type == "map" and .t >= $s.t and .title == "petlauncher")][0] as $map
        | ($s.windows[] | select(.seq == $map.seq)) as $window
        | $s.monitors[$monitor].workArea as $work
        | ($work.y + (($work.height - $collapsed) / 2 | floor)) as $empty
        | [$step, $monitor, $map.frame.height, $window.frame.y, $empty,
           (if $window.frame.y == $empty then "yes" else "no" end)] | @tsv'
done
} | column -t -s $'\t'

echo
echo "time from the command to the window, in milliseconds"
echo "  map: GNOME Shell got the first buffer; paint: the first painted frame"
echo "  command: the run time of \`petlauncher toggle\` (empty: not measured)"
{
printf 'step\tmap\tpaint\tcommand\n'
launcher_steps | while read -r step; do
    command=$(awk -v step="$step" '$1 == step { printf "%.1f", $2 / 1000 }' "$out/toggle-times.txt")
    jq -n -r --arg step "$step" --arg command "${command:--}" \
        --slurpfile events "$out/events.json" --slurpfile state "$out/state-$step.json" '
        $state[0].t as $t
        | [$events[0][] | select(.type == "map" and .t >= $t and .title == "petlauncher")][0] as $map
        | [$events[0][] | select(.type == "paint" and .seq == $map.seq)][0] as $paint
        | [$step, (($map.t - $t) * 10 | round / 10), (($paint.t - $t) * 10 | round / 10), $command]
        | @tsv'
done
} | column -t -s $'\t'

echo
echo "actions of the program for each marked step: the outcome, the execution by the desktop or the error, and the close"
awk '
    /^== / { step = $2; next }
    step != "" && /: outcome / { sub(/.*: outcome /, ""); print "  " step ": outcome " $0 }
    step != "" && /: executed / { sub(/.*: executed /, ""); print "  " step ": executed " $0 }
    step != "" && /: action error: / { sub(/.*: action error: /, ""); print "  " step ": error " $0 }
    step != "" && /: hide, / { sub(/.*: hide, /, ""); print "  " step ": hide " $0; step = "" }
' "$out/app-program.log"

echo
echo "calls that reached the mock services:"
jq -r '"  \(.name) \(.method) \(.args | tojson)"' "$out/mock-calls.jsonl" 2>/dev/null || echo "  none"

echo
echo "starts of the fixture applications:"
sed 's/^/  /' "$out/launches.log" 2>/dev/null || echo "  none"

if [[ -s $out/acceptance.tsv ]]; then
    echo
    echo "acceptance steps: requirement step, result, check, evidence"
    column -t -s $'\t' "$out/acceptance.tsv" | sed 's/^/  /'
    echo "  PASS $(grep -c $'\tPASS\t' "$out/acceptance.tsv" || true), FAIL $(grep -c $'\tFAIL\t' "$out/acceptance.tsv" || true)"
fi

if [[ -s $out/checks.txt ]]; then
    echo
    echo "checks: launcher shows, focus, tray icon"
    while read -r check; do
        jq -r --arg check "$check" '
            "  \($check): launcher=\(if any(.windows[]; .title == "petlauncher") then "shows" else "hidden" end)" +
            " focus=\(.focus.title // "none") tray=\(.tray.visible)"' "$out/state-$check.json"
    done <"$out/checks.txt"
fi

if compgen -G "$out/clipboard-*.txt" >/dev/null; then
    echo
    echo "clipboard after the window hid:"
    for file in "$out"/clipboard-*.txt; do
        name=$(basename "$file" .txt)
        echo "  ${name#clipboard-}: $(<"$file")"
    done
fi

if [[ -s $out/commands.txt ]]; then
    echo
    echo "other commands:"
    sed 's/^/  /' "$out/commands.txt"
fi

echo
echo "errors and warnings of the program:"
grep -E -i "critical|warning|error|panic" "$out/app-program.log" | sed 's/^/  /' || echo "  none"
