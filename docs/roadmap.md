# Roadmap

What this project intends to do — and deliberately not do — over roughly
the next year (written 2026-08). A roadmap is a statement of direction,
not a promise; when it changes, this file changes with it.

## Will do

- **Maintain spec-exactness as the invariant.** `edtf-core` implements
  ISO 8601-2:2019 Annex A levels 0–2 completely; the ongoing work there is
  defence (fuzzing, corpus growth, decision-register upkeep), not feature
  growth. Interpretation changes happen only as numbered D-decisions in
  [spec-notes.md](spec-notes.md).
- **Grow the normaliser tables.** `edtf-normalize`'s English and Russian
  pattern tables expand as real archive prose surfaces new forms; the
  Russian table in particular tracks an active research corpus. Every
  addition keeps the existing contract: ambiguous input returns every
  reading, never a guess.
- **Keep the deployment shapes first-class.** The Postgres extension
  (pg14–18, amd64+arm64 tarballs and the OCI image), the npm WebAssembly
  package, and the prebuilt CLI binaries all publish from the same
  attested release pipeline; new Postgres majors are picked up as pgrx
  supports them, and Debian base bumps ride the release cadence.
- **Close the remaining release-pipeline gaps** tracked in
  [issue #83](https://github.com/CarlAllenn/edtf/issues/83): the strip /
  debug-symbols decision, canary architecture coverage, and binding SBOMs
  to the tarball subjects.
- **Serve the downstream consumers** (the Monumental Archive's database
  and web layers, and the IIIF server) — their integration needs drive
  priority, since running the same `edtf-core` in Postgres and in the
  browser, never diverging, is the reason this project exists.

## Will not do

- **No levels beyond the standard.** Annex A defines levels 0–2; there is
  no level 3, and no vendor extensions to the grammar will be accepted.
- **No guessing.** The normaliser will not adopt fuzzy or ML-based date
  extraction, and will not silently pick a reading of ambiguous input —
  `12/04/1985` returns both readings forever.
- **No substring extraction.** The normaliser matches whole expressions;
  scanning free text for embedded dates is out of scope.
- **No calendars beyond Julian conversion.** `edtf-calendars` converts
  Julian ↔ proleptic Gregorian; Hebrew, Islamic, French Revolutionary and
  friends are out of scope.
- **No databases other than Postgres**, and no ORM layers; the
  extension's contract is its SQL surface.
- **No new runtime dependencies in the core.** `edtf-core` stays
  `#![no_std]` and zero-dependency.
