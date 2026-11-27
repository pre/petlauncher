// Unit test of launcher.js. Run: gjs -m gnome/extension/test/test-launcher.js
import System from 'system';

import {
    APP_ID,
    COLLAPSED_HEIGHT,
    LAUNCHER_TITLE,
    launcherPosition,
    isLauncher,
    movedPosition,
    positionSetting,
    queryKept,
    relativePlace,
    TOP_MARGIN,
    trayIconVisible,
} from '../petlauncher@repo.pet/launcher.js';

const failures = [];

function test(name, body) {
    try {
        body();
        print(`ok     ${name}`);
    } catch (e) {
        failures.push(name);
        print(`FAILED ${name}: ${e.message}`);
    }
}

function assert(condition, message) {
    if (!condition)
        throw new Error(message);
}

function assertEqual(actual, expected) {
    const a = JSON.stringify(actual);
    const e = JSON.stringify(expected);
    if (a !== e)
        throw new Error(`expected ${e}, got ${a}`);
}

test('centers the collapsed launcher in the work area of a large monitor', () => {
    const size = {width: 750, height: COLLAPSED_HEIGHT};
    const workArea = {x: 0, y: 32, width: 3008, height: 1660};

    const position = launcherPosition(size, workArea);

    assert(COLLAPSED_HEIGHT === 64, `COLLAPSED_HEIGHT is ${COLLAPSED_HEIGHT}`);
    assertEqual(position, {x: 1129, y: 830});
});

test('centers the collapsed launcher in a monitor that is right of a different monitor', () => {
    const size = {width: 750, height: COLLAPSED_HEIGHT};
    const workArea = {x: 3008, y: 234, width: 1920, height: 1200};

    const position = launcherPosition(size, workArea);

    assertEqual(position, {x: 3593, y: 802});
});

test('puts a launcher with rows at the place of the collapsed launcher', () => {
    const workArea = {x: 0, y: 32, width: 3008, height: 1660};
    const collapsed = launcherPosition({width: 750, height: COLLAPSED_HEIGHT}, workArea);

    const tall = launcherPosition({width: 750, height: 520}, workArea);

    assertEqual(tall, collapsed);
});

test('moves a tall launcher up when its bottom is below the work area', () => {
    const size = {width: 750, height: 700};
    const workArea = {x: 0, y: 32, width: 1280, height: 1000};

    const position = launcherPosition(size, workArea);

    assertEqual(position, {x: 265, y: 332});
});

test('rounds a half pixel down', () => {
    const size = {width: 751, height: COLLAPSED_HEIGHT};
    const workArea = {x: 0, y: 0, width: 1920, height: 1201};

    const position = launcherPosition(size, workArea);

    assertEqual(position, {x: 584, y: 568});
});

test('keeps the top left corner in the work area when the window is larger', () => {
    const size = {width: 2000, height: 1300};
    const workArea = {x: 3008, y: 234, width: 1920, height: 1200};

    const position = launcherPosition(size, workArea);

    assertEqual(position, {x: 3008, y: 234});
});

test('accepts the window with the application ID and the launcher title', () => {
    const accepted = isLauncher(APP_ID, LAUNCHER_TITLE);

    assertEqual(accepted, true);
});

test('refuses a different window of petlauncher', () => {
    const settings = isLauncher(APP_ID, 'petlauncher Settings');
    const dictionary = isLauncher(APP_ID, 'Dictionary');

    assertEqual([settings, dictionary], [false, false]);
});

test('refuses a window of a different application with the launcher title', () => {
    const accepted = isLauncher('org.example.Other', LAUNCHER_TITLE);

    assertEqual(accepted, false);
});

test('refuses a window that has no application ID or title yet', () => {
    const noAppId = isLauncher(null, LAUNCHER_TITLE);
    const noTitle = isLauncher(APP_ID, null);

    assertEqual([noAppId, noTitle], [false, false]);
});

test('shows the tray icon only when the action state is true', () => {
    const states = [true, false, null];

    const visible = states.map(state => trayIconVisible(state));

    assertEqual(visible, [true, false, false]);
});

const WORK_AREAS = [
    {x: 0, y: 32, width: 3008, height: 1660},
    {x: 3008, y: 234, width: 1920, height: 1200},
];

test('a query is kept until the time of the state, or with no limit for -1', () => {
    const now = 1795722113580300;
    const states = [now + 1, now, now - 1, -1, 0, null];

    const kept = states.map(state => queryKept(state, now));

    assertEqual(kept, [true, false, false, true, false, false]);
});

test('opens a moved launcher at its place on each monitor', () => {
    const size = {width: 750, height: 300};

    const first = movedPosition({x: 100, y: 200}, size, WORK_AREAS);
    const second = movedPosition({x: 3100, y: 300}, size, WORK_AREAS);

    assertEqual([first, second], [{x: 100, y: 200}, {x: 3100, y: 300}]);
});

test('forgets a moved place whose query line is not in a work area', () => {
    const size = {width: 750, height: COLLAPSED_HEIGHT};

    const goneMonitor = movedPosition({x: 5000, y: 300}, size, WORK_AREAS);
    const onTwoMonitors = movedPosition({x: 2500, y: 300}, size, WORK_AREAS);
    const underTopBar = movedPosition({x: 100, y: 10}, size, WORK_AREAS);

    assertEqual([goneMonitor, onTwoMonitors, underTopBar], [null, null, null]);
});

test('moves a moved launcher up when its rows go below the work area', () => {
    const size = {width: 750, height: 500};

    const position = movedPosition({x: 100, y: 1500}, size, WORK_AREAS);

    assertEqual(position, {x: 100, y: 1192});
});

test('puts the query line at one third of the work area for upper-third', () => {
    const size = {width: 750, height: 300};

    const position = launcherPosition(size, WORK_AREAS[0], {mode: 'upper-third'});

    assertEqual(position, {x: 1129, y: 553});
});

test('puts the query line at one quarter of the work area for upper-quarter', () => {
    const size = {width: 750, height: COLLAPSED_HEIGHT};

    const first = launcherPosition(size, WORK_AREAS[0], {mode: 'upper-quarter'});
    const second = launcherPosition(size, WORK_AREAS[1], {mode: 'upper-quarter'});

    assertEqual([first, second], [{x: 1129, y: 415}, {x: 3593, y: 502}]);
});

test('puts the launcher below the top bar with a margin for top', () => {
    const size = {width: 750, height: COLLAPSED_HEIGHT};

    const first = launcherPosition(size, WORK_AREAS[0], {mode: 'top'});
    const second = launcherPosition(size, WORK_AREAS[1], {mode: 'top'});

    assert(TOP_MARGIN === 24, `TOP_MARGIN is ${TOP_MARGIN}`);
    assertEqual([first, second], [{x: 1129, y: 56}, {x: 3593, y: 258}]);
});

test('puts a custom place at the same fractions of each work area', () => {
    const size = {width: 750, height: COLLAPSED_HEIGHT};
    const custom = {mode: 'custom', x: 0.25, y: 0.1};

    const first = launcherPosition(size, WORK_AREAS[0], custom);
    const second = launcherPosition(size, WORK_AREAS[1], custom);

    assertEqual([first, second], [{x: 377, y: 198}, {x: 3113, y: 354}]);
});

test('keeps a custom place in the work area', () => {
    const custom = {mode: 'custom', x: 1, y: 0.95};

    const collapsed = launcherPosition({width: 750, height: COLLAPSED_HEIGHT}, WORK_AREAS[0], custom);
    const tall = launcherPosition({width: 750, height: 500}, WORK_AREAS[0], custom);

    assertEqual([collapsed, tall], [{x: 2258, y: 1609}, {x: 2258, y: 1192}]);
});

test('gives the fractions of a frame that open the launcher at the same frame again', () => {
    const frame = {x: 377, y: 198, width: 750, height: 300};

    const place = relativePlace(frame, WORK_AREAS[0]);
    const position = launcherPosition(frame, WORK_AREAS[0], {mode: 'custom', ...place});

    assertEqual(place, {x: 0.25, y: 0.1});
    assertEqual(position, {x: 377, y: 198});
});

test('reads the position setting from the action state, with center as the default', () => {
    const states = [['top', 0, 0], ['upper-quarter', 0, 0], ['custom', 0.25, 0.1],
        ['sideways', 0, 0], null];

    const settings = states.map(state => positionSetting(state));

    assertEqual(settings, [
        {mode: 'top', x: 0, y: 0},
        {mode: 'upper-quarter', x: 0, y: 0},
        {mode: 'custom', x: 0.25, y: 0.1},
        {mode: 'center', x: 0, y: 0},
        {mode: 'center', x: 0, y: 0},
    ]);
});

if (failures.length > 0) {
    print(`${failures.length} test(s) failed`);
    System.exit(1);
}
print('all tests passed');
