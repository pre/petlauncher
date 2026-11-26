//! R1: the typed path decides what the keys do.

mod common;

use std::path::PathBuf;

use common::{row_index, search, titles, TempHome};
use petlauncher_core::{activate, Action, Engine, Key, Mode, RowKind, TypedPath};

/// The tree of the R1 steps below the temporary home directory:
///
/// ```text
/// comfy-output/
///   H3-IMAGE/ComfyUI_00001_.png, ComfyUI_00002_.png (newer)
///   H3-VIDEO/clip.mp4
///   H3-notes.txt
///   single/only.png
///   Zeta.txt
/// ```
struct Comfy {
    home: TempHome,
    engine: Engine,
}

impl Comfy {
    fn new() -> Self {
        let home = TempHome::new();
        home.file("comfy-output/H3-IMAGE/ComfyUI_00001_.png", 1);
        home.file("comfy-output/H3-IMAGE/ComfyUI_00002_.png", 2);
        home.file("comfy-output/H3-VIDEO/clip.mp4", 3);
        home.file("comfy-output/H3-notes.txt", 4);
        home.file("comfy-output/single/only.png", 5);
        home.file("comfy-output/Zeta.txt", 6);
        let engine = home.engine();
        Self { home, engine }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.home.path(relative)
    }
}

#[test]
fn r1_step1_enter_on_a_typed_directory_with_a_last_slash_opens_it_in_the_file_manager() {
    let comfy = Comfy::new();

    let results = search(&comfy.engine, "~/comfy-output/H3-IMAGE/");
    let outcome = activate(&results, results.selected, Key::Enter);

    let h3_image = comfy.path("comfy-output/H3-IMAGE");
    assert_eq!(results.mode, Mode::Path);
    assert_eq!(results.selected, None);
    assert_eq!(
        results.typed_path,
        Some(TypedPath::Directory(h3_image.clone()))
    );
    assert_eq!(outcome.action, Action::OpenFolder(h3_image));
    assert!(outcome.close);
}

#[test]
fn r1_step2_enter_on_a_typed_directory_without_the_last_slash_opens_the_same_directory() {
    let comfy = Comfy::new();

    let results = search(&comfy.engine, "~/comfy-output/H3-IMAGE");
    let outcome = activate(&results, results.selected, Key::Enter);

    assert_eq!(
        outcome.action,
        Action::OpenFolder(comfy.path("comfy-output/H3-IMAGE"))
    );
    assert_ne!(
        outcome.action,
        Action::OpenFolder(comfy.path("comfy-output"))
    );
    assert!(outcome.close);
}

#[test]
fn r1_step3_down_then_enter_below_a_typed_directory_opens_the_first_file() {
    let comfy = Comfy::new();
    let results = search(&comfy.engine, "~/comfy-output/H3-IMAGE/");

    let outcome = activate(&results, Some(0), Key::Enter);

    let newest = comfy.path("comfy-output/H3-IMAGE/ComfyUI_00002_.png");
    assert_eq!(
        titles(&results),
        ["ComfyUI_00002_.png", "ComfyUI_00001_.png"]
    );
    assert_eq!(outcome.action, Action::OpenFile(newest));
    assert!(outcome.close);
}

#[test]
fn r1_step4_down_then_ctrl_enter_shows_the_file_selected_in_its_folder() {
    let comfy = Comfy::new();
    let results = search(&comfy.engine, "~/comfy-output/H3-IMAGE/");

    let outcome = activate(&results, Some(0), Key::CtrlEnter);

    let file = comfy.path("comfy-output/H3-IMAGE/ComfyUI_00002_.png");
    assert_eq!(outcome.action, Action::ShowInFolder(file));
    assert!(outcome.close);
}

#[test]
fn r1_step5_ctrl_enter_on_the_full_path_of_a_file_shows_it_selected_in_its_folder() {
    let comfy = Comfy::new();
    let file = comfy.path("comfy-output/H3-IMAGE/ComfyUI_00001_.png");
    let results = search(&comfy.engine, file.to_str().unwrap());

    let outcome = activate(&results, results.selected, Key::CtrlEnter);

    assert_eq!(results.typed_path, Some(TypedPath::File(file.clone())));
    assert_eq!(outcome.action, Action::ShowInFolder(file));
    assert!(outcome.close);
}

#[test]
fn r1_step6_enter_on_the_full_path_of_a_file_opens_the_file() {
    let comfy = Comfy::new();
    let file = comfy.path("comfy-output/H3-IMAGE/ComfyUI_00001_.png");
    let results = search(&comfy.engine, file.to_str().unwrap());

    let outcome = activate(&results, results.selected, Key::Enter);

    assert_eq!(outcome.action, Action::OpenFile(file));
    assert!(outcome.close);
}

#[test]
fn r1_step7_enter_on_the_first_match_changes_the_query_and_a_second_enter_opens_the_directory() {
    let comfy = Comfy::new();
    let results = search(&comfy.engine, "~/comfy-output/H3");

    let first = activate(&results, results.selected, Key::Enter);
    let second_results = search(&comfy.engine, "~/comfy-output/H3-IMAGE/");
    let second = activate(&second_results, second_results.selected, Key::Enter);

    assert_eq!(titles(&results), ["H3-IMAGE", "H3-notes.txt", "H3-VIDEO"]);
    assert_eq!(results.selected, Some(0));
    assert_eq!(results.rows[0].kind, RowKind::Directory);
    assert_eq!(
        first.action,
        Action::SetQuery("~/comfy-output/H3-IMAGE/".into())
    );
    assert!(!first.close);
    assert_eq!(
        second.action,
        Action::OpenFolder(comfy.path("comfy-output/H3-IMAGE"))
    );
    assert!(second.close);
}

#[test]
fn r1_step8_a_single_match_of_a_path_that_does_not_exist_is_selected_without_down() {
    let comfy = Comfy::new();
    let results = search(&comfy.engine, "~/comfy-output/sing");

    let first = activate(&results, results.selected, Key::Enter);
    let Action::SetQuery(new_query) = &first.action else {
        panic!("Enter gave {:?}", first.action);
    };
    let second_results = search(&comfy.engine, new_query);
    let second = activate(&second_results, second_results.selected, Key::Enter);

    assert_eq!(titles(&results), ["single"]);
    assert_eq!(results.selected, Some(0));
    assert_eq!(new_query, "~/comfy-output/single/");
    assert!(!first.close);
    assert_eq!(
        second.action,
        Action::OpenFolder(comfy.path("comfy-output/single"))
    );
}

#[test]
fn r1_step9_enter_on_a_directory_with_one_file_opens_the_directory_not_the_file() {
    let comfy = Comfy::new();
    let results = search(&comfy.engine, "~/comfy-output/single");

    let outcome = activate(&results, results.selected, Key::Enter);

    assert_eq!(titles(&results), ["only.png"]);
    assert_eq!(results.selected, None);
    assert_eq!(
        outcome.action,
        Action::OpenFolder(comfy.path("comfy-output/single"))
    );
}

#[test]
fn r1_table_ctrl_enter_on_a_typed_directory_with_no_selected_row_is_the_same_as_enter() {
    let comfy = Comfy::new();
    let results = search(&comfy.engine, "~/comfy-output/H3-IMAGE/");

    let outcome = activate(&results, results.selected, Key::CtrlEnter);

    assert_eq!(
        outcome.action,
        Action::OpenFolder(comfy.path("comfy-output/H3-IMAGE"))
    );
}

#[test]
fn r1_table_a_directory_row_below_a_typed_directory_changes_the_query_or_opens_with_ctrl_enter() {
    let comfy = Comfy::new();
    let results = search(&comfy.engine, "~/comfy-output/");
    let video = row_index(&results, "H3-VIDEO");

    let enter = activate(&results, Some(video), Key::Enter);
    let ctrl_enter = activate(&results, Some(video), Key::CtrlEnter);

    assert_eq!(
        enter.action,
        Action::SetQuery("~/comfy-output/H3-VIDEO/".into())
    );
    assert!(!enter.close);
    assert_eq!(
        ctrl_enter.action,
        Action::OpenFolder(comfy.path("comfy-output/H3-VIDEO"))
    );
    assert!(ctrl_enter.close);
}

#[test]
fn r1_table_ctrl_enter_on_a_file_match_of_a_path_that_does_not_exist_shows_it_in_its_folder() {
    let comfy = Comfy::new();
    let results = search(&comfy.engine, "~/comfy-output/H3-n");

    let outcome = activate(&results, results.selected, Key::CtrlEnter);

    assert_eq!(titles(&results), ["H3-notes.txt"]);
    assert_eq!(
        outcome.action,
        Action::ShowInFolder(comfy.path("comfy-output/H3-notes.txt"))
    );
}
