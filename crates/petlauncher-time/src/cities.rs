use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{env, fmt, fs, io};

use jiff::tz::TimeZone;

use crate::fold::fold;
use crate::{gweather, tzdb};

/// The city list of libgweather 4, which GNOME Weather and GNOME Clocks use.
pub const GWEATHER_LOCATIONS: &str = "/usr/share/libgweather-4/Locations.xml";

/// The time zone database of the system when `TZDIR` is not set.
const SYSTEM_ZONEINFO: &str = "/usr/share/zoneinfo";

/// A GNOME city and a tzdb city with the same name and country are one city
/// when they are nearer than this. On 2026-11-26 the system files gave at
/// most 122 km for one city in the two sources, and at least 700 km for two
/// cities with the same name in one country.
const SAME_CITY_KM: f64 = 200.0;

/// A city and its time zone.
#[derive(Debug, Clone)]
pub struct City {
    /// The name as the source writes it, for example "Las Vegas".
    pub name: String,
    /// The state and the country, for example "Nevada, United States".
    pub place: String,
    pub time_zone: TimeZone,
}

/// The table of city names, made one time from the system files.
#[derive(Debug)]
pub struct Cities {
    cities: Vec<City>,
    /// The folded name gives the cities in the match order.
    by_name: HashMap<String, Vec<usize>>,
}

/// A city of one source before the merge.
#[derive(Debug)]
pub(crate) struct Place {
    pub name: String,
    pub state: Option<String>,
    pub country_code: String,
    pub country: String,
    pub time_zone: String,
    /// Latitude and longitude in degrees.
    pub position: Option<(f64, f64)>,
}

impl Cities {
    /// Reads `zone.tab` and `iso3166.tab` in `zoneinfo_dir`, then the GNOME
    /// city list `locations_xml`. A missing GNOME file is not an error,
    /// because GNOME Weather is optional.
    pub fn load(zoneinfo_dir: &Path, locations_xml: &Path) -> Result<Cities, LoadError> {
        let zone_tab = read(&zoneinfo_dir.join("zone.tab"))?;
        let iso3166_tab = read(&zoneinfo_dir.join("iso3166.tab"))?;
        let zones = tzdb::places(&zone_tab, &iso3166_tab);
        let gnome_cities = match fs::read_to_string(locations_xml) {
            Ok(xml) => gweather::places(&xml).map_err(|error| LoadError::Xml {
                path: locations_xml.to_owned(),
                message: error.to_string(),
            })?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(source) => {
                return Err(LoadError::Read {
                    path: locations_xml.to_owned(),
                    source,
                })
            }
        };
        Ok(Cities::merge(zones, gnome_cities))
    }

    /// Loads the files of the system: the time zone database in `TZDIR` or
    /// `/usr/share/zoneinfo`, and [`GWEATHER_LOCATIONS`].
    pub fn load_system() -> Result<Cities, LoadError> {
        let zoneinfo =
            env::var_os("TZDIR").map_or_else(|| PathBuf::from(SYSTEM_ZONEINFO), PathBuf::from);
        Cities::load(&zoneinfo, Path::new(GWEATHER_LOCATIONS))
    }

    /// Gives the cities with this name in the match order: first the tzdb
    /// city, then the GNOME cities in the order of the file. The match
    /// ignores case, accents and punctuation.
    pub fn find(&self, name: &str) -> Vec<&City> {
        let indexes = self.by_name.get(&fold(name)).map_or(&[][..], Vec::as_slice);
        indexes.iter().map(|&index| &self.cities[index]).collect()
    }

    pub fn len(&self) -> usize {
        self.cities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cities.is_empty()
    }

    /// A GNOME city that is also a tzdb city merges into the tzdb city: the
    /// tzdb zone stays, because some GNOME hints are older or less exact, and
    /// the GNOME names replace the tzdb names, because they have the accents
    /// and the state. A city whose zone is not in the system database is
    /// dropped.
    fn merge(zones: Vec<Place>, gnome_cities: Vec<Place>) -> Cities {
        let mut zones: Vec<(String, Place)> = zones.into_iter().map(with_key).collect();
        let mut zone_indexes: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, (key, _)) in zones.iter().enumerate() {
            zone_indexes.entry(key.clone()).or_default().push(index);
        }
        let mut separate = Vec::new();
        for (key, city) in gnome_cities.into_iter().map(with_key) {
            let indexes = zone_indexes.get(&key).map_or(&[][..], Vec::as_slice);
            match indexes
                .iter()
                .find(|&&index| is_same_city(&zones[index].1, &city))
            {
                Some(&index) => zones[index].1.take_names(city),
                None => separate.push((key, city)),
            }
        }

        let mut cities = Cities {
            cities: Vec::new(),
            by_name: HashMap::new(),
        };
        for (key, place) in zones.into_iter().chain(separate) {
            let Ok(time_zone) = jiff::tz::db().get(&place.time_zone) else {
                continue;
            };
            cities
                .by_name
                .entry(key)
                .or_default()
                .push(cities.cities.len());
            cities.cities.push(City {
                place: place.label(),
                name: place.name,
                time_zone,
            });
        }
        cities
    }
}

fn with_key(place: Place) -> (String, Place) {
    (fold(&place.name), place)
}

impl Place {
    fn take_names(&mut self, gnome_city: Place) {
        self.name = gnome_city.name;
        self.state = gnome_city.state;
        self.country = gnome_city.country;
    }

    fn label(&self) -> String {
        [self.state.as_deref().unwrap_or(""), &self.country]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn is_same_city(zone: &Place, city: &Place) -> bool {
    let near = match (zone.position, city.position) {
        (Some(a), Some(b)) => distance_km(a, b) < SAME_CITY_KM,
        _ => false,
    };
    zone.country_code == city.country_code && near
}

/// The distance between two positions on the earth. The approximation is
/// good for the short distances that [`is_same_city`] compares.
fn distance_km(
    (latitude_a, longitude_a): (f64, f64),
    (latitude_b, longitude_b): (f64, f64),
) -> f64 {
    const KM_PER_DEGREE: f64 = 111.2;
    let mean_latitude = ((latitude_a + latitude_b) / 2.0).to_radians();
    let north = latitude_b - latitude_a;
    let east = (longitude_b - longitude_a) * mean_latitude.cos();
    KM_PER_DEGREE * north.hypot(east)
}

fn read(path: &Path) -> Result<String, LoadError> {
    fs::read_to_string(path).map_err(|source| LoadError::Read {
        path: path.to_owned(),
        source,
    })
}

/// A source file that cannot be read.
#[derive(Debug)]
pub enum LoadError {
    Read { path: PathBuf, source: io::Error },
    Xml { path: PathBuf, message: String },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Read { path, source } => {
                write!(f, "cannot read {}: {source}", path.display())
            }
            LoadError::Xml { path, message } => {
                write!(f, "{} is not valid XML: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LoadError::Read { source, .. } => Some(source),
            LoadError::Xml { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_distance_from_helsinki_to_tallinn_is_about_80_km() {
        // Arrange
        let helsinki = (60.17, 24.94);
        let tallinn = (59.44, 24.75);

        // Act
        let distance = distance_km(helsinki, tallinn);

        // Assert
        assert!((75.0..90.0).contains(&distance), "{distance}");
    }

    #[test]
    fn a_city_with_no_position_is_not_the_same_city() {
        // Arrange
        let place = |position| Place {
            name: "A".into(),
            state: None,
            country_code: "FI".into(),
            country: "Finland".into(),
            time_zone: "Europe/Helsinki".into(),
            position,
        };

        // Act
        let same = is_same_city(&place(Some((60.0, 25.0))), &place(None));

        // Assert
        assert!(!same);
    }

    #[test]
    fn a_read_error_names_the_file_and_has_the_cause() {
        // Arrange
        let error = read(Path::new("/no/such/zone.tab")).unwrap_err();

        // Act
        let message = error.to_string();

        // Assert
        assert!(
            message.starts_with("cannot read /no/such/zone.tab: "),
            "{message}"
        );
        assert!(std::error::Error::source(&error).is_some());
    }
}
