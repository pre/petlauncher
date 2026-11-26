//! The list of entries in the search: load, change check, search, browsers.

use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::entry::{DesktopEntry, EntryKind};
use crate::environment::Environment;
use crate::locale::Locale;
use crate::search::{self, Query, SearchKey, Tier};
use crate::usage::UsageCounts;

/// Decides if an entry is a settings page. `gnome::is_settings_page` is the
/// rule for GNOME.
pub type SettingsPageRule = fn(&DesktopEntry) -> bool;

/// A `.desktop` file or directory that the loader could not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
    pub path: PathBuf,
    pub message: String,
}

/// One search result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit<'a> {
    pub entry: &'a DesktopEntry,
    /// The match points plus the usage points. A higher score is a better
    /// result.
    pub score: u32,
}

/// The applications and settings pages that the search shows.
#[derive(Debug)]
pub struct Applications {
    env: Environment,
    settings_page: SettingsPageRule,
    entries: Vec<DesktopEntry>,
    keys: Vec<SearchKey>,
    errors: Vec<LoadError>,
    /// The modification time of each scanned directory and file at the
    /// load. `None` for a directory that did not exist.
    times: Vec<(PathBuf, Option<SystemTime>)>,
}

impl Applications {
    /// Reads the `applications` directory of each data directory.
    pub fn load(env: Environment, settings_page: SettingsPageRule) -> Applications {
        let mut scan = Scan::default();
        for dir in &env.data_dirs {
            let root = dir.join("applications");
            scan.directory(&root, &root);
        }

        let locale = env.locale.as_deref().and_then(Locale::parse);
        let mut entries = Vec::new();
        for found in scan.found {
            let parsed = fs::read(&found.path)
                .map_err(|error| error.to_string())
                .and_then(|bytes| {
                    let text = String::from_utf8_lossy(&bytes);
                    DesktopEntry::parse(&found.id, &found.path, &text, locale.as_ref())
                });
            match parsed {
                Ok(Some(mut entry)) => {
                    if settings_page(&entry) {
                        entry.kind = EntryKind::SettingsPage;
                    }
                    if is_in_search(&entry, &env) {
                        entries.push(entry);
                    }
                }
                Ok(None) => {}
                Err(message) => scan.errors.push(LoadError {
                    path: found.path,
                    message,
                }),
            }
        }
        entries.sort_by(|a, b| a.id.cmp(&b.id));
        let keys = entries.iter().map(SearchKey::new).collect();

        Applications {
            env,
            settings_page,
            entries,
            keys,
            errors: scan.errors,
            times: scan.times,
        }
    }

    /// Reads all again with the same environment and rule. The caller can
    /// do this on a different thread and then replace the old list.
    pub fn reload(&self) -> Applications {
        Applications::load(self.env.clone(), self.settings_page)
    }

    /// `true` when a directory or a `.desktop` file has a different
    /// modification time than at the load, or was added or removed.
    pub fn has_changed(&self) -> bool {
        self.times
            .iter()
            .any(|(path, time)| modified(path) != *time)
    }

    /// The entries in the order of their IDs.
    pub fn entries(&self) -> &[DesktopEntry] {
        &self.entries
    }

    pub fn get(&self, id: &str) -> Option<&DesktopEntry> {
        let index = self
            .entries
            .binary_search_by(|entry| entry.id.as_str().cmp(id))
            .ok()?;
        Some(&self.entries[index])
    }

    /// The `.desktop` files and directories that the loader could not read.
    /// A file that cannot be read still hides the entries with the same ID
    /// in later directories.
    pub fn errors(&self) -> &[LoadError] {
        &self.errors
    }

    /// The best `limit` entries for `text`, best first. `usage` gives the
    /// number of starts of each desktop file ID.
    pub fn search(&self, text: &str, limit: usize, usage: &UsageCounts) -> Vec<Hit<'_>> {
        let Some(query) = Query::new(text) else {
            return Vec::new();
        };
        let mut matches: Vec<(Tier, &DesktopEntry, &SearchKey)> = self
            .entries
            .iter()
            .zip(&self.keys)
            .filter_map(|(entry, key)| Some((search::tier(key, &query)?, entry, key)))
            .collect();
        // A typing error is a fallback. Next to correct matches it is noise.
        if matches.iter().any(|(tier, ..)| *tier != Tier::TypingError) {
            matches.retain(|(tier, ..)| *tier != Tier::TypingError);
        }

        let mut hits: Vec<(Hit, &str)> = matches
            .into_iter()
            .map(|(tier, entry, key)| {
                let starts = usage.get(&entry.id).copied().unwrap_or(0);
                let score = tier.points() + search::usage_bonus(starts);
                (Hit { entry, score }, key.sort_name())
            })
            .collect();
        hits.sort_by(|(a, a_name), (b, b_name)| {
            b.score
                .cmp(&a.score)
                .then_with(|| a_name.chars().count().cmp(&b_name.chars().count()))
                .then_with(|| a_name.cmp(b_name))
                .then_with(|| a.entry.id.cmp(&b.entry.id))
        });
        hits.into_iter().take(limit).map(|(hit, _)| hit).collect()
    }

    /// The applications that open web pages, in the order of their names.
    pub fn browsers(&self) -> Vec<&DesktopEntry> {
        let mut browsers: Vec<&DesktopEntry> = self
            .entries
            .iter()
            .filter(|entry| entry.kind == EntryKind::Application && entry.is_browser())
            .collect();
        browsers.sort_by_cached_key(|entry| (entry.name.to_lowercase(), entry.id.clone()));
        browsers
    }
}

/// The filter rule of `docs/applications.md`.
fn is_in_search(entry: &DesktopEntry, env: &Environment) -> bool {
    let installed = |program: &str| find_program(program, &env.path);
    (entry.kind == EntryKind::SettingsPage || !entry.no_display)
        && entry.shows_in(&env.current_desktops)
        && entry.try_exec.as_deref().is_none_or(installed)
        && entry.exec.first().is_none_or(|program| installed(program))
}

fn find_program(program: &str, path: &[PathBuf]) -> bool {
    if program.contains('/') {
        return is_executable(Path::new(program));
    }
    path.iter().any(|dir| is_executable(&dir.join(program)))
}

fn is_executable(path: &Path) -> bool {
    fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}

/// A `.desktop` file that owns its desktop file ID.
struct Found {
    id: String,
    path: PathBuf,
}

#[derive(Default)]
struct Scan {
    found: Vec<Found>,
    ids: HashSet<String>,
    /// Canonical paths, so that a directory link to a parent is read once.
    visited: HashSet<PathBuf>,
    times: Vec<(PathBuf, Option<SystemTime>)>,
    errors: Vec<LoadError>,
}

impl Scan {
    /// Reads `dir` and its subdirectories. `root` is the `applications`
    /// directory that the desktop file IDs are relative to.
    fn directory(&mut self, root: &Path, dir: &Path) {
        self.times.push((dir.to_path_buf(), modified(dir)));
        let Ok(canonical) = fs::canonicalize(dir) else {
            return;
        };
        if !self.visited.insert(canonical) {
            return;
        }
        let mut paths: Vec<PathBuf> = match fs::read_dir(dir) {
            Ok(items) => items.filter_map(|item| Some(item.ok()?.path())).collect(),
            Err(error) => {
                self.errors.push(LoadError {
                    path: dir.to_path_buf(),
                    message: error.to_string(),
                });
                return;
            }
        };
        paths.sort();
        for path in paths {
            // A broken link has no metadata. It is not an entry.
            let Ok(metadata) = fs::metadata(&path) else {
                continue;
            };
            if metadata.is_dir() {
                self.directory(root, &path);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "desktop")
            {
                let id = desktop_file_id(root, &path);
                if self.ids.insert(id.clone()) {
                    self.times.push((path.clone(), metadata.modified().ok()));
                    self.found.push(Found { id, path });
                }
            }
        }
    }
}

/// The path below `root` with `-` in place of each `/`.
fn desktop_file_id(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let parts: Vec<_> = relative.iter().map(|part| part.to_string_lossy()).collect();
    parts.join("-")
}
