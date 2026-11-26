// The GNOME part of petlauncher: it puts the launcher window at the center of
// the monitor with the pointer each time the window shows, with the query line
// at the same place for each window height. It also adds the petlauncher icon
// to the top bar. docs/gnome-extension.md gives the contract with the program
// and the test results.
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

import {isLauncher, launcherPosition} from './launcher.js';
import {Tray} from './tray.js';

export default class PetlauncherExtension extends Extension {
    enable() {
        this._mapId = global.window_manager.connect('map',
            (_wm, actor) => this._onMap(actor.meta_window));
        this._tray = new Tray(this.uuid);
    }

    disable() {
        global.window_manager.disconnect(this._mapId);
        this._mapId = 0;
        this._tray.destroy();
        this._tray = null;
    }

    // GNOME emits `map` after the first buffer of the window and before it
    // paints the window. Mutter does a move that keeps the size at once, so
    // the first painted frame is already at its place.
    _onMap(window) {
        if (!isLauncher(window.get_wm_class(), window.get_title()))
            return;
        const monitor = global.display.get_current_monitor();
        const workArea = window.get_work_area_for_monitor(monitor);
        const {x, y} = launcherPosition(window.get_frame_rect(), workArea);
        window.move_frame(false, x, y);
        Main.activateWindow(window);
    }
}
