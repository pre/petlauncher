//! R9: the query stays for a set time after the window closes.

mod common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use common::{fake_applications, fake_calculator, search, TempHome};
use petlauncher_core::config::{KeepQuery, Settings};
use petlauncher_core::{
    activate, keep_query_warning, Clock, CloseReason, Engine, KeptQuery, KeptUntil, Key,
    ManualClock, Slot,
};

const SUM: &str = "42.3+68.7+14.1+0.5";

struct Launcher {
    clock: Arc<ManualClock>,
    kept: KeptQuery,
    settings: Settings,
}

impl Launcher {
    fn new() -> Self {
        let clock = Arc::new(ManualClock::new(SystemTime::UNIX_EPOCH));
        let kept = KeptQuery::new(clock.clone());
        Self {
            clock,
            kept,
            settings: Settings::default(),
        }
    }

    fn close(&mut self, query: &str, reason: CloseReason) {
        self.kept.window_closed(query, reason, &self.settings);
    }

    fn open_after(&mut self, seconds: u64) -> Option<String> {
        self.clock.advance(Duration::from_secs(seconds));
        self.kept.window_opened(&self.settings)
    }
}

fn engine(home: &TempHome) -> Engine {
    let mut engine = home.engine();
    engine.register(Slot::Calculator, fake_calculator);
    engine.register(
        Slot::Applications,
        fake_applications(home.dir("applications")),
    );
    engine
}

#[test]
fn r9_step1_and_2_after_a_focus_loss_the_query_and_its_result_come_back_in_30_seconds() {
    let home = TempHome::new();
    let engine = engine(&home);
    let mut launcher = Launcher::new();
    launcher.close(SUM, CloseReason::FocusLoss);

    let restored = launcher.open_after(29);
    let results = search(&engine, restored.as_deref().unwrap_or_default());

    assert_eq!(restored.as_deref(), Some(SUM));
    assert_eq!(results.rows[0].title, "125.6");
}

#[test]
fn r9_step3_after_more_than_30_seconds_the_query_line_is_empty() {
    let mut launcher = Launcher::new();
    launcher.close(SUM, CloseReason::FocusLoss);

    assert_eq!(launcher.open_after(31), None);
}

#[test]
fn r9_step4_enter_on_a_calculation_keeps_the_query() {
    let home = TempHome::new();
    let engine = engine(&home);
    let mut launcher = Launcher::new();
    let results = search(&engine, "42.3+68.7");
    let outcome = activate(&results, results.selected, Key::Enter);

    launcher.close(
        &results.query,
        CloseReason::Activated {
            keep_query: outcome.keep_query,
        },
    );

    assert!(outcome.close);
    assert_eq!(launcher.open_after(10).as_deref(), Some("42.3+68.7"));
}

#[test]
fn r9_step5_enter_on_an_application_clears_the_query() {
    let home = TempHome::new();
    let engine = engine(&home);
    let mut launcher = Launcher::new();
    let results = search(&engine, "firefox");
    let outcome = activate(&results, results.selected, Key::Enter);

    launcher.close(
        &results.query,
        CloseReason::Activated {
            keep_query: outcome.keep_query,
        },
    );

    assert!(outcome.close);
    assert_eq!(launcher.open_after(0), None);
}

#[test]
fn r9_step6_with_disable_the_query_line_is_always_empty() {
    let mut launcher = Launcher::new();
    launcher.settings.keep_query = KeepQuery::Disabled;
    launcher.close(SUM, CloseReason::FocusLoss);

    assert_eq!(launcher.open_after(0), None);
}

#[test]
fn r9_step7_with_forever_the_query_stays_after_more_than_30_seconds() {
    let mut launcher = Launcher::new();
    launcher.settings.keep_query = KeepQuery::Forever;
    launcher.close(SUM, CloseReason::FocusLoss);

    assert_eq!(launcher.open_after(3600 * 24).as_deref(), Some(SUM));
}

#[test]
fn r9_step8_with_run_in_background_off_the_settings_window_gets_a_warning() {
    let settings = Settings {
        run_in_background: false,
        ..Settings::default()
    };

    assert!(keep_query_warning(&settings).is_some_and(|text| text.contains("Run in background")));
    assert_eq!(keep_query_warning(&Settings::default()), None);
}

#[test]
fn r9_table_with_run_in_background_off_the_query_is_not_kept() {
    let mut launcher = Launcher::new();
    launcher.settings.run_in_background = false;
    launcher.close(SUM, CloseReason::FocusLoss);

    assert_eq!(launcher.open_after(0), None);
}

#[test]
fn r9_table_escape_and_the_global_key_keep_the_query() {
    for reason in [CloseReason::Escape, CloseReason::GlobalKey] {
        let mut launcher = Launcher::new();
        launcher.close(SUM, reason);

        assert_eq!(launcher.open_after(1).as_deref(), Some(SUM));
    }
}

#[test]
fn r9_table_a_typed_number_of_seconds_sets_the_time() {
    let mut launcher = Launcher::new();
    launcher.settings.keep_query = KeepQuery::Seconds(300);
    launcher.close(SUM, CloseReason::FocusLoss);

    assert_eq!(launcher.open_after(299).as_deref(), Some(SUM));
}

#[test]
fn r9_the_setting_at_the_open_decides() {
    let mut launcher = Launcher::new();
    launcher.close(SUM, CloseReason::FocusLoss);
    launcher.settings.keep_query = KeepQuery::Forever;

    assert_eq!(launcher.open_after(60).as_deref(), Some(SUM));
}

#[test]
fn r9_the_query_comes_back_one_time_only() {
    let mut launcher = Launcher::new();
    launcher.close(SUM, CloseReason::FocusLoss);

    assert_eq!(launcher.open_after(1).as_deref(), Some(SUM));
    assert_eq!(launcher.open_after(1), None);
}

#[test]
fn r9_an_empty_query_is_not_kept() {
    let mut launcher = Launcher::new();
    launcher.close("  ", CloseReason::Escape);

    assert_eq!(launcher.open_after(1), None);
}

/// Gives the times of the list, one for each call.
struct ListClock(Mutex<Vec<SystemTime>>);

impl Clock for ListClock {
    fn now(&self) -> SystemTime {
        self.0.lock().unwrap().remove(0)
    }
}

#[test]
fn r9_a_wall_clock_that_goes_back_does_not_clear_the_query() {
    let start = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let clock = ListClock(Mutex::new(vec![start, start - Duration::from_secs(10)]));
    let mut kept = KeptQuery::new(Arc::new(clock));
    let settings = Settings::default();
    kept.window_closed(SUM, CloseReason::FocusLoss, &settings);

    let restored = kept.window_opened(&settings);

    assert_eq!(restored.as_deref(), Some(SUM));
}

fn kept_until(launcher: &Launcher) -> KeptUntil {
    launcher.kept.kept_until(&launcher.settings)
}

#[test]
fn r9_kept_until_gives_the_end_of_the_keep_time_after_a_close() {
    let mut launcher = Launcher::new();
    launcher.clock.advance(Duration::from_secs(100));

    launcher.close(SUM, CloseReason::Escape);

    assert_eq!(
        kept_until(&launcher),
        KeptUntil::Time(SystemTime::UNIX_EPOCH + Duration::from_secs(130))
    );
}

#[test]
fn r9_kept_until_is_nothing_with_no_kept_query_and_after_the_open() {
    let mut launcher = Launcher::new();
    assert_eq!(kept_until(&launcher), KeptUntil::Nothing);

    launcher.close("", CloseReason::Escape);
    assert_eq!(kept_until(&launcher), KeptUntil::Nothing);

    launcher.close(SUM, CloseReason::Escape);
    launcher.open_after(1);
    assert_eq!(kept_until(&launcher), KeptUntil::Nothing);
}

#[test]
fn r9_kept_until_follows_the_settings_of_now() {
    let mut launcher = Launcher::new();
    launcher.close(SUM, CloseReason::Escape);

    launcher.settings.keep_query = KeepQuery::Forever;
    let forever = kept_until(&launcher);
    launcher.settings.keep_query = KeepQuery::Disabled;
    let disabled = kept_until(&launcher);
    launcher.settings.keep_query = KeepQuery::Seconds(30);
    launcher.settings.run_in_background = false;
    let no_background = kept_until(&launcher);

    assert_eq!(forever, KeptUntil::Forever);
    assert_eq!(disabled, KeptUntil::Nothing);
    assert_eq!(no_background, KeptUntil::Nothing);
}
