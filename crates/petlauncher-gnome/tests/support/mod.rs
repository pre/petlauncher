//! The test environment of `petlauncher-gnome`. No test reaches the session
//! of the owner:
//!
//! - D-Bus: a private `dbus-daemon` with no service directories, so that no
//!   call can start a real service such as Nautilus. Mock services on it
//!   record the calls.
//! - GSettings: the memory backend, in a process whose default backend is
//!   also the memory backend.
//! - Programs: a child process with its own `HOME`, XDG directories, session
//!   bus and GSettings backend, and no display. Its only applications are
//!   recorders that write their arguments to a file.

#![allow(dead_code)]

use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{mpsc, Arc, Mutex, Once, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use gio::prelude::*;

// ---------------------------------------------------------------- D-Bus

const BUS_CONFIG: &str = r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:path=SOCKET</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#;

/// A `dbus-daemon` of the test. Its configuration has no service
/// directories, so a call to a name with no owner fails and starts nothing.
pub struct PrivateBus {
    daemon: Child,
    address: String,
    _dir: tempfile::TempDir,
}

impl PrivateBus {
    pub fn start() -> Self {
        let dir = tempfile::tempdir().expect("temporary directory");
        let config = dir.path().join("bus.conf");
        let socket = dir.path().join("socket");
        fs::write(
            &config,
            BUS_CONFIG.replace("SOCKET", socket.to_str().expect("UTF-8 path")),
        )
        .expect("bus configuration");
        let mut daemon = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--print-address=1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon starts");
        let mut address = String::new();
        BufReader::new(daemon.stdout.take().expect("stdout"))
            .read_line(&mut address)
            .expect("bus address");
        let address = address.trim().to_owned();
        assert!(
            address.contains(dir.path().to_str().unwrap()),
            "the bus is the private bus: {address}"
        );
        Self {
            daemon,
            address,
            _dir: dir,
        }
    }

    pub fn address(&self) -> &str {
        &self.address
    }

    pub fn connect(&self) -> gio::DBusConnection {
        connect(&self.address)
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

fn connect(address: &str) -> gio::DBusConnection {
    gio::DBusConnection::for_address_sync(
        address,
        gio::DBusConnectionFlags::AUTHENTICATION_CLIENT
            | gio::DBusConnectionFlags::MESSAGE_BUS_CONNECTION,
        None,
        gio::Cancellable::NONE,
    )
    .expect("connection to the private bus")
}

/// What a mock service answers to each call.
#[derive(Debug, Clone, Copy)]
pub enum Reply {
    Empty,
    Error,
}

/// One interface on a private bus that records each method call as
/// `Method (signature) (arguments)`, for example
/// `Logout (u) (0,)`.
pub struct MockService {
    calls: Arc<Mutex<Vec<String>>>,
    main_loop: glib::MainLoop,
    thread: Option<JoinHandle<()>>,
}

impl MockService {
    /// Owns `name` and serves the first interface of `xml` at `path`. The
    /// service runs in its own thread, so a synchronous call of the code
    /// under test gets its reply.
    pub fn start(bus: &PrivateBus, name: &str, path: &str, xml: &str, reply: Reply) -> Self {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let (sender, receiver) = mpsc::channel();
        let (address, name, path, xml) = (
            bus.address.clone(),
            name.to_owned(),
            path.to_owned(),
            xml.to_owned(),
        );
        let recorded = calls.clone();
        let thread = std::thread::spawn(move || {
            let context = glib::MainContext::new();
            context
                .with_thread_default(|| {
                    let connection = connect(&address);
                    let node = gio::DBusNodeInfo::for_xml(&xml).expect("interface XML");
                    let interface = node.interfaces()[0].clone();
                    let _registration = connection
                        .register_object(&path, &interface)
                        .method_call(move |_, _, _, _, method, parameters, invocation| {
                            recorded.lock().unwrap().push(format!(
                                "{method} {} {}",
                                parameters.type_(),
                                parameters.print(false)
                            ));
                            match reply {
                                Reply::Empty => invocation.return_value(None),
                                Reply::Error => invocation.return_dbus_error(
                                    "org.freedesktop.DBus.Error.Failed",
                                    "the mock service fails on purpose",
                                ),
                            }
                        })
                        .build()
                        .expect("object registration");
                    request_name(&connection, &name);
                    let main_loop = glib::MainLoop::new(Some(&context), false);
                    sender.send(main_loop.clone()).unwrap();
                    main_loop.run();
                })
                .expect("thread default context");
        });
        let main_loop = receiver.recv().expect("the mock service starts");
        Self {
            calls,
            main_loop,
            thread: Some(thread),
        }
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl Drop for MockService {
    fn drop(&mut self) {
        self.main_loop.quit();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn request_name(connection: &gio::DBusConnection, name: &str) {
    const DO_NOT_QUEUE: u32 = 4;
    const PRIMARY_OWNER: u32 = 1;
    let reply = connection
        .call_sync(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "RequestName",
            Some(&(name, DO_NOT_QUEUE).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            -1,
            gio::Cancellable::NONE,
        )
        .expect("RequestName");
    assert_eq!(reply.get::<(u32,)>(), Some((PRIMARY_OWNER,)), "{name}");
}

/// The interfaces as `gdbus introspect` showed them on GNOME 46, with only
/// the methods that the crate calls.
pub const FILE_MANAGER_XML: &str = r#"<node><interface name="org.freedesktop.FileManager1">
  <method name="ShowFolders"><arg type="as" name="URIs" direction="in"/><arg type="s" name="StartupId" direction="in"/></method>
  <method name="ShowItems"><arg type="as" name="URIs" direction="in"/><arg type="s" name="StartupId" direction="in"/></method>
</interface></node>"#;

pub const SCREEN_SAVER_XML: &str = r#"<node><interface name="org.gnome.ScreenSaver">
  <method name="Lock"/>
</interface></node>"#;

pub const SESSION_MANAGER_XML: &str = r#"<node><interface name="org.gnome.SessionManager">
  <method name="Logout"><arg type="u" name="mode" direction="in"/></method>
  <method name="Reboot"/>
  <method name="Shutdown"/>
</interface></node>"#;

pub const LOGIN1_XML: &str = r#"<node><interface name="org.freedesktop.login1.Manager">
  <method name="Suspend"><arg type="b" name="interactive" direction="in"/></method>
</interface></node>"#;

pub fn file_manager(bus: &PrivateBus, reply: Reply) -> MockService {
    MockService::start(
        bus,
        "org.freedesktop.FileManager1",
        "/org/freedesktop/FileManager1",
        FILE_MANAGER_XML,
        reply,
    )
}

// ------------------------------------------------------------- GSettings

/// Makes the memory backend the default GSettings backend of this test
/// process, proves it, and gives a new memory backend. Each GSettings test
/// calls it first, before it makes a settings object, so that no code path
/// can reach the dconf database of the owner.
pub fn memory_backend() -> gio::SettingsBackend {
    static ISOLATE: Once = Once::new();
    ISOLATE.call_once(|| std::env::set_var("GSETTINGS_BACKEND", "memory"));
    assert_eq!(
        gio::SettingsBackend::default().type_().name(),
        "GMemorySettingsBackend",
        "the default GSettings backend of the test process"
    );
    let backend = gio::memory_settings_backend_new();
    assert_eq!(backend.type_().name(), "GMemorySettingsBackend");
    backend
}

/// The directory with the compiled test schemas of `tests/schemas`.
pub fn test_schema_dir() -> &'static Path {
    static DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = tempfile::tempdir().expect("temporary directory");
        let status = Command::new("glib-compile-schemas")
            .arg("--strict")
            .arg(format!("--targetdir={}", dir.path().display()))
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/schemas"))
            .status()
            .expect("glib-compile-schemas runs");
        assert!(status.success(), "glib-compile-schemas: {status}");
        dir
    })
    .path()
}

/// The test schemas only, with no fallback to the installed schemas.
pub fn test_schemas() -> gio::SettingsSchemaSource {
    gio::SettingsSchemaSource::from_directory(test_schema_dir(), None, true)
        .expect("compiled test schemas")
}

pub const MEDIA_KEYS: &str = "org.gnome.settings-daemon.plugins.media-keys";
pub const CUSTOM_KEYBINDING: &str =
    "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
pub const ULAUNCHER_PATH: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/ulauncher/";
pub const KUKKA_PATH: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/kukka-hotkey-window/";

/// Reads and writes the test backend directly, as GNOME Settings does.
pub struct SettingsFixture {
    pub backend: gio::SettingsBackend,
    pub source: gio::SettingsSchemaSource,
}

impl SettingsFixture {
    pub fn new() -> Self {
        Self {
            backend: memory_backend(),
            source: test_schemas(),
        }
    }

    pub fn list(&self) -> gio::Settings {
        let schema = self.source.lookup(MEDIA_KEYS, false).unwrap();
        gio::Settings::new_full(&schema, Some(&self.backend), None)
    }

    pub fn entry(&self, path: &str) -> gio::Settings {
        let schema = self.source.lookup(CUSTOM_KEYBINDING, false).unwrap();
        gio::Settings::new_full(&schema, Some(&self.backend), Some(path))
    }

    pub fn paths(&self) -> Vec<String> {
        self.list()
            .strv("custom-keybindings")
            .iter()
            .map(|path| path.to_string())
            .collect()
    }

    /// `name`, `command` and `binding` of the entry.
    pub fn read(&self, path: &str) -> [String; 3] {
        let entry = self.entry(path);
        ["name", "command", "binding"].map(|key| entry.string(key).to_string())
    }

    pub fn write(&self, path: &str, name: &str, command: &str, binding: &str) {
        let entry = self.entry(path);
        entry.set_string("name", name).unwrap();
        entry.set_string("command", command).unwrap();
        entry.set_string("binding", binding).unwrap();
    }

    pub fn set_paths(&self, paths: &[&str]) {
        self.list().set_strv("custom-keybindings", paths).unwrap();
    }

    /// The two custom shortcuts of the owner on 2026-11-26.
    pub fn owner_setup(&self) {
        self.write(
            ULAUNCHER_PATH,
            "Show Ulauncher",
            "ulauncher toggle",
            "<Super>space",
        );
        self.write(
            KUKKA_PATH,
            "kukka Hotkey Window",
            "gapplication action dev.kukka.Kukka toggle-quick-terminal",
            "<Control><Super>plus",
        );
        self.set_paths(&[ULAUNCHER_PATH, KUKKA_PATH]);
    }
}

// ----------------------------------------------------- child process

pub const SCENARIO_VAR: &str = "PETLAUNCHER_GNOME_SCENARIO";
pub const ROOT_VAR: &str = "PETLAUNCHER_GNOME_ROOT";

/// The directories of an isolated desktop environment for a child process.
pub struct IsolatedEnv {
    pub root: tempfile::TempDir,
    pub bus: PrivateBus,
}

/// The desktop entries of the isolated environment. Each starts the
/// recorder with its tag as the first argument.
const APPS: &[(&str, &str, &str)] = &[
    ("petlauncher-test-text.desktop", "text", "text/plain;"),
    (
        "petlauncher-test-files.desktop",
        "files",
        "inode/directory;",
    ),
    (
        "petlauncher-test-browser.desktop",
        "default-browser",
        "x-scheme-handler/http;x-scheme-handler/https;",
    ),
    ("petlauncher-test-edge.desktop", "edge", ""),
    ("petlauncher-test-app.desktop", "app", ""),
];

const MIMEAPPS: &str = "[Default Applications]
text/plain=petlauncher-test-text.desktop
inode/directory=petlauncher-test-files.desktop
x-scheme-handler/http=petlauncher-test-browser.desktop
x-scheme-handler/https=petlauncher-test-browser.desktop
";

impl IsolatedEnv {
    pub fn new() -> Self {
        let root = tempfile::tempdir().expect("temporary directory");
        let dir = root.path();
        for sub in [
            "home",
            "config",
            "config-dirs",
            "data/applications",
            "data-dirs",
            "cache",
            "state",
            "run",
            "bin",
        ] {
            fs::create_dir_all(dir.join(sub)).unwrap();
        }
        fs::set_permissions(dir.join("run"), fs::Permissions::from_mode(0o700)).unwrap();
        // The system MIME database finds the type of a file name. The
        // applications of the system are not visible.
        std::os::unix::fs::symlink("/usr/share/mime", dir.join("data-dirs/mime")).unwrap();

        let recorder = dir.join("bin/recorder");
        fs::write(
            &recorder,
            format!(
                "#!/bin/sh\n\
                 # Records one start: the tag, then each argument in brackets.\n\
                 {{\n  printf '%s' \"$1\"\n  shift\n  \
                 for argument in \"$@\"; do printf ' [%s]' \"$argument\"; done\n  \
                 printf '\\n'\n}} >> '{}'\n",
                dir.join("calls.txt").display()
            ),
        )
        .unwrap();
        fs::set_permissions(&recorder, fs::Permissions::from_mode(0o755)).unwrap();
        for (id, tag, mime_types) in APPS {
            fs::write(
                dir.join("data/applications").join(id),
                format!(
                    "[Desktop Entry]\nType=Application\nName=petlauncher test {tag}\n\
                     Exec={} {tag} %U\nMimeType={mime_types}\n",
                    recorder.display()
                ),
            )
            .unwrap();
        }
        fs::write(dir.join("config/mimeapps.list"), MIMEAPPS).unwrap();
        Self {
            root,
            bus: PrivateBus::start(),
        }
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.path().join(relative)
    }

    /// Runs the ignored test `test_name` of this test binary in a child
    /// process with the isolated environment. The child does `scenario`.
    /// Its standard output and error go to `<scenario>.out` and
    /// `<scenario>.err` in the root, so that a program that the child
    /// starts cannot keep the call waiting.
    pub fn run_child(&self, test_name: &str, scenario: &str, extra: &[(&str, &str)]) -> ExitStatus {
        let dir = self.root.path();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                test_name,
                "--exact",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear();
        for name in [
            "PATH",
            "LANG",
            "TMPDIR",
            "RUST_BACKTRACE",
            "LLVM_PROFILE_FILE",
        ] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        command
            .env("HOME", dir.join("home"))
            .env("XDG_CONFIG_HOME", dir.join("config"))
            .env("XDG_CONFIG_DIRS", dir.join("config-dirs"))
            .env("XDG_DATA_HOME", dir.join("data"))
            .env("XDG_DATA_DIRS", dir.join("data-dirs"))
            .env("XDG_CACHE_HOME", dir.join("cache"))
            .env("XDG_STATE_HOME", dir.join("state"))
            .env("XDG_RUNTIME_DIR", dir.join("run"))
            .env("DBUS_SESSION_BUS_ADDRESS", self.bus.address())
            .env(
                "DBUS_SYSTEM_BUS_ADDRESS",
                format!("unix:path={}", dir.join("run/no-system-bus").display()),
            )
            .env("GSETTINGS_BACKEND", "memory")
            .env("GIO_USE_VFS", "local")
            .env("GIO_USE_VOLUME_MONITOR", "unix")
            .env(ROOT_VAR, dir)
            .env(SCENARIO_VAR, scenario)
            .envs(extra.iter().copied())
            .stdin(Stdio::null())
            .stdout(fs::File::create(dir.join(format!("{scenario}.out"))).unwrap())
            .stderr(fs::File::create(dir.join(format!("{scenario}.err"))).unwrap());
        command.status().expect("the child process runs")
    }

    pub fn output(&self, scenario: &str) -> String {
        fs::read_to_string(self.path(&format!("{scenario}.out"))).unwrap_or_default()
            + &fs::read_to_string(self.path(&format!("{scenario}.err"))).unwrap_or_default()
    }

    /// The lines of the recorder. The programs start after a double fork,
    /// so this waits until `count` lines are there, for 10 s at most.
    pub fn wait_for_calls(&self, count: usize) -> Vec<String> {
        wait_for_lines(&self.path("calls.txt"), count)
    }
}

pub fn wait_for_lines(file: &Path, count: usize) -> Vec<String> {
    let start = Instant::now();
    loop {
        let lines: Vec<String> = fs::read_to_string(file)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect();
        if lines.len() >= count || start.elapsed() > Duration::from_secs(10) {
            return lines;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// In the child process: the scenario and the root, or `None` when the test
/// runs outside `run_child`. Before it gives them, it proves that the
/// process has the isolated environment.
pub fn child_scenario() -> Option<(String, PathBuf)> {
    let scenario = std::env::var(SCENARIO_VAR).ok()?;
    let root = PathBuf::from(std::env::var_os(ROOT_VAR).expect("root of the isolated environment"));
    for name in ["WAYLAND_DISPLAY", "DISPLAY"] {
        assert!(std::env::var_os(name).is_none(), "{name} must not be set");
    }
    for name in ["HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_DATA_DIRS"] {
        let value = PathBuf::from(std::env::var_os(name).unwrap());
        assert!(value.starts_with(&root), "{name} is in the isolated root");
    }
    let bus = std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap();
    assert!(
        !bus.contains("/run/user/"),
        "the session bus is a private bus"
    );
    Some((scenario, root))
}
