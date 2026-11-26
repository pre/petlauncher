//! A `Launcher` on temporary directories. `tempfile` makes them in
//! `TMPDIR`, so run the tests with `TMPDIR=$HOME/tmp`. No test reads the
//! real home directory, the real settings or the network.

#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use petlauncher_calc::RatesError;
use petlauncher_core::{CancelToken, ManualClock, Results};
use petlauncher_providers::{Dirs, Environment, Hooks, Launcher, Update};
use petlauncher_time::jiff::tz::TimeZone;
use petlauncher_time::jiff::ZonedDateTime;
use tempfile::TempDir;

/// The fixed time of the tests, in the owner's time zone.
pub const NOW: &str = "2026-11-26T18:04[Europe/Helsinki]";

pub const PALETTE_ZOOM: &str =
    "daybook-evidence/kukka/font-size-keys-palette/screenshots-final/palette-zoom.png";

/// Rates of the day before `NOW`.
pub const OLD_RATES_DATE: &str = "2026-11-25";

pub struct Fixture {
    _root: TempDir,
    pub root: PathBuf,
    pub dirs: Dirs,
    pub clock: Arc<ManualClock>,
    pub fetch: Arc<FakeFetch>,
    pub updates: Mutex<Receiver<Update>>,
    update_sender: Mutex<Option<mpsc::Sender<Update>>>,
}

impl Fixture {
    pub fn new() -> Self {
        let root_dir = tempfile::Builder::new()
            .prefix("petlauncher-providers-")
            .tempdir()
            .expect("make a temporary directory");
        let root = root_dir.path().to_owned();
        let home = root.join("home");
        let dirs = Dirs {
            config: home.join(".config/petlauncher"),
            cache: home.join(".cache/petlauncher"),
            state: home.join(".local/state/petlauncher"),
            applications: Environment {
                data_dirs: vec![root.join("data")],
                current_desktops: vec!["GNOME".to_owned()],
                locale: None,
                path: vec![root.join("bin")],
            },
            zoneinfo: root.join("zoneinfo"),
            city_list: root.join("Locations.xml"),
            home,
        };
        fs::create_dir_all(&dirs.config).unwrap();
        let (sender, receiver) = mpsc::channel();
        let fixture = Self {
            _root: root_dir,
            root,
            dirs,
            clock: Arc::new(ManualClock::new(at(NOW))),
            fetch: Arc::new(FakeFetch::offline()),
            updates: Mutex::new(receiver),
            update_sender: Mutex::new(Some(sender)),
        };
        fixture.write_applications();
        fixture.write_cities();
        write_wordnet(&fixture.wordnet_dir());
        // The tests must not read real WordNet data in /usr/share/wordnet.
        fixture.write_settings(&format!(
            "[dictionary]\nwordnet_path = \"{}\"\n",
            fixture.wordnet_dir().display()
        ));
        fixture.write_rates(OLD_RATES_DATE, "1.0512");
        fixture.write_file_tree();
        fixture
    }

    pub fn home(&self, relative: &str) -> PathBuf {
        self.dirs.home.join(relative)
    }

    pub fn wordnet_dir(&self) -> PathBuf {
        self.root.join("wordnet")
    }

    pub fn desktop_file(&self, id: &str) -> PathBuf {
        self.root.join("data/applications").join(id)
    }

    /// The hooks of the tests: the fixed clock, Helsinki, the fake fetch,
    /// the GNOME settings page rule, no autostart file and no settings
    /// watch.
    pub fn hooks(&self) -> Hooks {
        let sender = self.update_sender.lock().unwrap().clone().unwrap();
        let sender = Mutex::new(sender);
        let mut hooks = Hooks::new(petlauncher_apps::gnome::is_settings_page, move |update| {
            let _ = sender.lock().unwrap().send(update);
        });
        hooks.clock = self.clock.clone();
        hooks.time_zone = TimeZone::get("Europe/Helsinki").unwrap();
        let fetch = Arc::clone(&self.fetch);
        hooks.fetch_rates = Arc::new(move || fetch.call());
        hooks.watch_config = false;
        hooks.outside_ignore_files = false;
        hooks
    }

    /// Starts a `Launcher` with `hooks()` and waits for its background work.
    pub fn start(&self) -> Launcher {
        self.start_with(self.hooks())
    }

    pub fn start_with(&self, hooks: Hooks) -> Launcher {
        let launcher = Launcher::start(self.dirs.clone(), hooks).expect("start the launcher");
        launcher.wait_idle();
        launcher
    }

    /// Waits for an update that `accept` accepts, at most 10 seconds.
    pub fn wait_for_update(&self, accept: impl Fn(Update) -> bool) -> Option<Update> {
        let updates = self.updates.lock().unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            match updates.recv_timeout(left) {
                Ok(update) if accept(update) => return Some(update),
                Ok(_) => {}
                Err(_) => return None,
            }
        }
    }

    pub fn received_updates(&self) -> Vec<Update> {
        self.updates.lock().unwrap().try_iter().collect()
    }

    pub fn write_settings(&self, text: &str) {
        fs::write(self.dirs.config.join("settings.toml"), text).unwrap();
    }

    pub fn write_shortcuts(&self, text: &str) {
        fs::write(self.dirs.config.join("shortcuts.toml"), text).unwrap();
    }

    pub fn write_rates(&self, date: &str, usd: &str) {
        fs::create_dir_all(&self.dirs.cache).unwrap();
        fs::write(
            self.dirs.cache.join(petlauncher_calc::CACHE_FILE),
            ecb_xml(date, usd),
        )
        .unwrap();
    }

    pub fn file(&self, relative: &str) -> PathBuf {
        let path = self.home(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, relative).unwrap();
        path
    }

    fn write_file_tree(&self) {
        for relative in [
            PALETTE_ZOOM,
            "daybook-evidence/kukka/font-size-keys-palette/screenshots-final/palette-small.png",
            "tmp/palette-zoom-in-tmp.png",
            "go/pkg/mod/palette-zoom-in-go.png",
            "dev/project/README.md",
            "comfy-output/H3-IMAGE/image-001.png",
        ] {
            self.file(relative);
        }
    }

    fn write_applications(&self) {
        let bin = self.root.join("bin");
        fs::create_dir_all(&bin).unwrap();
        for program in [
            "firefox",
            "nautilus",
            "microsoft-edge",
            "gnome-control-center",
            "clocks",
        ] {
            let path = bin.join(program);
            fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let applications = self.root.join("data/applications");
        fs::create_dir_all(&applications).unwrap();
        for (id, text) in [
            (
                "firefox.desktop",
                "Name=Firefox\nComment=Browse the web\nExec=firefox %u\nIcon=firefox\n\
                 Categories=Network;WebBrowser;\nMimeType=x-scheme-handler/https;\n",
            ),
            (
                "microsoft-edge.desktop",
                "Name=Microsoft Edge\nExec=microsoft-edge %U\nIcon=microsoft-edge\n\
                 Categories=Network;WebBrowser;\n",
            ),
            (
                "org.example.Files.desktop",
                "Name=Files\nGenericName=File Manager\nExec=nautilus --new-window %U\n\
                 Icon=/opt/icons/files.svg\n",
            ),
            (
                "gnome-wifi-panel.desktop",
                "Name=Wi-Fi\nComment=Control the wireless connections\n\
                 Exec=gnome-control-center wifi\nKeywords=wireless;network;\n\
                 Icon=network-wireless\nNoDisplay=true\n",
            ),
            (
                "org.example.Clocks.desktop",
                "Name=Clocks\nComment=Clocks for world times\nExec=clocks\n",
            ),
            (
                "gnome-sound-panel.desktop",
                "Name=Sound\nExec=gnome-control-center sound\nNoDisplay=true\n",
            ),
        ] {
            fs::write(
                applications.join(id),
                format!("[Desktop Entry]\nType=Application\n{text}"),
            )
            .unwrap();
        }
    }

    fn write_cities(&self) {
        fs::create_dir_all(&self.dirs.zoneinfo).unwrap();
        fs::write(
            self.dirs.zoneinfo.join("zone.tab"),
            "FI\t+6010+02458\tEurope/Helsinki\n\
             US\t+404251-0740023\tAmerica/New_York\tEastern (most areas)\n\
             US\t+340308-1181434\tAmerica/Los_Angeles\tPacific\n",
        )
        .unwrap();
        fs::write(
            self.dirs.zoneinfo.join("iso3166.tab"),
            "FI\tFinland\nUS\tUnited States\n",
        )
        .unwrap();
        fs::write(
            &self.dirs.city_list,
            r#"<?xml version="1.0" encoding="utf-8"?>
<gweather format="1.0">
  <region>
    <_name>North America</_name>
    <country>
      <_name>United States</_name>
      <iso-code>US</iso-code>
      <tz-hint>America/New_York</tz-hint>
      <state>
        <_name>Nevada</_name>
        <tz-hint>America/Los_Angeles</tz-hint>
        <city>
          <_name>Las Vegas</_name>
          <coordinates>36.174971 -115.137223</coordinates>
        </city>
      </state>
    </country>
  </region>
</gweather>
"#,
        )
        .unwrap();
    }
}

/// The download of the rates in the tests. It counts the calls and gives the
/// response that the test sets. The first response is "no network".
pub struct FakeFetch {
    pub calls: AtomicUsize,
    response: Mutex<Result<String, String>>,
}

impl FakeFetch {
    fn offline() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            response: Mutex::new(Err("no network".to_owned())),
        }
    }

    pub fn respond(&self, date: &str, usd: &str) {
        *self.response.lock().unwrap() = Ok(ecb_xml(date, usd));
    }

    pub fn fail(&self) {
        *self.response.lock().unwrap() = Err("no network".to_owned());
    }

    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn call(&self) -> Result<String, RatesError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.response
            .lock()
            .unwrap()
            .clone()
            .map_err(RatesError::Http)
    }
}

pub fn ecb_xml(date: &str, usd: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <gesmes:Envelope xmlns:gesmes=\"http://www.gesmes.org/xml/2002-08-01\" \
         xmlns=\"http://www.ecb.int/vocabulary/2002-08-01/eurofxref\">\n\
         <Cube><Cube time='{date}'><Cube currency='USD' rate='{usd}'/>\
         <Cube currency='GBP' rate='0.83125'/></Cube></Cube>\n\
         </gesmes:Envelope>\n"
    )
}

pub fn at(time: &str) -> SystemTime {
    let time: ZonedDateTime = time.parse().unwrap();
    SystemTime::try_from(time.timestamp()).unwrap()
}

pub fn search(launcher: &Launcher, query: &str) -> Results {
    launcher.search(query, &CancelToken::new())
}

pub fn titles(results: &Results) -> Vec<&str> {
    results.rows.iter().map(|row| row.title.as_str()).collect()
}

pub fn row_index(results: &Results, title: &str) -> usize {
    results
        .rows
        .iter()
        .position(|row| row.title == title)
        .unwrap_or_else(|| panic!("no row {title:?} in {:?}", titles(results)))
}

/// One synset of the WordNet fixture: the word class letter, the words and
/// the gloss.
type Synset = (char, &'static [&'static str], &'static str);

/// Invented meanings in the WordNet 3.0 file format. The data is not the
/// real WordNet data.
const SYNSETS: &[Synset] = &[
    (
        'n',
        &["launcher", "rocket_launcher"],
        "a device that fires rockets",
    ),
    (
        'n',
        &["catapult", "launcher"],
        "a machine that throws aircraft from a ship",
    ),
    ('n', &["launch"], "a small boat with a motor"),
    (
        'n',
        &["launch_pad", "launchpad"],
        "a platform from which a rocket goes up",
    ),
    (
        'n',
        &["launching"],
        "the act of sending a thing into the air",
    ),
    (
        'v',
        &["launch"],
        "send into the air with force; \"they launched a rocket\"",
    ),
    ('v', &["launch", "start_up"], "start a new thing"),
];

/// Writes the twelve files that `Dictionary::open` needs, with the byte
/// offsets that the format needs.
pub fn write_wordnet(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    for (letter, suffix) in [('n', "noun"), ('v', "verb"), ('a', "adj"), ('r', "adv")] {
        let mut data = String::new();
        let mut index: std::collections::BTreeMap<String, Vec<usize>> = Default::default();
        for (class, words, gloss) in SYNSETS.iter().filter(|(class, ..)| *class == letter) {
            let offset = data.len();
            let mut line = format!("{offset:08} 00 {class} {:02x}", words.len());
            for word in *words {
                line.push_str(&format!(" {word} 0"));
                index.entry(word.to_lowercase()).or_default().push(offset);
            }
            line.push_str(&format!(" 000 | {gloss}  \n"));
            data.push_str(&line);
        }
        let index: String = index
            .iter()
            .map(|(lemma, offsets)| {
                let offsets: Vec<String> = offsets.iter().map(|o| format!("{o:08}")).collect();
                format!(
                    "{lemma} {letter} {count} 0 {count} 0 {}  \n",
                    offsets.join(" "),
                    count = offsets.len()
                )
            })
            .collect();
        fs::write(dir.join(format!("data.{suffix}")), data).unwrap();
        fs::write(dir.join(format!("index.{suffix}")), index).unwrap();
        fs::write(dir.join(format!("{suffix}.exc")), "").unwrap();
    }
}

/// `settings.toml` with comments, for the tests that change it.
pub fn settings_with_comments(wordnet: &Path) -> String {
    format!(
        "# My settings.\n\
         \n\
         window_width = 900      # wider than the initial value\n\
         \n\
         [file_search]\n\
         keyword = \"f\"\n\
         directories = [\"~\"]\n\
         # Do not search these.\n\
         ignore = [\n\
         \x20   \"node_modules\",\n\
         \x20   \"~/go/pkg/mod\",\n\
         \x20   \"~/tmp\",   # the TMPDIR\n\
         ]\n\
         \n\
         [dictionary]\n\
         wordnet_path = \"{}\"\n",
        wordnet.display()
    )
}
