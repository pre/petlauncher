//! Fixture directories below `TMPDIR` for the integration tests.

#![allow(dead_code)]

use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use petlauncher_apps::Environment;
use tempfile::TempDir;

/// A temporary root with a user data directory, a system data directory and
/// a `bin` directory for the programs of `Exec` and `TryExec`.
pub struct Fixture {
    _dir: TempDir,
    pub root: PathBuf,
}

impl Fixture {
    pub fn new() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        fs::create_dir_all(root.join("bin")).unwrap();
        Fixture { _dir: dir, root }
    }

    /// The first data directory, as `XDG_DATA_HOME`.
    pub fn user(&self) -> PathBuf {
        self.root.join("home/.local/share")
    }

    /// The second data directory, as `/usr/share`.
    pub fn system(&self) -> PathBuf {
        self.root.join("usr/share")
    }

    pub fn bin(&self) -> PathBuf {
        self.root.join("bin")
    }

    /// Writes `data_dir/applications/relative`.
    pub fn write(&self, data_dir: &Path, relative: &str, text: &str) -> PathBuf {
        let path = data_dir.join("applications").join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path
    }

    /// Makes an executable file `bin/name`.
    pub fn program(&self, name: &str) -> PathBuf {
        let path = self.bin().join(name);
        fs::write(&path, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    /// Sets the modification time of each file and directory to 1970-01-12.
    /// The clock of the file system is coarse, so a change soon after a
    /// load can have the same time as the load. After this call, it cannot.
    pub fn make_old(&self) {
        fn set_old(path: &Path) {
            let old = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
            if path.is_dir() {
                for item in fs::read_dir(path).unwrap() {
                    set_old(&item.unwrap().path());
                }
            }
            File::open(path).unwrap().set_modified(old).unwrap();
        }
        set_old(&self.root);
    }

    /// The user and system data directories, the desktop `ubuntu:GNOME`,
    /// no locale, and `bin` as `PATH`.
    pub fn env(&self) -> Environment {
        Environment {
            data_dirs: vec![self.user(), self.system()],
            current_desktops: vec!["ubuntu".into(), "GNOME".into()],
            locale: None,
            path: vec![self.bin()],
        }
    }
}

/// The text of an application entry. `extra` holds more `Key=Value` lines.
pub fn app(name: &str, exec: &str, extra: &str) -> String {
    format!("[Desktop Entry]\nType=Application\nName={name}\nExec={exec}\n{extra}")
}
