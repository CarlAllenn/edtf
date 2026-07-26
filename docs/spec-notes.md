# EDTF implementation notes (from ISO 8601-1:2019 and ISO 8601-2:2019)

Working distillation for building `edtf-core`. Paraphrased from the ISO documents in
`spec/` (never committed); section numbers cite ISO 8601-2:2019 unless prefixed "P1"
(= ISO 8601-1:2019). This is the authority for what the parser accepts and rejects.

## 1. The single most important fact

EDTF is **Annex A of ISO 8601-2** — an ISO 8601 *profile* (Clause 15) that selects a
strict subset of the two documents. Everything we implement is defined by Annex A's
feature list, not by ISO 8601-2 at large. Consequences:

- **Extended format only** (A.3): implicit form with separators (`1985-04-12`).
  Basic format (`19850412`) and the entire "explicit form" system (`1985Y4M12D`,
  designators K/O/J/C/G/U/L/N…) are **not EDTF** and must be rejected.
- Features of ISO 8601-1 not listed in Level 0 are **suppressed** (A.2): ordinal
  dates (`1985-102`), week dates (`1985-W15-5`), durations (`P1Y…`), recurring
  intervals (`R…`) are all invalid EDTF.
- Whole clauses of 8601-2 are irrelevant to EDTF: Clause 5 (grouped units G…U),
  Clause 7 (explicit representation), Clause 11 (explicit duration), Clause 12
  (selection rules), Clause 13 (repeat rules), Clause 14 (arithmetic). We do not
  implement them.
- Annex A is *informative* and is the ISO codification of Library of Congress EDTF
  1.0 (2018-10-04) (Bibliography [4]). Where Annex A is ambiguous, LoC EDTF is the
  tiebreaker for interop — flagged in §9 below.

Conformance claims are per level, cumulative: L1 ⊃ L0, L2 ⊃ L1 (A.2).

## 2. Level 0 (A.4)

### 2.1 Date (A.4.2)
- `[year]-[month]-[day]` → `1985-04-12` (P1 5.2.2.1 b)
- `[year]-[month]` → `1985-04` (P1 5.2.2.2 a)
- `[year]` → `1985` (P1 5.2.2.2 b)

Rules inherited from Part 1:
- Year: exactly 4 digits `0000`–`9999` at L0; leading zeros mandatory (P1 4.5).
  Year 0000 exists (proleptic; P1 4.3.2).
- Month `01`–`12`; day `01`–`28/29/30/31` per month lengths in P1 4.2.1 Table 1.
- Leap year: divisible by 4 and not a centennial, or divisible by 400 (P1 3.1.1.21).
- **Day-of-month must be validated against month+year** (`1985-02-30` invalid;
  `2001-02-29` invalid; `2000-02-29` valid).
- No space characters anywhere (P1 3.2.1).

### 2.2 Date and time (A.4.3)
`[date]T[hh]:[mm]:[ss]` with optional shift; date part must be complete (P1 5.4.1):
- `1985-04-12T23:20:30` (local)
- `1985-04-12T23:20:30Z` (UTC)
- `1985-04-12T23:20:30+04:30` / `-05:00` (hours+minutes shift, P1 4.3.13 c)
- `1985-04-12T23:20:30+04` (hour-only shift, P1 4.3.13 b)

Rules:
- Hour `00`–`23` — **`24:00` is explicitly disallowed** (P1 5.3.2); minute `00`–`59`;
  second `00`–`60` where `60` exists only for positive leap seconds (P1 4.3.10) —
  see decision D3 in §9.
- Time is always complete hh:mm:ss in the EDTF profile (A.4.3 shows only complete
  [timeI]); reduced-precision times (`T23:20`) and decimal fractions are not listed
  in Annex A → reject. (LoC EDTF agrees: complete time only.)
- Shift sign: `+` for ahead-of/equal-to UTC, `-` behind (P1 4.3.13). `-00:00` is
  not given meaning by the spec — see D4.

### 2.3 Time interval (A.4.4)
`start/end` where **both sides are dates only** (no times, no durations):
- `1964/2008`, `2004-06/2006-08`, `2004-02-01/2005-02-08`
- Mixed precision endpoints allowed: `2004-02-01/2005`, `2005/2006-02` (A.4.4
  Examples 4–6; the interval's overall precision is then "undefined").
- Semantics: starts "sometime in" start, ends "sometime in" end.
- The Part 1 allowance to omit higher-order components from the end
  (`2018-01-15/02-20`, P1 5.5.1) is **not** in Annex A → reject (LoC agrees).
- End must not precede start when both resolve (cf. 7.14.2 Example 2 states this
  for explicit form; apply the same rule).

## 3. Level 1 (A.5)

### 3.1 Letter-prefixed ("extended") year (A.5.2, 4.7.2)
`Y[-]digits` — `Y170000002`, `Y-170000002`. For years beyond ±9999. 4.7.2 says it
"should be used only" outside the 4-digit range → we reject `Y1985` (see D1).
Year-only precision (4.7.2: "only for dates that include the calendar year only") —
no month/day may follow a Y-year.

### 3.2 Negative calendar year (A.5.2, 4.4.1.2)
`-1985` (4 digits). `-0000` is not a thing: negative years count *before year 0*,
so the year before `0000` is `-0001` (4.4.1.2 Example 3). Reject `-0000` (D2).

### 3.3 Seasons (A.5.3, 4.8.1, 4.8.3 a)
Year + season code in the month slot: `2001-21`. L1 codes are **21–24**
(Spring/Summer/Autumn/Winter, location-independent). Season may not carry a
day component (`2001-21-05` invalid — 4.8.3 applies to year-and-month
expressions only).

### 3.4 Qualification — whole-expression only (A.5.4)
Qualifier symbols (3.2.6): `?` uncertain, `~` approximate, `%` both.
At L1 a single qualifier may appear **only at the rightmost end** of:
- complete date: `1985-04-12?` (8.4.1 a)
- year-month: `1985-04?` (8.4.2 a)
- year: `1985~` (8.4.2 b)
It qualifies the entire expression (8.2.1). Note 8.4.2's NOTE: `198%`-style decade
qualification means "the decade might be a different decade," not "fuzzy edges."
(Decades/centuries as bare `198`/`19` are NOT EDTF — not listed in Annex A.)

### 3.5 Unspecified digits — restricted right-to-left set (A.5.5)
`X` replaces digits. L1 allows exactly these shapes:
- `1985-04-XX` (day unspecified, 9.2.1.1 a)
- `1985-XX-XX` (month+day unspecified, 9.2.1.1 b)
- `2004-XX` (month unspecified, reduced precision, 9.2.1.2 a)
- `201X`, `20XX` (1 or 2 rightmost year digits, 9.2.1.2 c 1–2)
NOT at L1: `XXXX`, `2XXX`, `XXXX-XX`, `156X-12-25`, `1560-X2` (those are L2 or
disallowed entirely — `XXXX`/`2XXX` are 9.2.1.2 c 3–4, listed only under… nothing
in Annex A explicitly; LoC puts `XXXX` at L2 "unspecified digit anywhere". See D5.)
Semantics note (9.2.1.2 NOTE): `1985-XX-XX` has *day precision* (some day in 1985);
`1985` has year precision. Different meanings.

### 3.6 Extended intervals (A.5.6, 10.2, 10.3.2)
Interval endpoints gain two special values:
- `..` = **open** (unbounded): `1985-04-12/..`, `../1985-04-12` (10.2 a–b)
- empty = **unknown**: `1985-04/`, `/1985` (10.2 c–d)
- `/..`, `../`, and both-sides cases: `../..` and `/` are not exemplified anywhere —
  reject both (D6).
Endpoint dates may carry L1 whole-date qualification: `1984?/2004%`,
`1984-01-02~/2004-06-04`, `1984~/2004-06` (10.3.2), and combine with open/unknown:
`../1985-04-12?`, `1985-04-12~/` (A.5.6 Examples 8–9, 10.5).

## 4. Level 2 (A.6)

### 4.1 Exponential year (A.6.2, 4.7.3)
`Y[-]significandE[exponent]`: `Y17E7` = 170 000 000; `Y-17E7`. Exponent is a
positive integer power of 10 (4.4.2). Year-only precision.

### 4.2 Significant digits on years (A.6.3, 4.4.3, 4.7.4)
`S[precision]` suffix, three forms:
- `1950S2` — plain 4-digit year: some year 1900–1999, estimated 1950
- `Y171010000S3` — prefixed year: some year 171010000–171010999
- `Y3388E2S3` — exponential year: some year 338000–338999, estimated 338800
Precision counts significant digits **from the left** of the resolved value (4.4.3).
Precision must be ≥1 and ≤ number of digits of the value (D7).

### 4.3 Extended sub-year groupings (A.6.4, 4.8.1)
Month-slot codes **25–41**: hemispheric seasons 25–32 (N-Spring…S-Winter),
quarters 33–36, quadrimesters 37–39, semestrals 40–41. So at L2 the month slot
accepts 01–12 and 21–41.

### 4.4 Sets (A.6.5, Clause 6)
- `{a,b,…}` = "all members" (6.1); `[a,b,…]` = "one member" (6.2).
- Elements are date expressions, may differ in precision: `{1960, 1961-12}`.
- `..` inside sets (6.3): prefix `..1984` (on or before), suffix `1984..` (on or
  after), infix `1670..1673` (all between, inclusive; both sides should share
  precision). Combinations allowed:
  `..1983-12-31,1984-10-10..1984-11-01,1984-11-05..` (6.3 d) — note A.6.5
  Example 6 shows this bare, without braces (see D8).
- No spaces after commas (6.1 NOTE).
- What may be an element: Annex A examples show dates only. Qualified/unspecified
  dates inside sets: LoC allows them; Annex A silent (D9).

### 4.5 Group + individual qualification (A.6.6, 8.2–8.4)
Three placements now:
- **Complete** (L1): trailing `?~%` qualifies everything (8.2.1).
- **Group** (8.2.2): qualifier immediately *right* of a component qualifies that
  component **and everything to its left**: `2004-06~-11` (year+month approx),
  `2004?-06-11` (year only).
- **Individual** (8.2.3): qualifier immediately *left* of a component qualifies
  that component only: `?2004-06-~11` (year uncertain, day approx, month known),
  `2004-%06-11` (month uncertain+approx only).
- Mixing group and individual is legal: `2004-06?-~11` (8.4.6).
- Preferred/canonical form (8.2.4): complete > group > individual; strip redundant
  qualifiers; combine adjacent individuals into a group. (Parser accepts all;
  a formatter should emit preferred form.)
- Interval endpoints may carry partial qualification: `2004-06-~01/2004-06-~20`
  (10.3.3), and `..`-prefixed/suffixed endpoints combine with qualifiers:
  `..2004-06-01/2004-06-~20`, `2004-06-01~/2004-06-20..` (10.6) — i.e. at L2 an
  interval endpoint may be "before or on X" / "X or after". Note A.6.6 Example 6
  misprints this as `..2004-06-01/~2004-06-20` — see D10.

### 4.6 Unspecified digit anywhere (A.6.7, 9.2.2, 4.6.3)
`X` may replace **any** digit in year, month, or day: `156X-12-25`, `15XX-12-25`,
`XXXX-12-XX`, `1XXX-XX`, `1XXX-12`, `1XX3`, `1560-XX-25`, `1560-X2` (= Feb or Dec —
partially-specified components constrain the value set, 9.2.2 Example 6).
Validation question for partially specified components: does `1985-0X-31` need at
least one consistent completion? See D11.

## 5. What is NOT EDTF (reject list)

Basic format `19850412`; ordinal dates `1985-102`; week dates `1985-W15`;
durations `P1Y2M`; recurring intervals `R12/…`; explicit-form designators
`Y/M/D/K/O/J/C/B` as component suffixes (`1985Y4M12D`, `12YB`); decade `198` and
century `19` as bare forms; grouped units `G…U`; selections `L…N`; repeat rules
`F…`; date arithmetic `+ - ×`; expanded `±YYYYYY` years (P1 5.2.2.3 — EDTF uses
`Y`-prefix instead); reduced-precision or fractional times; interval end
higher-order-component elision (`2018-01-15/02-20`); negative
week/day-of-month/day-of-year; `X*` (explicit-form-only construct, 4.6.2).

## 6. Semantics the API must expose (not just valid/invalid)

- **Precision** of every expression (year / month / season / day / datetime;
  intervals may have mixed or undefined precision, A.4.4 Ex. 4–5).
- **Qualification flags** per component (uncertain / approximate / both), derived
  from complete/group/individual placement.
- **Unspecified masks** per component (which digits are X).
- **Interval endpoint kinds**: date | open (`..`) | unknown (empty).
- **Set semantics**: all-members vs one-member; range elements expand only for
  enumeration purposes (6.3–6.4).
- For range queries (`edtf_min`/`edtf_max` style): every expression denotes a time
  interval on the axis (a year = its whole year, `1985-XX-XX` = the year's days,
  `195X` = 1950–1959, `2001-21` = a fuzzy season — season boundaries are
  location/custom dependent per 3.1.3.1; define a documented convention, D12).

## 7. Validation rules checklist (drawn from both parts)

1. Extended-format separators exactly `-` between date components, `:` in time,
   `T` before time, `/` between interval endpoints (P1 3.2.6).
2. Fixed widths at L0/L1: year 4 (unless `Y`-prefixed or negative-4), month 2,
   day 2, hh/mm/ss 2 each; leading zeros required (P1 4.5).
3. Month/day range + calendar validity incl. leap rule (P1 4.2.1, 3.1.1.21).
4. Season codes 21–24 (L1) / 21–41 (L2) valid only in the month slot with no day.
5. Hour ≤ 23 (never 24), minute ≤ 59, second ≤ 60 (leap-second policy D3).
6. Qualifiers only `?` `~` `%`; at L1 trailing only; at L2 per 8.2 placements;
   at most one qualifier per position.
7. `X` placement per level (L1 restricted shapes; L2 anywhere in date components;
   never in time-of-day components at any level — 9.3 is explicit-form-only and
   not in Annex A).
8. Interval endpoints: L0 plain dates; L1 + qualified dates, `..`, empty; L2 +
   partially qualified/unspecified dates, `..`-bounded endpoints (10.6).
9. `Y`-prefix only with |year| > 9999 (D1); no month/day after it.
10. Sets non-empty, comma-separated, no spaces, `..` per 6.3; braces balanced.
11. Reject whitespace anywhere in any expression.

## 8. Level classification (a parse result should report the minimum level)

- L0: plain date / date-time / plain-date interval.
- L1: `Y`-year, negative year, season 21–24, trailing qualifier, restricted `X`
  shapes, `..`/empty interval ends, whole-endpoint interval qualification.
- L2: exponential year, `S` significant digits, codes 25–41, sets, group or
  individual qualifiers, `X` anywhere, partially qualified/`..`-bounded interval
  endpoints.

## 9. Decisions (RESOLVED — all implemented and tested as stated below)

Where the ISO text is ambiguous or silent, these are the canonical behaviors
of this implementation. Each is enforced by tests in `crates/edtf-core/tests/`.

- **D1 — `Y1985`:** 4.7.2 says the Y-form "should" (not "shall") be used only
  beyond ±9999. LoC EDTF requires >4 digits. Decision: reject `Y1985`. Document.
- **D2 — `-0000`:** not defined anywhere; year before 0000 is `-0001`. Reject.
- **D3 — leap second `:60`:** P1 permits `60` only for actual positive leap
  seconds (P1 4.3.10). Options: (a) accept `60` always, (b) reject always,
  (c) validate against the real leap-second table (unmaintainable). LoC EDTF L0
  inherits 8601-1. Proposal: accept `ss=60` syntactically, don't verify against
  the leap-second table; document. Interop: edtf.js rejects `:60` outright;
  recorded as a documented divergence in the interop corpus.
- **D4 — `-00`/`-00:00` shift:** P1 4.3.13 defines `+` for "ahead or equal";
  RFC 3339 gives `-00:00` special meaning but ISO doesn't. Proposal: reject
  negative zero shifts.
- **D5 — `XXXX` and `2XXX` level:** 9.2.1.2 c 3–4 define them; Annex A L1 lists
  only c 1–2; L2 "unspecified digit anywhere" (9.2.2) covers them via examples
  (`XXXX-12-XX`). Proposal: valid at L2, not L1 (matches LoC). Interop:
  edtf.js classifies `XXXX`, `XXXX-XX` and `XXXX-XX-XX` at L1; the corpus
  records its claims as `their_level` and asserts ours.
- **D6 — degenerate intervals `../..`, `/`, `../`, `/..`:** no example allows
  them. Proposal: reject all four (an interval needs at least one dated
  endpoint). Interop: edtf.js accepts `/` and `../..`; must-rejects in the
  interop corpus.
- **D7 — significant digits bounds:** e.g. `1950S5` (precision > digit count) or
  `1950S0`. Spec silent. Proposal: require 1 ≤ S ≤ digits(value).
- **D8 — bare `..`-lists without braces:** A.6.5 Example 6 shows
  `..1983-12-31,1984-10-10..…` with no `{}`. Clause 6 defines commas only inside
  braces. LoC's corresponding L2 example also shows braces. Proposal: require
  braces; treat the Annex example as shorthand.
- **D9 — set element types:** allow qualified/unspecified dates and datetimes
  inside sets? Proposal: allow anything parseable as a (possibly qualified/
  unspecified) date; disallow intervals inside sets. Cross-check LoC tests.
- **D10 — A.6.6 Example 6 misprint:** `..2004-06-01/~2004-06-20` contradicts the
  10.6 body text (`..2004-06-01/2004-06-~20`, qualifier as individual/group on
  the day). We follow Clause 10.6; a *leading* `~` on an endpoint is nonetheless
  valid as individual qualification of the year (8.2.3) — the two parses differ
  in meaning, not validity. Note in tests.
- **D11 — partially unspecified consistency:** must `1985-0X-31` admit at least
  one valid completion (X∈{1,3,5,7,8} works), and is `1985-02-3X` (no valid
  completion) invalid? Spec silent. Proposal: require ≥1 valid completion.
- **D12 — numeric bounds for seasons/sub-year codes** when computing min/max for
  range queries: define fixed conventional boundaries (e.g. 21=Mar–May in N-hem
  convention or the LoC convention) and document that ISO leaves them
  location-dependent (3.1.3.1).
- **D13 — negative years beyond 4 digits without `Y`:** `-12345` — 4.4.1.2 lifts
  the old restriction generally, but EDTF's L1 feature list pairs "negative year"
  with the 4-digit implicit form and provides `Y-…` for longer. Proposal: 4-digit
  `-YYYY` only; longer requires `Y-`.
- **D14 — masked months match calendar months only:** `X`-masked month
  candidates are drawn from 01–12 (9.2.2 Example 6 reads `X2` as Feb/Dec, not
  22/32); sub-year codes 21–41 must be written explicitly. So `2X` as a month
  is invalid (no calendar month matches).
- **D15 — time-shift bound:** |shift| ≤ 14:00, minutes ≤ 59 (adopted from the
  legacy conformance corpus, which rejects `+15:00`; ISO gives no bound).
  Interop: edtf.js bounds shifts to the real-world range [-12:00, +14:00];
  we keep the symmetric documented bound (ISO is silent), so `-13:00` and
  `-14:00` are documented divergences in the interop corpus.
- **D16 — unspecified digits inside interval endpoints are Level 2:** §10.4
  is listed in neither A.5 nor A.6; LoC EDTF places interval-with-unspecified
  at L2, so `2004-06-XX/2004-07-03` classifies as L2.
- **D17 — negative years may carry month/day:** `-1985-04-12` is accepted
  (4.4.1.2 extends the year component generally), classified L1. LoC examples
  show year-only negatives. Interop evidence (2026-07-26): python-edtf
  accepts `-2005/-1999-02` — a negative year-month endpoint — confirming
  negative years compose with lower-order components in the ecosystem;
  edtf.js has no test either way. Decision stands.
- **D18 — intervals and set ranges must be ordered:** an interval (or `a..b`
  range) whose end cannot possibly reach its start — start's earliest
  completion later than end's latest — is invalid (`2004/2003`,
  `{1672..1670}`), mirroring 7.14.2 Example 2. Overlap suffices: `2004-06/2004`
  and `198X/1985` remain valid.
- **D19 — Annex A.6.3 Example 2 is an erratum:** it prints `Y171010000S3` as
  "between 171010000 and 171010999", contradicting the normative rule in
  §4.4.3 (precision counts significant digits *from the left*: its own
  Example 2 gives `3141592653S4` → 3141000000–3141999999) and the LoC EDTF
  original. We follow §4.4.3: `Y171010000S3` → 171000000–171999999. Interop:
  python-edtf's expected estimate range for `Y171010000S3` is
  171000000–171999999 — independent agreement with this reading.
- **D20 — no leading zeros in Y-year significands:** `Y018470` and
  `Y08470847E1` are invalid. Leading zeros are mandatory padding for
  four-digit years (P1 4.5) but carry no information after `Y`, and they
  desynchronize the S-digit budget from the value: `Y08470847E1S9` counted 9
  written digits, yet its canonical zero-stripped form `Y8470847E1S9` has a
  value of only 8 digits and failed to reparse (round-trip violation, found
  by coverage-guided fuzzing). Significant digits count digits of the value
  (§4.4.3), so the written form must not inflate them.
- **D21 — masked years must be non-negative:** `-01XX` is invalid. Every
  unspecified-digit example in 9.2.1.2, 9.2.2 and Annex A is non-negative,
  and completions of a negative mask would include `-0000`, which does not
  exist (4.4.1.2, D2). python-edtf accepts `-01XX`; recorded as a
  python-edtf-ism (must-reject) in the interop corpus.
- **D22 — seasons are ordinary year-month expressions wherever a date may
  appear:** sub-year codes occupy the month slot (4.8.1), so season dates
  qualify like year-month expressions (`2001-33?`, 8.4.2), serve as interval
  endpoints (`2012-21/2012-22`, 10.2 endpoints are date expressions), and
  combine with masked years (`20XX-41`). ISO neither exemplifies nor forbids
  any of these; edtf.js gates them all behind its non-standard "level 3"
  (rejecting them at spec levels). We accept at the level the parts imply;
  documented divergences in the interop corpus.
- **D23 — temporal relations use possible-completions semantics over bounds
  regions:** `relation(a, b)` treats each expression as denoting some
  nonempty day-interval lying within its `bounds()` region — the "sometime
  during" reading, consistent with how masks bound to their completions
  (D11) and how D18 orders intervals. The six coarsened Allen relations
  (before / after / overlaps / contains / within / equal) are exhaustive
  and mutually exclusive over concrete day-intervals; each is reported as
  impossible / possible (some completion pair) / definite (every
  completion pair), so a lone possible relation is automatically definite.
  Documented consequences: (a) qualification never moves bounds (8.4.2
  NOTE) — `1985?` relates exactly as `1985`; (b) only before, after and
  equal can ever be definite, because any region wider than one day admits
  single-day completions, so containment/overlap can never be forced —
  `1985` vs `1985-06` is possibly-contains, not definitely (the "sometime
  during" vs "throughout" conflation, resolved in favor of never
  over-asserting); (c) interval endpoint linkage is coarsened away —
  `2004/2005` vs `2004-06` reports possibly-before although every true
  completion straddles June 2004; (d) Unknown bounds are
  possible-everything and never definite, even where the other endpoint
  could in principle constrain them (`1985/` vs `../1980`); (e) bounds are
  day-granular, so same-day datetimes are definitely equal. Enforced by
  `tests/relation.rs` (pinned table) and `tests/props.rs` (converse
  symmetry, soundness, D18 agreement).

## 10. Test-suite sources

1. Every quoted example string in Annex A (the conformance surface).
2. Every implicit-form example in Clauses 4.5–4.8, 6, 8, 9, 10 that falls inside
   the profile (many above are already listed).
3. Every example on the LoC EDTF specification page
   (loc.gov/standards/datetime), transcribed verbatim into
   `tests/fixtures/loc/loc-edtf-examples.json` and enforced by
   `crates/edtf-core/tests/loc.rs` — the interop cross-check against the
   original the ISO profile codifies. ISO Annex A wins on disagreement.
4. The old monument JS engine's test suite as behavioral oracle (read-only).
5. Adversarial negatives from §5's reject list and every D-decision.
6. The test suites of the reference implementations the ecosystem runs —
   edtf.js (test/parser.js and companions) and python-edtf
   (tests/test_parser.py) — harvested verbatim into
   `tests/fixtures/interop/reference-impl-corpus.json` and enforced by
   `crates/edtf-core/tests/interop.rs`. Four buckets: shared accepts (with
   ISO-derived levels), shared rejects, implementation-isms (must-reject),
   and documented divergences where we accept with ISO backing (D3, D15,
   D22). ISO Annex A wins every disagreement.
