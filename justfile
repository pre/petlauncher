# Shows the recipes.
default:
    @just --list

# Builds the release program in target/release/petlauncher.
build:
    cargo build -p petlauncher --release

# Runs the Rust tests and the unit test of the GNOME Shell extension.
test:
    cargo test --workspace
    gjs -m gnome/extension/test/test-launcher.js

# Checks the format and the lints.
lint:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings

# Builds and installs petlauncher for the current user. ARGS go to scripts/install.sh, for example --prefix DIR.
install *ARGS:
    scripts/install.sh {{ ARGS }}

# Installs as `install` does, then stops the running petlauncher and starts the new program with no window. ARGS go to both scripts, for example --prefix DIR.
install-replace *ARGS:
    scripts/install.sh {{ ARGS }}
    scripts/restart.sh {{ ARGS }}

# Removes the installed files. ARGS go to scripts/uninstall.sh.
uninstall *ARGS:
    scripts/uninstall.sh {{ ARGS }}

# Makes a draft GitHub release at HEAD. ARGS go to scripts/release-github, for example v0.1.0 or patch.
release *ARGS:
    scripts/release-github {{ ARGS }}
