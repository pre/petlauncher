# The extension places the launcher by a fixed collapsed height

Status: accepted. Changes ADR 0005 for the position of the launcher. ADR 0009 extends it for a launcher that the owner moved.

The query line must have the same place on the monitor for each height of the launcher: a launcher that opens with a kept query and rows must not have its query line higher than an empty launcher. The extension puts the top of the launcher where a launcher of 64 pixels is at the center of the work area, and the program makes the launcher 64 pixels high when it has no rows. The value is `COLLAPSED_HEIGHT` in `gnome/extension/petlauncher@repo.pet/launcher.js` and in `crates/petlauncher/src/window.rs`.

## Considered Options

- Center the full height: a launcher with a kept query has its query line higher.
- Show with no rows and add the rows after the window shows: the rows come one or two frames late.
- The program gives its height to the extension through an action state: more contract and code on the two sides for a value that changes only with the design.

## Consequences

- A change of the height of the query line needs a change in the two files.
- With large text the empty launcher can be higher than 64 pixels. It is then a few pixels below the center, but the query line keeps its place.
