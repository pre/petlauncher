use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};

/// One of the two files in the settings directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigFile {
    Settings,
    Shortcuts,
}

impl ConfigFile {
    pub const ALL: [Self; 2] = [Self::Settings, Self::Shortcuts];

    pub fn file_name(self) -> &'static str {
        match self {
            Self::Settings => "settings.toml",
            Self::Shortcuts => "shortcuts.toml",
        }
    }

    /// The path of this file in the settings directory `dir`.
    pub fn path_in(self, dir: &Path) -> PathBuf {
        dir.join(self.file_name())
    }
}

impl fmt::Display for ConfigFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.file_name())
    }
}

/// The settings directory `~/.config/petlauncher`. A set
/// `XDG_CONFIG_HOME` replaces `~/.config`. `None` when neither
/// `XDG_CONFIG_HOME` nor `HOME` gives an absolute path.
pub fn config_dir() -> Option<PathBuf> {
    user_config_home(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
    .map(|base| base.join("petlauncher"))
}

/// The base directory for user settings by the XDG Base Directory rules:
/// `XDG_CONFIG_HOME` when it is an absolute path, else `HOME/.config`.
pub fn user_config_home(xdg_config_home: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    let absolute =
        |value: Option<&OsStr>| value.map(PathBuf::from).filter(|path| path.is_absolute());
    absolute(xdg_config_home).or_else(|| absolute(home).map(|home| home.join(".config")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_xdg_config_home_when_it_is_absolute() {
        let base = user_config_home(Some(OsStr::new("/x/conf")), Some(OsStr::new("/home/u")));

        assert_eq!(base, Some(PathBuf::from("/x/conf")));
    }

    #[test]
    fn uses_home_dot_config_when_xdg_config_home_is_missing_empty_or_relative() {
        for xdg in [None, Some(""), Some("relative/conf")] {
            let base = user_config_home(xdg.map(OsStr::new), Some(OsStr::new("/home/u")));

            assert_eq!(base, Some(PathBuf::from("/home/u/.config")), "{xdg:?}");
        }
    }

    #[test]
    fn gives_no_directory_without_an_absolute_home() {
        assert_eq!(user_config_home(None, None), None);
        assert_eq!(user_config_home(None, Some(OsStr::new("home"))), None);
    }

    #[test]
    fn gives_the_file_path_in_a_directory() {
        let dir = Path::new("/c/petlauncher");

        assert_eq!(
            ConfigFile::Settings.path_in(dir),
            PathBuf::from("/c/petlauncher/settings.toml")
        );
        assert_eq!(ConfigFile::Shortcuts.to_string(), "shortcuts.toml");
    }

    #[test]
    fn config_dir_ends_in_petlauncher() {
        if let Some(dir) = config_dir() {
            assert!(dir.ends_with("petlauncher"));
        }
    }
}
