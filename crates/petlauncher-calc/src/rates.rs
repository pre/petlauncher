//! Currency rates of the European Central Bank: the parser of the daily file,
//! the cache file, and the download (cargo feature `fetch`).

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::str::FromStr;

use rust_decimal::Decimal;

/// The daily reference rates. No API key is necessary.
pub const ECB_DAILY_URL: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml";

/// The name of the cache file in the directory that the caller gives. The
/// file is the unchanged ECB file.
pub const CACHE_FILE: &str = "ecb-eurofxref-daily.xml";

/// The largest download that `fetch_ecb` accepts. The real file is about 2 KB.
#[cfg(feature = "fetch")]
const MAX_DOWNLOAD_BYTES: u64 = 1_000_000;

/// Currency rates against EUR, with the date that the ECB gives them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rates {
    date: String,
    /// Units of the currency for 1 EUR, by upper case ISO 4217 code.
    per_eur: BTreeMap<String, Decimal>,
}

#[derive(Debug)]
pub enum RatesError {
    /// The text is not a valid ECB rates file.
    Format(String),
    Io(io::Error),
    /// The download failed: no network, a timeout, a certificate error or an
    /// HTTP status that is not 2xx.
    Http(String),
}

impl fmt::Display for RatesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RatesError::Format(message) => write!(f, "invalid rates file: {message}"),
            RatesError::Io(error) => write!(f, "cannot read or write the rates file: {error}"),
            RatesError::Http(message) => write!(f, "cannot download the rates: {message}"),
        }
    }
}

impl std::error::Error for RatesError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RatesError::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for RatesError {
    fn from(error: io::Error) -> Self {
        RatesError::Io(error)
    }
}

impl Rates {
    /// `date` has the form `YYYY-MM-DD`. Each pair is a currency code and the
    /// units of that currency for 1 EUR, for example `("USD", "1.0512")`.
    pub fn new(date: &str, rates: &[(&str, &str)]) -> Result<Rates, RatesError> {
        if !is_iso_date(date) {
            return Err(RatesError::Format(format!(
                "`{date}` is not a YYYY-MM-DD date"
            )));
        }
        let mut per_eur = BTreeMap::new();
        for (code, rate) in rates {
            if code.len() != 3 || !code.bytes().all(|b| b.is_ascii_alphabetic()) {
                return Err(RatesError::Format(format!(
                    "`{code}` is not a currency code"
                )));
            }
            let value = Decimal::from_str(rate)
                .ok()
                .filter(|value| *value > Decimal::ZERO)
                .ok_or_else(|| {
                    RatesError::Format(format!("`{rate}` is not a positive rate for {code}"))
                })?;
            per_eur.insert(code.to_ascii_uppercase(), value);
        }
        Ok(Rates {
            date: date.to_string(),
            per_eur,
        })
    }

    /// The date of the rates, `YYYY-MM-DD`.
    pub fn date(&self) -> &str {
        &self.date
    }

    /// The codes that have a rate, EUR not included.
    pub fn currencies(&self) -> impl Iterator<Item = &str> {
        self.per_eur.keys().map(String::as_str)
    }

    pub(crate) fn per_eur(&self, code: &str) -> Option<Decimal> {
        if code == "EUR" {
            return Some(Decimal::ONE);
        }
        self.per_eur.get(code).copied()
    }
}

fn is_iso_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(i, b)| match i {
            4 | 7 => *b == b'-',
            _ => b.is_ascii_digit(),
        })
}

/// Reads the ECB file `eurofxref-daily.xml`: one `Cube` element with the
/// attribute `time`, and in it one `Cube` element for each currency.
pub fn parse_ecb(xml: &str) -> Result<Rates, RatesError> {
    let document =
        roxmltree::Document::parse(xml).map_err(|error| RatesError::Format(error.to_string()))?;
    let day = document
        .descendants()
        .find(|node| node.has_tag_name("Cube") && node.has_attribute("time"))
        .ok_or_else(|| RatesError::Format("no Cube element with a time".into()))?;
    let pairs: Vec<(&str, &str)> = day
        .children()
        .filter(|node| node.has_tag_name("Cube"))
        .filter_map(|node| Some((node.attribute("currency")?, node.attribute("rate")?)))
        .collect();
    if pairs.is_empty() {
        return Err(RatesError::Format("no rates".into()));
    }
    Rates::new(day.attribute("time").unwrap_or_default(), &pairs)
}

/// Reads the cache file in `dir`. Gives `Ok(None)` when there is no cache file.
pub fn load_cache(dir: &Path) -> Result<Option<Rates>, RatesError> {
    match fs::read_to_string(dir.join(CACHE_FILE)) {
        Ok(xml) => parse_ecb(&xml).map(Some),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Writes an ECB file to the cache in `dir` and gives its rates. Makes `dir`
/// when it does not exist. A text that is not a valid ECB file does not
/// replace the cache.
pub fn save_cache(dir: &Path, xml: &str) -> Result<Rates, RatesError> {
    let rates = parse_ecb(xml)?;
    fs::create_dir_all(dir)?;
    // A rename replaces the file in one step, so a reader never sees half a file.
    let partial = dir.join(format!("{CACHE_FILE}.part"));
    fs::write(&partial, xml)?;
    fs::rename(&partial, dir.join(CACHE_FILE))?;
    Ok(rates)
}

/// Downloads the ECB daily file. The certificates come from the system trust
/// store, not from a list in the program. `timeout` is for the full request.
#[cfg(feature = "fetch")]
pub fn fetch_ecb(timeout: std::time::Duration) -> Result<String, RatesError> {
    fetch(ECB_DAILY_URL, timeout)
}

/// Downloads the ECB daily file and writes it to the cache in `dir`.
#[cfg(feature = "fetch")]
pub fn update_cache(dir: &Path, timeout: std::time::Duration) -> Result<Rates, RatesError> {
    save_cache(dir, &fetch_ecb(timeout)?)
}

#[cfg(feature = "fetch")]
fn fetch(url: &str, timeout: std::time::Duration) -> Result<String, RatesError> {
    use ureq::tls::{RootCerts, TlsConfig};

    let http_error = |error: ureq::Error| RatesError::Http(error.to_string());
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .new_agent();
    agent
        .get(url)
        .call()
        .map_err(http_error)?
        .body_mut()
        .with_config()
        .limit(MAX_DOWNLOAD_BYTES)
        .read_to_string()
        .map_err(http_error)
}

#[cfg(all(test, feature = "fetch"))]
mod fetch_tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::{Duration, Instant};

    use super::*;

    /// Serves one HTTP response on a local port and gives its URL.
    fn serve_once(response: String, delay: Duration) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://{}/eurofxref-daily.xml",
            listener.local_addr().unwrap()
        );
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 4096];
            let _ = stream.read(&mut request);
            thread::sleep(delay);
            let _ = stream.write_all(response.as_bytes());
        });
        url
    }

    fn response(status: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: text/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    #[test]
    fn fetch_gives_the_body_of_the_response() {
        // Arrange
        let body = include_str!("../tests/fixtures/eurofxref-daily.xml");
        let url = serve_once(response("200 OK", body), Duration::ZERO);

        // Act
        let result = fetch(&url, Duration::from_secs(5)).unwrap();

        // Assert
        assert_eq!(result, body);
    }

    #[test]
    fn fetch_with_an_error_status_is_an_http_error() {
        let url = serve_once(response("503 Service Unavailable", "busy"), Duration::ZERO);

        let result = fetch(&url, Duration::from_secs(5));

        assert!(matches!(result, Err(RatesError::Http(_))));
    }

    #[test]
    fn fetch_stops_at_the_timeout() {
        // Arrange
        let url = serve_once(response("200 OK", "late"), Duration::from_secs(3));
        let start = Instant::now();

        // Act
        let result = fetch(&url, Duration::from_millis(300));

        // Assert
        assert!(matches!(result, Err(RatesError::Http(_))));
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
