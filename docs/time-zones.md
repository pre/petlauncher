# Time in other time zones

The crate `petlauncher-time` answers the time queries of requirement R20. This file gives the query forms, the city sources, the match rule and the result texts.

## Query forms

| Query | Example | Result |
|---|---|---|
| `time now in <city>` | `time now in new york` | The time in the city now. |
| `time in <city>` or `now in <city>` | `time in new york` | The same result as `time now in <city>`. |
| `<time> in <city>` | `17:00 in las vegas` | The time in the owner's time zone when the time in the city is `<time>`. |
| `<time> here in <city>` | `17:00 here in las vegas` | The time in the city when the time in the owner's time zone is `<time>`. R20 does not ask for this form. |

The forms of `<time>`:

| Form | Examples | Value |
|---|---|---|
| Hour, 24-hour clock | `6`, `18` | 06:00, 18:00 |
| Hour and minutes | `18:30`, `0:05` | 18:30, 00:05 |
| 12-hour clock | `6am`, `6 am`, `6:30pm`, `6 p.m.` | 06:00, 06:00, 18:30, 18:00 |
| 12-hour clock at 12 | `12am`, `12pm` | 00:00, 12:00 |
| Words | `noon`, `midnight` | 12:00, and 00:00 at the start of the day |

Capital letters and extra spaces have no effect.

These forms are not in scope:

- A conversion between two cities, for example `17:00 helsinki time in las vegas`. petlauncher converts only between the owner's time zone and one city.
- A date, seconds, or a period between the hour and the minutes, for example `18.30`.
- A part of a city name. `17:00 in las` does not find Las Vegas.

### The date of a time

A time in a query is on today's date in the time zone where the owner gives the time:

- `17:00 in las vegas` is 17:00 on today's date in Las Vegas.
- `17:00 here in las vegas` is 17:00 on today's date in the owner's time zone.

Reason: the owner reads the time as a time of that place, so the date of that place applies.

Example: at 01:00 on Thursday in Helsinki, the time in Las Vegas is 15:00 on Wednesday. Thus `17:00 in las vegas` is 17:00 on Wednesday in Las Vegas, which is 03:00 today in Helsinki.

The result tells the day of the converted time with `today`, `tomorrow` or `yesterday`. The word compares the date of the result with today's date in the same time zone.

### Daylight saving time

| Case | Result |
|---|---|
| The time does not exist, because the clocks go forward. Example: `02:30 in new york` on 8 March 2026. | One row with the time after the change, 03:30. The second text says that 02:30 does not exist. |
| The time exists two times, because the clocks go back. Example: `01:30 in new york` on 1 November 2026. | Two rows. The first row has the summer offset and the words "first of two". The second row has the winter offset and the words "second of two". |

## Is the text a time query?

The engine gives each query to `Cities::answer`. The parse finds the first word `in`. The words after it are the city. The words before it must be a keyword form or a time. If they are not, the text is not a time query and the result has no rows. This check does not use the city table.

A query is clear when it can only be a time query: it starts with `time`, or the time has `:`, `am`, `pm`, `noon` or `midnight`. A query that starts with `now` and a bare hour such as `18` are not clear, because the text can mean a different thing, for example `now in stock`.

| Text | Result |
|---|---|
| `firefox in`, `log in` | No rows, because no city follows `in`. |
| `5 GiB in MB` | No rows, because `5 GiB` is not a time. |
| `10 in 20` | No rows, because the hour 10 is not clear and `20` is not a city. |
| `now in stock` | No rows, because the query is not clear and `stock` is not a city. |
| `17:00 in atlantis` | One row, "City not found: atlantis". |
| `18 in helsinki` | The time, because Helsinki is a city. |

A clear query with an unknown city gives the row "City not found". Reason: the owner sees that petlauncher read the query and that the city name is the problem. This row also shows while the owner types a city name, for example at `17:00 in las`.

## City sources

petlauncher reads the cities one time when it starts, with `Cities::load_system`. The repository has no city list.

1. The time zone database of the system, in the directory `TZDIR` or `/usr/share/zoneinfo`:
   - `zone.tab` gives a zone, a country code and the coordinates on each line. The city name is the last part of the zone name. For example, `America/New_York` gives "New York".
   - `iso3166.tab` gives the name of each country.
   - `zone.tab` has a zone for each country, thus it also has zones such as `Europe/Oslo` that `zone1970.tab` does not have. Old names such as `Europe/Kiev` and `Asia/Calcutta` are not in `zone.tab`, and petlauncher does not find them.
2. The GNOME city list `/usr/share/libgweather-4/Locations.xml` of libgweather 4, when the file exists. A missing file is not an error. A file that is not valid XML is an error.
   - The city name is in the `<_name>` or `<name>` element of the `<city>`.
   - The place is the name of the enclosing `<state>`, when there is one, and of the enclosing `<country>`.
   - The zone is the `<tz-hint>` of the city. When the city has no `<tz-hint>`, the zone is the `<tz-hint>` of the nearest enclosing element.
   - Weather stations (`<location>`), zone names (`<timezone>`) and the elements `<named-timezone>` are not cities.

On 2026-11-26 this computer had `tzdata` 2026e with 418 zones in `zone.tab`, and `libgweather-4-common` 4.4.2 with 4348 cities. Of the cities, 224 have their own `<tz-hint>`.

### Merge of the two sources

A GNOME city and a tzdb zone are the same city when they have the same folded name, the same country code, and are less than 200 km apart. The merged city keeps the tzdb zone and takes the GNOME names, which have the accents and the state.

Reasons:

- Some GNOME hints are older or less exact than the tzdb. For example, GNOME gives `America/Mazatlan` (UTC-7) for Chihuahua, but Chihuahua uses `America/Chihuahua` (UTC-6) since 2022.
- The distance keeps different cities apart. For example, Juneau in Wisconsin is not the tzdb city `America/Juneau` in Alaska. On 2026-11-26 the system files gave at most 122 km between the two records of one city, and at least 700 km between two cities with the same name in one country.

The table drops a city whose zone is not in the system database. On Ubuntu 24.04 this is only Godthåb, with the old zone name `America/Godthab`, which is in the package `tzdata-legacy`. The query `now in nuuk` gives the same city.

Result on 2026-11-26: 259 GNOME cities merged into tzdb zones, and the table has 4506 cities.

## Match rule

The match compares folded names. The fold does these steps:

- It makes all letters lower case.
- It removes the accents, so `godthab` finds "Godthåb". It changes `ø`, `æ`, `œ`, `ß`, `ł`, `ð`, `þ` and `ı` to ASCII letters.
- It removes `.` and `'`, and it changes `-` and `_` to a space.
- It changes the word `saint` to `st`, so `st johns` finds "Saint John's".

The full name must match. The order of the cities with the same name is:

1. The tzdb city.
2. The GNOME cities, in the order of the file: by region, then by country, then by state. For example, Las Vegas in Nevada comes before Las Vegas in New Mexico.

Each time zone gives one row. A city with the same zone as a city before it gives no row. For example, `17:00 in paris` gives a row for Paris, France, and a row for Paris, Illinois. Paris, Texas has the same zone as Paris, Illinois, thus it gives no row.

## Time zone rules

The crate uses `jiff` 1. jiff reads the rules from the system database in `TZDIR` or `/usr/share/zoneinfo`, so the rules stay current with the package `tzdata`. The caller gives the current time and the owner's time zone as one `ZonedDateTime`. Thus a test can use a fixed time and a fixed zone.

## Result texts

All times use the 24-hour clock. Each row has a main text, a second text, and the text that Enter copies. Enter copies the main text, as Enter does for a calculation (R8).

The examples use the owner's zone `Europe/Helsinki`, and Thursday 26 November 2026 when the example gives no other date.

| Query | Main text | Second text |
|---|---|---|
| `time now in new york` at 18:04 | `11:04, Thursday` | `Now in New York, New York, United States (EST, UTC-5), 7 h behind Helsinki` |
| `17:00 in las vegas` | `03:00, Friday` | `17:00 in Las Vegas, Nevada, United States (PST, UTC-8) is 03:00 tomorrow in Helsinki` |
| `17:00 here in las vegas` | `07:00, Thursday` | `17:00 in Helsinki is 07:00 today in Las Vegas, Nevada, United States (PST, UTC-8)` |
| `02:30 in new york` on 8 March 2026 | `09:30, Sunday` | `02:30 does not exist today in New York, New York, United States, the clocks go forward. 03:30 (EDT, UTC-4) is 09:30 today in Helsinki` |
| `01:30 in new york` on 1 November 2026, first row | `07:30, Sunday` | `01:30 in New York, New York, United States (EDT, UTC-4, first of two) is 07:30 today in Helsinki` |
| `01:30 in new york` on 1 November 2026, second row | `08:30, Sunday` | `01:30 in New York, New York, United States (EST, UTC-5, second of two) is 08:30 today in Helsinki` |
| `17:00 in atlantis` | `City not found: atlantis` | `petlauncher has no time zone for this city name` |

The rules for the parts of the texts:

- The label of a city is the city name, the state when there is one, and the country.
- After the label of a city, the parentheses give the zone abbreviation and the UTC offset, for example `(PST, UTC-8)` or `(IST, UTC+5:30)`. A zone with no abbreviation shows only the offset, for example `(UTC+14)` for Kiritimati.
- The label of the owner is the city part of the owner's zone name, for example "Helsinki" for `Europe/Helsinki`. A zone with no name gives "your time zone". The zone of the owner shows only for a time that exists two times.
- The row "City not found" has no text to copy, so Enter does nothing on it.

## Speed

Measured on 2026-11-26 on this computer (x86_64, 32 processor threads) with the example program `time-query`:

| Step | Release build | Debug build |
|---|---|---|
| Load the table of 4506 cities | 20 ms to 22 ms | 221 ms to 225 ms |
| Answer a time query, mean of 10 000 calls | 1.2 µs to 3.4 µs | 18 µs |
| Answer text that is not a time query, for example `firefox in` or `5 GiB in MB` | 48 ns to 84 ns | 0.5 µs |
| Answer `10 in 20`, which needs a city lookup | 0.2 µs | not measured |

In the release build, the parse of `Locations.xml` takes about 8 ms, the city extraction about 2 ms, and the load of about 450 zone rules about 7 ms. Load the table one time, in a background thread, because the debug build needs more than 0.2 s.

To measure again:

```sh
cargo run --release -p petlauncher-time --example time-query -- "17:00 in las vegas" "firefox in"
```

## Use in the engine

```rust
use petlauncher_time::{jiff::ZonedDateTime, Cities};

let cities = Cities::load_system()?; // one time, at the start
let rows = cities.answer(query, &ZonedDateTime::now());
```

When `load_system` gives an error, write the error to the log and give no time rows. For each `Answer`, `copy: Some(text)` becomes `Action::Copy(text)`, and `copy: None` becomes `Action::Nothing`.

## Tests

The tests use small copies of the three files in `crates/petlauncher-time/tests/fixtures/`. The zone rules come from the system database. The test dates are on or before 2026-11-26, so a new `tzdata` does not change the results. One test reads the real system files, and it does nothing when `Locations.xml` is not on the computer.
