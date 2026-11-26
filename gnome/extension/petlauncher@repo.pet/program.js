// The actions of the petlauncher program on the session bus, through
// org.gtk.Actions (ADR 0005). The group keeps the states of the actions, so
// a read of a state does not wait for D-Bus.
import Gio from 'gi://Gio';

import {APP_ID, OBJECT_PATH} from './launcher.js';

export class ProgramActions {
    /**
     * @param {Function} onChange - called after a change of the actions or
     *   of their states, and when the program starts or stops
     */
    constructor(onChange) {
        this._onChange = onChange;
        this._actions = null;
        this._watch = Gio.bus_watch_name(Gio.BusType.SESSION, APP_ID,
            Gio.BusNameWatcherFlags.NONE,
            (_connection, _name, owner) => this._connect(owner),
            () => this._disconnect());
    }

    destroy() {
        Gio.bus_unwatch_name(this._watch);
        this._disconnect();
    }

    activate(name) {
        this._actions?.activate_action(name, null);
    }

    /**
     * @param {string} name - the name of the action
     * @returns {*} the unpacked state, or null when the program does not run
     *   or does not have the action
     */
    state(name) {
        if (!this._actions?.has_action(name))
            return null;
        return this._actions.get_action_state(name)?.unpack() ?? null;
    }

    _connect(owner) {
        this._disconnect();
        this._actions = Gio.DBusActionGroup.get(Gio.DBus.session, owner, OBJECT_PATH);
        this._actions.connectObject(
            'action-added', () => this._onChange(),
            'action-removed', () => this._onChange(),
            'action-state-changed', () => this._onChange(),
            this);
        // The first query starts to read the actions of the program. The
        // group then emits action-added for each action.
        this._onChange();
    }

    _disconnect() {
        this._actions?.disconnectObject(this);
        this._actions = null;
        this._onChange();
    }
}
