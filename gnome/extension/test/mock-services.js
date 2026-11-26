// Recording mock services for `nested-test.sh --program`. They own the
// names that `petlauncher-gnome` calls, on the private session bus or the
// fake system bus of the test, and write each call to a JSON lines file.
// Nothing real happens: no file manager opens, no lock, no log out.
//
// Run: gjs -m mock-services.js --bus session|system --out FILE
// The process prints "ready" when it owns all its names.
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import System from 'system';

const SERVICES = {
    session: [
        {
            name: 'org.freedesktop.FileManager1',
            path: '/org/freedesktop/FileManager1',
            xml: `<node><interface name="org.freedesktop.FileManager1">
                <method name="ShowFolders"><arg type="as" direction="in"/><arg type="s" direction="in"/></method>
                <method name="ShowItems"><arg type="as" direction="in"/><arg type="s" direction="in"/></method>
                <method name="ShowItemProperties"><arg type="as" direction="in"/><arg type="s" direction="in"/></method>
                </interface></node>`,
            methods: ['ShowFolders', 'ShowItems', 'ShowItemProperties'],
        },
        {
            name: 'org.gnome.SessionManager',
            path: '/org/gnome/SessionManager',
            xml: `<node><interface name="org.gnome.SessionManager">
                <method name="Logout"><arg type="u" direction="in"/></method>
                <method name="Reboot"/>
                <method name="Shutdown"/>
                </interface></node>`,
            methods: ['Logout', 'Reboot', 'Shutdown'],
        },
        {
            name: 'org.gnome.ScreenSaver',
            path: '/org/gnome/ScreenSaver',
            xml: `<node><interface name="org.gnome.ScreenSaver">
                <method name="Lock"/>
                </interface></node>`,
            methods: ['Lock'],
        },
    ],
    system: [
        {
            name: 'org.freedesktop.login1',
            path: '/org/freedesktop/login1',
            xml: `<node><interface name="org.freedesktop.login1.Manager">
                <method name="Suspend"><arg type="b" direction="in"/></method>
                </interface></node>`,
            methods: ['Suspend'],
        },
    ],
};

let bus = null;
let out = null;
for (let i = 0; i < ARGV.length; i++) {
    if (ARGV[i] === '--bus')
        bus = ARGV[++i];
    else if (ARGV[i] === '--out')
        out = ARGV[++i];
}
if (!SERVICES[bus] || !out) {
    printerr('usage: gjs -m mock-services.js --bus session|system --out FILE');
    System.exit(2);
}

const connection = Gio.bus_get_sync(bus === 'system' ? Gio.BusType.SYSTEM : Gio.BusType.SESSION, null);
const file = Gio.File.new_for_path(out);

function record(service, method, args) {
    const line = `${JSON.stringify({
        t: Math.round(GLib.get_monotonic_time() / 100) / 10,
        bus,
        name: service.name,
        method,
        args,
    })}\n`;
    const stream = file.append_to(Gio.FileCreateFlags.NONE, null);
    stream.write_all(new TextEncoder().encode(line), null);
    stream.close(null);
}

const exported = [];
let waiting = SERVICES[bus].length;
for (const service of SERVICES[bus]) {
    const object = {};
    for (const method of service.methods)
        // gjs adds a last argument after the D-Bus arguments.
        object[method] = (...args) => record(service, method,
            args.filter(arg => arg !== undefined && !(arg instanceof Gio.DBusMethodInvocation)));
    const wrapper = Gio.DBusExportedObject.wrapJSObject(service.xml, object);
    wrapper.export(connection, service.path);
    exported.push(wrapper);
    Gio.bus_own_name_on_connection(connection, service.name,
        Gio.BusNameOwnerFlags.DO_NOT_QUEUE,
        () => {
            waiting -= 1;
            if (waiting === 0)
                print('ready');
        },
        () => {
            printerr(`the name ${service.name} is not available`);
            System.exit(1);
        });
}

new GLib.MainLoop(null, false).run();
