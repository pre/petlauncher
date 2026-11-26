//! R4: the GNOME key bindings that have a given key. GNOME Settings shows
//! them in Keyboard > View and Customize Shortcuts: the custom shortcuts and
//! the built-in bindings of five schemas. The names and the sections come
//! from the files of GNOME Settings in
//! `<data dir>/gnome-control-center/keybindings/*.xml`.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// The schemas of the built-in key bindings. Each key of type `as` is a
/// list of accelerators, except `custom-keybindings`, which is a list of
/// paths.
pub(crate) const BUILT_IN_SCHEMAS: [&str; 5] = [
    "org.gnome.desktop.wm.keybindings",
    "org.gnome.shell.keybindings",
    "org.gnome.mutter.keybindings",
    "org.gnome.mutter.wayland.keybindings",
    "org.gnome.settings-daemon.plugins.media-keys",
];

/// The key of the media-keys schema that lists the custom shortcuts.
pub(crate) const LIST_KEY: &str = "custom-keybindings";

/// The place of the key bindings in GNOME Settings.
pub const SHORTCUTS_PAGE: &str = "GNOME Settings > Keyboard > View and Customize Shortcuts";

/// The section of the custom shortcuts in GNOME Settings.
pub const CUSTOM_SECTION: &str = "Custom Shortcuts";

/// Where a conflicting key binding is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingKind {
    /// A GNOME custom shortcut at this GSettings path.
    Custom { path: String },
    /// A built-in key binding: a key of type `as` in one of the schemas of
    /// GNOME.
    BuiltIn { schema: String, key: String },
}

/// A GNOME key binding that has the key of petlauncher. It has all that
/// `CustomShortcuts::remove_key` needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub kind: BindingKind,
    /// The name for the owner: the description in GNOME Settings, the name
    /// of a custom shortcut, or the summary of the GSettings key when GNOME
    /// Settings does not show the key.
    pub name: String,
    /// The section in GNOME Settings, for example `Typing` or
    /// `Custom Shortcuts`. `None` when GNOME Settings does not show the key.
    pub section: Option<String>,
    /// The accelerator as GSettings stores it, for example `<Mod4>space`.
    pub accelerator: String,
}

impl Conflict {
    /// The place for the owner, for example
    /// `GNOME Settings > Keyboard > View and Customize Shortcuts > Typing`,
    /// or the `gsettings` schema and key when GNOME Settings does not show
    /// the key.
    pub fn place(&self) -> String {
        match (&self.section, &self.kind) {
            (Some(section), _) => format!("{SHORTCUTS_PAGE} > {section}"),
            (None, BindingKind::BuiltIn { schema, key }) => {
                format!("the GSettings key {schema} {key}, which GNOME Settings does not show")
            }
            (None, BindingKind::Custom { path }) => format!("the custom shortcut {path}"),
        }
    }
}

impl fmt::Display for Conflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\"{}\" ({}) in {}",
            self.name,
            self.accelerator,
            self.place()
        )
    }
}

/// One entry of the files of GNOME Settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CatalogEntry {
    pub schema: String,
    pub key: String,
    pub section: String,
    pub description: String,
}

/// The data directories of the system, where GNOME Settings has its files.
pub(crate) fn system_catalog_dirs() -> Vec<PathBuf> {
    glib::system_data_dirs()
        .into_iter()
        .map(|dir| dir.join("gnome-control-center/keybindings"))
        .collect()
}

/// The entries that GNOME Settings shows. An entry with `hidden="true"`
/// does not show. A file that cannot be read or is not correct gives no
/// entries.
pub(crate) fn read_catalog(dirs: &[PathBuf]) -> Vec<CatalogEntry> {
    let mut entries = Vec::new();
    for dir in dirs {
        let Ok(files) = fs::read_dir(dir) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = files
            .filter_map(|file| file.ok().map(|file| file.path()))
            .filter(|path| path.extension().is_some_and(|extension| extension == "xml"))
            .collect();
        paths.sort();
        for path in paths {
            entries.extend(read_catalog_file(&path));
        }
    }
    entries
}

fn read_catalog_file(path: &Path) -> Vec<CatalogEntry> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(document) = roxmltree::Document::parse(&text) else {
        return Vec::new();
    };
    let root = document.root_element();
    let (Some(schema), Some(section)) = (root.attribute("schema"), root.attribute("name")) else {
        return Vec::new();
    };
    root.children()
        .filter(|node| node.has_tag_name("KeyListEntry"))
        .filter(|node| node.attribute("hidden") != Some("true"))
        .filter_map(|node| {
            Some(CatalogEntry {
                schema: schema.to_owned(),
                key: node.attribute("name")?.to_owned(),
                section: section.to_owned(),
                description: node.attribute("description")?.to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, text: &str) {
        fs::write(dir.join(name), text).unwrap();
    }

    #[test]
    fn reads_the_section_and_the_description_of_each_entry() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "01-input-sources.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<KeyListEntries group="system" schema="org.gnome.desktop.wm.keybindings" name="Typing">
  <KeyListEntry name="switch-input-source" description="Switch to next input source"/>
  <KeyListEntry name="hidden-one" description="Hidden" hidden="true"/>
  <KeyListEntry xml:lang="fi">Translation</KeyListEntry>
</KeyListEntries>"#,
        );
        write(dir.path(), "broken.xml", "<KeyListEntries");
        write(dir.path(), "notes.txt", "not a catalog");

        let entries = read_catalog(&[dir.path().to_owned(), dir.path().join("missing")]);

        assert_eq!(
            entries,
            [CatalogEntry {
                schema: "org.gnome.desktop.wm.keybindings".into(),
                key: "switch-input-source".into(),
                section: "Typing".into(),
                description: "Switch to next input source".into(),
            }]
        );
    }

    #[test]
    fn the_place_names_the_section_or_the_gsettings_key() {
        let shown = Conflict {
            kind: BindingKind::Custom { path: "/a/".into() },
            name: "Show Notes".into(),
            section: Some(CUSTOM_SECTION.into()),
            accelerator: "<Super>space".into(),
        };
        let hidden = Conflict {
            kind: BindingKind::BuiltIn {
                schema: "org.gnome.shell.keybindings".into(),
                key: "toggle-overview".into(),
            },
            name: "Show the overview".into(),
            section: None,
            accelerator: "<Super>s".into(),
        };

        assert_eq!(
            shown.to_string(),
            "\"Show Notes\" (<Super>space) in GNOME Settings > Keyboard > \
             View and Customize Shortcuts > Custom Shortcuts"
        );
        assert_eq!(
            hidden.place(),
            "the GSettings key org.gnome.shell.keybindings toggle-overview, \
             which GNOME Settings does not show"
        );
    }
}
