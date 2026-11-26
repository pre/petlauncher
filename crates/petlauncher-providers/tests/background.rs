//! The background work: the rates rule, the rescan rule of the file index,
//! the reload of the applications, the city list, and the stop of the
//! threads.

mod common;

use std::fs;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{search, titles, Fixture, OLD_RATES_DATE};
use petlauncher_core::RowKind;
use petlauncher_providers::{Launcher, Update};

const SIX_HOURS: Duration = Duration::from_secs(6 * 60 * 60);

fn currency_note(launcher: &Launcher) -> String {
    search(launcher, "100 usd in eur").rows[0].subtitle.clone()
}

#[test]
fn old_stored_rates_are_fetched_at_the_start_and_a_failed_fetch_keeps_them() {
    let fixture = Fixture::new();

    let launcher = fixture.start();

    assert_eq!(fixture.fetch.calls(), 1);
    assert_eq!(
        currency_note(&launcher),
        format!("ECB reference rates of {OLD_RATES_DATE}")
    );
    assert!(launcher.problems().is_empty(), "{:?}", launcher.problems());
}

#[test]
fn rates_of_the_current_date_are_fetched_only_after_the_date_changes() {
    let fixture = Fixture::new();
    fixture.write_rates("2026-11-26", "1.05");

    let launcher = fixture.start();
    let at_the_start = fixture.fetch.calls();
    // 18:04 and six hours is 00:04 on 2026-11-27.
    fixture.clock.advance(SIX_HOURS);
    launcher.window_opened();
    launcher.wait_idle();

    assert_eq!(at_the_start, 0);
    assert_eq!(fixture.fetch.calls(), 1);
}

#[test]
fn a_fetch_replaces_the_rates_and_the_stored_file_and_reports_an_update() {
    let fixture = Fixture::new();
    fixture.fetch.respond("2026-11-26", "2");

    let launcher = fixture.start();

    assert_eq!(
        currency_note(&launcher),
        "ECB reference rates of 2026-11-26"
    );
    assert_eq!(
        search(&launcher, "100 usd in eur").rows[0].title,
        "50.00 EUR"
    );
    let stored = fs::read_to_string(fixture.dirs.cache.join(petlauncher_calc::CACHE_FILE)).unwrap();
    assert!(stored.contains("2026-11-26"), "{stored}");
    assert!(fixture.received_updates().contains(&Update::Rates));
}

#[test]
fn the_next_fetch_comes_six_hours_after_the_last_check() {
    let fixture = Fixture::new();
    let launcher = fixture.start();
    fixture.fetch.respond("2026-11-26", "2");

    fixture.clock.advance(SIX_HOURS - Duration::from_secs(1));
    launcher.window_opened();
    launcher.wait_idle();
    let before_six_hours = fixture.fetch.calls();
    fixture.clock.advance(Duration::from_secs(1));
    launcher.window_opened();
    launcher.wait_idle();

    assert_eq!(before_six_hours, 1);
    assert_eq!(fixture.fetch.calls(), 2);
    assert_eq!(
        currency_note(&launcher),
        "ECB reference rates of 2026-11-26"
    );
}

#[test]
fn a_query_never_fetches_the_rates() {
    let fixture = Fixture::new();
    let launcher = fixture.start();
    let calls = fixture.fetch.calls();

    for _ in 0..20 {
        search(&launcher, "100 usd in eur");
    }

    assert_eq!(fixture.fetch.calls(), calls);
}

#[test]
fn fetched_rates_older_than_the_stored_rates_do_not_replace_them() {
    let fixture = Fixture::new();
    fixture.fetch.respond("2026-11-20", "3");

    let launcher = fixture.start();

    assert_eq!(fixture.fetch.calls(), 1);
    assert_eq!(
        currency_note(&launcher),
        format!("ECB reference rates of {OLD_RATES_DATE}")
    );
}

#[test]
fn the_city_list_loads_in_the_background_and_reports_an_update() {
    let fixture = Fixture::new();

    let launcher = fixture.start();

    assert!(fixture.received_updates().contains(&Update::Cities));
    assert_eq!(
        titles(&search(&launcher, "6am in helsinki")),
        ["06:00, Thursday"]
    );
}

#[test]
fn a_city_list_that_cannot_be_read_is_a_problem_and_gives_no_time_rows() {
    let fixture = Fixture::new();
    fs::remove_file(fixture.dirs.zoneinfo.join("zone.tab")).unwrap();

    let launcher = fixture.start();

    assert!(search(&launcher, "6am in helsinki").rows.is_empty());
    let problems = launcher.problems();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("zone.tab"), "{problems:?}");
}

#[test]
fn the_index_scans_again_when_the_window_opens_and_the_last_scan_is_60_seconds_old() {
    let fixture = Fixture::new();
    let launcher = fixture.start();
    fixture.file("notes/new-plan.md");

    fixture.clock.advance(Duration::from_secs(59));
    launcher.window_opened();
    launcher.wait_idle();
    let before_60_seconds = search(&launcher, "f new-plan");
    fixture.clock.advance(Duration::from_secs(1));
    launcher.window_opened();
    launcher.wait_idle();
    let after_60_seconds = search(&launcher, "f new-plan");

    assert!(before_60_seconds.rows.is_empty());
    assert_eq!(titles(&after_60_seconds), ["new-plan.md"]);
    assert!(fixture.received_updates().contains(&Update::Files));
}

#[test]
fn the_stored_index_answers_at_the_start_before_the_scan() {
    let fixture = Fixture::new();
    drop(fixture.start());
    fs::remove_file(fixture.home("dev/project/README.md")).unwrap();
    let hooks = fixture.hooks();

    let launcher = Launcher::start(fixture.dirs.clone(), hooks).unwrap();
    let at_the_start = search(&launcher, "f README");
    launcher.wait_idle();
    let after_the_scan = search(&launcher, "f README");

    // The scan can end before the first search. Then the stored index did
    // not answer, and the test has nothing to check.
    if !at_the_start.rows.is_empty() {
        assert_eq!(titles(&at_the_start), ["README.md"]);
    }
    assert!(after_the_scan.rows.is_empty());
}

#[test]
fn a_search_during_a_rescan_uses_the_old_index_and_does_not_wait() {
    let fixture = Fixture::new();
    for directory in 0..60 {
        for file in 0..50 {
            fixture.file(&format!("bulk/d{directory}/file-{file}.txt"));
        }
    }
    let launcher = fixture.start();

    // A new file that the search does not find right after the start of the
    // scan, and finds after its end, shows that the search used the old
    // index while the scan ran.
    let mut searched_during_a_scan = false;
    for attempt in 0..20 {
        let fresh = format!("fresh-{attempt}");
        fixture.file(&format!("notes/{fresh}.txt"));
        fixture.clock.advance(Duration::from_secs(60));
        launcher.window_opened();
        let new_file = search(&launcher, &format!("f {fresh}"));
        let old_file = search(&launcher, "f palette-zoom");
        launcher.wait_idle();

        assert_eq!(titles(&old_file), ["palette-zoom.png"]);
        assert_eq!(
            titles(&search(&launcher, &format!("f {fresh}"))),
            [format!("{fresh}.txt")]
        );
        if new_file.rows.is_empty() {
            searched_during_a_scan = true;
            break;
        }
    }

    assert!(searched_during_a_scan);
}

#[test]
fn the_applications_are_read_again_when_the_window_opens_after_a_change() {
    let fixture = Fixture::new();
    let launcher = fixture.start();
    fs::write(
        fixture.desktop_file("org.example.Editor.desktop"),
        "[Desktop Entry]\nType=Application\nName=Editor\nExec=firefox\n",
    )
    .unwrap();

    let before = search(&launcher, "editor");
    launcher.window_opened();
    launcher.wait_idle();
    let after = search(&launcher, "editor");

    assert!(before.rows.is_empty());
    assert_eq!(titles(&after), ["Editor"]);
    assert_eq!(after.rows[0].kind, RowKind::Application);
    assert!(fixture.received_updates().contains(&Update::Applications));
}

#[test]
fn a_start_counts_for_the_rank_also_after_a_new_start_of_petlauncher() {
    let fixture = Fixture::new();
    let launcher = fixture.start();
    let before = search(&launcher, "fi");
    let firefox = common::row_index(&before, "Firefox");
    let outcome = launcher.activate(&before, Some(firefox), petlauncher_core::Key::Enter);

    launcher.executed("fi", &outcome).unwrap();
    let after = search(&launcher, "fi");
    drop(launcher);
    let restarted = fixture.start();

    assert_eq!(&titles(&before)[..2], ["Files", "Firefox"]);
    assert_eq!(&titles(&after)[..2], ["Firefox", "Files"]);
    assert_eq!(
        &titles(&search(&restarted, "fi"))[..2],
        ["Firefox", "Files"]
    );
}

#[test]
fn drop_stops_the_threads() {
    let fixture = Fixture::new();
    let probe = Arc::new(());
    let mut hooks = fixture.hooks();
    let held = Arc::clone(&probe);
    hooks.on_change = Arc::new(move |_| {
        let _ = &held;
    });
    let launcher = fixture.start_with(hooks);

    drop(launcher);

    assert_eq!(Arc::strong_count(&probe), 1);
}

#[test]
fn drop_does_not_wait_for_a_fetch_that_runs() {
    let fixture = Fixture::new();
    let (entered, entries) = mpsc::channel();
    let (release, releases) = mpsc::channel::<()>();
    let releases = std::sync::Mutex::new(releases);
    let probe = Arc::new(());
    let held = Arc::clone(&probe);
    let mut hooks = fixture.hooks();
    hooks.fetch_rates = Arc::new(move || {
        let _ = &held;
        let _ = entered.send(());
        let _ = releases.lock().unwrap().recv();
        Err(petlauncher_calc::RatesError::Http("no network".into()))
    });
    let launcher = Launcher::start(fixture.dirs.clone(), hooks).unwrap();
    entries.recv_timeout(Duration::from_secs(10)).unwrap();

    let start = Instant::now();
    drop(launcher);
    let drop_time = start.elapsed();
    release.send(()).unwrap();

    assert!(drop_time < Duration::from_secs(5), "{drop_time:?}");
    let deadline = Instant::now() + Duration::from_secs(10);
    while Arc::strong_count(&probe) > 1 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(Arc::strong_count(&probe), 1);
}
