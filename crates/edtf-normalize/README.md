# edtf-normalize

Deterministic prose-date → EDTF normalizer: `"1980s"` → `198X`,
`"circa 1920"` → `1920~`, `"около 1920 г."` → `1920~` — instant, offline,
the same answer on every keystroke. `no_std` + `alloc`, no third-party dependencies (only `edtf-core`).

Built for the human input boundary: values are constructed through
[`edtf-core`](https://crates.io/crates/edtf-core)'s model and rendered by its canonical `Display`,
then re-parsed, so **every output is valid, canonical EDTF by construction**.

The return type is honest — `Normalized { edtf, value, notes }`,
`Ambiguous { interpretations }`, or `NoMatch` — never a silent guess:

```rust
use edtf_normalize::{normalize, Outcome};

let Outcome::Normalized(n) = normalize("early 19th century") else { panic!() };
assert_eq!(n.edtf, "1801/1830");

// "12/04/1985" is DD/MM or MM/DD — the form gets both readings, not a guess.
assert!(matches!(normalize("12/04/1985"), Outcome::Ambiguous(_)));

// Freeform prose is the caller's problem, by design.
assert!(matches!(normalize("printed sometime after the war"), Outcome::NoMatch));
```

Pattern tables are per-language (English and Russian today); adding a locale
means adding one table struct, not touching the grammar. Every judgement call
— century arithmetic, BC year-zero handling, season codes, ambiguity policy —
is a numbered N-decision in
[`docs/normalize-notes.md`](https://github.com/CarlAllenn/edtf/blob/main/docs/normalize-notes.md),
cited from the `Note` values attached to every result. Disagree with one?
File an issue against the N-number.

Exposed to JavaScript through [`edtf-wasm`](https://www.npmjs.com/package/edtf-wasm)'s `normalize()`.
