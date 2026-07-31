# edtf-calendars

Proleptic **Julian (Old Style) ↔ Gregorian** conversion at the EDTF ingest
boundary. Deterministic day-number arithmetic, `#![no_std]`, no third-party
dependencies (only `edtf-core`).

- `julian_to_gregorian` / `gregorian_to_julian` — complete dates, both
  directions, each validated under its own calendar's leap rule (Julian
  `1900-02-29` exists and converts; Gregorian `1900-02-29` never existed).
- `convert` — year / year-month / complete-date parts as they arrive from
  archival records, producing the Gregorian parts EDTF expressions are
  built from.

Part of the [`edtf`](https://github.com/CarlAllenn/edtf) crate family
(`edtf-core`, `edtf-normalize`, `edtf-calendars`, `edtf-cli`, `edtf-wasm`,
`edtf-postgres`).
