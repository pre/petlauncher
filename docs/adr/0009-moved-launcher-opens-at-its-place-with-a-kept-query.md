# A moved launcher opens at its place while the query is kept

Status: accepted. Extends ADR 0005 with the action state `kept-query-until`, and ADR 0007 for a launcher that the owner moved.

The owner can move the launcher with the mouse. A show with the kept query of R9 must open at that place, and a show with no kept query at the place of ADR 0007. Only the extension knows and sets the place, and only the program knows whether a show has the kept query. The program gives the extension the wall-clock time until which a show has the kept query, as the state of a disabled action, and it changes the state after each hide and each change of the settings, never at a show. At `map` the extension compares the state with the wall clock, which R9 also uses. Reason: the extension reads the state from the cache of its action group with no wait, and the state is correct before the next show starts.

## Decision

- `kept-query-until`, type `x`: the time in microseconds since 1970, `0` for no kept query, `-1` for no time limit. The value of `i64::MAX` gives a warning in gjs at each read.
- The extension remembers the frame place at the signal `grab-op-end` of a launcher that is not at the place that the extension gave it. The launcher cannot change its size with the mouse, so this is a move by the owner.
- At `map` with the kept query and a remembered place whose query line is fully in a work area, the launcher opens at that place (`movedPosition`). Else the extension forgets the place and uses `launcherPosition` on the monitor with the pointer.

## Considered Options

- A boolean state that the program sets at the show: GDBus sends the change from an idle, and the map can come first.
- A boolean state that the program sets at the hide and clears with a timer at the end of the keep time: the timer uses the monotonic clock, which stops in the suspend state, but R9 uses the wall clock.
- The extension opens each show at the moved place: a new query must open at the place of R11, on the monitor with the pointer.

## Consequences

- A show within some milliseconds after a hide can read the state of the hide before it. The nested shell measured 0.9 ms to 5.0 ms from the change in the program to the change in GNOME Shell.
- A show just before the end of the keep time can map just after it. The launcher then shows the kept query at the place of R11.
- Without the extension, the program works, and each show opens where GNOME puts a new window.
