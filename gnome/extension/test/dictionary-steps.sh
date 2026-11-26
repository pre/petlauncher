# shellcheck shell=bash disable=SC2154
# SC2154: nested-test.sh sets here, OUT, PROGRAM and the other variables.
#
# The steps of the dictionary window (R19, acceptance steps 3 to 6), for
#   PETLAUNCHER_WORDNET_PATH=DIR nested-test.sh --program PATH \
#       --steps gnome/extension/test/dictionary-steps.sh --out OUT
# DIR is a WordNet directory, for example the files of the package
# wordnet-base that `dpkg -x` extracted. The program must give dictionary
# rows for `d <word>`.
#
# nested-test.sh sources this file in its inner part, so the functions of
# that part are available. The functions of program-steps.sh are not: this
# file has its own copies, so that a change of one file does not touch the
# other.
#
# Evidence that this file adds to OUT:
#   app-program.log        the log of the program, with "== NAME" before steps
#   dict-*.png             screenshots of the dictionary window
#   clipboard-*.txt        the text on the clipboard, read in the shell
#   dictionary-checks.txt  the windows and the focus after each step
#   dictionary-moves.txt   for each map of the dictionary window: the frame
#                          at the map and after it (the extension must not
#                          move it)
#   dictionary-pages.txt   the pages that the window showed, from the log
#   dictionary-summary.txt all of these tables

app=pet.repo.Petlauncher
dict='petlauncher Dictionary'
config=$HOME/.config/petlauncher
program_log=$OUT/app-program.log
wordnet=${PETLAUNCHER_WORDNET_PATH:?set PETLAUNCHER_WORDNET_PATH to a WordNet directory}
# The color of a link in the light style of libadwaita 1.5, as the
# screenshots have it.
link_color='#5493E0'
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

shot() { # name [title]
    shell_eval "petProbe.screenshotWindow('${2:-$dict}', '$OUT/$1.png')" >/dev/null
    sleep 0.5
}

read_clipboard() { # name
    shell_eval 'petProbe.readClipboard()' >/dev/null
    sleep 0.5
    shell_eval 'petProbe.clipboard()' | jq -r '. // "(no text)"' >"$OUT/clipboard-$1.txt"
}

# The whole settings file: the [dictionary] table must come after the keys
# of the top level.
write_settings() { # run_in_background wordnet_path
    printf 'run_in_background = %s\n\n[dictionary]\nwordnet_path = "%s"\n' "$1" "$2" \
        >"$config/settings.toml.new"
    mv "$config/settings.toml.new" "$config/settings.toml"
    sleep 0.7
}

# `petlauncher toggle` with the pointer on a monitor. With a step name, the
# show is a line of steps.txt, and analyse.sh gives the place of the
# launcher for it.
toggle() { # monitor [step]
    local t
    pointer_to_monitor "$1"
    sleep 0.2
    t=$(now)
    "$PROGRAM" toggle >>"$program_log" 2>&1
    sleep 1.2
    if [[ -n ${2:-} ]]; then
        snapshot "$2" "$t"
        echo "$2" >>"$OUT/steps.txt"
    fi
}

# The launcher: `d WORD` and Enter.
open_from_launcher() { # name word
    toggle 0
    type_text "d $2"
    mark "$1"
    keys Return
    sleep 1.2
}

check() { # name
    shell_eval 'petProbe.state()' >"$OUT/state-dict-$1.json"
    jq -r --arg name "$1" --arg dict "$dict" '
        "\($name): dictionary=\(if any(.windows[]; .title == $dict) then "shows" else "hidden" end)" +
        " launcher=\(if any(.windows[]; .title == "petlauncher") then "shows" else "hidden" end)" +
        " focus=\(.focus.title // "none")"' "$OUT/state-dict-$1.json" >>"$OUT/dictionary-checks.txt"
}

# The frame of the dictionary window: "x y width height".
dict_frame() {
    shell_eval 'petProbe.state()' |
        jq -r --arg dict "$dict" '.windows[] | select(.title == $dict) | .frame | "\(.x) \(.y) \(.width) \(.height)"'
}

# The box of the first line of links below the header bar in a screenshot of
# the dictionary window: "x y width height" in screen pixels. The screenshot
# has a margin of 32 pixels around the frame.
first_link_box() { # screenshot
    local x y w fx fy fw fh
    # The header bar and the focus ring of the search field are above 68.
    local top=68
    read -r fx fy fw fh < <(dict_frame)
    read -r _ _ x y < <(magick "$1" -crop "${fw}x${fh}+32+32" +repage -gravity North \
        -chop "0x$top" +repage -fuzz 12% -fill black -opaque "$link_color" \
        -fill white +opaque black -format '%@\n' info: | tr 'x+' '  ')
    # The first line of links: the same color in a strip of 14 pixels.
    read -r w _ x _ < <(magick "$1" -crop "${fw}x${fh}+32+32" +repage -gravity North \
        -chop "0x$((top + y))" +repage -crop "x14+0+0" +repage -fuzz 12% \
        -fill black -opaque "$link_color" -fill white +opaque black -format '%@\n' info: |
        tr 'x+' '  ')
    echo "$((fx + x)) $((fy + top + y)) $w 14"
}

# The lines of dark text below the header bar in a screenshot of the
# dictionary window: "top bottom" in frame pixels, one line for each band of
# rows with a pixel darker than the threshold. A dim label is lighter than
# 40 % and is not a band. The rounded corners at the bottom are not in the
# area.
text_bands() { # screenshot
    local fw fh
    read -r _ _ fw fh < <(dict_frame)
    magick "$1" -crop "${fw}x$((fh - 80))+32+92" +repage -colorspace Gray -threshold 40% \
        -negate -scale "1x$((fh - 80))!" -depth 8 txt:- |
        awk -F'[(,:)]' 'NR > 1 {
            if ($4 + 0 > 0) { if (!inside) { top = $2; inside = 1 } bottom = $2 }
            else if (inside) { print top + 60, bottom + 60; inside = 0 } }'
}

# The left and right end of the text between two rows: "left right" in
# frame pixels. A threshold of 75 % also finds dim text and links.
band_ends() { # screenshot top bottom [threshold]
    local fw
    read -r _ _ fw _ < <(dict_frame)
    magick "$1" -crop "${fw}x$(($3 - $2 + 1))+32+$((32 + $2))" +repage -colorspace Gray \
        -threshold "${4:-40%}" -negate -format '%@\n' info: |
        awk -F'[x+]' '{ print $3, $3 + $1 }'
}

# A drag with the primary button, for a text selection. The probe has only
# click(), so this makes a second virtual pointer of the same seat.
drag() { # x1 y1 x2 y2
    shell_eval '(() => {
        const Clutter = imports.gi.Clutter;
        const GLib = imports.gi.GLib;
        if (!globalThis.petDrag) {
            const seat = Clutter.get_default_backend().get_default_seat();
            globalThis.petDrag = seat.create_virtual_device(Clutter.InputDeviceType.POINTER_DEVICE);
        }
        globalThis.petDragTo = (x, y) =>
            globalThis.petDrag.notify_absolute_motion(GLib.get_monotonic_time(), x, y);
        globalThis.petDragButton = pressed => globalThis.petDrag.notify_button(
            GLib.get_monotonic_time(), 1,
            pressed ? Clutter.ButtonState.PRESSED : Clutter.ButtonState.RELEASED);
        return true;
    })()' >/dev/null
    shell_eval "petDragTo($1, $2)" >/dev/null
    sleep 0.2
    shell_eval "petDragButton(true)" >/dev/null
    sleep 0.2
    for i in 1 2 3 4 5 6 7 8 9 10; do
        shell_eval "petDragTo($(($1 + ($3 - $1) * i / 10)), $(($2 + ($4 - $2) * i / 10)))" >/dev/null
        sleep 0.05
    done
    sleep 0.2
    shell_eval "petDragButton(false)" >/dev/null
    sleep 0.4
}

click() { # x y
    shell_eval "petProbe.click($1, $2, 1)" >/dev/null
    sleep 0.8
}

: >"$program_log"
: >"$OUT/toggle-times.txt"
: >"$OUT/checks.txt"
: >"$OUT/commands.txt"
: >"$OUT/dictionary-checks.txt"
"$PROGRAM" --version >>"$OUT/versions.txt"
echo "GTK $(pkg-config --modversion gtk4), libadwaita $(pkg-config --modversion libadwaita-1)" >>"$OUT/versions.txt"
write_settings true "$wordnet"
shell_eval 'petProbe.prepareKeyboard()' >/dev/null
sleep 1
start_program --hidden
sleep 1

# R19 step 3: `d launcher` and Enter. The dictionary window opens with the
# word and the launcher hides.
toggle 0 launcher-dict-first-m0
type_text 'd launcher'
shot launcher-d-launcher petlauncher
mark enter-d-launcher
keys Return
sleep 1.5
check after-enter-d-launcher
shot dict-launcher-light

# R19 step 4: select text with the pointer and copy it with Ctrl+C. First
# one line of the first meaning, then from that line to the line of its
# synonyms: one selection over the parts of one meaning.
# The text lines of the page of "launcher": the headword, the word class,
# then the definition of the first meaning. The line of the synonyms below
# it starts at the left edge of the text, right of the number.
read -r lx ly lw _ < <(first_link_box "$OUT/dict-launcher-light.png")
read -r fx fy _ _ < <(dict_frame)
text_bands "$OUT/dict-launcher-light.png" >"$OUT/text-lines-launcher.txt"
read -r top bottom < <(sed -n 3p "$OUT/text-lines-launcher.txt")
read -r _ right < <(band_ends "$OUT/dict-launcher-light.png" "$top" "$bottom")
read -r left _ < <(band_ends "$OUT/dict-launcher-light.png" "$((ly - fy))" "$((ly - fy + 13))" 75%)
definition_y=$((fy + (top + bottom) / 2))
echo "first link at $lx,$ly width $lw; first definition at x $((fx + left)) to $((fx + right)), y $definition_y" \
    >>"$OUT/dictionary-checks.txt"
mark select-definition-line
drag "$((fx + left + 1))" "$definition_y" "$((fx + right + 40))" "$definition_y"
shot dict-launcher-selected-line
keys Control_L+c
read_clipboard definition-line
# A drag that starts in selected text moves the text (drag and drop). A
# click first removes the selection.
click "$((fx + left + 1))" "$definition_y"
mark select-definition-and-synonyms
drag "$((fx + left + 1))" "$definition_y" "$((lx + lw))" "$((ly + 7))"
shot dict-launcher-selected-meaning
keys Control_L+c
read_clipboard definition-and-synonyms

# R19 step 5: a click on a synonym shows that word in the same window. The
# back button shows "launcher" again. Alt+Right and Alt+Left too. The link
# is in the selected text of step 4, and in selected text the first click of
# GTK only removes the selection. Thus a click on the definition first.
click "$((fx + left + 1))" "$definition_y"
mark click-synonym
click "$((lx + 10))" "$((ly + 7))"
check after-click-synonym
shot dict-rocket-launcher-light
read -r fx fy _ _ < <(dict_frame)
mark click-back
click "$((fx + 23))" "$((fy + 23))"
check after-click-back
shot dict-back-to-launcher-light
mark alt-right
keys Alt_L+Right
mark alt-left
keys Alt_L+Left

# R19 step 6: the launcher shows and hides again. The dictionary window
# stays.
toggle 0 launcher-over-dictionary-m0
check launcher-shows-again
mark hide-launcher-with-escape
keys Escape
sleep 0.5
check launcher-hidden-again

# The search field: Ctrl+L, typed text shows suggestions, Down selects one,
# Enter opens it.
keys Control_L+l
type_text 'launch'
shot dict-suggestions-launch-light
keys Down Down
mark enter-on-second-suggestion
keys Return
sleep 0.5
# Enter on the start of a word opens the first suggestion.
keys Control_L+f
type_text 'rocke'
mark enter-on-start-of-word
keys Return
sleep 0.5

# Words for the screenshots: many meanings and classes, base forms, a word
# with two parts.
open_word() { # name word
    keys Control_L+l
    type_text "$2"
    mark "$1"
    keys Return
    sleep 0.8
}
open_word open-run run
shot dict-run-light
keys Page_Down Page_Down Page_Down Page_Down
shot dict-run-scrolled-light
open_word open-better better
shot dict-better-light
for page in 1 2 3 4 5 6 7; do
    keys Page_Down
    shot "dict-better-page-$page-light"
done
open_word open-launch-pad 'launch pad'
shot dict-launch-pad-light

# The scroll position stays for back: the page of "better" shows at its
# base forms again.
mark back-to-better
keys Alt_L+Left
sleep 0.5
shot dict-better-back-scroll-light

# A word that is not in the dictionary: the "not found" page with similar
# words. A click on the first similar word opens it.
keys Control_L+l
type_text 'launchr'
shot dict-typing-launchr-light
mark enter-launchr
keys Return
sleep 0.8
shot dict-not-found-light
read -r lx ly lw _ < <(first_link_box "$OUT/dict-not-found-light.png")
mark click-similar-word
click "$((lx + 10))" "$((ly + 7))"
check after-click-similar-word

# Escape: with text in the field it clears the field, with an empty field it
# closes the window.
keys Control_L+l
type_text 'abc'
mark escape-with-text
keys Escape
check after-escape-with-text
mark escape-with-empty-field
keys Escape
sleep 0.5
check after-escape-with-empty-field

# The dark style: the same pages.
gsettings set org.gnome.desktop.interface color-scheme prefer-dark
sleep 1
open_from_launcher dark-launcher launcher
check dark-after-enter
shot dict-launcher-dark
open_word dark-run run
shot dict-run-dark
open_word dark-better better
shot dict-better-dark
for page in 1 2 3 4 5 6 7; do
    keys Page_Down
    shot "dict-better-page-$page-dark"
done
open_word dark-launch-pad 'launch pad'
shot dict-launch-pad-dark
open_word dark-launchr launchr
shot dict-not-found-dark
keys Escape
sleep 0.5

# Missing data: the page with the cause, the path and the command. The
# command is the last line of dark text. A drag over it selects it, and
# Ctrl+C copies it.
write_settings true /nonexistent/wordnet
open_from_launcher missing-data launcher
check missing-data
shot dict-missing-data-dark
gsettings set org.gnome.desktop.interface color-scheme default
sleep 1
shot dict-missing-data-light
text_bands "$OUT/dict-missing-data-light.png" >"$OUT/text-lines-missing-data.txt"
read -r top bottom < <(tail -n 1 "$OUT/text-lines-missing-data.txt")
read -r left right < <(band_ends "$OUT/dict-missing-data-light.png" "$top" "$bottom")
read -r fx fy _ _ < <(dict_frame)
mark select-install-command
drag "$((fx + left - 20))" "$((fy + (top + bottom) / 2))" "$((fx + right + 20))" "$((fy + (top + bottom) / 2))"
shot dict-missing-data-selected
keys Control_L+c
read_clipboard install-command
keys Escape
sleep 0.5
write_settings true "$wordnet"

# The dictionary window on monitor 1: GNOME places it, and the extension
# does not move it.
toggle 1 launcher-dict-m1
type_text 'd launcher'
mark enter-on-monitor-1
keys Return
sleep 1.5
check on-monitor-1
keys Escape
sleep 0.5

# "Run in background" off. Required: the dictionary window stays after the
# launcher hides, and the process stops after the dictionary window closes.
# The stop of the launcher in window.rs decides the first part
# (docs/dictionary-window.md), so this step records what happens.
app_action $app quit
wait_name_gone $app
write_settings false "$wordnet"
pointer_to_monitor 0
sleep 0.2
start_program
sleep 1.5
type_text 'd launcher'
mark enter-no-background
keys Return
sleep 3
check no-background-after-launcher-hid
if gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
    --method org.freedesktop.DBus.NameHasOwner $app | grep -q true; then
    echo "run_in_background false: the process runs 3 s after the launcher hid" >>"$OUT/commands.txt"
    mark close-dictionary-no-background
    keys Escape
    if wait_name_gone $app; then
        echo "run_in_background false: the process stopped after the dictionary window closed" \
            >>"$OUT/commands.txt"
    fi
else
    echo "run_in_background false: the process stopped after the launcher hid" >>"$OUT/commands.txt"
fi
write_settings true "$wordnet"

# The tables.
shell_eval 'petProbe.events()' >"$OUT/events-dictionary.json"
# Mutter places a new window before its map. A move after the map would be
# the extension.
jq -r --arg dict "$dict" '
    . as $events
    | [.[] | select(.title == $dict and .type == "map")][] | . as $map
    | ([$events[] | select(.type == "map-settled" and .seq == $map.seq)][0]) as $settled
    | ([$events[] | select(.type == "position-changed" and .seq == $map.seq and .t > $map.t)]
       | length) as $later
    | "seq \($map.seq): at the map \($map.frame.x),\($map.frame.y) \($map.frame.width)x\($map.frame.height)," +
      " after the map \($settled.frame.x),\($settled.frame.y), moves after the map: \($later)"
    ' "$OUT/events-dictionary.json" >"$OUT/dictionary-moves.txt"
awk '/^== / { step = $2; next } /dictionary window: (page|opened|open|hide)/ {
        sub(/^petlauncher [0-9.]+: dictionary window: /, ""); print "  " step ": " $0 }' \
    "$program_log" >"$OUT/dictionary-pages.txt"
{
    echo "dictionary window: windows and focus after each step"
    sed 's/^/  /' "$OUT/dictionary-checks.txt"
    echo
    echo "pages of the dictionary window, from the log (lookup: the dictionary calls; widgets: the page)"
    cat "$OUT/dictionary-pages.txt"
    echo
    echo "maps of the dictionary window (the extension must not move it)"
    sed 's/^/  /' "$OUT/dictionary-moves.txt"
    echo
    echo "clipboard:"
    for file in "$OUT"/clipboard-*.txt; do
        echo "  $(basename "$file" .txt):"
        sed 's/^/    | /' "$file"
    done
    echo
    echo "other commands:"
    sed 's/^/  /' "$OUT/commands.txt"
    echo
    echo "errors and warnings of the program:"
    grep -E -i "critical|warning|error|panic" "$program_log" | sed 's/^/  /' || echo "  none"
} >"$OUT/dictionary-summary.txt"
