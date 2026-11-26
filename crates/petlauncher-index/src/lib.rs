//! The file index of petlauncher: the search by file name (R14, R21).
//!
//! [`FileIndex`] is the API for the program. It loads the stored index at the
//! start, answers searches from a snapshot in memory, and scans again in the
//! background. `docs/file-search.md` gives the ignore rules, the match and
//! rank rule, the stored file format and the rescan rule.

mod file_index;
mod index;
mod scan;
mod scope;
mod search;
pub mod store;

pub use file_index::FileIndex;
pub use index::{FileEntry, Index};
pub use scan::{scan, ScanError};
pub use scope::{IgnoreEntry, Scope, ScopeError};
