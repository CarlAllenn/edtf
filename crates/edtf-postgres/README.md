# edtf-postgres

Postgres extension (pgrx) exposing
[`edtf-core`](https://crates.io/crates/edtf-core) in SQL — the same
validator the application runs via WebAssembly, so the two layers can never
diverge.

SQL surface: `edtf_valid(text)`, `edtf_level(text)`,
`edtf_canonical(text)`, `edtf_min(text)` / `edtf_max(text)` (index-friendly
date bounds), and `edtf_relation(text, text)` (three-valued temporal
relations).

## Installing

Prebuilt, attested tarballs are attached to each `edtf-postgres-v*` release
from v1.1.0 onward, for Postgres 14–18 on `amd64` and `arm64` — no Rust toolchain and no
`cargo-pgrx` required. Download the tarball for your major and architecture,
verify it, extract into `/`, then:

```sql
CREATE EXTENSION edtf_postgres;
```

The extension is `trusted`, so any user with `CREATE` on the database can
install it. Full instructions, the support matrix and the glibc floor are in
the [repository README](https://github.com/CarlAllenn/edtf#installing-the-postgres-extension).

Building from source instead needs `cargo-pgrx` and an initialised
`$PGRX_HOME`; see the repository for the development workflow.

Its contract is the SQL surface, not a Rust API. Part of the
[`edtf`](https://github.com/CarlAllenn/edtf) crate family.
