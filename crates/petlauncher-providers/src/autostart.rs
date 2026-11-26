//! Start at login (R3 "Run in background"): the XDG autostart file
//! `autostart/pet.repo.Petlauncher.desktop` below the config home.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The path of the autostart file below the config home.
pub const FILE: &str = "autostart/pet.repo.Petlauncher.desktop";

/// Where the autostart file is and what it starts. Give it to
/// `Hooks::autostart`, and the `Launcher` writes or removes the file each
/// time "Run in background" can have changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Autostart {
    /// `$XDG_CONFIG_HOME`, or `~/.config`.
    pub config_home: PathBuf,
    /// The value of the `Exec` key: the program and its arguments, with the
    /// quoting rules of the Desktop Entry Specification.
    pub command: String,
}

impl Autostart {
    /// The config home of the process (`XDG_CONFIG_HOME` when it is an
    /// absolute path, else `HOME/.config`). `None` without an absolute
    /// `HOME`.
    pub fn from_env(command: impl Into<String>) -> Option<Self> {
        let config_home = petlauncher_config::paths::user_config_home(
            std::env::var_os("XDG_CONFIG_HOME").as_deref(),
            std::env::var_os("HOME").as_deref(),
        )?;
        Some(Self {
            config_home,
            command: command.into(),
        })
    }

    pub fn path(&self) -> PathBuf {
        file_path(&self.config_home)
    }

    /// See [`update`].
    pub fn update(&self, enabled: bool) -> io::Result<()> {
        update(&self.config_home, &self.command, enabled)
    }
}

pub fn file_path(config_home: &Path) -> PathBuf {
    config_home.join(FILE)
}

/// Writes the autostart file when `enabled` is true, and removes it when
/// `enabled` is false. The file is written only when its text is different,
/// so a change of the program path takes effect at the next start. A
/// command with a line end is an error, because it would add keys to the
/// file.
pub fn update(config_home: &Path, command: &str, enabled: bool) -> io::Result<()> {
    let path = file_path(config_home);
    if !enabled {
        return match fs::remove_file(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        };
    }
    if command.trim().is_empty() || command.contains(['\n', '\r']) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("the autostart command {command:?} is empty or has a line end"),
        ));
    }
    let text = desktop_file(command);
    if fs::read_to_string(&path).is_ok_and(|old| old == text) {
        return Ok(());
    }
    let dir = path.parent().expect("the file has a directory");
    fs::create_dir_all(dir)?;
    // A rename replaces the file in one step, so the session never reads
    // half a file.
    let partial = dir.join(".pet.repo.Petlauncher.desktop.partial");
    fs::write(&partial, text)?;
    fs::rename(&partial, &path)
}

fn desktop_file(command: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=petlauncher\n\
         Comment=Start petlauncher in the background at login\n\
         Exec={command}\n\
         Terminal=false\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_writes_the_file_with_the_command() {
        let dir = tempfile::tempdir().unwrap();

        update(
            dir.path(),
            "/opt/petlauncher/bin/petlauncher --hidden",
            true,
        )
        .unwrap();

        let text = fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(text.starts_with("[Desktop Entry]\n"), "{text}");
        assert!(text.contains("\nType=Application\n"), "{text}");
        assert!(
            text.contains("\nExec=/opt/petlauncher/bin/petlauncher --hidden\n"),
            "{text}"
        );
    }

    #[test]
    fn disabled_removes_the_file_and_a_missing_file_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        update(dir.path(), "petlauncher", true).unwrap();

        update(dir.path(), "petlauncher", false).unwrap();
        let second_remove = update(dir.path(), "petlauncher", false);

        assert!(!dir.path().join(FILE).exists());
        assert!(second_remove.is_ok());
    }

    #[test]
    fn a_new_command_replaces_the_old_file() {
        let dir = tempfile::tempdir().unwrap();
        update(dir.path(), "/old/petlauncher", true).unwrap();

        update(dir.path(), "/new/petlauncher", true).unwrap();

        let text = fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(text.contains("Exec=/new/petlauncher\n"), "{text}");
        assert!(!text.contains("/old/"), "{text}");
    }

    #[test]
    fn the_same_text_does_not_write_the_file_again() {
        let dir = tempfile::tempdir().unwrap();
        update(dir.path(), "petlauncher", true).unwrap();
        let path = dir.path().join(FILE);
        let old = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000);
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(old)
            .unwrap();

        update(dir.path(), "petlauncher", true).unwrap();

        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), old);
    }

    #[test]
    fn a_command_with_a_line_end_or_no_text_is_an_error() {
        let dir = tempfile::tempdir().unwrap();

        for command in ["petlauncher\nHidden=true", "  "] {
            let error = update(dir.path(), command, true).unwrap_err();

            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{command:?}");
        }
        assert!(!dir.path().join(FILE).exists());
    }

    #[test]
    fn the_autostart_value_gives_the_path_below_the_config_home() {
        let autostart = Autostart {
            config_home: "/home/u/.config".into(),
            command: "petlauncher".into(),
        };

        assert_eq!(
            autostart.path(),
            PathBuf::from("/home/u/.config/autostart/pet.repo.Petlauncher.desktop")
        );
    }
}
