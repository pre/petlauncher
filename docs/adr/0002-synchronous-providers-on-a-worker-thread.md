# Providers are synchronous and the engine runs on a worker thread

Status: accepted.

A slow provider, such as the file index or the dictionary, must not block typing. The `Provider` trait has one synchronous method, `search(&self, text, settings, cancel) -> Vec<Row>`, and the user interface runs `Engine::search` on one worker thread with a new `CancelToken` for each query. Only fast providers answer in the general search, and a slow provider answers only in its keyword mode.

## Considered Options

- An `async` trait or a stream of rows: the core and each leaf crate would need an async runtime, and the leaf crates are plain Rust.
- One thread for each provider, with partial results: more code in the engine and in the user interface. The general search has only fast providers, so the gain is small.

## Consequences

- A provider that does not read `cancel` keeps the worker busy after the query changes, and the next query waits for it.
- The rows of the general search come in one piece, not provider by provider.
