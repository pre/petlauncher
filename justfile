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

# Removes the installed files. ARGS go to scripts/uninstall.sh.
uninstall *ARGS:
    scripts/uninstall.sh {{ ARGS }}
