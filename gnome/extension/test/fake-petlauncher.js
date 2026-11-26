// A stand-in for the petlauncher program, for the test of the GNOME Shell
// extension. It has the windows and the D-Bus actions that
// docs/gnome-extension.md requires from the program, and some actions that
// only the test uses.
//
// Run: gjs -m fake-petlauncher.js [--app-id ID] [--launcher-title TITLE]
//          [--no-tray-icon]
//
// The process stays in the background with no window, as petlauncher does
// with "Run in background" on. Use the actions to show the windows, for example:
//   gdbus call --session --dest pet.repo.Petlauncher \
//     --object-path /pet/repo/Petlauncher \
//     --method org.freedesktop.Application.ActivateAction show '[]' '{}'
//
// Actions of the contract: show, settings, quit, tray-icon (boolean state).
// Test-only actions: hide, toggle, dictionary, grow, shrink, set-tray-icon
// (boolean), show-late-title (shows the launcher with no title and sets the
// title 100 ms later).
import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import Gtk from 'gi://Gtk?version=4.0';
import System from 'system';

const options = {
    appId: 'pet.repo.Petlauncher',
    launcherTitle: 'petlauncher',
    trayIcon: true,
};
for (let i = 0; i < ARGV.length; i++) {
    switch (ARGV[i]) {
    case '--app-id':
        options.appId = ARGV[++i];
        break;
    case '--launcher-title':
        options.launcherTitle = ARGV[++i];
        break;
    case '--no-tray-icon':
        options.trayIcon = false;
        break;
    default:
        printerr(`unknown argument ${ARGV[i]}`);
        System.exit(2);
    }
}

const QUERY_HEIGHT = 48;
const RESULTS_HEIGHT = 300;

function log(text) {
    print(`fake-petlauncher ${GLib.get_monotonic_time() / 1000 | 0} ${text}`);
}

const app = new Gtk.Application({
    application_id: options.appId,
    flags: Gio.ApplicationFlags.DEFAULT_FLAGS,
});

let launcher = null;
let results = null;
let settings = null;
let dictionary = null;

function makeLauncher() {
    const window = new Gtk.ApplicationWindow({
        application: app,
        title: options.launcherTitle,
        decorated: false,
        resizable: false,
    });
    const box = new Gtk.Box({orientation: Gtk.Orientation.VERTICAL});
    box.set_size_request(750, -1);
    const entry = new Gtk.Entry({placeholder_text: 'Query'});
    entry.set_size_request(-1, QUERY_HEIGHT);
    results = new Gtk.Box();
    results.set_size_request(-1, 0);
    box.append(entry);
    box.append(results);
    window.set_child(box);
    // The program hides the window; it does not destroy it.
    window.set_hide_on_close(true);
    window.connect('notify::is-active', () => log(`launcher is-active=${window.is_active}`));
    return window;
}

function makeWindow(title, width, height) {
    const window = new Gtk.ApplicationWindow({
        application: app,
        title,
        default_width: width,
        default_height: height,
    });
    window.set_child(new Gtk.Label({label: title}));
    window.set_hide_on_close(true);
    return window;
}

function addAction(name, handler, parameterType = null) {
    const action = new Gio.SimpleAction({
        name,
        parameter_type: parameterType ? new GLib.VariantType(parameterType) : null,
    });
    action.connect('activate', (_action, parameter) => {
        log(`action ${name}${parameter ? ` ${parameter.print(false)}` : ''}`);
        handler(parameter);
    });
    app.add_action(action);
}

app.connect('startup', () => {
    app.hold();
    launcher = makeLauncher();
    settings = makeWindow('petlauncher Settings', 600, 400);
    dictionary = makeWindow('Dictionary', 500, 500);

    addAction('show', () => launcher.present());
    addAction('hide', () => launcher.set_visible(false));
    addAction('toggle', () => {
        if (launcher.visible)
            launcher.set_visible(false);
        else
            launcher.present();
    });
    addAction('show-late-title', () => {
        launcher.set_title(null);
        launcher.present();
        GLib.timeout_add(GLib.PRIORITY_DEFAULT, 100, () => {
            launcher.set_title(options.launcherTitle);
            return GLib.SOURCE_REMOVE;
        });
    });
    addAction('settings', () => settings.present());
    addAction('dictionary', () => dictionary.present());
    addAction('grow', () => results.set_size_request(-1, RESULTS_HEIGHT));
    addAction('shrink', () => results.set_size_request(-1, 0));
    addAction('quit', () => app.quit());

    const trayIcon = new Gio.SimpleAction({
        name: 'tray-icon',
        state: new GLib.Variant('b', options.trayIcon),
        enabled: false,
    });
    app.add_action(trayIcon);
    addAction('set-tray-icon', value => trayIcon.set_state(value), 'b');
    log(`started app-id=${options.appId} launcher-title=${options.launcherTitle}`);
});

// A second start (for example `petlauncher toggle`) activates the first
// instance. The test does not use it.
app.connect('activate', () => log('activate'));

app.run([System.programInvocationName]);
