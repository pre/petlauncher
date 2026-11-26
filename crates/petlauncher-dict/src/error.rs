use std::fmt;
use std::io;
use std::path::PathBuf;

/// An error of the dictionary. The user interface can show its text.
#[derive(Debug)]
pub enum Error {
    /// The WordNet directory does not exist.
    NotInstalled { path: PathBuf },
    /// The WordNet directory exists, but some of the necessary files are missing.
    Incomplete {
        path: PathBuf,
        missing: Vec<&'static str>,
    },
    /// A file exists, but the reader cannot read it.
    Read { path: PathBuf, source: io::Error },
    /// A file does not have the WordNet format.
    Format { path: PathBuf, detail: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotInstalled { path } => write!(
                f,
                "the package wordnet-base is not installed: there is no directory {}",
                path.display()
            ),
            Error::Incomplete { path, missing } => write!(
                f,
                "the package wordnet-base is not installed: the directory {} has no {}",
                path.display(),
                missing.join(", ")
            ),
            Error::Read { path, source } => write!(f, "cannot read {}: {source}", path.display()),
            Error::Format { path, detail } => {
                write!(f, "{} is not a WordNet file: {detail}", path.display())
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Read { source, .. } => Some(source),
            _ => None,
        }
    }
}
