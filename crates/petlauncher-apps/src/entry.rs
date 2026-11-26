//! One desktop entry, read from one `.desktop` file.

use std::path::{Path, PathBuf};

use crate::exec::{self, Fields};
use crate::keyfile;
use crate::locale::Locale;

/// The kind of a search result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryKind {
    Application,
    /// A page of the desktop settings, for example Wi-Fi. The rule that the
    /// caller gives to `Applications::load` decides this kind.
    SettingsPage,
}

/// An entry of `Type=Application`. The localized values use the locale of
/// the `Environment`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopEntry {
    /// The desktop file ID, for example `org.gnome.Nautilus.desktop`.
    pub id: String,
    /// The full path of the `.desktop` file.
    pub path: PathBuf,
    pub kind: EntryKind,
    pub name: String,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub keywords: Vec<String>,
    /// An icon name or an absolute path.
    pub icon: Option<String>,
    /// The arguments of `Exec` after the quoting rules. The field codes are
    /// not expanded. Empty when the file has no `Exec`.
    pub exec: Vec<String>,
    pub try_exec: Option<String>,
    pub categories: Vec<String>,
    pub mime_types: Vec<String>,
    pub only_show_in: Vec<String>,
    pub not_show_in: Vec<String>,
    pub startup_wm_class: Option<String>,
    /// The program must run in a terminal. `command` does not add one.
    pub terminal: bool,
    pub no_display: bool,
}

impl DesktopEntry {
    /// Reads the text of a `.desktop` file. Gives `Ok(None)` for an entry
    /// that is not an application or that has `Hidden=true`.
    pub(crate) fn parse(
        id: &str,
        path: &Path,
        text: &str,
        locale: Option<&Locale>,
    ) -> Result<Option<DesktopEntry>, String> {
        let group = keyfile::desktop_entry_group(text).ok_or("no group [Desktop Entry]")?;
        if group.boolean("Hidden") || group.string("Type").as_deref() != Some("Application") {
            return Ok(None);
        }
        let name = group.localized("Name", locale).ok_or("no Name")?;
        let exec = match group.string("Exec") {
            Some(value) => exec::split(&value)?,
            None => Vec::new(),
        };
        Ok(Some(DesktopEntry {
            id: id.to_string(),
            path: path.to_path_buf(),
            kind: EntryKind::Application,
            name,
            generic_name: group.localized("GenericName", locale),
            comment: group.localized("Comment", locale),
            keywords: group.localized_list("Keywords", locale),
            icon: group.localized("Icon", locale),
            exec,
            try_exec: group.string("TryExec").filter(|value| !value.is_empty()),
            categories: group.list("Categories"),
            mime_types: group.list("MimeType"),
            only_show_in: group.list("OnlyShowIn"),
            not_show_in: group.list("NotShowIn"),
            startup_wm_class: group.string("StartupWMClass"),
            terminal: group.boolean("Terminal"),
            no_display: group.boolean("NoDisplay"),
        }))
    }

    /// The argument list that starts the entry, with the field codes
    /// expanded. `target` is a file or URL for `%f`, `%F`, `%u` and `%U`.
    /// Empty when the entry has no `Exec`.
    pub fn command(&self, target: Option<&str>) -> Vec<String> {
        let fields = Fields {
            target,
            name: &self.name,
            icon: self.icon.as_deref(),
            path: &self.path,
        };
        exec::expand(&self.exec, &fields)
    }

    /// The rule of `OnlyShowIn` and `NotShowIn` for the desktop names of
    /// `XDG_CURRENT_DESKTOP`, in the order of that list.
    pub(crate) fn shows_in(&self, desktops: &[String]) -> bool {
        for desktop in desktops {
            if self.only_show_in.contains(desktop) {
                return true;
            }
            if self.not_show_in.contains(desktop) {
                return false;
            }
        }
        self.only_show_in.is_empty()
    }

    /// Handles `http` or `https` URLs, or has the category `WebBrowser`.
    pub(crate) fn is_browser(&self) -> bool {
        self.categories
            .iter()
            .any(|category| category == "WebBrowser")
            || self
                .mime_types
                .iter()
                .any(|mime| mime == "x-scheme-handler/http" || mime == "x-scheme-handler/https")
    }
}

#[cfg(test)]
impl DesktopEntry {
    /// An application with only a name and an `Exec`.
    pub(crate) fn for_test(name: &str, exec: &[&str]) -> DesktopEntry {
        DesktopEntry {
            id: format!("{}.desktop", name.to_lowercase().replace(' ', "-")),
            path: PathBuf::from("/usr/share/applications/test.desktop"),
            kind: EntryKind::Application,
            name: name.into(),
            generic_name: None,
            comment: None,
            keywords: vec![],
            icon: None,
            exec: exec.iter().map(|arg| arg.to_string()).collect(),
            try_exec: None,
            categories: vec![],
            mime_types: vec![],
            only_show_in: vec![],
            not_show_in: vec![],
            startup_wm_class: None,
            terminal: false,
            no_display: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATH: &str = "/usr/share/applications/org.gnome.Nautilus.desktop";

    fn parse(text: &str) -> Result<Option<DesktopEntry>, String> {
        DesktopEntry::parse("org.gnome.Nautilus.desktop", Path::new(PATH), text, None)
    }

    fn entry(text: &str) -> DesktopEntry {
        parse(text).unwrap().expect("an application")
    }

    fn desktops(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn reads_all_keys_of_an_application() {
        let text = "[Desktop Entry]\nType=Application\nName=Files\nGenericName=File Manager\n\
                    Comment=Access and organize files\nKeywords=folder;nautilus;\n\
                    Exec=nautilus --new-window %U\nTryExec=nautilus\nIcon=org.gnome.Nautilus\n\
                    Categories=GNOME;GTK;Utility;\nMimeType=inode/directory;\n\
                    OnlyShowIn=GNOME;\nNotShowIn=KDE;\nStartupWMClass=org.gnome.Nautilus\n\
                    Terminal=false\nNoDisplay=false\n";

        let entry = entry(text);

        assert_eq!(
            entry,
            DesktopEntry {
                id: "org.gnome.Nautilus.desktop".into(),
                path: PATH.into(),
                kind: EntryKind::Application,
                name: "Files".into(),
                generic_name: Some("File Manager".into()),
                comment: Some("Access and organize files".into()),
                keywords: vec!["folder".into(), "nautilus".into()],
                icon: Some("org.gnome.Nautilus".into()),
                exec: vec!["nautilus".into(), "--new-window".into(), "%U".into()],
                try_exec: Some("nautilus".into()),
                categories: vec!["GNOME".into(), "GTK".into(), "Utility".into()],
                mime_types: vec!["inode/directory".into()],
                only_show_in: vec!["GNOME".into()],
                not_show_in: vec!["KDE".into()],
                startup_wm_class: Some("org.gnome.Nautilus".into()),
                terminal: false,
                no_display: false,
            }
        );
    }

    #[test]
    fn reads_the_localized_name_generic_name_comment_and_keywords() {
        let text = "[Desktop Entry]\nType=Application\nName=Files\nName[fi]=Tiedostot\n\
                    GenericName=File Manager\nGenericName[fi]=Tiedostonhallinta\n\
                    Comment=Organize files\nComment[fi]=Järjestä tiedostoja\n\
                    Keywords=folder;\nKeywords[fi]=kansio;\nExec=nautilus\n";
        let locale = Locale::parse("fi_FI.UTF-8");

        let entry = DesktopEntry::parse("a.desktop", Path::new(PATH), text, locale.as_ref())
            .unwrap()
            .unwrap();

        assert_eq!(entry.name, "Tiedostot");
        assert_eq!(entry.generic_name.as_deref(), Some("Tiedostonhallinta"));
        assert_eq!(entry.comment.as_deref(), Some("Järjestä tiedostoja"));
        assert_eq!(entry.keywords, ["kansio"]);
    }

    #[test]
    fn gives_no_entry_for_other_types_and_hidden_entries() {
        for text in [
            "[Desktop Entry]\nType=Link\nName=Home page\nURL=https://example.com\n",
            "[Desktop Entry]\nType=Directory\nName=Games\n",
            "[Desktop Entry]\nName=No type\nExec=x\n",
            "[Desktop Entry]\nType=Application\nName=Old\nExec=x\nHidden=true\n",
        ] {
            assert_eq!(parse(text), Ok(None), "{text}");
        }
    }

    #[test]
    fn rejects_a_file_without_a_desktop_entry_group() {
        assert!(parse("[Something]\nName=x\n").is_err());
    }

    #[test]
    fn rejects_an_application_without_a_name() {
        assert!(parse("[Desktop Entry]\nType=Application\nExec=x\n").is_err());
    }

    #[test]
    fn rejects_an_application_with_a_quote_that_does_not_end() {
        assert!(parse("[Desktop Entry]\nType=Application\nName=x\nExec=prog \"a\n").is_err());
    }

    #[test]
    fn accepts_an_application_without_exec() {
        let entry = entry("[Desktop Entry]\nType=Application\nName=Portal\nDBusActivatable=true\n");

        assert!(entry.exec.is_empty());
        assert!(entry.command(None).is_empty());
    }

    #[test]
    fn undoes_the_key_file_escapes_before_the_exec_quoting() {
        let text = "[Desktop Entry]\nType=Application\nName=x\n\
                    Exec=sh -c \"printf \\\\\\\\n\" a\\sb\n";

        let entry = entry(text);

        assert_eq!(entry.exec, ["sh", "-c", "printf \\n", "a", "b"]);
    }

    #[test]
    fn gives_the_command_with_expanded_field_codes() {
        let text = "[Desktop Entry]\nType=Application\nName=Firefox\nIcon=firefox\n\
                    Exec=/snap/bin/firefox %u %i\n";

        let entry = entry(text);

        assert_eq!(
            entry.command(Some("https://example.com/")),
            [
                "/snap/bin/firefox",
                "https://example.com/",
                "--icon",
                "firefox"
            ]
        );
        assert_eq!(
            entry.command(None),
            ["/snap/bin/firefox", "--icon", "firefox"]
        );
    }

    #[test]
    fn shows_an_entry_without_show_in_keys_on_each_desktop() {
        let entry = entry("[Desktop Entry]\nType=Application\nName=x\n");

        assert!(entry.shows_in(&desktops(&["ubuntu", "GNOME"])));
        assert!(entry.shows_in(&[]));
    }

    #[test]
    fn shows_an_only_show_in_entry_when_a_current_desktop_is_in_the_list() {
        let entry = entry("[Desktop Entry]\nType=Application\nName=x\nOnlyShowIn=ubuntu;\n");

        assert!(entry.shows_in(&desktops(&["ubuntu", "GNOME"])));
        assert!(!entry.shows_in(&desktops(&["GNOME"])));
        assert!(!entry.shows_in(&[]));
    }

    #[test]
    fn hides_a_not_show_in_entry_when_a_current_desktop_is_in_the_list() {
        let entry = entry("[Desktop Entry]\nType=Application\nName=x\nNotShowIn=GNOME;\n");

        assert!(!entry.shows_in(&desktops(&["ubuntu", "GNOME"])));
        assert!(entry.shows_in(&desktops(&["KDE"])));
    }

    #[test]
    fn uses_the_first_current_desktop_that_a_list_names() {
        let entry = entry(
            "[Desktop Entry]\nType=Application\nName=x\nOnlyShowIn=GNOME;\nNotShowIn=ubuntu;\n",
        );

        assert!(!entry.shows_in(&desktops(&["ubuntu", "GNOME"])));
        assert!(entry.shows_in(&desktops(&["GNOME", "ubuntu"])));
    }

    #[test]
    fn compares_desktop_names_with_case() {
        let entry = entry("[Desktop Entry]\nType=Application\nName=x\nOnlyShowIn=GNOME;\n");

        assert!(!entry.shows_in(&desktops(&["gnome"])));
    }

    #[test]
    fn finds_a_browser_by_url_scheme_or_category() {
        let base = "[Desktop Entry]\nType=Application\nName=x\n";
        let http = entry(&format!(
            "{base}MimeType=text/html;x-scheme-handler/http;\n"
        ));
        let https = entry(&format!("{base}MimeType=x-scheme-handler/https;\n"));
        let category = entry(&format!("{base}Categories=Network;WebBrowser;\n"));
        let editor = entry(&format!("{base}MimeType=text/html;\nCategories=Utility;\n"));

        assert!(http.is_browser());
        assert!(https.is_browser());
        assert!(category.is_browser());
        assert!(!editor.is_browser());
    }
}
