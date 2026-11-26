# Calculator

The crate `petlauncher-calc` calculates the query line (R8, R15). It does
arithmetic, percent, unit conversions and currency conversions.

## Interface

| Item | Function |
|---|---|
| `calculate(query, rates)` | Gives `None` when the query is not a calculation. Then the launcher shows no calculation row. Else it gives a `Calculation`. |
| `is_calculation(query)` | The same decision as `calculate`, with no calculation. |
| `Calculation::text()` | The title of the row: `125.6`, `5368.70912 MB`, or an error message. |
| `Calculation::copy_text()` | The text that Enter copies: the number only, `125.6` or `5368.70912`. `None` for an error. |
| `Calculation::note` | Text for the subtitle, for example `ECB reference rates of 2026-11-25`. |
| `Calculation::result` | The value as an `Answer` (number and unit), or a `CalcError`. |

Each `Calculation` is a calculation row, also when it holds an error. The core
uses this for R9: Enter on a calculation row keeps the query.

The evaluator has no network code. The caller gives the currency rates. See
[Currencies](#currencies).

## When the query is a calculation

The launcher shows a calculation row for each query that passes this rule, so
the rule must not accept a normal search text.

A query is a calculation when all of these are true:

1. The query has 1000 characters or fewer, after the spaces at the start and
   at the end are removed.
2. The query is not a date. A date is three groups of digits with `-` or `/`
   between them, with no spaces, and the first or last group has four digits.
3. The full query is valid syntax. Each name is a known function, constant,
   keyword or unit.
4. The query has an operation: an operator, a function, a percent sign or a
   unit conversion. A sign before a bare number does not count.

| Query | Row | Cause |
|---|---|---|
| `42.3+68.7+14.1+0.5` | `125.6` | Operator |
| `15%` | `0.15` | Percent sign |
| `sqrt(2)` | `1.4142135623731` | Function |
| `5 GiB in MB` | `5368.70912 MB` | Unit conversion |
| `1/0` | `Division by zero` | Operator. An error also gives a row. |
| `5 kg in MB` | `Cannot convert mass to data size` | Known units, different dimensions |
| `42`, `-42`, `(42)`, `1e3`, `+358401234567` | None | Bare number |
| `e`, `pi`, `-pi` | None | Bare constant. `e` and `pi` are also the start of names such as `evince`. |
| `firefox`, `d launch`, `spur 123`, `c++` | None | Unknown name |
| `2024-11-24`, `24/11/2024`, `040-123-4567` | None | Date rule |
| `24.11.2024`, `1.2.3`, `192.168.1.1` | None | Not valid syntax |
| `~/a/b`, `/usr/bin`, `17:00 in las vegas` | None | Character or position that no calculation uses |
| `5 GiB`, `100 usd` | None | A unit with no target unit |
| `5 kg in parsec` | None | Unknown unit |
| `10,5`, `1,000+1` | None | A comma is valid only between function arguments |
| `2pi` | None | No implicit multiplication |

Two corrections help while the owner types:

- One operator at the end is ignored. `42+1+` shows `43`, so the row stays
  while the owner types. The operator does not count for step 4: `42+` and
  `e-` show no row.
- Missing `)` at the end are added. `sqrt(16` shows `4`. A `)` with no `(`
  makes the query not valid.

Known limits of the rule: `1/2/3` and `10-5-2` are calculations, because no
group has four digits. `2024-11` is a calculation with the result `2013`.

## Syntax

### Numbers

- The decimal mark is `.`. `,` is never a decimal mark and never a group
  separator. Reason: `1,000` can mean one or one thousand, and a wrong number
  is worse than no row. The owner types `.`.
- `.5` and `5.` are accepted.
- An exponent is accepted: `1.5e3`, `2E-3`. `2e` is the number 2 and the
  name `e`, so it is not valid.

### Operators

From the lowest to the highest precedence:

| Operators | Meaning | Associativity |
|---|---|---|
| `+`, `-` | Add, subtract | Left |
| `*`, `/`, `mod`, `of` | Multiply, divide, modulo, percent of | Left. `of` takes the rest of the term. |
| `-`, `+` before an operand | Sign | |
| `^`, `**` | Power | Right: `2^3^2` is 512 |
| `%` after an operand | Percent | |

- `-2^2` is -4. `2^-2` is 0.25.
- `mod` gives a result with the sign of the divisor: `-7 mod 3` is 2,
  `7 mod -3` is -2.
- `×`, `÷` and `−` (U+2212) are accepted for `*`, `/` and `-`.
- There is no implicit multiplication. Write `2*pi`, not `2pi`.
- Keywords (`mod`, `of`, `in`, `to`), function names and constant names
  ignore case.

### Percent

`%` is always percent. Modulo is `mod`.

| Form | Meaning | Example |
|---|---|---|
| `p% of x` | p/100 × x | `15% of 240` is 36 |
| `x + p%` | x × (1 + p/100) | `240 + 15%` is 276 |
| `x - p%` | x × (1 - p/100) | `240 - 15%` is 204 |
| `p%` in other places | p/100 | `15%` is 0.15, `240 * 15%` is 36 |

The rule for `+` and `-` applies when the right operand is a percent: `15%`,
`(15%)`, `(10+5)%` or `-15%`. In `1 + 15% * 2` the right operand is a product,
so the result is 1.3. `of` needs a percent before it: `3 of 240` is not valid.
`50% of 50% of 240` is 60.

### Functions and constants

Each function needs brackets. Angles are in radians.

| Function | Meaning |
|---|---|
| `sqrt(x)` | Square root |
| `exp(x)` | e to the power x |
| `ln(x)` | Natural logarithm |
| `log(x)`, `log10(x)` | Logarithm with base 10 |
| `log(x, b)` | Logarithm with base b |
| `log2(x)` | Logarithm with base 2 |
| `sin(x)`, `cos(x)`, `tan(x)` | Sine, cosine, tangent |
| `asin(x)`, `acos(x)`, `atan(x)` | Inverse sine, cosine, tangent |
| `sinh(x)`, `cosh(x)`, `tanh(x)` | Hyperbolic functions |
| `asinh(x)`, `acosh(x)`, `atanh(x)` | Inverse hyperbolic functions |
| `abs(x)` | Absolute value |
| `round(x)`, `round(x, n)` | Round to 0 or n decimal places, n from 0 to 28. A half goes away from zero: `round(2.5)` is 3, `round(-2.5)` is -3. |
| `floor(x)`, `ceil(x)` | Round down, round up |

| Constant | Value |
|---|---|
| `pi`, `π` | 3.1415926535897932384626433833 |
| `e` | 2.7182818284590452353602874714 |

Ulauncher also has `erf`, `erfc`, `gamma` and `lgamma`. petlauncher does not
have them.

### Unit conversion

`<value> <unit> in <unit>`, or `to` in place of `in`.

- The value can be an expression: `(1+2) kg in g`, `2^10 KiB in MiB`,
  `15% of 240 GiB in MiB`. The unit applies to the full expression, so
  `1 + 2 kg in g` is 3000 g.
- The space between the value and the unit is optional: `5GiB in MB`.
- Unit names ignore case: `5 gib to mb`.
- `in` is also the inch: `5 in in cm` is 12.7 cm.
- The result shows the symbol of the target unit. Enter copies the number
  only.

### Currency conversion

`<value> <code> in <code>`, for example `100 usd in eur`. The codes are ISO
4217 codes and ignore case. See [Currencies](#currencies).

## Precision and result text

The calculator uses the decimal type of the crate `rust_decimal`, not binary
floating point. The type holds 28 to 29 significant digits and 28 decimal
places at most. The range is about ±7.9e28.

Each value is exact or not exact:

- Exact: number literals, and `+`, `-`, `*`, `mod`, integer powers, `abs`,
  `round`, `floor` and `ceil` of exact values. A division is exact when it
  has no remainder: `10/4` is 2.5. A square root is exact when its square is
  the argument: `sqrt(16)` is 4. A unit conversion with exact factors is exact.
- Not exact: a division with a remainder, roots, powers with a fraction
  exponent, logarithms, trigonometric and hyperbolic functions, and the
  constants. A result that needs more than 28 digits is also not exact, for
  example `1.0825^12`.

The result text:

| Value | Text |
|---|---|
| Exact | All digits. `42.3+68.7+14.1+0.5` is `125.6`, `0.1+0.2` is `0.3`, `1 GB in GiB` is `0.931322574615478515625 GiB`. |
| Not exact, 1 or more | 15 significant digits. `1/7*7000` is `1000`, `10^20/3` is `33333333333333300000`. |
| Not exact, less than 1 | 15 decimal places. `1/3` is `0.333333333333333`. This removes the rounding noise: `1/3*3-1` is `0`, `sin(pi)` is `0`. Thus a very small value shows as `0`: `exp(-50)` is `0`. |
| Money | Two decimal places: `95.13`. Less than 1: four significant digits, with the zeros at the end removed: `0.006195`, `0.00574`. |

Rounding goes half away from zero. The text has `.` as the decimal mark, no
group separators, no exponent, no zeros at the end of the decimals, and no
`-0`. Thus another program can read the copied text.

Accuracy:

- The decimal functions are `sqrt`, `exp`, `ln`, `log`, `sin`, `cos`, `tan`
  and powers with a fraction exponent. For `sqrt(2)`, `ln(2)`, `exp(1)` and
  `sin(1)` they gave 27 correct digits. In all checks against `bc -l`, the 15
  shown digits were correct.
- `sin`, `cos` and `tan` give the error "The number is too large" when the
  argument is more than 1e14. A larger argument gives fewer than 15 correct
  digits.
- `tan` is `sin/cos`, because the tangent of `rust_decimal` has only about 8
  correct digits.
- `asin`, `acos`, `atan` and the hyperbolic functions use `f64`, because the
  decimal type does not have them. `f64` has about 16 significant digits, and
  the result shows 15.
- The check that a result is exact multiplies the digits in a 128-bit
  integer, which holds 38 digits. When the check needs more digits, the
  result shows as not exact, with 15 significant digits. Example:
  `1 TB in TiB` is `0.909494701772928 TiB`, but the exact value
  `0.9094947017729282379150390625` has 28 decimal places.

Errors:

| Error | Example | Text |
|---|---|---|
| `DivisionByZero` | `1/0`, `5 mod 0`, `0^-1` | `Division by zero` |
| `TooLarge` | `10^30`, `exp(100)`, `sin(1e15)` | `The number is too large` |
| `Undefined` | `sqrt(-1)`, `ln(0)`, `acos(2)`, `(-8)^0.5`, `tan(pi/2)` | `No real result for sqrt` |
| `InvalidArgument` | `round(1, 1.5)` | `The places of round must be a whole number from 0 to 28` |
| `IncompatibleUnits` | `5 kg in MB` | `Cannot convert mass to data size` |
| `NoRates` | `100 usd in eur` with no rates | `No currency rates available` |
| `MissingRate` | A code that is in the list below but not in the rates | `No rate for SEK` |

## Units

All factors are exact decimals. The base units are the bit, the second, the
metre and the kilogram.

### Data size

| Symbol | Other names | Size |
|---|---|---|
| `bit` | bits | 1 bit |
| `kbit` | kilobit, kilobits | 1000 bits |
| `Mbit` | megabit, megabits | 10^6 bits |
| `Gbit` | gigabit, gigabits | 10^9 bits |
| `Tbit` | terabit, terabits | 10^12 bits |
| `B` | byte, bytes | 8 bits |
| `kB` | kilobyte, kilobytes | 1000 B |
| `MB` | megabyte, megabytes | 10^6 B |
| `GB` | gigabyte, gigabytes | 10^9 B |
| `TB` | terabyte, terabytes | 10^12 B |
| `PB` | petabyte, petabytes | 10^15 B |
| `KiB` | kibibyte, kibibytes | 1024 B |
| `MiB` | mebibyte, mebibytes | 1024^2 B |
| `GiB` | gibibyte, gibibytes | 1024^3 B |
| `TiB` | tebibyte, tebibytes | 1024^4 B |
| `PiB` | pebibyte, pebibytes | 1024^5 B |

Case is ignored, so `KB`, `kb` and `Mb` are bytes: `KB` is 1000 B and `Mb` is
a megabyte. For bits, write `bit`: `Mbit`. For 1024 B, write `KiB`.

### Time

| Symbol | Other names | Size |
|---|---|---|
| `ms` | millisecond, milliseconds | 0.001 s |
| `s` | sec, secs, second, seconds | 1 s |
| `min` | mins, minute, minutes | 60 s |
| `h` | hr, hrs, hour, hours | 3600 s |
| `d` | day, days | 86400 s |
| `wk` | wks, week, weeks | 604800 s |

There are no months and no years, because their length changes.

### Length

| Symbol | Other names | Size |
|---|---|---|
| `mm` | millimeter(s), millimetre(s) | 0.001 m |
| `cm` | centimeter(s), centimetre(s) | 0.01 m |
| `m` | meter(s), metre(s) | 1 m |
| `km` | kilometer(s), kilometre(s) | 1000 m |
| `in` | inch, inches | 0.0254 m |
| `ft` | foot, feet | 0.3048 m |
| `yd` | yard, yards | 0.9144 m |
| `mi` | mile, miles | 1609.344 m |
| `nmi` | | 1852 m |

### Mass

| Symbol | Other names | Size |
|---|---|---|
| `mg` | milligram, milligrams | 0.000001 kg |
| `g` | gram, grams | 0.001 kg |
| `kg` | kilogram, kilograms, kilo, kilos | 1 kg |
| `t` | tonne, tonnes | 1000 kg |
| `oz` | ounce, ounces | 0.028349523125 kg |
| `lb` | lbs, pound, pounds | 0.45359237 kg |
| `st` | stone, stones | 6.35029318 kg |

### Temperature

| Symbol | Other names | Conversion |
|---|---|---|
| `°C` | C, degC, celsius | |
| `°F` | F, degF, fahrenheit | °F = °C × 1.8 + 32 |
| `K` | kelvin, kelvins | K = °C + 273.15 |

`100 C in F` is `212 °F`. `100 F in C` is `37.7777777777778 °C`.

Not supported: volume, area, speed, energy, pressure, and units with `/`
such as `km/h`.

## Currencies

Source: the daily reference rates of the European Central Bank (ECB),
https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml

The ECB gives each rate as units of the currency for 1 EUR. A conversion goes
through EUR: amount × rate of the target / rate of the source.

The calculator knows these codes also when it has no rates: EUR, AUD, BRL,
CAD, CHF, CNY, CZK, DKK, GBP, HKD, HUF, IDR, ILS, INR, ISK, JPY, KRW, MXN,
MYR, NOK, NZD, PHP, PLN, RON, SEK, SGD, THB, TRY, USD, ZAR. This is the list
of the ECB file on 2026-11-26. A code that is not in the list is not a unit,
and the query gives no row.

| State | Row |
|---|---|
| Rates are available | `91.45 EUR`, with the note `ECB reference rates of 2026-11-25` |
| No rates | `No currency rates available`. The row shows, so the owner sees why there is no value. |
| The rates have no rate for the code | `No rate for XXX` |

The note always gives the date of the rates. When the network is not
available, the program uses the stored rates, and the note shows their date.

Functions of the module `rates`:

| Function | Function |
|---|---|
| `parse_ecb(xml)` | Reads the ECB file into `Rates`. |
| `load_cache(dir)` | Reads `dir/ecb-eurofxref-daily.xml`. `Ok(None)` when the file does not exist. |
| `save_cache(dir, xml)` | Makes `dir` if necessary and writes the file. It writes a temporary file and renames it. A text that is not a valid ECB file does not replace the cache. |
| `fetch_ecb(timeout)` | Downloads the ECB file. |
| `update_cache(dir, timeout)` | `fetch_ecb`, then `save_cache`. |

The program gives `~/.cache/petlauncher/` as `dir`. The cache file is the
unchanged ECB file, so one parser reads the download and the cache.

The download:

- is in the cargo feature `fetch`, which is on by default. With
  `default-features = false`, the crate has no HTTP client.
- uses `ureq` with `rustls` and the system trust store
  (`rustls-platform-verifier`). On Ubuntu this is
  `/etc/ssl/certs/ca-certificates.crt`. Thus a local proxy that replaces the
  certificates works when its root certificate is in the system store. The
  list of root certificates that `ureq` compiles in is not used.
- stops at the timeout that the caller gives, for the full request.
- accepts 1 MB at most. The real file is about 1.5 KB.

When and how often to download is the decision of the caller. The ECB updates
the rates on working days at about 16:00 CET.

To download the rates by hand:

```sh
cargo run -p petlauncher-calc --example fetch_rates -- ~/.cache/petlauncher
```
