//! R10: the full path of an existing file shows that file as one row.

mod common;

use std::path::PathBuf;

use common::{search, titles, TempHome};
use petlauncher_core::{activate, Action, Engine, Key, Results, TypedPath};

const FOLDER: &str = "lolcat-evidence/kukka/font-size-keys-palette/screenshots-final";

/// The folder of the R10 example with seven `.png` files.
struct Screenshots {
    home: TempHome,
    engine: Engine,
    file: PathBuf,
}

impl Screenshots {
    fn new() -> Self {
        let home = TempHome::new();
        let names = [
            "palette-zoom.png",
            "palette-full.png",
            "palette-dark.png",
            "keys-small.png",
            "keys-large.png",
            "menu.png",
            "title.png",
        ];
        for (rank, name) in names.into_iter().enumerate() {
            home.file(&format!("{FOLDER}/{name}"), rank as u64);
        }
        let file = home.path(&format!("{FOLDER}/palette-zoom.png"));
        let engine = home.engine();
        Self { home, engine, file }
    }

    fn absolute(&self) -> String {
        self.file.to_str().unwrap().to_owned()
    }

    fn assert_one_selected_row_for_the_file(&self, results: &Results) {
        assert_eq!(titles(results), ["palette-zoom.png"]);
        assert_eq!(results.selected, Some(0));
        assert_eq!(results.rows[0].local_file.as_ref(), Some(&self.file));
        assert_eq!(results.typed_path, Some(TypedPath::File(self.file.clone())));
    }
}

#[test]
fn r10_step1_and_2_a_pasted_full_path_shows_one_selected_row_for_the_file() {
    let screenshots = Screenshots::new();

    let results = search(&screenshots.engine, &screenshots.absolute());

    screenshots.assert_one_selected_row_for_the_file(&results);
}

#[test]
fn r10_step3_enter_opens_the_file() {
    let screenshots = Screenshots::new();
    let results = search(&screenshots.engine, &screenshots.absolute());

    let outcome = activate(&results, results.selected, Key::Enter);

    assert_eq!(outcome.action, Action::OpenFile(screenshots.file.clone()));
    assert!(outcome.close);
}

#[test]
fn r10_step4_ctrl_enter_shows_the_file_selected_in_its_folder() {
    let screenshots = Screenshots::new();
    let results = search(&screenshots.engine, &screenshots.absolute());

    let outcome = activate(&results, results.selected, Key::CtrlEnter);

    assert_eq!(
        outcome.action,
        Action::ShowInFolder(screenshots.file.clone())
    );
    assert_eq!(
        screenshots.file.parent(),
        Some(screenshots.home.path(FOLDER).as_path())
    );
}

#[test]
fn r10_step5_the_path_with_a_tilde_gives_the_same_row_and_actions() {
    let screenshots = Screenshots::new();
    let query = format!("~/{FOLDER}/palette-zoom.png");

    let results = search(&screenshots.engine, &query);
    let enter = activate(&results, results.selected, Key::Enter);

    screenshots.assert_one_selected_row_for_the_file(&results);
    assert_eq!(enter.action, Action::OpenFile(screenshots.file.clone()));
    assert_eq!(results.rows[0].subtitle, query);
}

#[test]
fn r10_step6_a_space_before_and_a_line_end_after_the_path_have_no_effect() {
    let screenshots = Screenshots::new();

    let results = search(
        &screenshots.engine,
        &format!(" {}\n", screenshots.absolute()),
    );

    screenshots.assert_one_selected_row_for_the_file(&results);
}

#[test]
fn r10_step7_quotation_marks_around_the_path_have_no_effect() {
    let screenshots = Screenshots::new();

    let double = search(
        &screenshots.engine,
        &format!("\"{}\"", screenshots.absolute()),
    );
    let single = search(
        &screenshots.engine,
        &format!("'{}'", screenshots.absolute()),
    );

    screenshots.assert_one_selected_row_for_the_file(&double);
    screenshots.assert_one_selected_row_for_the_file(&single);
}

#[test]
fn r10_step8_file_url_before_the_path_has_no_effect() {
    let screenshots = Screenshots::new();

    let results = search(
        &screenshots.engine,
        &format!("file://{}", screenshots.absolute()),
    );

    screenshots.assert_one_selected_row_for_the_file(&results);
}

#[test]
fn r10_a_file_url_with_percent_codes_finds_a_name_with_spaces() {
    let screenshots = Screenshots::new();
    let file = screenshots.home.file(&format!("{FOLDER}/two words.png"), 9);
    let url = format!("file://{}", file.to_str().unwrap().replace(' ', "%20"));

    let results = search(&screenshots.engine, &url);

    assert_eq!(titles(&results), ["two words.png"]);
    assert_eq!(results.typed_path, Some(TypedPath::File(file)));
}

#[test]
fn r10_note_a_path_that_does_not_exist_shows_the_nearest_match_first() {
    let screenshots = Screenshots::new();
    let query = format!("~/{FOLDER}/palette-zoom.html");

    let results = search(&screenshots.engine, &query);

    assert_eq!(results.rows[0].title, "palette-zoom.png");
    assert_eq!(results.selected, Some(0));
    assert_eq!(results.typed_path, None);
}
