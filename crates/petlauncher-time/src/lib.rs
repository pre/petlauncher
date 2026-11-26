//! Time in other time zones for the petlauncher query line (requirement R20).
//!
//! Load the city table one time with [`Cities::load_system`]. Then give each
//! query to [`Cities::answer`], which gives the rows for the user interface.
//! Text that is not a time query gives no rows. The query forms, the city
//! sources and the result texts are in `docs/time-zones.md`.

mod answer;
mod cities;
mod fold;
mod gweather;
mod query;
mod tzdb;

pub use answer::Answer;
pub use cities::{Cities, City, LoadError, GWEATHER_LOCATIONS};
/// The time library of the API, so a caller uses the same version, for
/// example `petlauncher_time::jiff::ZonedDateTime::now()`.
pub use jiff;
