//! Match and rank with a fixture of realistic entries from an Ubuntu 24.04
//! GNOME desktop.

mod common;

use common::{app, Fixture};
use petlauncher_apps::{gnome, Applications, EntryKind, UsageCounts};

const ENTRIES: &[(&str, &str, &str, &str)] = &[
    (
        "org.gnome.Terminal.desktop",
        "Terminal",
        "gnome-terminal",
        "Keywords=shell;prompt;command;commandline;cmd;\n",
    ),
    (
        "debian-xterm.desktop",
        "XTerm",
        "xterm",
        "Keywords=shell;\n",
    ),
    (
        "dev.kukka.Kukka.desktop",
        "kukka",
        "kukka",
        "Keywords=shell;\n",
    ),
    (
        "org.gnome.TextEditor.desktop",
        "Text Editor",
        "gnome-text-editor %U",
        "GenericName=Text Editor\nKeywords=write;notepad;\n",
    ),
    (
        "org.gnome.Nautilus.desktop",
        "Files",
        "nautilus --new-window %U",
        "Keywords=folder;manager;explore;disk;filesystem;\n",
    ),
    (
        "org.gnome.Calculator.desktop",
        "Calculator",
        "gnome-calculator",
        "Keywords=calculation;arithmetic;scientific;financial;\n",
    ),
    (
        "org.gnome.Characters.desktop",
        "Characters",
        "gnome-characters",
        "Keywords=characters;unicode;punctuation;emoji;symbols;\n",
    ),
    (
        "org.gnome.Calendar.desktop",
        "Calendar",
        "gnome-calendar",
        "Name[fi]=Päivyri\n",
    ),
    (
        "org.gnome.SystemMonitor.desktop",
        "System Monitor",
        "gnome-system-monitor",
        "Keywords=Monitor;System;Process;CPU;Memory;Task;Manager;\n",
    ),
    (
        "org.kde.plasma-systemmonitor.desktop",
        "System Monitor",
        "plasma-systemmonitor",
        "GenericName=System Monitor\nKeywords=task;manager;process;\n",
    ),
    (
        "org.gnome.Settings.desktop",
        "Settings",
        "gnome-control-center",
        "OnlyShowIn=GNOME;\n",
    ),
    (
        "firefox_firefox.desktop",
        "Firefox",
        "firefox %u",
        "GenericName=Web Browser\nCategories=Network;WebBrowser;\n",
    ),
    (
        "google-chrome.desktop",
        "Google Chrome",
        "google-chrome-stable %U",
        "GenericName=Web Browser\nCategories=Network;WebBrowser;\n",
    ),
    (
        "microsoft-edge.desktop",
        "Microsoft Edge",
        "microsoft-edge-stable %U",
        "GenericName=Web Browser\nCategories=Network;WebBrowser;\n",
    ),
    (
        "msedge-teams.desktop",
        "Microsoft Teams (PWA)",
        "microsoft-edge --app-id=teams",
        "",
    ),
    (
        "com.microsoft.VSCode.desktop",
        "Visual Studio Code",
        "code %F",
        "GenericName=Text Editor\nKeywords=vscode;\n",
    ),
    (
        "chatgpt.desktop",
        "ChatGPT",
        "env CODEX_HOME=/x chatgpt %U",
        "",
    ),
    ("slack.desktop", "Slack", "slack %U", ""),
    (
        "engineering-ai-suite.desktop",
        "SLAI Suite",
        "slai-suite",
        "",
    ),
    (
        "gnome-wifi-panel.desktop",
        "Wi-Fi",
        "gnome-control-center wifi",
        "NoDisplay=true\nKeywords=Network;Wireless;Wi-Fi;Wifi;IP;LAN;DNS;Hotspot;\n",
    ),
    (
        "gnome-sound-panel.desktop",
        "Sound",
        "gnome-control-center sound",
        "NoDisplay=true\nKeywords=Card;Microphone;Volume;Balance;Bluetooth;Headset;Audio;\n",
    ),
    (
        "gnome-bluetooth-panel.desktop",
        "Bluetooth",
        "gnome-control-center bluetooth",
        "NoDisplay=true\nKeywords=share;sharing;bluetooth;obex;\n",
    ),
];

fn load(locale: Option<&str>) -> (Fixture, Applications) {
    let fixture = Fixture::new();
    for (id, name, exec, extra) in ENTRIES {
        let program = exec
            .split(' ')
            .find(|arg| *arg != "env" && !arg.contains('='))
            .unwrap();
        fixture.program(program);
        fixture.program("env");
        fixture.write(&fixture.system(), id, &app(name, exec, extra));
    }
    let mut env = fixture.env();
    env.locale = locale.map(String::from);
    let apps = Applications::load(env, gnome::is_settings_page);
    assert_eq!(apps.entries().len(), ENTRIES.len(), "{:?}", apps.errors());
    (fixture, apps)
}

fn names(apps: &Applications, text: &str, usage: &UsageCounts) -> Vec<String> {
    apps.search(text, 10, usage)
        .iter()
        .map(|hit| hit.entry.name.clone())
        .collect()
}

fn usage(counts: &[(&str, u32)]) -> UsageCounts {
    counts
        .iter()
        .map(|(id, count)| (id.to_string(), *count))
        .collect()
}

#[test]
fn puts_names_that_start_with_the_text_before_names_with_a_later_word() {
    let (_fixture, apps) = load(None);

    let result = names(&apps, "te", &UsageCounts::new());

    assert_eq!(
        result,
        [
            "Terminal",
            "Text Editor",
            "Microsoft Teams (PWA)",
            "Visual Studio Code"
        ]
    );
}

#[test]
fn puts_the_entry_that_the_owner_starts_more_first_in_the_same_tier() {
    let (_fixture, apps) = load(None);
    let usage = usage(&[
        ("org.gnome.TextEditor.desktop", 3),
        ("org.gnome.Terminal.desktop", 1),
    ]);

    let result = names(&apps, "te", &usage);

    assert_eq!(result[..2], ["Text Editor", "Terminal"]);
}

#[test]
fn lets_a_frequent_word_match_pass_names_that_start_with_the_text() {
    let (_fixture, apps) = load(None);
    let without_usage = names(&apps, "ch", &UsageCounts::new());
    let usage = usage(&[("google-chrome.desktop", 20)]);

    let with_usage = names(&apps, "ch", &usage);

    assert_eq!(without_usage, ["ChatGPT", "Characters", "Google Chrome"]);
    assert_eq!(with_usage, ["Google Chrome", "ChatGPT", "Characters"]);
}

#[test]
fn does_not_let_usage_lift_a_match_inside_a_word_above_a_name_start() {
    let (_fixture, apps) = load(None);
    let usage = usage(&[("org.gnome.Characters.desktop", 100_000)]);

    let result = names(&apps, "ter", &usage);

    assert_eq!(result, ["Terminal", "Characters", "XTerm"]);
}

#[test]
fn finds_google_chrome_by_its_initials() {
    let (_fixture, apps) = load(None);

    assert_eq!(names(&apps, "gc", &UsageCounts::new())[0], "Google Chrome");
}

#[test]
fn finds_an_entry_by_a_keyword_only() {
    let (_fixture, apps) = load(None);

    assert_eq!(names(&apps, "prompt", &UsageCounts::new()), ["Terminal"]);
}

#[test]
fn orders_equal_scores_by_the_length_and_then_the_letters_of_the_name_without_case() {
    let (_fixture, apps) = load(None);

    let result = names(&apps, "shell", &UsageCounts::new());

    assert_eq!(result, ["kukka", "XTerm", "Terminal"]);
}

#[test]
fn does_not_find_text_that_crosses_a_word_boundary_of_the_name() {
    let (_fixture, apps) = load(None);

    assert!(
        names(&apps, "alst", &UsageCounts::new()).is_empty(),
        "Visual Studio Code"
    );
    assert!(
        names(&apps, "emmon", &UsageCounts::new()).is_empty(),
        "System Monitor"
    );
}

#[test]
fn finds_an_entry_by_its_program_name() {
    let (_fixture, apps) = load(None);

    assert_eq!(names(&apps, "nautilus", &UsageCounts::new()), ["Files"]);
}

#[test]
fn finds_a_name_with_more_than_one_word_from_a_later_word_or_from_parts_of_words() {
    let (_fixture, apps) = load(None);

    assert_eq!(
        names(&apps, "studio code", &UsageCounts::new()),
        ["Visual Studio Code"]
    );
    assert_eq!(
        names(&apps, "vis code", &UsageCounts::new()),
        ["Visual Studio Code"]
    );
}

#[test]
fn finds_a_settings_page_first_by_its_name_without_punctuation() {
    let (_fixture, apps) = load(None);

    let hits = apps.search("wifi", 10, &UsageCounts::new());

    assert_eq!(hits[0].entry.id, "gnome-wifi-panel.desktop");
    assert_eq!(hits[0].entry.kind, EntryKind::SettingsPage);
}

#[test]
fn finds_a_settings_page_by_a_keyword() {
    let (_fixture, apps) = load(None);

    assert_eq!(names(&apps, "volume", &UsageCounts::new()), ["Sound"]);
}

#[test]
fn does_not_match_the_settings_program_of_the_settings_pages() {
    let (_fixture, apps) = load(None);

    assert_eq!(
        names(&apps, "gnome-control", &UsageCounts::new()),
        ["Settings"]
    );
}

#[test]
fn finds_a_name_with_one_typing_error() {
    let (_fixture, apps) = load(None);

    assert_eq!(names(&apps, "termnial", &UsageCounts::new()), ["Terminal"]);
    assert_eq!(names(&apps, "firefx", &UsageCounts::new()), ["Firefox"]);
}

#[test]
fn gives_typing_error_results_only_when_nothing_matches_without_an_error() {
    let (_fixture, apps) = load(None);

    let result = names(&apps, "term", &UsageCounts::new());

    assert_eq!(result, ["Terminal", "XTerm"], "not Microsoft Teams (PWA)");
}

#[test]
fn ignores_case_and_accents() {
    let (_fixture, apps) = load(Some("fi_FI.UTF-8"));

    assert_eq!(names(&apps, "TERMINAL", &UsageCounts::new()), ["Terminal"]);
    assert_eq!(names(&apps, "paivyri", &UsageCounts::new()), ["Päivyri"]);
    assert_eq!(names(&apps, "PÄIV", &UsageCounts::new()), ["Päivyri"]);
}

#[test]
fn orders_entries_with_the_same_name_by_id() {
    let (_fixture, apps) = load(None);

    let ids: Vec<&str> = apps
        .search("system mon", 10, &UsageCounts::new())
        .iter()
        .map(|hit| hit.entry.id.as_str())
        .collect();

    assert_eq!(
        ids,
        [
            "org.gnome.SystemMonitor.desktop",
            "org.kde.plasma-systemmonitor.desktop"
        ]
    );
}

#[test]
fn gives_at_most_the_limit_of_results() {
    let (_fixture, apps) = load(None);

    assert_eq!(apps.search("s", 3, &UsageCounts::new()).len(), 3);
}

#[test]
fn gives_no_results_for_an_empty_text_or_a_text_with_no_match() {
    let (_fixture, apps) = load(None);

    assert!(apps.search("", 10, &UsageCounts::new()).is_empty());
    assert!(apps.search("  ", 10, &UsageCounts::new()).is_empty());
    assert!(apps.search("xq", 10, &UsageCounts::new()).is_empty());
}

#[test]
fn gives_a_higher_score_to_a_better_result() {
    let (_fixture, apps) = load(None);

    let hits = apps.search("te", 10, &UsageCounts::new());

    assert!(hits.windows(2).all(|pair| pair[0].score >= pair[1].score));
    assert!(hits[0].score > hits[2].score);
}
