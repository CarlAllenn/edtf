# edtf-normalize — decisions and scope

The authority document for `crates/edtf-normalize` (issue #20): a bounded,
deterministic pattern grammar over date prose. Companion to
`docs/spec-notes.md`, which governs what valid EDTF *is*; this document
governs how prose becomes EDTF. Every judgement call below has a numbered
**N-decision** id; `Note` values returned by the crate cite these ids, test
files reference them in doc comments, and the oracle fixture
(`tests/fixtures/natlang/python-edtf-natlang.json`) cites them per entry.
Disagree with one? Each is a single table row or match arm — file an issue
against the N-number.

## 1. Scope and architecture

- The grammar is **whole-input**: the entire (noise-stripped) input must match
  one pattern. There is no substring extraction (N11) — pulling dates out of
  running prose is the job of the caller (a human, or an LLM triage step for
  bulk imports whose no-match rows escalate to a human).
- Values are constructed through the `edtf-core` model and rendered by core's
  canonical `Display`, then **re-parsed**. Core has no validating
  constructors, so the re-parse is the calendar check (a constructed
  February 31 fails closed) and the proof that every output is valid EDTF at
  a known level.
- The return type is honest: `Normalized { edtf, value, notes }`,
  `Ambiguous { interpretations }`, or `NoMatch { reason }`. Nothing ever
  guesses silently; every non-obvious mapping carries a `Note` citing its
  N-decision, and `NoMatchReason` discriminates out-of-grammar (N11),
  explicitly-undated (N12), and impossible-date (N14) failures so a
  bulk-import triage step can route them differently.
- Pattern tables are per-language (`Language::English`, `Language::Russian`).
  The grammar in `engine.rs` is language-neutral; a locale is one `Lang`
  const in `tables.rs` (word tables, era phrases, modifier phrases, range
  words, numeric-order convention). Contributions add tables, not code.
- Already-valid EDTF passes through untouched (canonicalized only, noted) —
  with one deliberate exception, N13.

## 2. Decisions

- **N1 — early/mid/late and halves of centuries:** ISO 8601-2 is silent on
  prose; python-edtf silently *discards* these modifiers ("1857-mid 1860s" →
  `1857/186X` in its own tests). We map standalone part-of-century phrases to
  decade-rounded year intervals: for the century starting `s`, **early** =
  `s..s+29`, **mid** = `s+30..s+69`, **late** = `s+70..s+99`, **first half**
  = `s..s+49`, **second half** = `s+50..s+99`. "early 19th century" →
  `1801/1830`. Thirds and halves each partition the century exactly
  (property-tested). BC centuries take their parts chronologically:
  "late 2nd century BC" → `-0129/-0100`. Scope limits, both recorded with
  `Note::ModifierDropped`: on **decades** ("early 1930s") the modifier is
  dropped — sub-decade thirds would be false precision; inside **range
  endpoints** ("конец XIX - начало XX века") a century modifier collapses to
  the whole century, because an interval cannot nest inside an interval.
- **N2 — century arithmetic:** the *n*th century runs `(n-1)·100+1 ..= n·100`
  (19th century = 1801–1900), so it masks as `(n-1)XX`: "19th century" →
  `18XX`, "1st century" → `00XX`. **BC centuries cannot be masked**:
  edtf-core rejects unspecified digits in negative years (Annex A's L1 mask
  shapes are positive-only), and the 1st century BC would need `-0000`. They
  are emitted as exact astronomical intervals: "2nd century BC" →
  `-0199/-0100`, "1st century BC" → `-0099/0000`, carrying
  `BcCenturyInterval` (never `CenturyMask`, whose masking claim would be
  false for them). python-edtf emits `-01XX`, which our core rejects —
  recorded as an ism, not adopted.
- **N3 — astronomical BC years:** EDTF/ISO 8601-2 uses astronomical
  numbering: year 0 exists and is 1 BC. Year *b* BC = `-(b-1)`: "500 BC" →
  `-0499`, "64 BCE" → `-0063`. Year markers may sit *before* the era phrase
  ("44 г. до н. э.", the standard Russian textbook form) — trailing noise is
  stripped again after the era splits off. python-edtf applies the offset to
  centuries but *not* to years (its tests demand `c64 BCE` → `-0064~` yet
  `2nd century bc` → `-01XX` = −199..−100) — the internal inconsistency is
  recorded as an ism and its year outputs are not adopted.
- **N4 — elided end years:** in a hyphen range whose right side is two
  digits, the end year inherits the start's century: "1914-18" → `1914/1918`,
  "1851-52" → `1851/1852`. The elided value must exceed the start year;
  otherwise no range reading exists. Two-digit ends 01–12 are never ranges
  (they read as EDTF months) and 21–41 collide with sub-year codes (N13) —
  and these limits apply identically to the spaced and range-word forms
  ("1914 - 21", "from 1914 to 21"), so spacing cannot smuggle a reading past
  them. A **BC start refuses elision entirely** ("500 BC to 18" → `NoMatch`):
  no deterministic reading exists — the CE reading, the BC-ordinal reading,
  and truncating century arithmetic all disagree — so per the contract the
  normalizer refuses rather than fabricates.
- **N5 — all-numeric dates:** "12/04/1985" is DD/MM or MM/DD; without context
  the order is *unknowable* and the result is `Ambiguous` with both readings
  — never python-edtf's silent `dayfirst=False` guess. Resolution, in
  precedence order, each with its own note: a field > 12 proves the order;
  equal fields make it irrelevant; `Options::numeric_order` (the form's
  locale) resolves it; the *language's* convention resolves it (Russian
  `12.04.1985` is day-first by national convention). Year-first
  (`1985/04/12`) is inherently unambiguous. Bare two-digit years
  ("12/04/85", "90") are not adopted: the century is unknowable.
- **N6 — decades:** "1980s"/"1980-е" → `198X`. Two forms are inherently
  ambiguous and reported as such: "**1900s**" is the 1900–1909 decade
  (`190X`) *or* the century (`19XX`); a **bare decade** ("the 80s", "80-е")
  could sit in any century — we offer the 1800s and 1900s readings, and
  `Options::default_century` (e.g. `Some(1900)`, domain `0..=9999`) resolves
  it for forms whose material has a known era. A decade tied to an explicit
  century is not ambiguous at all: "60-е годы XIX века" → `186X`
  (`DecadeOfCentury`), the idiom behind «шестидесятники». python-edtf is
  internally inconsistent here ("1800s" → `18XX` but "c1900s" → `190X~`) —
  recorded, not adopted.
- **N7 — seasons:** season names map to ISO 8601-2 §4.8 sub-year grouping
  codes 21–24 (northern-hemisphere reading: spring=21, summer=22,
  autumn/fall=23, winter=24; Russian весна/лето/осень/зима likewise). Codes
  25–41 (hemisphere-specific, quarters, semesters) are **never emitted** —
  no deterministic English or Russian prose form maps to them.
- **N8 — before/after are open intervals:** "before 1917" → `../1917`,
  "after 1917" → `1917/..` (current-spec `..`, not python-edtf's old-draft
  bare slash `/1928`). Interval, not set, because the prose asserts a single
  event somewhere in an open span — `[..1917]` (a one-of set) says the value
  is one of an enumerated list, which is a different claim and a level-2
  feature besides. "no later than" style phrases (`не позднее`) read the
  same way.
- **N9 — missing years are masked:** a *standalone* month with no year
  ("January", "January 12", "апрель") gets year `XXXX`: `XXXX-01`,
  `XXXX-01-12`. Never an assumed current year — the normalizer has no clock
  on purpose (determinism: same input, same output, forever). Inside a range
  the year is elided, not missing — N16 governs there, and `XXXX` never
  leaks into a range that states its year. python-edtf's broader
  masked-precision prose ("day in Spring 1849" → `1849-21-XX`) is not
  adopted: "a day in" is annotation, not a date.
- **N10 — whole-expression qualifiers distribute:** a qualifier scoping the
  whole input applies to every component ("circa June 1940" → `1940-06~`,
  rendered in core's canonical trailing form) and, on intervals, to every
  dated endpoint: "circa 1914-1918" → `1914~/1918~`, "1868-1871?" →
  `1868?/1871?`. python-edtf attaches the trailing qualifier to the end year
  only (`1868/1871?`); prose uncertainty scopes the whole range, so we
  diverge. Endpoint-local qualifiers stay local: "1856-ca. 1865" →
  `1856/1865~`.
- **N11 — no substring extraction:** inputs with trailing annotations
  ("1861-67 (later print)", "1863, printed 1870", "birthday in 1872") are
  `NoMatch`, full stop. python-edtf extracts a date from all of these; that
  behavior is the single largest recorded ism family. The boundary is
  deliberate: deterministic keystroke normalization here, freeform
  extraction in the caller's triage layer.
- **N12 — "unknown" is the form's problem:** "unknown", "undated", "n.d.",
  "без даты", "не датировано" → `NoMatch` with
  `NoMatchReason::ExplicitNoDate` — distinguishable from out-of-grammar
  prose (`OutOfGrammar`, N11) so a triage step can route "explicitly
  undated" rows without re-reading the input. Whether that means an EDTF
  `XXXX`, an empty column, or a required-field error is form policy, not
  normalizer policy.
- **N13 — the sub-year-code / elided-range collision:** `NNNN-NN` with NN in
  21–41 is *valid EDTF* (a sub-year code: "1914-21" is spring 1914) **and**
  a plausible elided range (1914–1921). This is the one place already-valid
  EDTF does not pass through silently: when the elided reading is
  chronologically possible, both readings are reported. Elided ends 13–20
  and 42–99 are not valid codes, so only the range reading exists
  ("1914-18" → `1914/1918`).
- **N14 — alternatives stay alternatives, and dead readings poison:**
  "1863 or 1864" ("1863 или 1864") → `Ambiguous` with both years.
  python-edtf silently keeps the first. When any reading of an ambiguous
  input dies at the calendar check ("31 June 1940 or 1 July 1940", a
  reversed masked range like "1910s to 1900s"), the whole input fails
  closed with `NoMatchReason::ImpossibleDate` — promoting the survivor
  would fake certainty the prose never had. (A future extension could offer
  a one-of set `[1863, 1864]` as a third reading; today the two plain
  readings keep the form's choice simple.)
- **N15 — Roman-numeral centuries (Russian):** "XIX век", "XIX в.",
  "XVII-XIX вв." parse as centuries, including with Cyrillic lookalike
  letters (Х/х for X, etc.), which real Russian sources routinely contain,
  and including numerals that *start* with a subtractive pair ("IV век",
  "IX-X вв." — the accumulator is signed for exactly this reason). A *bare*
  Roman numeral is accepted as a century only when it is at least two
  letters ("I" alone is too weak a signal; "I век" is fine). Roman
  provenance is cited on results as `Note::RomanCentury`.
- **N16 — range endpoints inherit the stated year:** "June-July 1940",
  "с июня по июль 1940", "12 January to 15 March 1940" — when exactly one
  endpoint lacks a year and the other states one, the year scopes both: the
  year-less endpoint inherits it (`EndpointYearDistributed`), symmetric to
  N4's century inheritance and direction-aware ("from January 1940 to June"
  fills rightward). Emitting `XXXX` there would assert the year is unknown
  when the prose gives it. Reversed month ranges ("July-June 1940") fail
  closed at the calendar check. Both-endpoints-year-less ranges
  ("May-June") keep their masks: no year exists anywhere in the input.
- **N17 — cross-year winters are ambiguous:** "winter 1941-42" (also
  "-1942", "зима 1941-1942 гг.") names either ONE winter spanning the year
  boundary — EDTF's code 24 wraps, so `1941-24` already covers Dec 1941 –
  Feb 1942 — or winter 1941 through the whole of 1942. Both readings are
  reported (`CrossYearSeason`), N13-style. A non-adjacent end year
  ("winter 1941-45") has only the range reading. Winter is the only emitted
  season that wraps; the others compose as plain ranges.

## 3. Test ceremony

- `tests/traps.rs` / `tests/traps_ru.rs` — the acceptance trap lists; every
  expected output is re-parsed in core and its level asserted.
- `tests/props.rs` — constructive property tests: century parts partition
  exactly; BC arithmetic; and the umbrella invariant that anything emitted
  parses in core and equals its returned `value`.
- `tests/oracle.rs` over
  `tests/fixtures/natlang/python-edtf-natlang.json` — the python-edtf
  corpus, bucketed: `agreements` (enforced equal), `divergences` (enforced
  to OUR documented output, each citing an N-decision, each recording their
  ism), `not_adopted` (their extractions/guesses, enforced `NoMatch`).
  python-edtf is an **oracle, not an authority**: agreement is evidence,
  divergence is documented policy, and this crate takes precedence.

## 4. Non-goals

Freeform NLP, sentence extraction, multilingual guessing without tables,
wall-clock-relative dates ("last summer"), and any output that does not
parse in `edtf-core`.
