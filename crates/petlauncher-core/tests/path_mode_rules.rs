//! The order and the match rules of the path mode (docs/core.md).

mod common;

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};

use common::{search, set_modified, titles, TempHome};
use petlauncher_core::{Action, Icon, RowKind, TypedPath};

#[test]
fn the_contents_of_a_directory_come_newest_first() {
    let home = TempHome::new();
    home.file("box/old.txt", 1);
    home.file("box/new.txt", 3);
    let dir = home.dir("box/middle");
    set_modified(&dir, 2);

    let results = search(&home.engine(), "~/box");

    assert_eq!(titles(&results), ["new.txt", "middle", "old.txt"]);
}

#[test]
fn entries_with_the_same_time_come_in_name_order_with_no_letter_case() {
    let home = TempHome::new();
    home.file("box/b.txt", 1);
    home.file("box/A.txt", 1);
    home.file("box/c.txt", 1);

    let results = search(&home.engine(), "~/box/");

    assert_eq!(titles(&results), ["A.txt", "b.txt", "c.txt"]);
}

#[test]
fn hidden_entries_show_only_when_the_typed_name_starts_with_a_dot() {
    let home = TempHome::new();
    home.file(".bashrc", 1);
    home.file(".bash_history", 2);
    home.file("bash-notes.txt", 3);

    let listing = search(&home.engine(), "~/");
    let partial = search(&home.engine(), "~/bas");
    let dotted = search(&home.engine(), "~/.bash");

    assert_eq!(titles(&listing), ["bash-notes.txt"]);
    assert_eq!(titles(&partial), ["bash-notes.txt"]);
    assert_eq!(titles(&dotted), [".bash_history", ".bashrc"]);
}

#[test]
fn a_partial_name_gives_the_same_name_then_names_that_start_with_it_then_names_that_contain_it() {
    let home = TempHome::new();
    home.file("box/x-report-old", 1);
    home.file("box/Report.txt", 2);
    home.file("box/report", 3);
    home.file("box/other", 4);

    let results = search(&home.engine(), "~/box/REPORT");

    assert_eq!(titles(&results), ["report", "Report.txt", "x-report-old"]);
}

#[test]
fn with_no_name_that_contains_the_typed_name_the_longest_shared_start_comes_first() {
    let home = TempHome::new();
    home.file("box/palette-zoom.png", 1);
    home.file("box/palette-full.png", 2);
    home.file("box/pal.txt", 3);
    home.file("box/menu.png", 4);

    let results = search(&home.engine(), "~/box/palette-zoom.html");

    assert_eq!(
        titles(&results),
        ["palette-zoom.png", "palette-full.png", "pal.txt"]
    );
}

#[test]
fn a_name_with_a_shared_start_of_less_than_three_characters_is_not_a_near_match() {
    let home = TempHome::new();
    home.file("box/pa.txt", 1);

    let results = search(&home.engine(), "~/box/pzz");

    assert!(results.rows.is_empty());
    assert_eq!(results.selected, None);
}

#[test]
fn a_mistake_in_a_folder_name_shows_the_matches_in_the_last_existing_folder() {
    let home = TempHome::new();
    home.file("comfy-output/H3-IMAGE/a.png", 1);

    let results = search(&home.engine(), "~/comfy-outptu/H3-IMAGE/");

    assert_eq!(titles(&results), ["comfy-output"]);
    assert_eq!(
        results.rows[0].enter,
        Action::SetQuery("~/comfy-output/".into())
    );
}

#[test]
fn an_absolute_query_gives_an_absolute_query_for_a_directory_row() {
    let home = TempHome::new();
    let dir = home.dir("projects");
    let query = format!("{}/proj", home.home.to_str().unwrap());

    let results = search(&home.engine(), &query);

    assert_eq!(
        results.rows[0].enter,
        Action::SetQuery(format!("{}/", dir.to_str().unwrap()))
    );
    assert_eq!(results.rows[0].subtitle, dir.to_str().unwrap());
}

#[test]
fn a_file_path_with_a_last_slash_is_the_file() {
    let home = TempHome::new();
    let file = home.file("notes.txt", 1);

    let results = search(&home.engine(), "~/notes.txt/");

    assert_eq!(titles(&results), ["notes.txt"]);
    assert_eq!(results.selected, Some(0));
    assert_eq!(results.typed_path, Some(TypedPath::File(file)));
}

#[test]
fn a_file_in_the_middle_of_a_path_is_the_match_for_that_part() {
    let home = TempHome::new();
    let file = home.file("plain.txt", 1);

    let results = search(&home.engine(), "~/plain.txt/inner/x");

    assert_eq!(titles(&results), ["plain.txt"]);
    assert_eq!(results.rows[0].enter, Action::OpenFile(file));
    assert_eq!(results.typed_path, None);
}

#[test]
fn a_link_to_a_directory_is_a_directory_row_and_a_broken_link_is_not_shown() {
    let home = TempHome::new();
    let target = home.dir("real");
    symlink(&target, home.path("box-link")).unwrap();
    symlink(home.path("missing"), home.path("box-broken")).unwrap();

    let results = search(&home.engine(), "~/box");

    assert_eq!(titles(&results), ["box-link"]);
    assert_eq!(results.rows[0].kind, RowKind::Directory);
}

#[test]
fn a_path_row_has_the_icon_of_its_file_and_the_file_as_local_file() {
    let home = TempHome::new();
    let file = home.file("notes.txt", 1);

    let results = search(&home.engine(), "~/notes.txt");

    assert_eq!(results.rows[0].icon, Icon::ForFile(file.clone()));
    assert_eq!(results.rows[0].local_file, Some(file));
    assert_eq!(results.rows[0].kind, RowKind::File);
}

#[test]
fn the_tilde_alone_is_the_home_directory() {
    let home = TempHome::new();
    home.file("a.txt", 1);

    let results = search(&home.engine(), "~");

    assert_eq!(
        results.typed_path,
        Some(TypedPath::Directory(home.home.clone()))
    );
    assert_eq!(titles(&results), ["a.txt"]);
}

#[test]
fn a_large_directory_shows_at_most_fifty_rows() {
    let home = TempHome::new();
    for number in 0..60 {
        home.file(&format!("many/file-{number:02}.txt"), number);
    }

    let listing = search(&home.engine(), "~/many/");
    let partial = search(&home.engine(), "~/many/file");

    assert_eq!(listing.rows.len(), 50);
    assert_eq!(listing.rows[0].title, "file-59.txt");
    assert_eq!(partial.rows.len(), 50);
    assert_eq!(partial.rows[0].title, "file-00.txt");
}

#[test]
fn a_folder_that_cannot_be_read_gives_no_rows() {
    let home = TempHome::new();
    home.file("locked/secret.txt", 1);
    let locked = home.path("locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let permissions_have_effect = fs::read_dir(&locked).is_err();

    let results = search(&home.engine(), "~/locked/sec");

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    // The root user can read the folder with all permissions removed.
    if permissions_have_effect {
        assert!(results.rows.is_empty());
    }
}
