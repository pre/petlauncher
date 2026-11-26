//! `execute`: each action that starts a program, and the fallback of the
//! file manager. A program starts only in a child process with an isolated
//! environment (see `support::IsolatedEnv`), where each application is a
//! recorder that writes its arguments to a file.

mod support;

use std::cell::RefCell;
use std::fs;
use std::path::Path;

use petlauncher_core::config::Browser;
use petlauncher_core::{Action, Desktop, DesktopError, FolderOpened, SystemCommand};
use petlauncher_gnome::{execute, Executed, GnomeDesktop};
use support::{IsolatedEnv, Reply};

// ------------------------------------------------ in this process

/// A desktop that records the calls and answers with `folder`.
struct FakeDesktop {
    calls: RefCell<Vec<String>>,
    folder: Result<FolderOpened, DesktopError>,
}

impl FakeDesktop {
    fn new(folder: Result<FolderOpened, DesktopError>) -> Self {
        Self {
            calls: RefCell::new(Vec::new()),
            folder,
        }
    }
}

impl Desktop for FakeDesktop {
    fn open_folder(&self, folder: &Path) -> Result<FolderOpened, DesktopError> {
        self.calls
            .borrow_mut()
            .push(format!("open {}", folder.display()));
        self.folder.clone()
    }
    fn show_in_folder(&self, item: &Path) -> Result<FolderOpened, DesktopError> {
        self.calls
            .borrow_mut()
            .push(format!("show {}", item.display()));
        self.folder.clone()
    }
    fn run_system_command(&self, command: SystemCommand) -> Result<(), DesktopError> {
        self.calls.borrow_mut().push(command.title().to_owned());
        Ok(())
    }
    fn global_key(&self) -> Result<Option<String>, DesktopError> {
        unreachable!()
    }
    fn set_global_key(&self, _: &str, _: &str) -> Result<(), DesktopError> {
        unreachable!()
    }
    fn center_window(&self) -> Result<(), DesktopError> {
        unreachable!()
    }
    fn set_tray_icon_visible(&self, _: bool) -> Result<(), DesktopError> {
        unreachable!()
    }
}

#[test]
fn the_actions_of_the_program_are_not_handled_here() {
    let desktop = FakeDesktop::new(Ok(FolderOpened::FileManager));

    for action in [
        Action::Copy("125.6".into()),
        Action::SetQuery("~/a/".into()),
        Action::OpenDictionary("launcher".into()),
    ] {
        assert_eq!(
            execute(&action, &desktop, None),
            Ok(Executed::NotHandled),
            "{action:?}"
        );
    }
    assert!(desktop.calls.borrow().is_empty());
}

#[test]
fn nothing_does_nothing() {
    let desktop = FakeDesktop::new(Ok(FolderOpened::FileManager));

    assert_eq!(
        execute(&Action::Nothing, &desktop, None),
        Ok(Executed::Done)
    );
    assert!(desktop.calls.borrow().is_empty());
}

#[test]
fn the_folder_and_system_actions_go_to_the_desktop() {
    let desktop = FakeDesktop::new(Ok(FolderOpened::FileManager));

    for action in [
        Action::OpenFolder("/a".into()),
        Action::ShowInFolder("/a/b".into()),
        Action::System(SystemCommand::Suspend),
    ] {
        assert_eq!(execute(&action, &desktop, None), Ok(Executed::Done));
    }

    assert_eq!(*desktop.calls.borrow(), ["open /a", "show /a/b", "Suspend"]);
}

#[test]
fn a_fallback_of_the_desktop_is_reported_as_a_fallback() {
    let desktop = FakeDesktop::new(Ok(FolderOpened::Fallback {
        reason: "no file manager".into(),
    }));

    let executed = execute(&Action::ShowInFolder("/a/b".into()), &desktop, None);

    assert_eq!(
        executed,
        Ok(Executed::Fallback {
            reason: "no file manager".into()
        })
    );
}

#[test]
fn an_error_of_the_desktop_is_an_error() {
    let desktop = FakeDesktop::new(Err(DesktopError("broken".into())));

    let executed = execute(&Action::OpenFolder("/a".into()), &desktop, None);

    assert_eq!(executed, Err(DesktopError("broken".into())));
}

// ------------------------------------------- isolated child process

const URL: &str = "https://example.com/search?q=a%20b";

fn run(env: &IsolatedEnv, scenario: &str, extra: &[(&str, &str)]) -> String {
    let status = env.run_child("isolated_child", scenario, extra);
    let output = env.output(scenario);
    assert!(status.success(), "{output}");
    output
}

fn sorted(mut lines: Vec<String>) -> Vec<String> {
    lines.sort();
    lines
}

#[test]
fn open_file_launch_desktop_entry_and_open_url_start_the_right_applications() {
    let env = IsolatedEnv::new();
    let folder = env.path("home/notes and #1");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("100% ä.txt"), "text").unwrap();

    let output = run(&env, "launch", &[]);

    for label in ["file", "entry", "default", "app", "command"] {
        assert!(output.contains(&format!("{label}: Ok(Done)")), "{output}");
    }
    // GIO gives the local path of a file:// URI to %U, so the path shows
    // that the URI had the correct encoding.
    let file = env.path("home/notes and #1/100% ä.txt");
    assert_eq!(
        sorted(env.wait_for_calls(5)),
        sorted(vec![
            "app".to_owned(),
            format!("command [100% sure] [two words] [{URL}]"),
            format!("default-browser [{URL}]"),
            format!("edge [{URL}]"),
            format!("text [{}]", file.display()),
        ])
    );
}

#[test]
fn run_script_gives_the_argument_as_1_in_the_home_directory_and_survives_the_launcher() {
    let env = IsolatedEnv::new();

    run(&env, "script", &[]);

    // The launcher process has stopped now. The script sleeps 2 s first.
    let out = env.path("script.txt");
    let early = fs::read_to_string(&out).unwrap_or_default();
    assert!(!early.contains("survived"), "{early}");
    assert_eq!(
        support::wait_for_lines(&out, 4),
        [
            format!("cwd {}", env.path("home").display()),
            "argument [a b; $HOME 'quoted' 100%s]".to_owned(),
            "own process group".to_owned(),
            "survived".to_owned(),
        ]
    );
}

#[test]
fn when_the_file_manager_fails_the_default_application_opens_the_folder() {
    let env = IsolatedEnv::new();
    fs::create_dir_all(env.path("home/a b")).unwrap();
    let file_manager = support::file_manager(&env.bus, Reply::Error);

    let output = run(&env, "file-manager", &[]);

    assert!(
        output.contains("open: Ok(Fallback { reason: \"org.freedesktop.FileManager1.ShowFolders: "),
        "{output}"
    );
    assert!(output.contains("fails on purpose"), "{output}");
    assert!(
        output.contains("show: Ok(Fallback { reason: \"org.freedesktop.FileManager1.ShowItems: "),
        "{output}"
    );
    assert_eq!(file_manager.calls().len(), 2);
    let folder = format!("files [{}]", env.path("home/a b").display());
    assert_eq!(env.wait_for_calls(2), [folder.clone(), folder]);
}

#[test]
fn when_no_service_has_the_file_manager_name_the_default_application_opens_the_folder() {
    let env = IsolatedEnv::new();
    fs::create_dir_all(env.path("home/a b")).unwrap();

    let output = run(&env, "file-manager", &[]);

    assert!(output.contains("ServiceUnknown"), "{output}");
    assert_eq!(env.wait_for_calls(2).len(), 2);
}

#[test]
fn an_action_that_cannot_start_is_an_error_with_the_cause() {
    let env = IsolatedEnv::new();
    fs::create_dir_all(env.path("home/a b")).unwrap();
    fs::write(env.path("home/a b/x.txt"), "text").unwrap();

    // No applications and no default applications.
    let output = run(
        &env,
        "errors",
        &[
            ("XDG_DATA_HOME", env.path("no-data").to_str().unwrap()),
            ("XDG_CONFIG_HOME", env.path("no-config").to_str().unwrap()),
        ],
    );

    for expected in [
        "missing browser: Err(DesktopError(\"The browser petlauncher-missing.desktop is not installed.\"))",
        "missing entry: Err(DesktopError(\"",
        "empty command: Err(DesktopError(\"The browser command \\\"\\\" is not correct: ",
        "unclosed quote: Err(DesktopError(\"The browser command \\\"'unclosed\\\" is not correct: ",
        "missing program: Err(DesktopError(\"",
        "no file handler: Err(DesktopError(\"Could not open ",
        "no folder handler: Err(DesktopError(\"Could not open file://",
    ] {
        assert!(output.contains(expected), "{expected}\n{output}");
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(!env.path("calls.txt").exists(), "nothing started");
}

/// The child side of the tests above. It does nothing outside `run_child`.
#[test]
#[ignore = "runs only in the isolated child process of another test"]
fn isolated_child() {
    let Some((scenario, root)) = support::child_scenario() else {
        return;
    };
    let desktop = GnomeDesktop::new().expect("the private session bus");
    let home = root.join("home");
    let result = |label: &str, action: Action| {
        println!("{label}: {:?}", desktop.execute(&action));
    };
    let url = || URL.to_owned();
    match scenario.as_str() {
        "launch" => {
            result(
                "file",
                Action::OpenFile(home.join("notes and #1/100% ä.txt")),
            );
            result(
                "entry",
                Action::LaunchDesktopEntry(
                    root.join("data/applications/petlauncher-test-app.desktop"),
                ),
            );
            result(
                "default",
                Action::OpenUrl {
                    url: url(),
                    browser: None,
                },
            );
            result(
                "app",
                Action::OpenUrl {
                    url: url(),
                    browser: Some(Browser::App("petlauncher-test-edge.desktop".into())),
                },
            );
            result(
                "command",
                Action::OpenUrl {
                    url: url(),
                    browser: Some(Browser::Command(format!(
                        "'{}' command '100% sure' \"two words\"",
                        root.join("bin/recorder").display()
                    ))),
                },
            );
        }
        "script" => {
            let out = root.join("script.txt");
            let script = format!(
                "out='{}'\n\
                 printf 'cwd %s\\n' \"$(pwd)\" > \"$out\"\n\
                 printf 'argument [%s]\\n' \"$1\" >> \"$out\"\n\
                 [ \"$(ps -o pgid= $$ | tr -d ' ')\" = \"$$\" ] && echo 'own process group' >> \"$out\"\n\
                 sleep 2\n\
                 echo survived >> \"$out\"\n",
                out.display()
            );
            result(
                "script",
                Action::RunScript {
                    script,
                    argument: "a b; $HOME 'quoted' 100%s".into(),
                },
            );
        }
        "file-manager" => {
            result("open", Action::OpenFolder(home.join("a b")));
            result("show", Action::ShowInFolder(home.join("a b/x.txt")));
        }
        "errors" => {
            let browser = |browser: Browser| Action::OpenUrl {
                url: url(),
                browser: Some(browser),
            };
            result(
                "missing browser",
                browser(Browser::App("petlauncher-missing.desktop".into())),
            );
            result(
                "missing entry",
                Action::LaunchDesktopEntry(root.join("missing.desktop")),
            );
            result("empty command", browser(Browser::Command(String::new())));
            result(
                "unclosed quote",
                browser(Browser::Command("'unclosed".into())),
            );
            result(
                "missing program",
                browser(Browser::Command("petlauncher-no-such-program".into())),
            );
            result("no file handler", Action::OpenFile(home.join("a b/x.txt")));
            result("no folder handler", Action::OpenFolder(home.join("a b")));
        }
        other => panic!("unknown scenario {other}"),
    }
}
