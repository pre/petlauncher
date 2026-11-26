//! The input of the loader that comes from the environment of the process.

use std::path::{Path, PathBuf};

/// The values that decide which entries the loader finds. The program uses
/// `from_process`. A test gives its own values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment {
    /// The data directories, first `XDG_DATA_HOME`, then `XDG_DATA_DIRS`.
    /// The loader reads the `applications` directory of each. An entry in
    /// an earlier directory hides the entry with the same ID in a later one.
    pub data_dirs: Vec<PathBuf>,
    /// The names of `XDG_CURRENT_DESKTOP`, for example `ubuntu` and `GNOME`.
    pub current_desktops: Vec<String>,
    /// The locale of the localized keys, for example `fi_FI.UTF-8`.
    pub locale: Option<String>,
    /// The directories of `PATH`, for `TryExec` and the program of `Exec`.
    pub path: Vec<PathBuf>,
}

impl Environment {
    /// Reads the variables through `var`. `home` replaces `$HOME`.
    pub fn from_vars(home: &Path, var: impl Fn(&str) -> Option<String>) -> Environment {
        let var = |name: &str| var(name).filter(|value| !value.is_empty());

        let data_home = var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| home.join(".local/share"));
        let system_dirs =
            var("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
        let mut data_dirs = vec![data_home];
        for dir in system_dirs.split(':').map(PathBuf::from) {
            if dir.is_absolute() && !data_dirs.contains(&dir) {
                data_dirs.push(dir);
            }
        }

        Environment {
            data_dirs,
            current_desktops: var("XDG_CURRENT_DESKTOP")
                .map(|value| value.split(':').map(String::from).collect())
                .unwrap_or_default(),
            locale: var("LC_ALL")
                .or_else(|| var("LC_MESSAGES"))
                .or_else(|| var("LANG")),
            path: var("PATH")
                .map(|value| {
                    value
                        .split(':')
                        .filter(|dir| !dir.is_empty())
                        .map(PathBuf::from)
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    /// Reads the variables of this process.
    pub fn from_process() -> Environment {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        Environment::from_vars(&home, |name| std::env::var(name).ok())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn environment(vars: &[(&str, &str)]) -> Environment {
        let vars: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Environment::from_vars(Path::new("/home/u"), |name| vars.get(name).cloned())
    }

    fn paths(values: &[&str]) -> Vec<PathBuf> {
        values.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn uses_the_default_data_directories_when_the_variables_are_missing() {
        let env = environment(&[]);

        assert_eq!(
            env.data_dirs,
            paths(&["/home/u/.local/share", "/usr/local/share", "/usr/share"])
        );
    }

    #[test]
    fn uses_the_default_data_directories_when_the_variables_are_empty() {
        let env = environment(&[("XDG_DATA_HOME", ""), ("XDG_DATA_DIRS", "")]);

        assert_eq!(
            env.data_dirs,
            paths(&["/home/u/.local/share", "/usr/local/share", "/usr/share"])
        );
    }

    #[test]
    fn puts_the_data_home_before_the_data_directories_in_their_order() {
        let env = environment(&[
            ("XDG_DATA_HOME", "/data/home"),
            (
                "XDG_DATA_DIRS",
                "/usr/share/ubuntu:/home/u/.local/share/flatpak/exports/share:/usr/share/:/var/lib/snapd/desktop",
            ),
        ]);

        assert_eq!(
            env.data_dirs,
            paths(&[
                "/data/home",
                "/usr/share/ubuntu",
                "/home/u/.local/share/flatpak/exports/share",
                "/usr/share",
                "/var/lib/snapd/desktop",
            ])
        );
    }

    #[test]
    fn ignores_relative_and_repeated_data_directories() {
        let env = environment(&[
            ("XDG_DATA_HOME", "relative/home"),
            ("XDG_DATA_DIRS", "/usr/share/:share::/usr/share"),
        ]);

        assert_eq!(
            env.data_dirs,
            paths(&["/home/u/.local/share", "/usr/share"])
        );
    }

    #[test]
    fn splits_the_current_desktop_names() {
        let env = environment(&[("XDG_CURRENT_DESKTOP", "ubuntu:GNOME")]);

        assert_eq!(env.current_desktops, ["ubuntu", "GNOME"]);
        assert!(environment(&[]).current_desktops.is_empty());
    }

    #[test]
    fn takes_the_locale_from_lc_all_then_lc_messages_then_lang() {
        let all = environment(&[
            ("LC_ALL", "fi_FI.UTF-8"),
            ("LC_MESSAGES", "sv_SE"),
            ("LANG", "en_US"),
        ]);
        let messages = environment(&[("LC_ALL", ""), ("LC_MESSAGES", "sv_SE"), ("LANG", "en_US")]);
        let lang = environment(&[("LANG", "en_US.UTF-8")]);

        assert_eq!(all.locale.as_deref(), Some("fi_FI.UTF-8"));
        assert_eq!(messages.locale.as_deref(), Some("sv_SE"));
        assert_eq!(lang.locale.as_deref(), Some("en_US.UTF-8"));
        assert_eq!(environment(&[]).locale, None);
    }

    #[test]
    fn splits_path_and_ignores_empty_parts() {
        let env = environment(&[("PATH", "/usr/local/bin::/usr/bin:")]);

        assert_eq!(env.path, paths(&["/usr/local/bin", "/usr/bin"]));
    }
}
