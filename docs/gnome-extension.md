# GNOME Shell extension

The extension `petlauncher@repo.pet` is the GNOME part of petlauncher. It does two things on GNOME Shell 46 on Wayland:

1. It puts the launcher window at the center of the monitor with the pointer, each time the window shows (R11). The query line has the same place for each height of the window.
2. It shows the petlauncher icon in the top bar (R3, "Show tray icon").

On GNOME Wayland an application cannot set the position of its own window, and this desktop has no `StatusNotifierWatcher` for a tray icon. Thus these two functions are in the extension. The core of petlauncher does not know about the extension.

The files are in `gnome/extension/`. The [README](../gnome/extension/README.md) there lists them.

## Contract for the program

The program must obey this contract. The extension finds the program and its launcher window only through these values.

### Application and windows

| Subject | Requirement |
|---|---|
| Application ID | `pet.repo.Petlauncher`, given to `GtkApplication` (or `AdwApplication`). GTK then gives the same value as the Wayland application ID of each window of the application. |
| D-Bus | The application registers on the session bus, which is the default of `GApplication`. Do not use `NON_UNIQUE`. The bus name is `pet.repo.Petlauncher` and the object path is `/pet/repo/Petlauncher`. |
| Launcher window | A window of the application (`set_application`), for example `AdwApplicationWindow`. |
| Launcher title | Exactly `petlauncher`. Set it before the first `present()`, and do not change it. |
| Other windows | The settings window, the dictionary window and all other windows must never have the title `petlauncher`. A title such as `petlauncher Settings` is permitted: the extension compares the full title. |
| Show | `present()` on the launcher window. Do not try to set a position. |
| Hide | `set_visible(false)`, as in the test. GTK 4 then removes the window from Mutter, and each show makes a new window in Mutter. The extension moves each new window. |
| Size | Give the launcher its width (R5, initial value 750) and its height before `present()`. The extension does not change the size. |
| Collapsed height | With no rows, the launcher is 64 logical pixels high. This is `COLLAPSED_HEIGHT` in `launcher.js` and in `crates/petlauncher/src/window.rs`: change the two together. The extension puts the top of the launcher where a launcher of this height is at the center of the work area. Thus the query line has the same place for each height of the window, and a launcher that shows with a kept query and its rows (R9) goes down from it. |
| Size change after the show | The window grows downward: Mutter keeps the top left corner, also after a move by the owner. The extension does not move the window again. |
| Move | The owner can move the launcher with the mouse (a `GtkWindowHandle` in the program). A show with the kept query of R9 opens at the moved place (ADR 0009). |
| Window state | The launcher is not maximized and not full screen. |
| Focus | The program does not need to ask for the focus. The extension activates the launcher window when it shows. |

### D-Bus actions

The program adds these actions to the application, for example with `gio::SimpleAction` and `app.add_action()`. GTK names them `app.show` and so on in the program. `GApplication` exports them on D-Bus without the `app.` prefix, in the interface `org.gtk.Actions` at `/pet/repo/Petlauncher`.

| Action | Parameter | State | Function |
|---|---|---|---|
| `show` | none | none | Show the launcher window. When it shows already, nothing changes. |
| `settings` | none | none | Show the settings window. |
| `quit` | none | none | Stop the process, also when "Run in background" is on. |
| `tray-icon` | none | boolean | The value of the setting `show_tray_icon`. Make the action disabled, so that nobody can activate it. Change its state each time the setting changes, also after a change of `settings.toml` in an editor (R18). |
| `kept-query-until` | none | int64 | ADR 0009: the wall-clock time in microseconds since 1970 until which a show of the launcher has the kept query of R9. `0`: no kept query. `-1`: no time limit. Make the action disabled. Change its state after each hide and after each change of `settings.toml`, but not at a show, because the extension reads it at the map. |

A sketch in Rust with gtk-rs. It was not compiled:

```rust
let tray_icon = gio::SimpleAction::new_stateful(
    "tray-icon",
    None,
    &settings.show_tray_icon.to_variant(),
);
tray_icon.set_enabled(false);
app.add_action(&tray_icon);
// After each change of the settings:
tray_icon.set_state(&settings.show_tray_icon.to_variant());
```

The global key (R4) does not use the extension. It runs `petlauncher toggle`, which the program forwards to the running instance.

To check the program by hand:

```sh
gdbus call --session --dest pet.repo.Petlauncher --object-path /pet/repo/Petlauncher \
    --method org.gtk.Actions.Describe tray-icon
gdbus call --session --dest pet.repo.Petlauncher --object-path /pet/repo/Petlauncher \
    --method org.freedesktop.Application.ActivateAction show '[]' '{}'
```

## Method

### Position of the launcher window

1. The extension connects to the signal `map` of `global.window_manager`. GNOME Shell emits it after the first buffer of a window, and before it paints the window for the first time.
2. In the signal, the extension accepts the window only when its Wayland application ID (`get_wm_class()`) is `pet.repo.Petlauncher` and its title is `petlauncher`.
3. When the owner moved the launcher in an earlier show, and the state of `kept-query-until` is later than the wall clock (`queryKept`), the extension uses the moved place (`movedPosition` in `launcher.js`, ADR 0009) and continues at step 5. A launcher that goes below the work area at that place moves up. When the show has no kept query, or the query line at the moved place is not fully in a work area, the extension forgets the moved place.
4. Else it gets the monitor with the pointer (`global.display.get_current_monitor()`) and the work area of that monitor. The work area is the monitor without the top bar. It calculates the top left corner of the frame (`launcherPosition` in `launcher.js`). The frame is at the horizontal center of the work area. Its top is the top of a launcher with the collapsed height at the vertical center. A launcher that goes below the work area moves up. The calculation rounds a half pixel down. When the window is larger than the work area, the top left corner stays in the work area.
5. It moves the window with `move_frame()`. The size does not change, so Mutter does the move at once.
6. It activates the window with `Main.activateWindow()`.

The extension remembers a move at the signal `grab-op-end` of `global.display`, when the launcher is not at the place that the extension gave it at the map. The program does not tell the extension about a move.

Why not earlier: at the signal `window-created`, the title and the application ID of a GTK 4 window are still empty (measured, see the test results). GTK sends them after Mutter makes the window.

Why the title: the settings window and the dictionary window have the same application ID as the launcher. The title is a value that the program sets for each window, and the test shows that it is available at `map`. Alternatives that were not selected:

- The GTK window object path `/pet/repo/Petlauncher/window/N` of a `GtkApplicationWindow`. The number `N` depends on the order in which the program makes its windows, so the contract would be unclear.
- A D-Bus method in which the program tells the extension which window to move. This needs a D-Bus interface of its own in the program and a handshake in the extension. The title gives the same result with no code in the program.

### Monitor that the owner uses

The extension uses the monitor with the pointer. In Mutter terms this is `meta_display_get_current_monitor()`, which gives the logical monitor at the pointer position. Reasons:

- The owner's Ulauncher setting was `mouse-pointer-monitor`.
- Mutter puts each new window on the monitor with the pointer (measured, see the baseline). Thus the extension moves the launcher only inside one monitor. The window does not go to a monitor with a different scale.
- A click on the top bar icon shows the launcher on the monitor of the top bar, where the pointer is.
- The pointer is always on a monitor. A focused window is not always there, for example on an empty desktop or when the launcher has the focus.

The alternative is the monitor of the focused window (`global.display.focus_window.get_monitor()`). It is a change of one line in `extension.js`.

### Tray icon

- The extension watches the bus name `pet.repo.Petlauncher` with `Gio.bus_watch_name()`. It does not start the program.
- When the name has an owner, the extension reads the actions of that owner with `Gio.DBusActionGroup`. The group follows the changes of the actions through the signal `org.gtk.Actions.Changed`.
- The icon shows only when the name has an owner and the state of `tray-icon` is true. When the program stops, or the program does not have the action, the icon does not show.
- A click with the primary button calls `show`. A click with a different button, or a touch, opens the menu: "Show petlauncher", "Settings", a separator, "Quit". The menu items call `show`, `settings` and `quit`.
- The icon is `system-search-symbolic` from the icon theme. A petlauncher icon can replace it later.

### Disable

GNOME disables the extensions when the screen locks. `disable()` removes the icon, stops the bus name watch and disconnects the signal `map`.

## Test

### How to run

```sh
gjs -m gnome/extension/test/test-launcher.js
gnome/extension/test/nested-test.sh --out DIR                  # monitors 3008x1692 and 1920x1200 at scale 1
gnome/extension/test/nested-test.sh --owner-layout --out DIR   # 6016x3384 at scale 2, 2880x1800 at scale 1.5
gnome/extension/test/nested-test.sh --no-animations --out DIR  # enable-animations false
gnome/extension/test/nested-test.sh --no-extension --out DIR   # baseline with no extension
gnome/extension/test/nested-test.sh --program target/release/petlauncher --out DIR  # the real program
gnome/extension/test/nested-test.sh --program target/release/petlauncher \
    --steps gnome/extension/test/move-steps.sh --out DIR                   # a moved launcher
```

`nested-test.sh` starts a separate GNOME Shell with `gnome-shell --headless --wayland --no-x11` and one `--virtual-monitor` for each monitor. It shows nothing on the desktop. It isolates the test shell from the session of the user:

- a new session bus (`dbus-run-session`) and a fake system bus, so that the test shell cannot reach logind, UPower or NetworkManager,
- its own `HOME`, `XDG_RUNTIME_DIR` and XDG directories below `$HOME/tmp`,
- `GSETTINGS_BACKEND=keyfile`, with the extension enabled only in that key file.
- The services that the private bus starts, for example the portal backends, get the display of the test shell. Thus libadwaita reads the light or dark style through the Settings portal.

The script installs the extension in the test `HOME`, starts `test/fake-petlauncher.js` (a gjs GTK 4 stand-in for the program with the same application ID, windows and actions), and does the test steps. `test/shell-probe.js` runs in the test shell as its automation script. It moves a virtual pointer, clicks, types with a virtual keyboard, reads the clipboard, and records for each window: its creation, each move and size change, the map, and each frame that a monitor paints while the window is visible. `test/analyse.sh` makes the result table.

With `--program PATH`, the script runs the real program in place of the stand-in, with the steps of `test/program-steps.sh` and `test/acceptance-steps.sh`. `test/program-files.sh` writes a settings directory, recording fixture applications and a tree of test files in the test `HOME`. `test/mock-services.js` owns the names of the file manager, the session manager and the screen saver on the private session bus, and of logind on the fake system bus, and records each call. `test/analyse-program.sh` makes the tables of the query line, the times, the actions and the acceptance steps. [program.md](program.md) gives the results.

The test with `--owner-layout` uses the layout of the owner's monitors: 6016x3384 at scale 2, and 2880x1800 at scale 1.5 at the logical position 3008,234. The live session of the owner uses the logical layout mode (read from `org.gnome.Mutter.DisplayConfig.GetCurrentState`). The test shell starts in the physical layout mode, so the script sets the Mutter feature `scale-monitor-framebuffer` in the test key file for this test.

### Results

Environment: GNOME Shell 46.0, Mutter 46.2, gjs 1.80.2, GTK 4.14.5, Ubuntu 24.04, on 2026-11-26.

Each run writes one evidence directory, with `summary.txt` (the output of `analyse.sh`), `events.json` (the probe records), a `state-<step>.json` for each step, screenshots and the logs. The evidence directories of the runs below are not in the repository. The first runs of the extension had a rule that centered the full height of the window, with a stand-in at 750x48. The later runs have the current position rule and a stand-in at 750x64. The runs of the real program are in [program.md](program.md).

Positions are `x,y` of the top left corner of the window frame, in logical pixels. In the default run the monitors are 0: `0,0 3008x1692` (work area from y 32, below the top bar) and 1: `3008,0 1920x1200`. The stand-in launcher is 750x64, and 750x364 with results.

**Without the extension** (`2026-11-26-stand-in-no-extension`), GNOME puts the launcher near the top left of the monitor with the pointer: `3217,10` on monitor 1 and `2,43` on monitor 0. Each show is a new window in Mutter (window numbers 1, 2, 3 and so on). At `window-created` the title and the application ID of the window are empty.

**With the extension** (`2026-11-26-stand-in-logical-sizes`). "Place of the rule" is the position of `launcherPosition` for the monitor with the pointer:

| Step | Window | Pointer on | Before the move | After the map | Place of the rule | Result |
|---|---|---|---|---|---|---|
| launcher-first-m1 | launcher, first show of the process | 1 | 3217,10 | 3593,568 | 3593,568 | centered |
| launcher-m0 | launcher, shown again | 0 | 2,43 | 1129,830 | 1129,830 | centered |
| launcher-again-m1 | launcher, shown again | 1 | 3217,10 | 3593,568 | 3593,568 | centered |
| launcher-again-m0 | launcher, shown again | 0 | 2,43 | 1129,830 | 1129,830 | centered |
| launcher-tall-m0 | launcher, 750x364 at the show | 0 | 2,98 | 1129,830 | 1129,830 | the same top as with no rows |
| launcher-focus-m0-pointer-m1 | launcher; the focused window (dictionary) is on monitor 0 | 1 | 3217,10 | 3593,568 | 3593,568 | centered on the pointer monitor |
| settings-m1 | `petlauncher Settings` | 1 | 3066,132 | 3066,132 | not for this window | not moved, as required |
| dictionary-m0 | `Dictionary` | 0 | 1,84 | 1,84 | not for this window | not moved, as required |
| late-title-m1 | launcher with its title 100 ms after the show | 1 | 3217,10 | 3217,10 | 3593,568 | not moved: the title must come before the show |
| other-app-m1 | title `petlauncher`, application ID `org.example.Other` | 1 | 3217,10 | 3217,10 | 3593,568 | not moved, as required |
| ext-disabled-m1 | launcher, extension disabled | 1 | 3217,10 | 3217,10 | 3593,568 | not moved: `disable()` works |
| ext-enabled-again-m1 | launcher, extension enabled again | 1 | 3217,10 | 3593,568 | 3593,568 | centered |
| tray-left-click | launcher, primary click on the icon | 0 | 2,43 | 1129,830 | 1129,830 | centered |
| tray-menu-show | launcher, menu item "Show petlauncher" | 0 | 2,43 | 1129,830 | 1129,830 | centered |
| tray-settings | `petlauncher Settings`, menu item "Settings" | 0 | 1,50 | 1,50 | not for this window | shown, not moved |

When the results grow in a shown launcher (step launcher-grown-m1), the frame changes from `3593,568 750x64` to `3593,568 750x364`. The top left corner stays.

**Owner layout** (`2026-11-26-stand-in-owner-layout`, monitor 0 at scale 2, monitor 1 at scale 1.5 at `3008,234`): the same result for each step. On monitor 1 GNOME put the launcher at `3217,244` and the extension moved it to `3593,802`, the place of the rule in the work area `3008,234 1920x1200`. On monitor 0 the positions are the same as in the default run.

**No animations** (`2026-11-26-stand-in-no-animations`): the same positions for each step.

**First frame.** In the three runs with the extension, for each show of the launcher:

- no monitor painted the window before the map,
- the first painted frame had the window at its final position,
- all painted frames had the same window position, so there is no jump.

With the animations on, GNOME plays its open animation: the first frame has an opacity of 33 to 138 (of 255) and a small scale, and the window grows from the bottom center of its final frame. With the animations off, the first frame has the full opacity and the full size 750x64 at the final position.

**Focus.** The launcher had the keyboard focus after each show.

**Tray icon** (default run; the owner layout and the no-animations runs gave the same states):

| Step | Program | `tray-icon` state | Icon |
|---|---|---|---|
| start | not on the bus | | hidden |
| app-started | on the bus | true | shows |
| tray-menu | secondary click on the icon | | menu open: "Show petlauncher", "Settings", separator, "Quit" |
| tray-after-quit | menu item "Quit": the program left the bus | | hidden |
| tray-setting-off | started with the state false | false | hidden |
| tray-setting-on | the program changed the state | true | shows |
| tray-setting-off-again | the program changed the state | false | hidden |
| ext-disabled | extension disabled | | removed from the top bar |
| tray-enabled-again | extension enabled again | true | shows |

The shell logs of all runs have no JavaScript error or warning.

The unit test `test-launcher.js` has 15 tests for the position rule, the moved place, the window rule and the icon rule. All pass.

#### Moved launcher (ADR 0009)

`test/move-steps.sh` moves the launcher with the virtual pointer: a press on the search icon, five moves, and a release. Each case runs 3 times. Run with the debug build on 2026-11-26, 24 checks, 24 pass. Raw evidence: `~/daybook-evidence/petlauncher/2026-11-26-settings-nav/move-run1/` on the owner's computer, with `frames.tsv`, `events.json` and the program log.

| Case | Frame |
|---|---|
| A show with no kept query, monitor 0 | `1129,830` |
| After a move by 300,-200 | `1429,630` |
| The next show with the kept query, pointer on monitor 1, and the show after it | `1429,630` |
| After the query was cleared, or after the keep time of 2 s | `1129,830` |
| A kept query with no move, pointer on monitor 1 | `3593,568` |

The order of the state and the map: each change of `kept-query-until` came to GNOME Shell 0.9 ms to 5.0 ms after the program set it (21 changes, mean 1.5 ms; the log of the program and the probe use the same monotonic clock). In these steps each change came 1129 ms or more before the next map of the launcher.

The move of requirement 7 itself (a test with the debug build): the frame moved from `1129,830` to `1369,650` with the pointer, the launcher kept the focus, and new rows made it higher with the same top left corner. A drag that starts in the entry did not move the window, and a double click on the search icon did not change the frame.

### Limits and things that were not tested

- The test used virtual monitors in a headless GNOME Shell, not the real monitors of the owner. The extension was not installed in the live session.
- The headless frame clock does not run in real time: the 150 ms open animation of GNOME took 12 ms to 36 ms in the records. The evidence for "no jump" is the order of the events: the move comes in the same `map` signal, before the first painted frame, and all painted frames have the same position.
- The nested shell (`gnome-shell --nested --wayland`, X11 backend of Mutter 46.2) starts on this computer, but it is not usable for this test. With `MUTTER_DEBUG_NUM_DUMMY_MONITORS=2` and `MUTTER_DEBUG_DUMMY_MODE_SPECS="1000x563:640x400"` both dummy monitors got the mode 1000x563, and the second mode was not available to `ApplyMonitorsConfig`. The extension moved the launcher at `map` from 124,45 to 125,273, which is the center of monitor 0. But the nested shell did not paint the window in 2 s, and the probe recorded no frame. The cause was not examined. Also, a virtual pointer in the X11 backend can send input to the host display, so the test would not be safe.
- Focus: the window had the focus in all tests, also with no extension. A case where GNOME refuses the focus (the owner types before the window shows) was not tested.
- Keyboard use of the top bar icon (Ctrl+Alt+Tab to the top bar) was not tested.
- When the monitors change while the launcher shows, the extension does not move it.
- `metadata.json` lists GNOME Shell 46 only. Each new GNOME version needs a new test.
- To install the extension, copy `petlauncher@repo.pet/` to `~/.local/share/gnome-shell/extensions/`, log out and log in, then run `gnome-extensions enable petlauncher@repo.pet`. This was not done on this computer.
