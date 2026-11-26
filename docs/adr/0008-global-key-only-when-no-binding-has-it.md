# petlauncher writes its global key only when no GNOME key binding has it

Status: accepted.

A different shortcut can have the key that petlauncher wants, for example the GNOME default of "Switch to next input source" is Super+Space. At its first start petlauncher writes the custom shortcut "Show petlauncher" only when no key binding of GNOME Settings has the key. With a conflict it writes nothing and shows the bindings and their places in GNOME Settings. Only an explicit action of the owner removes a key from a different binding. Reason: petlauncher must never change the shortcut of a different program by itself, and it has no knowledge of specific programs.

## Considered Options

- Take the key from the other shortcut at the first start: this changes the settings of a different program with no action of the owner.

## Consequences

- On a GNOME installation with the default keys, the first start shows the conflict notice and sets no key.
- A marker file in the state directory records that the first installation is done, so petlauncher does not set the key again after the owner removes it.
