# Legacy cross-check fixtures

Copied read-only from monument-legacy on 2026-07-25. These are **oracles for
cross-checking, not authorities** — where they disagree with `docs/spec-notes.md`
(our ISO 8601-2 Annex A reading), the spec wins and the disagreement gets a note.

- `edtf-conformance-corpus.json` — parser-agnostic corpus built from Annex A
  examples + adversarial cases (53 valid across levels 0–2, 10 invalid), with
  earliest/latest bounds conventions designed for the Postgres layer
  (open end `..` → ±infinity, unknown end → null, un-date-boundable years → null).
- `edtf-golden.json` — behavior snapshot of the old edtf.js/plv8 engine
  (63 entries with min/max epoch-ms, type, qualification flags).

Known edtf.js-isms already spotted (do NOT carry over):

- `"198"` bare decade is valid in edtf-golden.json; bare decades are not in
  Annex A and we reject them (spec-notes §5).
