#!/usr/bin/env bash
# Writes the settings of petlauncher and the test files of program-steps.sh
# in the HOME of the test shell. Usage: program-files.sh HOME
set -euo pipefail
home=$1
config=$home/.config/petlauncher
mkdir -p "$config" "$home/pathtest/alpha-dir"

cat >"$config/settings.toml" <<'EOF'
# The settings of the test. program-steps.sh changes some values.
keep_query = "30s"
EOF

cat >"$config/shortcuts.toml" <<'EOF'
[[shortcut]]
name = "Search the web"
keyword = "g"
cmd = "https://www.example.com/search?q=%s"
icon = "web-browser"
EOF

# 13 entries, so that the list scrolls after 8 rows. The newest file is
# notes.txt, so it is the first row of ~/pathtest/ (R1 step 3). The second
# row has a name that is too long for the window.
for i in 01 02 03 04 05 06 07 08 09 10; do
    touch -d "2026-01-01 00:$i" "$home/pathtest/file-$i.txt"
done
touch -d "2026-01-01 01:00" "$home/pathtest/notes.txt"
long=a-file-with-a-very-long-name-that-does-not-fit-in-the-width-of-the-launcher-window-so-the-row-shows-it-with-an-ellipsis-at-the-end.txt
touch -d "2026-01-01 00:30" "$home/pathtest/$long"
touch -d "2025-12-31 23:00" "$home/pathtest/alpha-dir/inner.txt"
touch -d "2025-12-31 23:00" "$home/pathtest/alpha-dir"
