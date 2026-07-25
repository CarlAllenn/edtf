# edtf

Pure-Rust implementation of EDTF (Extended Date/Time Format, ISO 8601-2), levels 0–2.

One core crate, two thin wrappers — so the app and the database validate dates with literally the same code:

- **`edtf-core`** — all parsing/validation logic. Zero dependencies. Optional `serde` feature for JSON in/out.
- **`edtf-wasm`** — compiles the core to WebAssembly for use from JavaScript (replaces edtf.js).
- **`edtf-postgres`** — wraps the core as a Postgres extension via pgrx (`edtf_valid()` etc., replaces plv8).

## Development

Tooling is pinned via [mise](https://mise.jdx.dev). Common commands live in the Taskfile:

```
mise install   # install pinned Rust toolchain
task test      # run core test suite
task check     # fmt + clippy + test
```

The ISO 8601-1/-2 source documents live in `spec/` locally but are copyrighted and never committed.
