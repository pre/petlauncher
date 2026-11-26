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
    KEPT_QUERY_UNTIL: 'kept-query-until',
    WINDOW_POSITION: 'window-position',
    LAUNCHER_MOVED: 'launcher-moved',
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
 * The space between the top of the work area and the launcher for the
 * position `top`, in logical pixels.
 */
export const TOP_MARGIN = 24;

const MODES = ['center', 'upper-third', 'top', 'custom'];

/**
 * The setting `window_position` from the state of the action
 * `window-position` (ADR 0009).
 *
 * @param {Array|null} state - the unpacked state `(sdd)`: the name of the
 *   position and the place of `custom`, or null when the program does not
 *   have the action
 * @returns {{mode: string, x: number, y: number}}
 */
export function positionSetting(state) {
    const [mode, x, y] = state ?? [];
    return MODES.includes(mode) ? {mode, x, y} : {mode: 'center', x: 0, y: 0};
}

// The top of the query line below the top of the work area.
function queryLineTop(position, height) {
    switch (position.mode) {
    case 'upper-third':
        return Math.max(0, Math.floor(height / 3 - COLLAPSED_HEIGHT / 2));
    case 'top':
        return Math.min(TOP_MARGIN, Math.max(0, height - COLLAPSED_HEIGHT));
    case 'custom':
        return Math.round(position.y * height);
    default:
        return Math.max(0, Math.floor((height - COLLAPSED_HEIGHT) / 2));
    }
}

/**
 * The top left corner of the launcher frame for the setting
 * `window_position`. The query line has the same place for each height of
 * the window, and the rows go down from it (ADR 0007): the top of the frame
 * is the top of a launcher with the collapsed height at that place. The
 * launcher is at the horizontal center of the work area, or at the place of
 * `custom`. A launcher that goes below the work area moves up, and a
 * launcher that is larger than the work area has its top left corner in
 * the work area.
 *
 * @param {{width: number, height: number}} size - the frame size of the window
 * @param {{x: number, y: number, width: number, height: number}} workArea -
 *   the monitor without the top bar
 * @param {{mode: string, x: number, y: number}} position - the setting, see
 *   `positionSetting`
 * @returns {{x: number, y: number}}
 */
export function launcherPosition(size, workArea, position = {mode: 'center', x: 0, y: 0}) {
    const center = position.mode === 'custom'
        ? Math.round(position.x * workArea.width) : workArea.width / 2;
    const left = Math.floor(center - size.width / 2);
    const rightmost = Math.max(0, workArea.width - size.width);
    const top = workArea.y + queryLineTop(position, workArea.height);
    const lowestTop = workArea.y + Math.max(0, workArea.height - size.height);
    return {
        x: workArea.x + Math.min(Math.max(0, left), rightmost),
        y: Math.min(top, lowestTop),
    };
}

/**
 * The place of a frame as the fractions of `custom`: the horizontal center
 * of the window and the top of its query line in the work area.
 *
 * @param {{x: number, y: number, width: number}} frame - the frame of the window
 * @param {{x: number, y: number, width: number, height: number}} workArea -
 *   the work area of the monitor of the window
 * @returns {{x: number, y: number}}
 */
export function relativePlace(frame, workArea) {
    return {
        x: (frame.x + frame.width / 2 - workArea.x) / workArea.width,
        y: (frame.y - workArea.y) / workArea.height,
    };
}

/**
 * ADR 0009: the state of the action `kept-query-until` tells until when a
 * show of the launcher has the kept query of R9.
 *
 * @param {number|null} state - the wall-clock time in microseconds since
 *   1970, 0 for no kept query, -1 for no time limit, or null when the
 *   program does not have the action
 * @param {number} now - the wall-clock time in microseconds, for example
 *   `GLib.get_real_time()`
 * @returns {boolean} true when the launcher that maps now has the kept query
 */
export function queryKept(state, now) {
    return state === -1 || (typeof state === 'number' && state > now);
}

function contains(area, rect) {
    return rect.x >= area.x && rect.y >= area.y &&
        rect.x + rect.width <= area.x + area.width &&
        rect.y + rect.height <= area.y + area.height;
}

/**
 * The top left corner of the launcher frame at the place where the owner
 * moved it. A launcher that goes below the work area moves up, as in
 * `launcherPosition`.
 *
 * @param {{x: number, y: number}} moved - the top left corner of the frame
 *   after the move of the owner
 * @param {{width: number, height: number}} size - the frame size of the window
 * @param {Array<{x: number, y: number, width: number, height: number}>}
 *   workAreas - the monitors without the top bar
 * @returns {{x: number, y: number}|null} null when the query line at that
 *   place is not fully in one work area, for example after a monitor change
 */
export function movedPosition(moved, size, workAreas) {
    const queryLine = {...moved, width: size.width, height: COLLAPSED_HEIGHT};
    const workArea = workAreas.find(area => contains(area, queryLine));
    if (workArea === undefined)
        return null;
    const lowestTop = workArea.y + Math.max(0, workArea.height - size.height);
    return {x: moved.x, y: Math.min(moved.y, lowestTop)};
}

/**
 * @param {boolean|null} state - the state of the action `tray-icon`, or null
 *   when the program does not have the action
 * @returns {boolean}
 */
export function trayIconVisible(state) {
    return state === true;
}
