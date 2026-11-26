#!/usr/bin/env bash
# Writes the settings of petlauncher, the fixture applications and the test
# files of program-steps.sh and acceptance-steps.sh in the HOME of the test
# shell. Usage: program-files.sh HOME [--key-conflict]
#
# --key-conflict: a custom shortcut of an invented program has Super+Space,
# and the input source switch keeps its GNOME default, which is the same
# key. Without it, the input source switch has no Super+Space, so that the
# first start writes the global key and the launcher shows no notice.
#
# Each fixture application is a recorder: it writes its tag and its
# arguments to HOME/launches.log, one line for each start, with " | "
# between the arguments. Nothing else starts.
set -euo pipefail
home=$1
key_conflict=${2:-}
config=$home/.config/petlauncher
bin=$home/fixture/bin
apps=$home/.local/share/applications
mkdir -p "$config" "$bin" "$apps" "$home/.cache/petlauncher" "$home/pathtest/alpha-dir"

# The recorder, and a recorder with the name of GNOME Settings, so that the
# GNOME rule for settings pages accepts its entry (R16).
cat >"$bin/record" <<EOF
#!/bin/sh
# Writes the arguments to the launch record of the test.
tag=\$1
shift
line=\$tag
for argument in "\$@"; do
    line="\$line | \$argument"
done
printf '%s\n' "\$line" >>"$home/launches.log"
EOF
cat >"$bin/gnome-control-center" <<EOF
#!/bin/sh
exec "$bin/record" settings-page "\$@"
EOF
chmod +x "$bin/record" "$bin/gnome-control-center"

entry() { # file name, then the lines after [Desktop Entry]
    local file=$1
    shift
    { echo "[Desktop Entry]"; echo "Type=Application"; printf '%s\n' "$@"; } >"$apps/$file"
}
entry petfixture-notes.desktop "Name=Recorder Notes" \
    "Comment=Writes notes to a plain text file" \
    "Exec=$bin/record app-notes %U" "Icon=accessories-text-editor" "Categories=Utility;"
entry petfixture-long.desktop "Name=Recorder Long" \
    "Comment=An application with a long comment that does not fit in the width of the launcher window, so the row shows it with an ellipsis" \
    "Exec=$bin/record app-long" "Icon=utilities-terminal" "Categories=Utility;"
entry petfixture-browser.desktop "Name=Recording Browser" \
    "Exec=$bin/record browser %u" "Icon=web-browser" "Categories=Network;WebBrowser;" \
    "MimeType=x-scheme-handler/http;x-scheme-handler/https;text/html;"
entry petfixture-edge.desktop "Name=Recording Edge" \
    "Exec=$bin/record edge %u" "Icon=web-browser" "Categories=Network;WebBrowser;" \
    "MimeType=x-scheme-handler/http;x-scheme-handler/https;"
entry petfixture-viewer.desktop "Name=Recording Viewer" \
    "Exec=$bin/record viewer %f" "Icon=image-x-generic" "MimeType=text/plain;image/png;" \
    "NoDisplay=true"
entry petfixture-folder.desktop "Name=Recording Folder" \
    "Exec=$bin/record folder %u" "Icon=folder" "MimeType=inode/directory;" "NoDisplay=true"
# The same ID as the page of GNOME Settings, so it hides the real entry.
entry gnome-wifi-panel.desktop "Name=Wi-Fi" "Comment=Control Wi-Fi connections" \
    "Exec=$bin/gnome-control-center wifi" "Icon=network-wireless-symbolic" "NoDisplay=true" \
    "Categories=GTK;GNOME;Settings;HardwareSettings;X-GNOME-NetworkSettings;"

cat >"$home/.config/mimeapps.list" <<'EOF'
[Default Applications]
x-scheme-handler/http=petfixture-browser.desktop
x-scheme-handler/https=petfixture-browser.desktop
text/html=petfixture-browser.desktop
text/plain=petfixture-viewer.desktop
image/png=petfixture-viewer.desktop
inode/directory=petfixture-folder.desktop
EOF

wordnet=$HOME/tmp/wordnet-root/usr/share/wordnet
{
    echo "# The settings of the test. The steps change some values."
    echo 'keep_query = "30s"'
    if [[ -d $wordnet ]]; then
        printf '\n[dictionary]\nwordnet_path = "%s"\n' "$wordnet"
    fi
} >"$config/settings.toml"

cat >"$config/shortcuts.toml" <<EOF
[[shortcut]]
name = "Search the web"
keyword = "g"
cmd = "https://www.example.com/search?q=%s"
icon = "web-browser"

[[shortcut]]
name = "Rektangel pull request"
keyword = "rektangel"
cmd = "https://github.com/LOLm/cat/pull/%s"
cmd_without_argument = "https://github.com/LOLm/cat/pulls"

[[shortcut]]
name = "Search with Edge"
keyword = "edge"
cmd = "https://www.example.com/edge?q=%s"
browser = { app = "petfixture-edge.desktop" }

[[shortcut]]
name = "Search with a browser command"
keyword = "cmdb"
cmd = "https://www.example.com/cmd?q=%s"
browser = { command = "$bin/record command-browser --profile 'work one'" }

[[shortcut]]
name = "Search with a missing browser"
keyword = "missing"
cmd = "https://www.example.com/missing?q=%s"
browser = { app = "petfixture-missing.desktop" }
EOF

# The stored currency rates. PETLAUNCHER_OFFLINE stops the download.
cat >"$home/.cache/petlauncher/ecb-eurofxref-daily.xml" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<gesmes:Envelope xmlns:gesmes="http://www.gesmes.org/xml/2002-08-01" xmlns="http://www.ecb.int/vocabulary/2002-08-01/eurofxref">
<Cube><Cube time='2026-11-25'><Cube currency='USD' rate='1.0512'/><Cube currency='GBP' rate='0.83125'/></Cube></Cube>
</gesmes:Envelope>
EOF

# R4: the key bindings of GNOME with the default key of petlauncher.
if [[ $key_conflict == --key-conflict ]]; then
    cat >>"$home/.config/glib-2.0/settings/keyfile" <<'EOF'
[org/gnome/settings-daemon/plugins/media-keys]
custom-keybindings=['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/scratch-notes/']

[org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/scratch-notes]
name='Show Scratch Notes'
command='scratch-notes --toggle'
binding='<Super>space'
EOF
else
    printf "[org/gnome/desktop/wm/keybindings]\nswitch-input-source=['XF86Keyboard']\n" \
        >>"$home/.config/glib-2.0/settings/keyfile"
fi

file() { # relative path, modification time
    mkdir -p "$(dirname "$home/$1")"
    printf '%s\n' "$1" >"$home/$1"
    touch -d "$2" "$home/$1"
}

# The window steps: 13 entries, so that the list scrolls after 8 rows. The
# newest file is notes.txt, and the second row has a name that is too long
# for the window.
for i in 01 02 03 04 05 06 07 08 09 10; do
    file "pathtest/file-$i.txt" "2026-01-01 00:$i"
done
file pathtest/notes.txt "2026-01-01 01:00"
file pathtest/a-file-with-a-very-long-name-that-does-not-fit-in-the-width-of-the-launcher-window-so-the-row-shows-it-with-an-ellipsis-at-the-end.txt "2026-01-01 00:30"
file pathtest/alpha-dir/inner.txt "2025-12-31 23:00"
touch -d "2025-12-31 23:00" "$home/pathtest/alpha-dir"

# R1: L0L3-IMAGE has three images, the newest is image-003.png. L0L3-VIDEO also
# matches "L0L3". single has one file only.
file lolcat-output/L0L3-IMAGE/image-001.png "2026-02-01 10:00"
file lolcat-output/L0L3-IMAGE/image-002.png "2026-02-01 11:00"
file lolcat-output/L0L3-IMAGE/image-003.png "2026-02-01 12:00"
file lolcat-output/L0L3-VIDEO/clip.mp4 "2026-02-01 09:00"
file lolcat-output/single/only.png "2026-02-01 08:00"

# R10 and R14: the file of the requirements, and files that the ignore list
# of the file search removes.
file daybook-evidence/kukka/font-size-keys-palette/screenshots-final/palette-zoom.png "2026-03-01 10:00"
file daybook-evidence/kukka/font-size-keys-palette/screenshots-final/palette-small.png "2026-03-01 09:00"
file tmp/palette-zoom-in-tmp.png "2026-03-01 10:00"
file go/pkg/mod/palette-zoom-in-gomod.png "2026-03-01 10:00"
