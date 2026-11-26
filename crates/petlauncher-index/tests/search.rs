//! The match and rank rule of the search, on indexes made from path lists.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use petlauncher_index::{FileEntry, Index};

fn file(path: &str) -> FileEntry {
    file_at(path, 0)
}

fn file_at(path: &str, modified: u64) -> FileEntry {
    FileEntry {
        path: PathBuf::from(path),
        is_dir: false,
        modified: UNIX_EPOCH + Duration::from_secs(modified),
    }
}

fn dir(path: &str) -> FileEntry {
    FileEntry {
        is_dir: true,
        ..file(path)
    }
}

fn index(entries: Vec<FileEntry>) -> Index {
    Index::from_entries(entries, SystemTime::now())
}

fn paths(hits: Vec<FileEntry>) -> Vec<String> {
    hits.into_iter()
        .map(|hit| hit.path.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn finds_palette_zoom_png_by_the_text_palette_zoom() {
    let index = index(vec![
        file("/h/lolcat-evidence/kukka/screenshots-final/palette-zoom.png"),
        file("/h/lolcat-evidence/kukka/screenshots-final/palette-overview.png"),
        file("/h/dev/zoom.rs"),
    ]);

    let hits = index.search("palette-zoom", 10);

    assert_eq!(
        paths(hits),
        ["/h/lolcat-evidence/kukka/screenshots-final/palette-zoom.png"]
    );
}

#[test]
fn ranks_exact_name_then_stem_then_start_then_word_start_then_inside() {
    let index = index(vec![
        file("/h/1/inside-mypalette"),
        file("/h/2/my-palette-x"),
        file("/h/3/palette-x"),
        file("/h/4/palette.png"),
        file("/h/5/palette"),
    ]);

    let hits = index.search("palette", 10);

    assert_eq!(
        paths(hits),
        [
            "/h/5/palette",
            "/h/4/palette.png",
            "/h/3/palette-x",
            "/h/2/my-palette-x",
            "/h/1/inside-mypalette",
        ]
    );
}

#[test]
fn takes_the_name_before_the_last_dot_as_the_stem() {
    let index = index(vec![file("/h/archive.tar.gz"), file("/h/archive.tar")]);

    let hits = index.search("archive.tar", 10);

    assert_eq!(paths(hits), ["/h/archive.tar", "/h/archive.tar.gz"]);
}

#[test]
fn a_word_starts_after_punctuation_or_at_a_change_to_upper_case() {
    let index = index(vec![
        file("/h/a/xzoomx"),
        file("/h/b/PaletteZoom.svg"),
        file("/h/c/palette_zoomed"),
    ]);

    let hits = index.search("zoom", 10);

    assert_eq!(
        paths(hits),
        ["/h/c/palette_zoomed", "/h/b/PaletteZoom.svg", "/h/a/xzoomx"]
    );
}

#[test]
fn finds_a_word_start_that_overlaps_an_earlier_match() {
    let index = index(vec![file("/h/a/xa-a"), file("/h/b/xa-a-a")]);

    let hits = index.search("a-a", 10);

    assert_eq!(paths(hits), ["/h/b/xa-a-a", "/h/a/xa-a"]);
}

#[test]
fn does_not_match_across_two_names_and_finds_the_next_name() {
    let index = index(vec![file("/h/1/xa"), file("/h/2/aax")]);

    let hits = index.search("aa", 10);

    assert_eq!(paths(hits), ["/h/2/aax"]);
}

#[test]
fn ignores_case_when_the_text_has_no_upper_case_letter() {
    let index = index(vec![file("/h/README.md"), file("/h/Ärger.txt")]);

    assert_eq!(paths(index.search("readme", 10)), ["/h/README.md"]);
    assert_eq!(paths(index.search("ärger", 10)), ["/h/Ärger.txt"]);
}

#[test]
fn matches_case_when_the_text_has_an_upper_case_letter() {
    let index = index(vec![file("/h/a/Makefile"), file("/h/b/makefile.old")]);

    let hits = index.search("Make", 10);

    assert_eq!(paths(hits), ["/h/a/Makefile"]);
}

#[test]
fn needs_each_word_of_the_text_in_the_name() {
    let index = index(vec![
        file("/h/palette-zoom.png"),
        file("/h/palette-overview.png"),
        file("/h/zoom-palette.png"),
    ]);

    let hits = index.search("zoom  palette", 10);

    assert_eq!(paths(hits), ["/h/palette-zoom.png", "/h/zoom-palette.png"]);
}

#[test]
fn does_not_match_the_text_in_the_directory_names() {
    let index = index(vec![file("/h/palette/zoom.png")]);

    assert!(index.search("palette", 10).is_empty());
}

#[test]
fn a_tie_goes_to_the_newer_file_then_to_the_shorter_path_then_to_the_path() {
    let index = index(vec![
        file_at("/h/long/old/notes.md", 100),
        file_at("/h/long/new/notes.md", 200),
        file_at("/h/b/notes.md", 100),
        file_at("/h/a/notes.md", 100),
    ]);

    let hits = index.search("notes", 10);

    assert_eq!(
        paths(hits),
        [
            "/h/long/new/notes.md",
            "/h/a/notes.md",
            "/h/b/notes.md",
            "/h/long/old/notes.md",
        ]
    );
}

#[test]
fn a_shorter_name_goes_before_a_newer_name_in_the_same_rank() {
    let index = index(vec![
        file_at("/h/report-2026-final.pdf", 200),
        file_at("/h/report-2026.pdf", 100),
    ]);

    let hits = index.search("report", 10);

    assert_eq!(
        paths(hits),
        ["/h/report-2026.pdf", "/h/report-2026-final.pdf"]
    );
}

#[test]
fn finds_directories_and_search_files_leaves_them_out() {
    let index = index(vec![
        dir("/h/lolcat-output/L0L3-IMAGE"),
        file("/h/L0L3-IMAGE.txt"),
    ]);

    let all = index.search("l0l3-image", 10);
    let files = index.search_files("l0l3-image", 10);

    assert_eq!(all.len(), 2);
    assert!(all[0].is_dir);
    assert_eq!(all[0].path, PathBuf::from("/h/lolcat-output/L0L3-IMAGE"));
    assert_eq!(paths(files), ["/h/L0L3-IMAGE.txt"]);
}

#[test]
fn gives_at_most_the_limit_of_hits_in_the_rank_order() {
    let mut entries: Vec<_> = (0..500)
        .map(|i| file(&format!("/h/d{i}/x{i:03}-name")))
        .collect();
    entries.push(file("/h/name"));
    let index = index(entries);

    let hits = index.search("name", 3);

    assert_eq!(
        paths(hits),
        ["/h/name", "/h/d0/x000-name", "/h/d1/x001-name"]
    );
    assert!(index.search("name", 0).is_empty());
}

#[test]
fn gives_no_hits_for_an_empty_text() {
    let index = index(vec![file("/h/a")]);

    assert!(index.search("", 10).is_empty());
    assert!(index.search("  ", 10).is_empty());
}

#[test]
fn finds_a_name_that_is_not_utf8_and_keeps_its_bytes() {
    let path = PathBuf::from(OsStr::from_bytes(b"/h/Photo-\xff.jpg"));
    let index = index(vec![FileEntry {
        path: path.clone(),
        is_dir: false,
        modified: UNIX_EPOCH,
    }]);

    let hits = index.search("photo", 10);

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].path, path);
}
