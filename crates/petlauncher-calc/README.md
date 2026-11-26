# petlauncher-calc

The calculator of the petlauncher query line: arithmetic with exact decimals,
percent, unit conversions and currency conversions with the ECB rates.

`calculate(query, rates)` gives `None` when the query is not a calculation.
The cargo feature `fetch` (on by default) adds the download of the rates.

Syntax, the rule that decides if a query is a calculation, the units, the
precision and the currency behaviour: [docs/calculator.md](../../docs/calculator.md)
