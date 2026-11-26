// The petlauncher icon in the top bar. It shows while the program is on the
// session bus and the state of its action `tray-icon` is true. The icon and
// its menu call the actions of the program through org.gtk.Actions.
import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {Action, trayIconVisible} from './launcher.js';

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
    /**
     * @param {string} role - the role in the status area of the top bar
     * @param {ProgramActions} actions - the actions of the program
     */
    constructor(role, actions) {
        this._actions = actions;
        this._button = new TrayButton(name => actions.activate(name));
        this._button.visible = false;
        Main.panel.addToStatusArea(role, this._button);
    }

    destroy() {
        this._button.destroy();
        this._button = null;
    }

    update() {
        this._button.visible = trayIconVisible(this._actions.state(Action.TRAY_ICON));
    }
}
