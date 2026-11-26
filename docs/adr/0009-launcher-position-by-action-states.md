# The extension places the launcher by action states of the program

Status: accepted. Extends ADR 0005 with three actions, and ADR 0007 with the setting `window_position` and a launcher that the owner moved.

The owner can move the launcher with the mouse, select its place in the setting `window_position`, and pin a moved place. On Wayland only the extension knows and sets the place of the window, and only the program knows the settings and whether a show has the kept query of R9. The program gives the extension its decisions as the states of disabled actions, and the extension reads them at `map` from the cache of its action group with no wait. The extension tells the program about a move with a call of an action. Reason: no D-Bus interface of its own (ADR 0005), and each state changes before the next show starts, so the state is correct at the map.

## Decision

- `window-position`, state `(sdd)`: the name of `window_position` and the fractions of `custom_position`. The program sets it after each change of `settings.toml`.
- `kept-query-until`, state `x`: the wall-clock time in microseconds since 1970 until which a show has the kept query, `0` for no kept query, `-1` for no time limit. The program sets it after each hide and each change of the settings, never at a show. The extension compares it with the wall clock, which R9 also uses. The value of `i64::MAX` gives a warning in gjs at each read.
- `launcher-moved`, parameter `(dd)`: at the signal `grab-op-end` of a launcher that is not at the place that the extension gave it, the extension remembers the place and calls this action with the place as fractions of the work area of its monitor. The launcher cannot change its size with the mouse, so this is a move by the owner. The program shows the pin button, and the pin writes `custom_position` and `window_position = "custom"`.
- At `map`, a show with the kept query and a remembered place whose query line is fully in a work area opens at that place. Else the extension forgets the place and uses `launcherPosition` with the setting on the monitor with the pointer.
- A custom place is two fractions of the work area: the horizontal center of the window and the top of the query line. Thus it fits each monitor. The rule of ADR 0007 stays: the query line has the same place for each height, and a launcher that goes below the work area moves up.

## Considered Options

- A boolean "kept query" state that the program sets at the show: GDBus sends the change from an idle, and the map can come first.
- A boolean state that the program sets at the hide and clears with a timer at the end of the keep time: the timer uses the monotonic clock, which stops in the suspend state, but R9 uses the wall clock.
- The extension opens each show at the moved place: a new query must open at the place of the setting, on the monitor with the pointer.
- A custom place in pixels: it does not fit a monitor of a different size or scale.
- The program asks the extension for the place when the owner selects the pin: two more messages, but the extension knows the place at the end of the move already.

## Consequences

- A show within some milliseconds after a hide can read the state of the hide before it. The nested shell measured 0.9 ms to 5.0 ms from the change in the program to the change in GNOME Shell.
- A show just before the end of the keep time can map just after it. The launcher then shows the kept query at the place of the setting.
- Without the extension, the program works, but `window_position` has no effect, and the pin button never shows.
