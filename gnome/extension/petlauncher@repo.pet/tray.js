// The petlauncher icon in the top bar. It shows while the program is on the
// session bus and the state of its action `tray-icon` is true. The icon and
// its menu call the actions of the program through org.gtk.Actions.
import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GObject from 'gi://GObject';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {APP_ID, OBJECT_PATH, Action, trayIconVisible} from './launcher.js';

const TrayButton = GObject.registerClass(
class TrayButton extends PanelMenu.Button {
    _init(activate) {
        super._init(0.5, 'petlauncher');
        this._activate = activate;
        this.add_child(new St.Icon({
            icon_name: 'system-search-symbolic',
            style_class: 'system-status-icon',
        }));
        this.menu.addAction('Show petlauncher', () => activate(Action.SHOW));
        this.menu.addAction('Settings', () => activate(Action.SETTINGS));
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        this.menu.addAction('Quit', () => activate(Action.QUIT));
    }

    // The primary button shows the launcher. The other buttons and a touch
    // open the menu.
    vfunc_event(event) {
        if (event.type() === Clutter.EventType.BUTTON_PRESS &&
            event.get_button() === Clutter.BUTTON_PRIMARY) {
            this.menu.close();
            this._activate(Action.SHOW);
            return Clutter.EVENT_STOP;
        }
        return super.vfunc_event(event);
    }
});

export class Tray {
    constructor(role) {
        this._actions = null;
        this._button = new TrayButton(name => this._actions?.activate_action(name, null));
        this._button.visible = false;
        Main.panel.addToStatusArea(role, this._button);
        this._watch = Gio.bus_watch_name(Gio.BusType.SESSION, APP_ID,
            Gio.BusNameWatcherFlags.NONE,
            (_connection, _name, owner) => this._connect(owner),
            () => this._disconnect());
    }

    destroy() {
        Gio.bus_unwatch_name(this._watch);
        this._disconnect();
        this._button.destroy();
        this._button = null;
    }

    _connect(owner) {
        this._disconnect();
        this._actions = Gio.DBusActionGroup.get(Gio.DBus.session, owner, OBJECT_PATH);
        this._actions.connectObject(
            'action-added', () => this._update(),
            'action-removed', () => this._update(),
            'action-state-changed', () => this._update(),
            this);
        // The first query starts to read the actions of the program. The
        // group then emits action-added for each action.
        this._update();
    }

    _disconnect() {
        this._actions?.disconnectObject(this);
        this._actions = null;
        this._update();
    }

    _update() {
        let state = null;
        if (this._actions?.has_action(Action.TRAY_ICON))
            state = this._actions.get_action_state(Action.TRAY_ICON)?.unpack() ?? null;
        this._button.visible = trayIconVisible(state);
    }
}
