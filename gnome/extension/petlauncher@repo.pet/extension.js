// The GNOME part of petlauncher: it puts the launcher window at the center of
// the monitor with the pointer each time the window shows, with the query line
// at the same place for each window height. A launcher that the owner moved
// opens again at that place while the query is kept (ADR 0009). It also adds
// the petlauncher icon to the top bar. docs/gnome-extension.md gives the
// contract with the program and the test results.
import GLib from 'gi://GLib';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

import {Action, isLauncher, launcherPosition, movedPosition, queryKept} from './launcher.js';
import {ProgramActions} from './program.js';
import {Tray} from './tray.js';

export default class PetlauncherExtension extends Extension {
    enable() {
        this._placed = null;
        this._moved = null;
        this._actions = new ProgramActions(() => this._tray?.update());
        this._tray = new Tray(this.uuid, this._actions);
        this._mapId = global.window_manager.connect('map',
            (_wm, actor) => this._onMap(actor.meta_window));
        this._grabEndId = global.display.connect('grab-op-end',
            (_display, window) => this._onGrabEnd(window));
    }

    disable() {
        global.window_manager.disconnect(this._mapId);
        global.display.disconnect(this._grabEndId);
        this._mapId = this._grabEndId = 0;
        this._tray.destroy();
        this._tray = null;
        this._actions.destroy();
        this._actions = null;
    }

    // GNOME emits `map` after the first buffer of the window and before it
    // paints the window. Mutter does a move that keeps the size at once, so
    // the first painted frame is already at its place.
    _onMap(window) {
        if (!isLauncher(window.get_wm_class(), window.get_title()))
            return;
        const frame = window.get_frame_rect();
        const kept = queryKept(this._actions.state(Action.KEPT_QUERY_UNTIL), GLib.get_real_time());
        const moved = kept && this._moved ? movedPosition(this._moved, frame, workAreas(window)) : null;
        if (moved === null)
            this._moved = null;
        const monitor = global.display.get_current_monitor();
        this._placed = moved ?? launcherPosition(frame, window.get_work_area_for_monitor(monitor));
        window.move_frame(false, this._placed.x, this._placed.y);
        Main.activateWindow(window);
    }

    // The launcher cannot change its size with the mouse, so the end of a
    // grab at a different place is a move by the owner.
    _onGrabEnd(window) {
        if (!window || !isLauncher(window.get_wm_class(), window.get_title()))
            return;
        const {x, y} = window.get_frame_rect();
        if (x !== this._placed?.x || y !== this._placed?.y)
            this._moved = {x, y};
    }
}

function workAreas(window) {
    const count = global.display.get_n_monitors();
    return Array.from({length: count}, (_, monitor) => window.get_work_area_for_monitor(monitor));
}
