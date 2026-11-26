//! The GNOME rule for settings pages. Give `is_settings_page` to
//! `Applications::load` on GNOME. A different desktop gives its own rule.

use std::path::Path;

use crate::entry::DesktopEntry;
use crate::exec;

/// An entry is a page of GNOME Settings when its `Exec` starts GNOME
/// Settings with the name of a page, for example
/// `gnome-control-center wifi`. `gnome-control-center` with no page is the
/// Settings application itself.
pub fn is_settings_page(entry: &DesktopEntry) -> bool {
    match exec::program_args(&entry.exec).split_first() {
        Some((program, args)) => {
            Path::new(program)
                .file_name()
                .is_some_and(|name| name == "gnome-control-center")
                && args.iter().any(|arg| !arg.starts_with('-'))
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry_with_exec(args: &[&str]) -> DesktopEntry {
        DesktopEntry::for_test("Page", args)
    }

    #[test]
    fn finds_a_page_that_gnome_settings_opens() {
        for exec in [
            &["gnome-control-center", "wifi"][..],
            &["gnome-control-center", "system", "datetime"],
            &["/usr/bin/gnome-control-center", "sound"],
            &[
                "env",
                "XDG_CURRENT_DESKTOP=GNOME",
                "gnome-control-center",
                "display",
            ],
            &["gnome-control-center", "--verbose", "power"],
        ] {
            assert!(is_settings_page(&entry_with_exec(exec)), "{exec:?}");
        }
    }

    #[test]
    fn rejects_gnome_settings_without_a_page() {
        for exec in [
            &["gnome-control-center"][..],
            &["gnome-control-center", "--overview"],
        ] {
            assert!(!is_settings_page(&entry_with_exec(exec)), "{exec:?}");
        }
    }

    #[test]
    fn rejects_other_programs() {
        for exec in [
            &["/usr/bin/gnome-language-selector"][..],
            &["software-properties-gtk"],
            &["my-gnome-control-center", "wifi"],
            &[],
        ] {
            assert!(!is_settings_page(&entry_with_exec(exec)), "{exec:?}");
        }
    }
}
