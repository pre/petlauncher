// The contract between the extension and the petlauncher program, as plain
// values and functions with no GNOME Shell import, so that gjs can test them.
// docs/gnome-extension.md describes the contract.

export const APP_ID = 'pet.repo.Petlauncher';
export const OBJECT_PATH = '/pet/repo/Petlauncher';
export const LAUNCHER_TITLE = 'petlauncher';

export const Action = Object.freeze({
    SHOW: 'show',
    SETTINGS: 'settings',
    QUIT: 'quit',
    TRAY_ICON: 'tray-icon',
});

/**
 * @param {string|null} appId - the Wayland application ID of the window
 * @param {string|null} title - the window title
 * @returns {boolean} true for the launcher window of petlauncher
 */
export function isLauncher(appId, title) {
    return appId === APP_ID && title === LAUNCHER_TITLE;
}

/**
 * The top left corner that puts a window at the center of a work area. When
 * the window is larger than the work area, its top left corner stays in it.
 *
 * @param {{width: number, height: number}} size - the frame size of the window
 * @param {{x: number, y: number, width: number, height: number}} workArea -
 *   the monitor without the top bar
 * @returns {{x: number, y: number}}
 */
export function centeredPosition(size, workArea) {
    return {
        x: workArea.x + Math.max(0, Math.floor((workArea.width - size.width) / 2)),
        y: workArea.y + Math.max(0, Math.floor((workArea.height - size.height) / 2)),
    };
}

/**
 * @param {boolean|null} state - the state of the action `tray-icon`, or null
 *   when the program does not have the action
 * @returns {boolean}
 */
export function trayIconVisible(state) {
    return state === true;
}
