// Unit test of launcher.js. Run: gjs -m gnome/extension/test/test-launcher.js
import System from 'system';

import {
    APP_ID,
    COLLAPSED_HEIGHT,
    LAUNCHER_TITLE,
    launcherPosition,
    isLauncher,
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

if (failures.length > 0) {
    print(`${failures.length} test(s) failed`);
    System.exit(1);
}
print('all tests passed');
