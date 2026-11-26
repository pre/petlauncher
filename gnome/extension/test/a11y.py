"""Reads the widgets of a window of the test shell through AT-SPI.

The steps use it to find a widget by its role and name, and then click it or
type into it with the virtual pointer and keyboard of the shell, which is the
real input. Run it with the system Python, which has the Atspi typelib:

    /usr/bin/python3 -I a11y.py tree TITLE
    /usr/bin/python3 -I a11y.py find TITLE ROLE NAME [N]
    /usr/bin/python3 -I a11y.py text TITLE ROLE NAME [N]
    /usr/bin/python3 -I a11y.py states TITLE ROLE NAME [N]
    /usr/bin/python3 -I a11y.py tab TITLE NAME

TITLE is the title of the window. ROLE is the AT-SPI role name, for example
"push button" or "toggle button". NAME is the accessible name; a NAME that
starts with "~" is a regular expression. N selects the Nth match, from 0.
Only widgets that show count.

find prints "x y width height" of the widget, relative to the window. The
caller adds the place of the window. The exit status is 1 when nothing
matches.

tab prints the box of a page tab of the view switcher in the header bar.
GTK 4.14 gives the tabs an x that is not centered, so tab centers the row of
tabs in the window, as the screenshots show it.
"""

import re
import sys

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi  # noqa: E402


def windows(title):
    desktop = Atspi.get_desktop(0)
    for i in range(desktop.get_child_count()):
        app = desktop.get_child_at_index(i)
        if app is None:
            continue
        for j in range(app.get_child_count()):
            window = app.get_child_at_index(j)
            if window is not None and window.get_name() == title:
                yield window


def showing(node):
    states = node.get_state_set()
    return states.contains(Atspi.StateType.SHOWING) and states.contains(
        Atspi.StateType.VISIBLE
    )


def walk(node, depth=0):
    yield node, depth
    for i in range(node.get_child_count()):
        child = node.get_child_at_index(i)
        if child is not None and showing(child):
            yield from walk(child, depth + 1)


def extents(node):
    try:
        box = node.get_extents(Atspi.CoordType.WINDOW)
        return box.x, box.y, box.width, box.height
    except Exception:  # noqa: BLE001 - a node without the Component interface
        return None


def text(node):
    try:
        length = Atspi.Text.get_character_count(node)
        return Atspi.Text.get_text(node, 0, length)
    except Exception:  # noqa: BLE001 - a node without the Text interface
        return None


def state_names(node):
    names = []
    states = node.get_state_set()
    for state in (
        Atspi.StateType.CHECKED,
        Atspi.StateType.PRESSED,
        Atspi.StateType.SENSITIVE,
        Atspi.StateType.FOCUSED,
        Atspi.StateType.EDITABLE,
        Atspi.StateType.SELECTED,
    ):
        if states.contains(state):
            names.append(state.value_nick)
    return names


def matches(node, role, name):
    if node.get_role_name() != role:
        return False
    actual = node.get_name() or ""
    if name.startswith("~"):
        return re.search(name[1:], actual) is not None
    return actual == name


def found(title, role, name, index):
    hits = []
    for window in windows(title):
        for node, _ in walk(window):
            if matches(node, role, name):
                hits.append(node)
    if index < len(hits):
        return hits[index]
    sys.stderr.write(f"no {role} {name!r} number {index} in {title!r}\n")
    sys.exit(1)


def tab(title, name):
    for window in windows(title):
        tabs = [node for node, _ in walk(window) if node.get_role_name() == "page tab"]
        boxes = {node.get_name(): extents(node) for node in tabs}
        if name not in boxes:
            break
        left = min(box[0] for box in boxes.values())
        right = max(box[0] + box[2] for box in boxes.values())
        width = extents(window)[2]
        shift = (width - (right - left)) // 2 - left
        x, y, w, h = boxes[name]
        return x + shift, y, w, h
    sys.stderr.write(f"no page tab {name!r} in {title!r}\n")
    sys.exit(1)


def main(argv):
    command, title = argv[1], argv[2]
    if command == "tab":
        print("{} {} {} {}".format(*tab(title, argv[3])))
        return
    if command == "tree":
        for window in windows(title):
            for node, depth in walk(window):
                box = extents(node)
                line = f"{'  ' * depth}{node.get_role_name()} {node.get_name()!r}"
                if box:
                    line += " at {} {} {} {}".format(*box)
                states = state_names(node)
                if states:
                    line += f" [{' '.join(states)}]"
                value = text(node)
                if value:
                    line += f" text={value!r}"
                print(line)
        return
    role, name = argv[3], argv[4]
    index = int(argv[5]) if len(argv) > 5 else 0
    node = found(title, role, name, index)
    if command == "find":
        print("{} {} {} {}".format(*extents(node)))
    elif command == "text":
        print(text(node) or "")
    elif command == "states":
        print(" ".join(state_names(node)))
    else:
        sys.stderr.write(f"unknown command {command}\n")
        sys.exit(2)


if __name__ == "__main__":
    main(sys.argv)
