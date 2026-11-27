# Release

```sh
just release v1.2.3    # or major, minor or patch
```

The script `scripts/release-github` makes a draft GitHub release at HEAD, with notes from the conventional commits since the previous `v*` tag. Then it starts `.github/workflows/release.yml`. The workflow runs the unit test of the extension, writes the version to `version-name` in `metadata.json`, packs `petlauncher@repo.pet.zip` and attaches the zip to the draft. When the build is done, the script opens the draft. Examine the draft and press **Publish release**. Publishing creates the git tag.

`just release v1.2.3 --dry-run` shows the notes and makes no change on GitHub.

Requirements:

- The command `gh`, logged in to an account that can write to `pre/petlauncher`.
- HEAD is on `origin/main`. The draft points to HEAD, so push first.
- The version has the form `vX.Y.Z`. extensions.gnome.org accepts only letters, digits, spaces and periods in `version-name`, and the workflow removes the `v`.
- `version` in `Cargo.toml` has the same version. `petlauncher --version` shows that value, and the script does not change it.

The zip contains only the GNOME Shell extension, for an upload to https://extensions.gnome.org/upload/

The program is not in the zip. Users install the program and the extension from the source code of the release with `just install`, see [install.md](install.md).
