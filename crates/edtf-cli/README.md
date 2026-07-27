# edtf-cli

Command-line EDTF (ISO 8601-2:2019 Annex A) toolbox over `edtf-core`:

- `validate` — parse/validate expressions from arguments or stdin, with
  positioned errors.
- `canonical` — spec-preferred rendering.
- `level` — minimum conformance level (0/1/2).
- `info` — JSON summary of each input (kind, precision, bounds, flags).
- `relation` — three-valued temporal relations between two expressions.
- `from-julian` — proleptic Julian (Old Style) → Gregorian conversion via
  `edtf-calendars`.

Run `edtf --help` for usage. Part of the
[`edtf`](https://github.com/CarlAllenn/edtf) crate family.
