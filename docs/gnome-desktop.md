# GNOME desktop part

The crate `petlauncher-gnome` is the GNOME part of petlauncher. It implements the `Desktop` trait of `petlauncher-core` for GNOME 46, and it executes the parts of an `Action` that need no GTK. It uses GIO for D-Bus, GSettings and the start of programs. It does not use GTK.

This file gives the use for the GTK program, each D-Bus call and GSettings key that the crate uses, the evidence for them, the test method, and the manual check list for the live session.

## Use in the GTK program

```rust
use petlauncher_core::Desktop;
use petlauncher_gnome::{Executed, GnomeDesktop};

// In the `startup` signal of the application:
let desktop = GnomeDesktop::new()?;
desktop.add_actions(&app);                                    // the action `tray-icon` (R3)
desktop.set_tray_icon_visible(settings.show_tray_icon)?;      // also after each change of the settings
desktop.set_launch_context(Some(display.app_launch_context().upcast()));

// R16: the settings pages come from `petlauncher-apps` with the GNOME rule.
let apps = Applications::load(Environment::from_process(), petlauncher_gnome::is_settings_page);

// After a key, while the launcher window is still visible:
match desktop.execute(&outcome.action) {
    Ok(Executed::Done) => {}
    Ok(Executed::Fallback { reason }) => log(reason),    // the folder opened, with no selection
    Ok(Executed::NotHandled) => program_does(&outcome.action), // Copy, OpenDictionary, SetQuery
    Err(error) => show(error),
}
// Then hide the window when `outcome.close` is true.
```

`display` is the `gdk::Display` of the program. The launch context gives each new window an activation token, so that the window can take the focus on Wayland. Execute the action before the launcher window hides, so that the token comes from the window with the focus.

The calls are synchronous, with a timeout of 10 s. Their time on the live session was not measured. A call takes longer when the bus must start Nautilus, when the lock screen shows (GNOME Shell answers `Lock` after the lock screen shows), or when polkit asks for a password for suspend.

### R4, the global key and conflicts

The crate never changes the key of a different shortcut by itself. The program writes the petlauncher shortcut only when no other GNOME key binding has the key:

```rust
let shortcuts = desktop.shortcuts()?;
let conflicts = shortcuts.set_global_key_if_free("<Super>space", petlauncher_gnome::TOGGLE_COMMAND)?;
// Empty: the key is written. Else nothing is written, and each conflict has
// the name and the place in GNOME Settings, for a message to the owner.
```

`conflicts(accelerator)` gives each key binding that GNOME Settings shows in Keyboard > View and Customize Shortcuts and that has the key: the other custom shortcuts, and the built-in bindings of five schemas (see "GSettings keys"). Only an explicit action of the owner, the "Remove" button of the settings window, calls `remove_key(&conflict)`. For a custom shortcut it sets the key to empty and keeps the entry with its name and command, so GNOME Settings shows it as disabled. For a built-in binding it removes only the one accelerator from the list, and the other accelerators of the key stay. It never removes an entry.

## Public API

| Item | Purpose |
|---|---|
| `GnomeDesktop` | Implements `Desktop`. Holds the session bus, the system bus, the custom shortcuts, the launch context and the action `tray-icon`. |
| `GnomeDesktop::new()` | Connects to the session bus and opens the custom shortcuts. The system bus connects at the first suspend. |
| `GnomeDesktop::with_parts(session, system, shortcuts)` | The same with given connections and shortcuts, for the tests. |
| `GnomeDesktop::set_launch_context(context)` | The launch context for new windows. |
| `GnomeDesktop::shortcuts()` | The `CustomShortcuts`, or the error that `new` got. |
| `GnomeDesktop::add_actions(map)` | Adds the action `tray-icon` to the application. |
| `GnomeDesktop::execute(action)` | `execute` with this desktop and its launch context. |
| `execute(action, desktop, context)` | Executes an action. Has no GNOME code. |
| `Executed` | `Done`, `Fallback { reason }` or `NotHandled`. |
| `CustomShortcuts` | The GNOME custom shortcuts in GSettings. |
| `CustomShortcuts::new()`, `with_backend(source, backend)` | The installed schemas, dconf and the files of GNOME Settings, or given schemas and a given backend with no files. |
| `CustomShortcuts::with_catalog_dirs(dirs)` | The directories with the files `*.xml` of GNOME Settings, which give the names and the sections of the built-in bindings. |
| `CustomShortcuts::global_key()` | The key of the petlauncher shortcut, or `None`. |
| `CustomShortcuts::set_global_key(accelerator, command)` | Writes the petlauncher shortcut. An empty accelerator removes the key. |
| `CustomShortcuts::set_global_key_if_free(accelerator, command)` | Writes the petlauncher shortcut only when `conflicts` is empty, and else gives the conflicts. |
| `CustomShortcuts::others_with_key(accelerator)` | The other custom shortcuts with the same key. |
| `CustomShortcuts::conflicts(accelerator)` | Each GNOME key binding with the same key, as `Conflict`. |
| `CustomShortcuts::remove_key(conflict)` | Removes the key of one conflict. For an explicit action of the owner only. |
| `CustomShortcut` | `path`, `name`, `command`, `binding`. |
| `Conflict`, `BindingKind` | A key binding with the key: `Custom { path }` or `BuiltIn { schema, key }`, the `name` and the `section` in GNOME Settings, and the `accelerator` as stored. `place()` gives the place for the owner. |
| `SHORTCUTS_PAGE`, `CUSTOM_SECTION` | `GNOME Settings > Keyboard > View and Customize Shortcuts` and `Custom Shortcuts`. |
| `validate_accelerator(text)`, `AcceleratorError` | Checks a key text. The error text is for the owner. |
| `is_settings_page` | The GNOME rule for settings pages, from `petlauncher-apps`. |
| `SHORTCUT_PATH`, `SHORTCUT_NAME`, `TOGGLE_COMMAND`, `TRAY_ICON_ACTION` | The constants below. |

## D-Bus calls

| Function | Bus | Name | Object path | Method | Arguments |
|---|---|---|---|---|---|
| `open_folder` | session | `org.freedesktop.FileManager1` | `/org/freedesktop/FileManager1` | `org.freedesktop.FileManager1.ShowFolders(as URIs, s StartupId)` | `([folder URI], token)` |
| `show_in_folder` | session | `org.freedesktop.FileManager1` | `/org/freedesktop/FileManager1` | `org.freedesktop.FileManager1.ShowItems(as URIs, s StartupId)` | `([item URI], token)` |
| Lock | session | `org.gnome.ScreenSaver` | `/org/gnome/ScreenSaver` | `org.gnome.ScreenSaver.Lock()` | none |
| Suspend | system | `org.freedesktop.login1` | `/org/freedesktop/login1` | `org.freedesktop.login1.Manager.Suspend(b interactive)` | `(true)` |
| Log out | session | `org.gnome.SessionManager` | `/org/gnome/SessionManager` | `org.gnome.SessionManager.Logout(u mode)` | `(0)` |
| Restart | session | `org.gnome.SessionManager` | `/org/gnome/SessionManager` | `org.gnome.SessionManager.Reboot()` | none |
| Power off | session | `org.gnome.SessionManager` | `/org/gnome/SessionManager` | `org.gnome.SessionManager.Shutdown()` | none |

The meaning of the arguments:

- **URIs.** One `file://` URI. The crate makes it with `g_filename_to_uri`, which percent-encodes each byte that a URI path cannot have: a space is `%20`, `#` is `%23`, `%` is `%25`, and `ä` is `%C3%A4`. `ShowFolders` opens the folder. `ShowItems` opens the folder that contains the item and selects the item.
- **StartupId.** The activation token of the launch context (`g_app_launch_context_get_startup_notify_id`, for the default application of `inode/directory`). Nautilus gives it to the new window, so that the window can take the focus. With no launch context, the value is empty.
- **interactive** of `Suspend`. `true` permits polkit to ask for a password when the policy needs one. GNOME Shell 46 calls `Suspend(true)` in its own system menu.
- **mode** of `Logout`. `0` is the normal log out with the confirmation window of GNOME. GNOME Shell 46 calls `Logout(0)` in its own system menu. The confirmation window does not show when the GNOME setting `org.gnome.SessionManager logout-prompt` is false. The crate uses only `0`.
- `Reboot` and `Shutdown` have no arguments. GNOME Shell 46 calls them in its own system menu, which shows the confirmation window. `gnome-session-quit` has the method names `Logout`, `Shutdown` and `Reboot` in its binary, and its manual says that `--reboot` and `--power-off` "prompt the user to confirm". The crate does not use `RequestReboot` and `RequestShutdown`.

The calls permit the bus to start the service (no flag `NO_AUTO_START`), so a call to `org.freedesktop.FileManager1` starts Nautilus when it does not run.

### Fallback of the file manager

When the call to `org.freedesktop.FileManager1` fails, for example because no service has the name, the crate opens the folder with the default application for folders (`g_app_info_launch_default_for_uri` of the folder URI). For `show_in_folder` this is the folder that contains the item, with no selection. The result is `FolderOpened::Fallback { reason }`, not an error. `reason` is the D-Bus error, for the log. When the default application also fails, the result is an error with the two causes.

## GSettings keys

| Schema | Path | Key | Type | Read | Written |
|---|---|---|---|---|---|
| `org.gnome.settings-daemon.plugins.media-keys` | `/org/gnome/settings-daemon/plugins/media-keys/` | `custom-keybindings` | `as` | all functions | `set_global_key` adds `SHORTCUT_PATH` at the end when it is missing. The other paths and their order stay. |
| `org.gnome.settings-daemon.plugins.media-keys.custom-keybinding` (relocatable) | `SHORTCUT_PATH` | `name`, `command`, `binding` | `s` | `global_key` | `set_global_key` writes the three keys in one change (`delay` and `apply`). |
| the same | each other path in `custom-keybindings` | `name`, `command`, `binding` | `s` | `others_with_key`, `conflicts` | `remove_key` writes `binding` to `''`. |
| `org.gnome.desktop.wm.keybindings`, `org.gnome.shell.keybindings`, `org.gnome.mutter.keybindings`, `org.gnome.mutter.wayland.keybindings`, `org.gnome.settings-daemon.plugins.media-keys` | the fixed path of the schema | each key of type `as`, except `custom-keybindings` | `as` | `conflicts` | `remove_key` writes the list with one accelerator less. |

On GNOME 46 all keys of the first four schemas have the type `as`, and `org.gnome.settings-daemon.plugins.media-keys` has 98 keys of type `as` and the number `volume-step` (`gsettings list-recursively`, read only). The GNOME default of `org.gnome.desktop.wm.keybindings switch-input-source` is `['<Super>space', 'XF86Keyboard']`, so on a new installation of GNOME the default key of petlauncher has a conflict.

The names and the sections come from the files of GNOME Settings, `<data dir>/gnome-control-center/keybindings/*.xml` for each system data directory: the attribute `name` of `KeyListEntries` is the section, for example `Typing`, and the attribute `description` of each `KeyListEntry` is the name. An entry with `hidden="true"` does not show in GNOME Settings. A key that has no entry gets the summary of its schema key, and the place gives the `gsettings` schema and key.

| Constant | Value |
|---|---|
| `SHORTCUT_PATH` | `/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/petlauncher/` |
| `SHORTCUT_NAME` | `Show petlauncher` |
| `TOGGLE_COMMAND` | `petlauncher toggle` |

Rules:

- `global_key` gives `None` when `SHORTCUT_PATH` is not in `custom-keybindings`, and when the key is empty or `disabled`.
- After each write the crate calls `g_settings_sync`, so dconf has the values when the function returns.
- A path in the list that does not start and end with `/`, or that has `//`, is skipped, because GSettings cannot open it.
- `CustomShortcuts::new` looks up the schemas first. When they are missing, it gives an error. It does not call `g_settings_new` with a missing schema, because GLib then stops the process.

### Accelerator text

GNOME Shell (Mutter) reads `binding` with its own parser, `meta_parse_accelerator` in `src/core/meta-accel-parse.c` of Mutter 46.2. `validate_accelerator` and the comparison of keys use the same rules:

| Part | Rule |
|---|---|
| Modifiers | `<Shift>`, `<Shft>`, `<Control>`, `<Primary>`, `<Ctrl>`, `<Ctl>`, `<Alt>`, `<Mod1>` to `<Mod5>`, `<Meta>`, `<Super>`, `<Hyper>`. The letter case has no effect, and the order has no effect. `<Primary>`, `<Ctrl>` and `<Ctl>` are `<Control>`, `<Shft>` is `<Shift>`, `<Alt>` is `<Mod1>`. |
| Key | The rest of the text. A key name of libxkbcommon (`xkb_keysym_from_name` with `XKB_KEYSYM_CASE_INSENSITIVE`), then the same name with `XF86` before it. Also `0x` and two or more hexadecimal digits (a key code), and `Above_Tab`. |
| No key | An empty text and `disabled`. |

Two names of one key are the same key, for example `Prior` and `Page_Up`, and `a` and `A`. `<Mod4>` is the same modifier as `<Super>`, because the keyboard layouts of GNOME give the Super key the modifier Mod4. The order of the modifiers has no effect.

The crate is stricter than Mutter in two cases, so that the settings window can warn: an unknown modifier such as `<Win>` is an error (Mutter ignores it), and modifiers with no key are an error.

The crate loads `libxkbcommon.so.0` at run time with `dlopen`, because the build has no libxkbcommon development files. GNOME Shell always has the library. When the library cannot load, the crate accepts each key name and compares the names with no letter case.

## Execution of an action

`execute` does the actions that need no GTK. It has no GNOME code: it uses GIO and the `Desktop` trait. GIO is not GNOME, so a part for KDE can call the same function with its own `Desktop`. The function is in this crate now. When a second desktop part comes, move the module `launch.rs` to its own crate.

| Action | `execute` does |
|---|---|
| `Nothing` | Nothing. Gives `Done`. |
| `OpenFile(path)` | `g_app_info_launch_default_for_uri` with the `file://` URI. |
| `OpenFolder(path)`, `ShowInFolder(path)` | `Desktop::open_folder`, `Desktop::show_in_folder`. A fallback gives `Executed::Fallback`. |
| `LaunchDesktopEntry(path)` | `g_desktop_app_info_new_from_filename`, then `g_app_info_launch` with no files. GLib starts a D-Bus activatable entry through D-Bus, and an entry with `Terminal=true` in a terminal. These two cases were not tested. |
| `OpenUrl { browser: None }` | `g_app_info_launch_default_for_uri`: the default browser. |
| `OpenUrl { browser: Some(App(id)) }` | `g_desktop_app_info_new(id)`, then `g_app_info_launch_uris`. An ID that is not installed is an error. |
| `OpenUrl { browser: Some(Command(text)) }` | `g_app_info_create_from_commandline` with each `%` of the text doubled, then `g_app_info_launch_uris`. GIO splits the text with the quoting rules of the shell (`g_shell_parse_argv`) and adds the URL as the last argument. No shell runs the command, so `~` and `$NAME` stay as they are. An empty text and a quote that does not end are errors. |
| `RunScript { script, argument }` | `sh -c script petlauncher-script argument`, so `argument` is `$1` and `$0` is `petlauncher-script`. The working directory is the home directory. Standard input is `/dev/null`. The script has its own process group, so Ctrl+C or a hang-up of a terminal that started petlauncher does not stop it. A thread waits for the end of the script, so it does not stay as a zombie process. |
| `System(command)` | `Desktop::run_system_command`. |
| `Copy`, `OpenDictionary`, `SetQuery` | Nothing. Gives `NotHandled`: the program does them. |

GLib gives the activation token of the launch context to a program that it starts, when the entry permits startup notification (`StartupNotify=true`; `OpenUrl` with a command has the flag `SUPPORTS_STARTUP_NOTIFICATION`). This is the documented behaviour of GLib, and it was not tested here. A script gets no token.

GIO gives the local path, not the `file://` URI, to the field code `%u` or `%U` of an application, unless the entry has `X-GIO-NoFuse=true`. The tests show this.

Limit: the programs and scripts that petlauncher starts stay in the systemd unit of petlauncher. On this desktop gnome-session starts each autostart program in its own systemd scope, for example `app-gnome-engineering\x2dai\x2dsuite-117856.scope` with the description "Application launched by gnome-session-binary" (`systemctl --user list-units`). A stop of that unit, for example with `systemctl --user stop`, also stops the programs that petlauncher started. A normal quit of petlauncher does not stop them: a test shows this for the processes, but the scope was not tested. The crate does not move each program to its own scope, as GNOME Shell does.

## Decisions for the Desktop trait

These changes of `petlauncher-core` came with this crate:

| Change | Reason |
|---|---|
| `open_folder` and `show_in_folder` give `FolderOpened`. | The fallback is a success with a reason, not an error, and the program can write it to the log. |
| `settings_pages` is removed. | `petlauncher-apps` already finds the settings pages with a rule that the caller gives. The program gives `petlauncher_gnome::is_settings_page` to `Applications::load`. The rule is still in `petlauncher-apps::gnome`. |
| `color_scheme`, `watch_color_scheme` and `ColorScheme` are removed. | libadwaita follows the style of the system by itself (R17). On this desktop the settings portal gives `org.freedesktop.appearance color-scheme` = 1 (prefer dark), the same as `org.gnome.desktop.interface color-scheme` = `'prefer-dark'` in dconf. |
| `set_tray_icon_visible` stays. | The crate makes the disabled stateful action `tray-icon` of ADR 0005, the program adds it with `add_actions`, and `set_tray_icon_visible` changes its state. Thus the program does not know the contract of the extension. A KDE part could show a StatusNotifierItem in the same method. |
| `center_window` stays and does nothing on GNOME. | The extension centers the window (ADR 0005). |

## Evidence

| File | Contents |
|---|---|
| `introspection/FileManager1.txt` | `gdbus introspect` of `org.freedesktop.FileManager1` (Nautilus 46.4) |
| `introspection/ScreenSaver.txt` | `gdbus introspect` of `org.gnome.ScreenSaver` |
| `introspection/SessionManager.txt` | `gdbus introspect` of `org.gnome.SessionManager` (gnome-session 46.0) |
| `introspection/login1-busctl.txt` | `busctl --system --auto-start=no introspect` of `org.freedesktop.login1` (systemd 255.4) |
| `introspection/portal-desktop.txt` | `gdbus introspect` of `org.freedesktop.portal.Desktop` |
| `introspection/read-only-calls.txt` | `org.freedesktop.portal.Settings.ReadOne` of the color scheme, and the process that owns each name |
| `introspection/gnome-shell-46-system-actions-strings.txt` | The calls of the system menu of GNOME Shell 46, from the JavaScript in `libshell-14.so` |
| `introspection/gnome-shell-46-screenSaverService.js` | The service `org.gnome.ScreenSaver` of GNOME Shell 46 |
| `introspection/gnome-session-quit-man.txt` | The manual of `gnome-session-quit` |
| `introspection/versions.txt` | The package versions |
| `live-settings-read-only.txt` | The custom shortcuts and the color scheme of the owner, read with `dconf read` |
| `2026-11-26-cargo-test-*.txt`, `2026-11-26-clippy.txt`, `2026-11-26-llvm-cov-gnome.txt` | The test results |
| `2026-11-26-crate-versions.txt` | The gtk-rs versions on crates.io and in `Cargo.lock`, and the lowest GTK and libadwaita versions of `gtk4` 0.11 and `libadwaita` 0.9 |
| `2026-11-26-systemd-scopes.txt` | The systemd scopes of the programs that gnome-session and gsd-media-keys started |

All evidence was read on 2026-11-26 with read-only commands: introspection, `dconf read`, `ReadOne` of the settings portal, and `GetConnectionUnixProcessID` of the bus. No command changed the session or the settings.

Results that the code depends on:

- `org.freedesktop.FileManager1` has `ShowFolders(in as URIs, in s StartupId)`, `ShowItems(in as URIs, in s StartupId)` and `ShowItemProperties(in as URIs, in s StartupId)`. Nautilus (`/usr/bin/nautilus --gapplication-service`) owns the name.
- `org.gnome.ScreenSaver` has `Lock()`, `GetActive(out b)`, `SetActive(in b)` and `GetActiveTime(out u)`. The owner of the name is `/usr/bin/gjs -m /usr/share/gnome-shell/org.gnome.ScreenSaver`. Its `screenSaverService.js` gives `Lock` to `org.gnome.Shell.ScreenShield` in GNOME Shell. In GNOME Shell, `LockAsync` returns after the signal `lock-screen-shown`.
- `org.gnome.SessionManager` has `Logout(in u mode)`, `Reboot()`, `Shutdown()`, `CanShutdown(out b)`, `RequestReboot()` and `RequestShutdown()`, among others. `/usr/libexec/gnome-session-binary` owns the name.
- `org.freedesktop.login1.Manager` has `Suspend(b interactive)` and `CanSuspend() -> s`. `CanSuspend` was not called.
- The JavaScript of GNOME Shell 46 in `libshell-14.so`, read with `strings`: the log out, power off and restart actions of the system menu call `this._session.LogoutAsync(0)`, `this._session.ShutdownAsync(0)` and `this._session.RebootAsync()`, and the login manager calls `this._proxy.SuspendAsync(true)`. The lock action of the menu calls the screen shield directly, in the same process.
- The owner had two custom shortcuts of other programs, one of them with `<Super>space`. `org.gnome.desktop.wm.keybindings switch-input-source` is `[]`, so no window manager key uses Super+Space.
- In the sandbox of the agent, `gsettings get` gave the schema defaults and `gdbus` could not connect to the system bus. `dconf read` and `busctl` worked. The program does not run in that sandbox.

## Test

```
TMPDIR=$HOME/tmp cargo test -p petlauncher-gnome
```

The tests need `dbus-daemon`, `glib-compile-schemas`, `ps`, the MIME database `/usr/share/mime`, and a `TMPDIR` that permits executable files. No test reaches the session of the owner:

| Area | Method |
|---|---|
| D-Bus | A private `dbus-daemon` with its own configuration and no service directories, so a call to a name with no owner fails and starts no real service. Mock services own the names and record each call with its signature. The code gets the connections from `GnomeDesktop::with_parts`. A second private bus is the system bus. |
| GSettings | The memory backend, and test schemas compiled from `tests/schemas/` with a part of the built-in bindings and their GNOME defaults. The names come from a test copy of the files of GNOME Settings in `tests/keybindings/`. Before the first settings object, the test process sets `GSETTINGS_BACKEND=memory`, and each test asserts that the default backend is `GMemorySettingsBackend`. A test compares the test schemas with the installed schemas, and a test reads the installed files of GNOME Settings, read only. |
| R4 in a separate process | A child process with `GSETTINGS_BACKEND=keyfile` and a key file in its own `XDG_CONFIG_HOME`. The key file starts with two custom shortcuts with invented names, one with Super+Space. The child asserts that its default backend is `GKeyfileSettingsBackend` with that file, then uses `GnomeDesktop::new`, `set_global_key_if_free` (refused for the conflict), `remove_key` and `set_global_key_if_free` again. The parent reads the key file. |
| Programs | A child process with an empty environment and its own `HOME`, XDG directories, private session bus, `GSETTINGS_BACKEND=memory`, and no `WAYLAND_DISPLAY` or `DISPLAY`. Its `XDG_DATA_DIRS` has only the MIME database, so the applications of the system are not visible. Its only applications are recorders that write their arguments to a file. Its `mimeapps.list` makes them the default applications. |
| Script | A script that writes its working directory, `$1` and its process group to a file, sleeps 2 s, and writes `survived`. The child process stops first. |

Results on 2026-11-26, after the change to the conflict check of R4: 74 tests pass, and 2 ignored tests are the child side of the separate processes. Line coverage 94.3 % (`cargo llvm-cov`). After the tests, `dconf read` of the custom shortcuts of the owner gave the same entries and keys as before, with no petlauncher entry, and no test process stayed.

## Not tested on the live session

The rules of this work did not permit these steps on the owner's session. The owner does them by hand. Most need the GTK program; the `gdbus` steps can be done now.

Before the steps, make a copy of the shortcut settings:

```
dconf dump /org/gnome/settings-daemon/plugins/media-keys/ > ~/media-keys-before-petlauncher.ini
```

To restore them: `dconf load /org/gnome/settings-daemon/plugins/media-keys/ < ~/media-keys-before-petlauncher.ini`.

| # | Requirement | Step | Expected result |
|---|---|---|---|
| 1 | R2 | Run `gdbus call --session --dest org.freedesktop.FileManager1 --object-path /org/freedesktop/FileManager1 --method org.freedesktop.FileManager1.ShowItems "['file:///usr/share/applications/org.gnome.Nautilus.desktop']" ""` | Files opens `/usr/share/applications/` with `org.gnome.Nautilus.desktop` selected. This is the first call of `ShowItems` on this desktop. |
| 2 | R1 | The same with `ShowFolders` and the URI of a folder with a space and `#` in its name, encoded as in "URIs" above. | Files opens that folder. |
| 3 | R1 step 1 | In petlauncher, type `~/lolcat-output/L0L3-IMAGE/` and press Enter. | Files opens the folder. The new window has the focus. |
| 4 | R1 step 4, R10 step 4 | Ctrl+Enter on a file row. | Files opens the folder with the file selected. |
| 5 | R2 step 1 | Ctrl+Enter on an application row. | Files opens the folder of the `.desktop` file, with the file selected. The application does not start. |
| 6 | R16 | Select Lock. | The screen locks. |
| 7 | R16 | Select Suspend. | The computer suspends. No password dialog shows on a normal local session. |
| 8 | R16 | Select Log out. | The GNOME confirmation window for log out shows. Click Cancel. |
| 9 | R16 | Select Restart. | The GNOME confirmation window for restart shows. Click Cancel. |
| 10 | R16 | Select Power off. | The GNOME confirmation window for power off shows. Click Cancel. |
| 11 | R16 | Type `wifi` and press Enter. | GNOME Settings opens on the Wi-Fi page. |
| 12 | R4 | Start petlauncher while a different custom shortcut has Super+Space. | The launcher shows a notice with the name of that shortcut and its place in GNOME Settings. GNOME Settings shows no "Show petlauncher" entry, and the other shortcut did not change. |
| 13 | R4 | Press Super+Space. | petlauncher shows. Press it again: petlauncher hides. |
| 14 | R4 | Remove the key of the other shortcut in GNOME Settings, then open the launcher from the top bar icon. | "Show petlauncher" has Super+Space, and the notice is gone. |
| 15 | R4 | Change the key of "Show petlauncher" in GNOME Settings. | The settings window of petlauncher shows the new key (`global_key`). |
| 16 | R3 | Set "Show tray icon" on and off. | The icon of the extension shows and goes. |
| 17 | R12 | A shortcut with the browser Microsoft Edge, and one with a browser command. | The URL opens in Edge, and in the browser of the command. The new window has the focus. |
| 18 | R6 | A script shortcut, then quit petlauncher from the top bar menu while the script runs. | The script continues. |
| 19 | R17 | Change the style in GNOME Settings, Appearance, from dark to light. | The petlauncher window changes at once (libadwaita). |

Also not tested:

- The activation token. A test launch context (`GAppLaunchContext`) gives no token, so the tests send an empty `StartupId`. The Rust bindings have no subclass support for `GAppLaunchContext`, so a test could not give a fixed token. Steps 3, 4 and 17 check the focus.
- The connection to the real system bus at the first suspend.
- A call that waits longer than the timeout of 10 s, for example a polkit password dialog that stays open.
