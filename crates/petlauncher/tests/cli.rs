//! The commands that run with no display and no session bus.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn petlauncher(home: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_petlauncher"));
    command
        .args(args)
        .env_clear()
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"));
    // `cargo llvm-cov` records the coverage of the binary through this
    // variable.
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    command.output().unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn version_prints_the_package_version() {
    let home = tempfile::tempdir().unwrap();

    let output = petlauncher(home.path(), &["--version"]);

    assert!(output.status.success());
    assert_eq!(
        text(&output.stdout),
        format!("petlauncher {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn help_prints_the_commands() {
    let home = tempfile::tempdir().unwrap();

    let output = petlauncher(home.path(), &["--help"]);

    assert!(output.status.success());
    let usage = text(&output.stdout);
    for command in ["--hidden", "toggle", "convert-shortcuts [--force]"] {
        assert!(usage.contains(command), "{usage}");
    }
}

#[test]
fn an_unknown_argument_stops_with_status_2_and_the_usage() {
    let home = tempfile::tempdir().unwrap();

    let output = petlauncher(home.path(), &["show"]);

    assert_eq!(output.status.code(), Some(2));
    let error = text(&output.stderr);
    assert!(error.starts_with("Unknown argument: show"), "{error}");
    assert!(error.contains("Usage:"), "{error}");
}

#[test]
fn convert_shortcuts_writes_shortcuts_toml_and_prints_the_summary() {
    let home = tempfile::tempdir().unwrap();
    let ulauncher = home.path().join(".config/ulauncher");
    fs::create_dir_all(&ulauncher).unwrap();
    fs::write(
        ulauncher.join("shortcuts.json"),
        r#"{"a": {"name": "Cambridge", "keyword": "d", "cmd": "https://dictionary.example/%s",
                  "icon": null, "is_default_search": false, "run_without_argument": false}}"#,
    )
    .unwrap();
    let target = home.path().join(".config/petlauncher/shortcuts.toml");

    let first = petlauncher(home.path(), &["convert-shortcuts"]);
    let second = petlauncher(home.path(), &["convert-shortcuts"]);
    let forced = petlauncher(home.path(), &["convert-shortcuts", "--force"]);

    assert!(first.status.success(), "{}", text(&first.stderr));
    let summary = text(&first.stdout);
    assert!(summary.starts_with("Wrote 1 shortcuts to "), "{summary}");
    assert!(summary.contains("keyword \"dd\""), "{summary}");
    assert!(fs::read_to_string(&target)
        .unwrap()
        .contains("keyword = \"dd\""));
    assert_eq!(second.status.code(), Some(1));
    assert!(text(&second.stderr).contains("exists"));
    assert!(forced.status.success());
}

#[test]
fn convert_shortcuts_reports_a_missing_ulauncher_file() {
    let home = tempfile::tempdir().unwrap();

    let output = petlauncher(home.path(), &["convert-shortcuts"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(text(&output.stderr).contains("ulauncher/shortcuts.json"));
}
