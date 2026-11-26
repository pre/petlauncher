//! The locale of the localized keys, for example `Name[fi]`.

/// A locale in the form `lang_COUNTRY.ENCODING@MODIFIER`. The encoding has
/// no effect on the match, so it is not kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Locale {
    lang: String,
    country: Option<String>,
    modifier: Option<String>,
}

impl Locale {
    /// Gives `None` for an empty value and for the locales `C` and `POSIX`,
    /// which have no translations.
    pub(crate) fn parse(value: &str) -> Option<Locale> {
        let (rest, modifier) = match value.split_once('@') {
            Some((rest, modifier)) => (rest, Some(modifier.to_string())),
            None => (value, None),
        };
        let rest = rest.split_once('.').map_or(rest, |(rest, _encoding)| rest);
        let (lang, country) = match rest.split_once('_') {
            Some((lang, country)) => (lang, Some(country.to_string())),
            None => (rest, None),
        };
        if lang.is_empty() || lang == "C" || lang == "POSIX" {
            return None;
        }
        Some(Locale {
            lang: lang.to_string(),
            country,
            modifier,
        })
    }

    /// The key suffixes to try, best match first, as the Desktop Entry
    /// Specification orders them.
    pub(crate) fn suffixes(&self) -> Vec<String> {
        let lang = &self.lang;
        let mut suffixes = Vec::new();
        if let (Some(country), Some(modifier)) = (&self.country, &self.modifier) {
            suffixes.push(format!("{lang}_{country}@{modifier}"));
        }
        if let Some(country) = &self.country {
            suffixes.push(format!("{lang}_{country}"));
        }
        if let Some(modifier) = &self.modifier {
            suffixes.push(format!("{lang}@{modifier}"));
        }
        suffixes.push(lang.clone());
        suffixes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_suffixes_from_the_most_specific_to_the_language() {
        let locale = Locale::parse("fi_FI.UTF-8@euro").unwrap();

        let suffixes = locale.suffixes();

        assert_eq!(suffixes, ["fi_FI@euro", "fi_FI", "fi@euro", "fi"]);
    }

    #[test]
    fn ignores_the_encoding_of_a_locale_with_a_country() {
        let locale = Locale::parse("en_US.UTF-8").unwrap();

        assert_eq!(locale.suffixes(), ["en_US", "en"]);
    }

    #[test]
    fn accepts_a_language_without_a_country() {
        let locale = Locale::parse("sr@latin").unwrap();

        assert_eq!(locale.suffixes(), ["sr@latin", "sr"]);
    }

    #[test]
    fn gives_no_locale_for_c_posix_and_empty_values() {
        for value in ["", "C", "C.UTF-8", "POSIX"] {
            assert_eq!(Locale::parse(value), None, "{value}");
        }
    }
}
