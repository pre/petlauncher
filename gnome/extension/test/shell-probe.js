// Automation script for the test GNOME Shell (gnome-shell
// --automation-script). It is test equipment, not a part of the extension.
//
// - It sets the unsafe mode, so that org.gnome.Shell.Eval works.
// - It makes a virtual pointer and a virtual keyboard, so that the test can
//   move the pointer, click and type.
// - It records the windows from their creation: each move and size change of
//   the frame, the map, and each frame that a monitor paints while the window
//   is visible (opacity above 0 and on that monitor), from the creation of
//   the window until 2 s after its map.
//
// The test calls the functions of globalThis.petProbe through Eval.
import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const PAINT_RECORD_MS = 2000;
const TRAY_ROLE = 'petlauncher@repo.pet';

let pointer = null;
let keyboard = null;
let clipboardText = null;
let finish = null;
const events = [];
// MetaWindow -> time of its map, or null before the map.
const tracked = new Map();

const now = () => Math.round(GLib.get_monotonic_time() / 100) / 10;
const plain = r => ({x: r.x, y: r.y, width: r.width, height: r.height});

function describeWindow(window) {
    return {
        seq: window.get_stable_sequence(),
        title: window.get_title(),
        wmClass: window.get_wm_class(),
        gtkAppId: window.get_gtk_application_id(),
        frame: plain(window.get_frame_rect()),
        monitor: window.get_monitor(),
        focus: global.display.focus_window === window,
    };
}

function record(type, window, extra = {}) {
    events.push({t: now(), type, ...describeWindow(window), ...extra});
}

function watchWindow(window) {
    tracked.set(window, null);
    record('created', window);
    window.connect('position-changed', () => record('position-changed', window));
    window.connect('size-changed', () => record('size-changed', window));
    window.connect('unmanaged', () => {
        events.push({t: now(), type: 'unmanaged', seq: window.get_stable_sequence()});
        tracked.delete(window);
    });
}

function extentsOf(actor) {
    const e = actor.get_transformed_extents();
    return {
        x: Math.round(e.get_x()),
        y: Math.round(e.get_y()),
        width: Math.round(e.get_width()),
        height: Math.round(e.get_height()),
    };
}

function overlaps(a, b) {
    return Math.min(a.x + a.width, b.x + b.width) > Math.max(a.x, b.x) &&
        Math.min(a.y + a.height, b.y + b.height) > Math.max(a.y, b.y);
}

function monitorOfView(view) {
    const l = view.layout;
    return Main.layoutManager.monitors.findIndex(m => m.x === l.x && m.y === l.y);
}

function recordPaint(view) {
    const t = now();
    for (const [window, mapped] of tracked) {
        if (mapped !== null && t - mapped > PAINT_RECORD_MS)
            continue;
        const actor = window.get_compositor_private();
        if (!actor || !actor.is_mapped() || actor.get_paint_opacity() === 0)
            continue;
        const extents = extentsOf(actor);
        if (!overlaps(extents, view.layout))
            continue;
        const [actorX, actorY] = actor.get_position();
        events.push({
            t,
            type: 'paint',
            seq: window.get_stable_sequence(),
            title: window.get_title(),
            view: monitorOfView(view),
            msAfterMap: mapped === null ? null : Math.round((t - mapped) * 10) / 10,
            actor: {x: actorX, y: actorY},
            extents,
            opacity: actor.get_paint_opacity(),
            frame: plain(window.get_frame_rect()),
        });
    }
}

function trayState() {
    const button = Main.panel.statusArea[TRAY_ROLE];
    if (!button)
        return {exists: false};
    return {
        exists: true,
        visible: button.visible,
        mapped: button.is_mapped(),
        extents: extentsOf(button),
        menuOpen: button.menu.isOpen,
        items: button.menu._getMenuItems().map(item => ({
            label: item.label?.text ?? null,
            mapped: item.is_mapped(),
            extents: extentsOf(item),
        })),
    };
}

function state() {
    const display = global.display;
    const [px, py] = global.get_pointer();
    return {
        monitors: Main.layoutManager.monitors.map(m => ({
            index: m.index,
            geometry: plain(display.get_monitor_geometry(m.index)),
            scale: display.get_monitor_scale(m.index),
            workArea: plain(Main.layoutManager.getWorkAreaForMonitor(m.index)),
        })),
        pointer: {x: px, y: py},
        currentMonitor: display.get_current_monitor(),
        focus: display.focus_window ? describeWindow(display.focus_window) : null,
        windows: global.get_window_actors().map(a => describeWindow(a.meta_window)),
        tray: trayState(),
        overview: Main.overview.visible,
        animations: St.Settings.get().enable_animations,
    };
}

// The virtual pointer is made at the first use: in a nested shell (X11
// backend) a virtual device can send fake input to the host display.
function movePointer(x, y) {
    if (!pointer) {
        const seat = Clutter.get_default_backend().get_default_seat();
        pointer = seat.create_virtual_device(Clutter.InputDeviceType.POINTER_DEVICE);
    }
    pointer.notify_absolute_motion(GLib.get_monotonic_time(), x, y);
}

function click(x, y, button) {
    movePointer(x, y);
    pointer.notify_button(GLib.get_monotonic_time(), button, Clutter.ButtonState.PRESSED);
    pointer.notify_button(GLib.get_monotonic_time(), button, Clutter.ButtonState.RELEASED);
}

// Makes the virtual keyboard. Mutter adds the device a short time after this
// call, and drops the events before that. Wait some time before keys().
function prepareKeyboard() {
    if (!keyboard) {
        const seat = Clutter.get_default_backend().get_default_seat();
        keyboard = seat.create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE);
    }
}

function keyboardDevice() {
    if (!keyboard)
        throw new Error('call petProbe.prepareKeyboard() first');
    return keyboard;
}

function pressAndRelease(keyvals) {
    const device = keyboardDevice();
    for (const keyval of keyvals)
        device.notify_keyval(GLib.get_monotonic_time(), keyval, Clutter.KeyState.PRESSED);
    for (const keyval of [...keyvals].reverse())
        device.notify_keyval(GLib.get_monotonic_time(), keyval, Clutter.KeyState.RELEASED);
}

// Presses keys one after the other. A name is a Clutter key name such as
// 'Return' or 'Down'. 'Control_L+Return' holds Control_L during Return.
function keys(...names) {
    for (const name of names) {
        pressAndRelease(name.split('+').map(part => {
            const keyval = Clutter[`KEY_${part}`];
            if (keyval === undefined)
                throw new Error(`unknown key ${part}`);
            return keyval;
        }));
    }
}

// Types text. The virtual keyboard adds Shift for a character such as ~.
function type(text) {
    for (const character of text)
        pressAndRelease([Clutter.unicode_to_keysym(character.codePointAt(0))]);
}

// Puts the text on the clipboard, for a paste with Control+V.
function setClipboard(text) {
    St.Clipboard.get_default().set_text(St.ClipboardType.CLIPBOARD, text);
}

// Starts to read the text of the clipboard. clipboard() gives it later.
function readClipboard() {
    clipboardText = null;
    St.Clipboard.get_default().get_text(St.ClipboardType.CLIPBOARD, (_clipboard, text) => {
        clipboardText = text;
    });
}

async function screenshot(path) {
    const file = Gio.File.new_for_path(path);
    const stream = file.replace(null, false, Gio.FileCreateFlags.NONE, null);
    await new Shell.Screenshot().screenshot(false, stream);
    stream.close(null);
    return path;
}

// A screenshot of the frame of the window with this title, with a margin
// for its shadow.
async function screenshotWindow(title, path, margin = 32) {
    const window = global.get_window_actors().map(a => a.meta_window)
        .find(w => w.get_title() === title);
    if (!window)
        throw new Error(`no window with the title ${title}`);
    const r = window.get_frame_rect();
    const file = Gio.File.new_for_path(path);
    const stream = file.replace(null, false, Gio.FileCreateFlags.NONE, null);
    await new Shell.Screenshot().screenshot_area(r.x - margin, r.y - margin,
        r.width + 2 * margin, r.height + 2 * margin, stream);
    stream.close(null);
    return path;
}

function displayConfig(method, parameters) {
    return new Promise((resolve, reject) => {
        Gio.DBus.session.call('org.gnome.Mutter.DisplayConfig', '/org/gnome/Mutter/DisplayConfig',
            'org.gnome.Mutter.DisplayConfig', method, parameters, null, Gio.DBusCallFlags.NONE,
            -1, null, (bus, result) => {
                try {
                    resolve(bus.call_finish(result));
                } catch (e) {
                    reject(e);
                }
            });
    });
}

// Gives the monitors, in the order of --virtual-monitor, a place and a scale
// in the logical layout, for example [{x: 0, y: 0, scale: 2}, ...], and
// optionally a mode size such as size: '640x400'. The first monitor is the
// primary monitor. The change is temporary.
// The layout mode (1 logical, 2 physical) and the scales of each monitor.
async function displayState() {
    const [, monitors, , properties] =
        (await displayConfig('GetCurrentState', null)).deepUnpack();
    return {
        layoutMode: properties['layout-mode']?.unpack(),
        supportsChangingLayoutMode: properties['supports-changing-layout-mode']?.unpack(),
        monitors: monitors.map(([spec, modes]) => {
            const mode = modes.find(m => m[6]['is-current']?.unpack());
            return {connector: spec[0], mode: mode[0], supportedScales: mode[5]};
        }),
    };
}

async function applyLayout(layout) {
    const [serial, monitors, , properties] =
        (await displayConfig('GetCurrentState', null)).deepUnpack();
    const modeOf = (monitor, size) => monitor[1].find(mode => size
        ? `${mode[1]}x${mode[2]}` === size
        : mode[6]['is-current']?.unpack())[0];
    const logical = layout.map(({x, y, scale, size}, i) => [x, y, scale, 0, i === 0,
        [[monitors[i][0][0], modeOf(monitors[i], size), {}]]]);
    const options = {};
    if (properties['supports-changing-layout-mode']?.unpack())
        options['layout-mode'] = new GLib.Variant('u', 1);
    await displayConfig('ApplyMonitorsConfig',
        new GLib.Variant('(uua(iiduba(ssa{sv}))a{sv})', [serial, 1, logical, options]));
    return displayState();
}

export function init() {
    global.context.unsafe_mode = true;
    Main.messageTray.bannerBlocked = true;

    global.display.connect('window-created', (_display, window) => watchWindow(window));
    global.window_manager.connect('map', (_wm, actor) => {
        const window = actor.meta_window;
        tracked.set(window, now());
        record('map', window);
        // The extension handles the same signal. Its move is in the frame
        // after its handler, so record that frame once more.
        GLib.idle_add(GLib.PRIORITY_HIGH, () => {
            record('map-settled', window);
            return GLib.SOURCE_REMOVE;
        });
    });
    global.stage.connect('after-paint', (_stage, view) => recordPaint(view));
    Main.layoutManager.connect('startup-complete', () => Main.overview.hide());

    globalThis.petProbe = {
        state,
        events: (since = 0) => events.filter(e => e.t >= since),
        now,
        pointer: (x, y) => movePointer(x, y),
        click,
        prepareKeyboard,
        keys,
        type,
        setClipboard,
        readClipboard,
        clipboard: () => clipboardText,
        screenshot,
        screenshotWindow,
        applyLayout,
        displayState,
        finish: () => finish?.(),
    };
}

// The shell runs this after its start and quits when it returns.
export function run() {
    return new Promise(resolve => {
        finish = resolve;
    });
}
