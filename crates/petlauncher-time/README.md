# petlauncher-time

Time in other time zones for the petlauncher query line (R20), for example `time now in new york`, `17:00 in las vegas` and `6am in helsinki`.

The city names come from the time zone database of the system and from the GNOME city list of libgweather 4. The query forms, the city sources, the match rule and the result texts are in [docs/time-zones.md](../../docs/time-zones.md).

Try a query with the system files:

```sh
cargo run --release -p petlauncher-time --example time-query -- "17:00 in las vegas"
```
