# shellcheck shell=bash disable=SC2088,SC2154
# SC2088: each ~ is text that the test types. SC2154: nested-test.sh and
# program-steps.sh set the variables.
#
# The acceptance steps of the requirements with the real program, the real
# GnomeDesktop, real key input, the mock services and the recording fixture
# applications. program-steps.sh sources this file while the program runs.
# Each check writes one line to acceptance.tsv: the requirement step, PASS or
# FAIL, what was checked, and the evidence file. The functions of the checks
# are in program-steps.sh.

l0l3=$HOME/lolcat-output/L0L3-IMAGE
newest=$l0l3/image-003.png
palette=$HOME/daybook-evidence/kukka/font-size-keys-palette/screenshots-final/palette-zoom.png

# ----------------------------------------------------------------- R1

begin R1.1
query '~/lolcat-output/L0L3-IMAGE/'
expect "a typed directory has no selected row" "$log_file" rows_have "selected None"
keys Return
sleep 0.8
expect "ShowFolders of L0L3-IMAGE" mock-calls.jsonl mock_call ShowFolders "$home_uri/lolcat-output/L0L3-IMAGE\""
expect "no file opens" launches.log no_launch
expect "the launcher closes" state launcher_hidden

begin R1.2
query '~/lolcat-output/L0L3-IMAGE'
keys Return
sleep 0.8
expect "ShowFolders of L0L3-IMAGE, not of lolcat-output" mock-calls.jsonl mock_call ShowFolders "$home_uri/lolcat-output/L0L3-IMAGE\""
expect "no file opens" launches.log no_launch

begin R1.3
query '~/lolcat-output/L0L3-IMAGE/'
keys Down Return
expect "the newest file opens" launches.log launched "viewer | $newest"
expect "no folder opens" mock-calls.jsonl no_mock_call

begin R1.4
query '~/lolcat-output/L0L3-IMAGE/'
keys Down Control_L+Return
sleep 0.8
expect "ShowItems of the newest file" mock-calls.jsonl mock_call ShowItems "$home_uri/lolcat-output/L0L3-IMAGE/image-003.png"

begin R1.5
query "$l0l3/image-002.png"
keys Control_L+Return
sleep 0.8
expect "ShowItems of the typed file" mock-calls.jsonl mock_call ShowItems "$home_uri/lolcat-output/L0L3-IMAGE/image-002.png"

begin R1.6
query "$l0l3/image-002.png"
keys Return
expect "the typed file opens" launches.log launched "viewer | $l0l3/image-002.png"

begin R1.7
query '~/lolcat-output/L0L3'
keys Return
sleep 0.8
expect "the query changes to the first match, a directory" "$log_file" logged 'search "~/lolcat-output/L0L3-IMAGE/"'
expect "nothing opens" mock-calls.jsonl no_mock_call
expect "the launcher stays" state launcher_visible
keys Return
sleep 0.8
expect "the second Enter opens the directory" mock-calls.jsonl mock_call ShowFolders "$home_uri/lolcat-output/L0L3-IMAGE\""

begin R1.8
query '~/lolcat-output/L0L3-IM'
expect "one match is selected without Down" "$log_file" rows_have ": 1, selected Some(0)"
keys Return
sleep 0.8
expect "Enter changes the query to the directory" "$log_file" logged 'search "~/lolcat-output/L0L3-IMAGE/"'
hide_launcher

begin R1.9
query '~/lolcat-output/single/'
keys Return
sleep 0.8
expect "the directory with one file opens as a directory" mock-calls.jsonl mock_call ShowFolders "$home_uri/lolcat-output/single\""
expect "the file does not open" launches.log no_launch

# ----------------------------------------------------------------- R2

begin R2.1
query 'Recorder Notes'
expect "the application row is first" "$log_file" rows_have ": Recorder Notes"
keys Control_L+Return
sleep 0.8
expect "ShowItems of the .desktop file" mock-calls.jsonl mock_call ShowItems "/.local/share/applications/petfixture-notes.desktop"
expect "the application does not start" launches.log no_launch
expect "the launcher closes" state launcher_hidden

begin R2.2
query '42+1'
clipboard_before=$(clipboard_text)
keys Control_L+Return
sleep 0.8
expect "nothing opens" mock-calls.jsonl no_mock_call
expect "nothing starts" launches.log no_launch
expect "the launcher stays" state launcher_visible
expect "nothing is copied" clipboard clipboard_is "$clipboard_before"
hide_launcher

# ----------------------------------------------------------------- R7

begin R7.1
query 'spur 123'
keys Return
expect "the browser opens the pull request" launches.log launched "browser | https://github.com/LOLm/cat/pull/123"

begin R7.2
query 'spur'
keys Return
expect "the browser opens the list of pull requests" launches.log launched "browser | https://github.com/LOLm/cat/pulls"

# ----------------------------------------------------------------- R8

begin R8.1
query '42.3+68.7+14.1+0.5'
expect "the result row is 125.6" "$log_file" rows_have ": 125.6"
shot calculation-row
begin R8.2
keys Return
sleep 0.5
expect "the clipboard has 125.6" clipboard clipboard_is "125.6"
expect "the window closes" state launcher_hidden

# ----------------------------------------------------------------- R9

# A window of a different program, for the loss of focus.
gjs -m "$here/fake-petlauncher.js" --app-id org.example.Other >>"$OUT/app-other.log" 2>&1 &
app_pids+=($!)
wait_name_owned org.example.Other
app_action org.example.Other dictionary
sleep 1
read -r other_x other_y < <(shell_eval 'petProbe.state()' |
    jq -r '.windows[] | select(.title == "Dictionary") | .frame | "\(.x + 100) \(.y + 100)"')
click_other_window() {
    shell_eval "petProbe.click($other_x, $other_y, 1)" >/dev/null
    sleep 0.8
}
# Shows the launcher again with no change of the query. The next checks of
# the step read only the log after this show.
reopen() {
    log_count=$(wc -l <"$program_log")
    pointer_to_monitor 0
    sleep 0.2
    "$PROGRAM" toggle >>"$program_log" 2>&1
    sleep 1.2
}
calc='42.3+68.7+14.1+0.5'
set_setting keep_query '"3s"'

begin R9.1
query "$calc"
click_other_window
expect "a click on a different window closes petlauncher" state launcher_hidden
expect "the close reason is the focus loss" "$log_file" logged "hide, FocusLoss"
begin R9.2
reopen
expect "the query shows again" "$log_file" logged "show, kept query Some(\"$calc\")"
expect "the result row is 125.6" "$log_file" rows_have ": 125.6"
shot kept-calculation

begin R9.3
click_other_window
sleep 3.5
reopen
expect "after the keep time the query line is empty" "$log_file" logged "show, kept query None"
hide_launcher

begin R9.4
query '42.3+68.7'
keys Return
sleep 0.5
reopen
expect "Enter on a calculation keeps the query" "$log_file" logged 'show, kept query Some("42.3+68.7")'
hide_launcher

begin R9.5
query 'Recorder Notes'
keys Return
expect "the application starts" launches.log launched "app-notes"
reopen
expect "Enter on an application clears the query" "$log_file" logged "show, kept query None"
hide_launcher

set_setting keep_query '"disabled"'
begin R9.6
query "$calc"
click_other_window
reopen
expect "with disabled the query line is empty" "$log_file" logged "show, kept query None"
hide_launcher

set_setting keep_query '"forever"'
begin R9.7
query "$calc"
click_other_window
sleep 3.5
reopen
expect "with forever the query stays" "$log_file" logged "show, kept query Some(\"$calc\")"
hide_launcher
set_setting keep_query '"30s"'
close_window Dictionary
app_action org.example.Other quit
wait_name_gone org.example.Other

# ---------------------------------------------------------------- R10

begin R10.1-2
paste "$palette"
expect "one row with the file, selected" "$log_file" rows_have ": 1, selected Some(0): palette-zoom.png"
begin R10.3
keys Return
expect "the file opens" launches.log launched "viewer | $palette"

begin R10.4
paste "$palette"
keys Control_L+Return
sleep 0.8
expect "ShowItems of the file" mock-calls.jsonl mock_call ShowItems "/screenshots-final/palette-zoom.png"

begin R10.5
query "~/daybook-evidence/kukka/font-size-keys-palette/screenshots-final/palette-zoom.png"
expect "the ~ form gives the same row" "$log_file" rows_have ": 1, selected Some(0): palette-zoom.png"
keys Return
expect "the file opens" launches.log launched "viewer | $palette"

begin R10.6
paste " $palette
"
expect "a space before and a line end after the path" "$log_file" rows_have ": 1, selected Some(0): palette-zoom.png"
hide_launcher

begin R10.7
paste "\"$palette\""
expect "the path in quotation marks" "$log_file" rows_have ": 1, selected Some(0): palette-zoom.png"
hide_launcher

begin R10.8
paste "file://$palette"
expect "the path with file://" "$log_file" rows_have ": 1, selected Some(0): palette-zoom.png"
hide_launcher

# ---------------------------------------------------------------- R12

begin R12
query 'edge rust'
keys Return
expect "the browser of the shortcut opens the URL" launches.log launched "edge | https://www.example.com/edge?q=rust"

begin R12-command
query 'cmdb rust'
keys Return
expect "a browser command runs with no shell, the URL last" launches.log \
    launched "command-browser | --profile | work one | https://www.example.com/cmd?q=rust"

begin R12-missing
query 'missing rust'
keys Return
sleep 0.8
expect "a browser that is not installed shows an error" "$log_file" logged "action error: The browser petfixture-missing.desktop is not installed."
expect "the launcher stays open with the error" state launcher_visible
shot error-notice
hide_launcher

# ---------------------------------------------------------------- R13

begin R13
query '~/lolcat-output/L0L3-IMAGE/'
keys Down Shift_L+Return
sleep 0.5
expect "the clipboard has the full path" clipboard clipboard_is "$newest"
expect "the window closes" state launcher_hidden

# ---------------------------------------------------------------- R14

begin R14
query 'f palette-zoom'
expect "the file search finds palette-zoom.png" "$log_file" rows_have "palette-zoom.png"
expect "no file below ~/tmp" "$log_file" rows_lack "in-tmp"
expect "no file below ~/go/pkg/mod" "$log_file" rows_lack "in-gomod"
hide_launcher
begin R14-no-keyword
query 'palette-zoom'
expect "with no keyword there is no file search" "$log_file" rows_lack "palette-zoom.png"
hide_launcher

# ---------------------------------------------------------------- R16

system_step() { # step, typed title, mock method, argument text
    begin "$1"
    query "$2"
    expect "the first row is $2" "$log_file" rows_have ": $2"
    keys Return
    sleep 1
    expect "$3 reaches its service" mock-calls.jsonl mock_call "$3" "$4"
}
system_step R16-lock Lock Lock '"args":[]'
system_step R16-suspend Suspend Suspend '"args":[true]'
system_step R16-logout 'Log out' Logout '"args":[0]'
system_step R16-restart Restart Reboot '"args":[]'
system_step R16-poweroff 'Power off' Shutdown '"args":[]'

begin R16-settings-page
query 'wifi'
expect "the settings page row" "$log_file" rows_have ": Wi-Fi"
keys Return
expect "the entry of the page starts" launches.log launched "settings-page | wifi"

# ---------------------------------------------------------------- R19

# program-files.sh sets wordnet_path when the WordNet files exist.
if grep -q wordnet_path "$config/settings.toml"; then
    begin R19.1
    query 'd launch'
    expect "word suggestions" "$log_file" rows_have "launch"
    shot dictionary-rows
    hide_launcher
    begin R19.2
    query 'd launcher'
    expect "the meanings of launcher" "$log_file" rows_have ": "
    begin R19.3
    keys Return
    sleep 0.8
    expect "Enter gives OpenDictionary" "$log_file" logged 'OpenDictionary("launcher")'
    expect "the window closes" state launcher_hidden
    expect "the dictionary window shows the word" "$log_file" logged 'dictionary window: open "launcher"'
    shot dictionary-window 'petlauncher Dictionary'
    close_window 'petlauncher Dictionary'
fi

# ---------------------------------------------------------------- R20

for time_query in 'time now in new york' '17:00 in las vegas' '6am in helsinki'; do
    begin "R20 $time_query"
    query "$time_query"
    expect "a time row" "$log_file" rows_have ", selected Some(0): "
    [[ $time_query == 'time now in new york' ]] && shot time-row
    hide_launcher
done

# ------------------------------------------------------- R3 applications off

set_setting include_applications false
begin R3-applications-off
query 'Recorder Notes'
expect "no application row" "$log_file" rows_lack "Recorder Notes"
hide_launcher
query 'wifi'
expect "the settings page stays" "$log_file" rows_have ": Wi-Fi"
hide_launcher
set_setting include_applications true

# ----------------------------------------------------------------- R15

begin R15-currency
query '100 usd in eur'
expect "a currency row with the stored rates" "$log_file" rows_have "EUR"
shot currency-row
hide_launcher

# ----------------------------------------------------------------- R18

begin R18
cp "$config/settings.toml" "$OUT/settings-before-r18.toml"
printf 'window_width = \n' >>"$config/settings.toml"
sleep 0.8
open_launcher
expect "a settings file with an error shows a notice" "$log_file" logged "settings.toml has an error"
shot settings-error-notice
hide_launcher
cp "$OUT/settings-before-r18.toml" "$config/settings.toml"
sleep 0.8

# ------------------------------------------------------- screenshots of rows

query 'recorder'
shot application-rows
hide_launcher
query 'settings'
shot settings-rows
hide_launcher
