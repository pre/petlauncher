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
 * The height of the launcher frame with no rows, in logical pixels. The
 * program makes the launcher this high when it has no rows.
 */
export const COLLAPSED_HEIGHT = 64;

/**
 * The top left corner of the launcher frame. The launcher is at the
 * horizontal center of the work area, and its top is where the top of a
 * collapsed launcher is at the vertical center. Thus the query line has the
 * same place for each height of the window, and the rows go down from it.
 * A launcher that goes below the work area moves up, and a launcher that is
 * larger than the work area has its top left corner in the work area.
 *
 * @param {{width: number, height: number}} size - the frame size of the window
 * @param {{x: number, y: number, width: number, height: number}} workArea -
 *   the monitor without the top bar
 * @returns {{x: number, y: number}}
 */
export function launcherPosition(size, workArea) {
    const top = workArea.y + Math.max(0, Math.floor((workArea.height - COLLAPSED_HEIGHT) / 2));
    const lowestTop = workArea.y + Math.max(0, workArea.height - size.height);
    return {
        x: workArea.x + Math.max(0, Math.floor((workArea.width - size.width) / 2)),
        y: Math.min(top, lowestTop),
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
