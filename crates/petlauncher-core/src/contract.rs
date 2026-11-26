//! The data types that go between the engine and the user interface.

use std::path::{Path, PathBuf};

use petlauncher_config::Browser;

/// One row of the result list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub kind: RowKind,
    pub title: String,
    pub subtitle: String,
    pub icon: Icon,
    /// The local file of the row (R2, R13). Ctrl+Enter shows it in the file
    /// manager and Shift+Enter copies its path. `None`: the two keys do
    /// nothing.
    pub local_file: Option<PathBuf>,
    /// The primary function of the row. Enter does it.
    pub enter: Action,
}

impl Row {
    /// Makes a row with no subtitle, no icon and no local file.
    pub fn new(kind: RowKind, title: impl Into<String>, enter: Action) -> Self {
        Self {
            kind,
            title: title.into(),
            subtitle: String::new(),
            icon: Icon::None,
            local_file: None,
            enter,
        }
    }

    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = subtitle.into();
        self
    }

    pub fn with_icon(mut self, icon: Icon) -> Self {
        self.icon = icon;
        self
    }

    pub fn with_local_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.local_file = Some(path.into());
        self
    }
}

/// The type of a row. `activate` uses it for two rules: Ctrl+Enter on a
/// `Directory` opens the directory itself, and Enter on a `Calculation`
/// keeps the query (R9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowKind {
    Calculation,
    Time,
    /// `local_file` is the `.desktop` file of the application.
    Application,
    SystemCommand,
    /// `local_file` is the `.desktop` file of the page.
    SettingsPage,
    Shortcut,
    File,
    Directory,
    Dictionary,
}

/// The icon of a row. The user interface finds the image.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Icon {
    #[default]
    None,
    /// A name in the icon theme, for example `system-lock-screen`.
    Name(String),
    /// An image file.
    Path(PathBuf),
    /// The icon of the content type of this file or directory. With GTK the
    /// user interface asks GIO for it.
    ForFile(PathBuf),
}

/// What the program does. The core only makes these values. The program
/// executes them, and gives the parts that depend on the desktop to the
/// `Desktop` trait.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Action {
    #[default]
    Nothing,
    /// Opens the file with its default application.
    OpenFile(PathBuf),
    /// Opens this directory in the file manager (`Desktop::open_folder`).
    OpenFolder(PathBuf),
    /// Opens the folder that contains this file or directory in the file
    /// manager, with the item selected (`Desktop::show_in_folder`).
    ShowInFolder(PathBuf),
    /// Puts this text in the query line. The window stays open.
    SetQuery(String),
    /// Puts this text on the clipboard.
    Copy(String),
    /// Starts the application of this `.desktop` file.
    LaunchDesktopEntry(PathBuf),
    /// Opens the URL. `browser` is `None` for the default browser.
    OpenUrl {
        url: String,
        browser: Option<Browser>,
    },
    /// Runs the script with `sh`, with `argument` as `$1`. `%s` in the
    /// script is already replaced.
    RunScript { script: String, argument: String },
    /// `Desktop::run_system_command`.
    System(SystemCommand),
    /// Opens the dictionary window for the word (R19).
    OpenDictionary(String),
}

impl Action {
    /// True when the window closes after the action. Only `Nothing` and
    /// `SetQuery` keep the window open.
    pub fn closes_window(&self) -> bool {
        !matches!(self, Self::Nothing | Self::SetQuery(_))
    }
}

/// The system commands of R16.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemCommand {
    Lock,
    Suspend,
    LogOut,
    Restart,
    PowerOff,
}

impl SystemCommand {
    pub const ALL: [Self; 5] = [
        Self::Lock,
        Self::Suspend,
        Self::LogOut,
        Self::Restart,
        Self::PowerOff,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Lock => "Lock",
            Self::Suspend => "Suspend",
            Self::LogOut => "Log out",
            Self::Restart => "Restart",
            Self::PowerOff => "Power off",
        }
    }
}

/// The mode that the engine selected for the query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// The query is empty or has only spaces.
    #[default]
    Empty,
    /// The query is a file path (R1, R10).
    Path,
    /// The query starts with the file search keyword and a space (R14).
    FileSearch,
    /// The query starts with the dictionary keyword and a space (R19).
    Dictionary,
    /// The query starts with a shortcut keyword and a space (R6).
    Shortcut,
    /// All other queries.
    General,
}

/// The existing path that the query names, in path mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedPath {
    Directory(PathBuf),
    File(PathBuf),
}

impl TypedPath {
    pub fn path(&self) -> &Path {
        match self {
            Self::Directory(path) | Self::File(path) => path,
        }
    }
}

/// The result of one search.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Results {
    /// The query text of this search. The user interface uses it to drop
    /// the results of an old query.
    pub query: String,
    pub mode: Mode,
    pub rows: Vec<Row>,
    /// The row that is selected at the start. `None` when there are no rows,
    /// and for a typed path that is an existing directory (R1).
    pub selected: Option<usize>,
    /// The existing file or directory that the query names, in path mode.
    pub typed_path: Option<TypedPath>,
}

/// A key on the selected row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Enter,
    CtrlEnter,
    ShiftEnter,
}

/// What the program does after a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub action: Action,
    /// Close the window.
    pub close: bool,
    /// R9: the close keeps the query. True only for Enter on a calculation.
    pub keep_query: bool,
}
