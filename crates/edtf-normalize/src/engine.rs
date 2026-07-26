//! The language-neutral pattern grammar: preprocessing, qualifier stripping,
//! and the ordered match arms. Everything language-specific comes in through
//! a [`Lang`] table set. Whole-input only — no substring extraction (N11);
//! anything unmatched is `NoMatch`, never a guess.
//!
//! Values are built through the `edtf_core` model and rendered via its
//! canonical `Display`, then re-parsed before being returned ([`render`]).
//! Core has no validating constructors, so the re-parse is the calendar
//! check (it rejects e.g. a constructed February 31) and the proof that
//! every output is valid EDTF.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use edtf_core::{Date, DateField, Edtf, Interval, IntervalEndpoint, Qualifier, Year, YearKind};

use crate::tables::{lang_for, season_name, Lang, Span};
use crate::{Ambiguous, Interpretation, Normalized, Note, NumericOrder, Options, Outcome};

// ---------------------------------------------------------------------------
// Internal shapes

/// A matched expression before qualification and rendering.
#[derive(Debug, Clone)]
enum Expr {
    Date(Date),
    Interval(Interval),
}

/// One reading of an ambiguous input.
#[derive(Debug, Clone)]
struct Candidate {
    expr: Expr,
    reading: String,
    notes: Vec<Note>,
}

/// Result of the single-expression grammar.
#[derive(Debug, Clone)]
enum Single {
    One(Expr, Vec<Note>),
    Many(Vec<Candidate>),
}

// ---------------------------------------------------------------------------
// Construction helpers (core's fields are public; there are no constructors)

fn quiet(v: u8) -> DateField {
    DateField {
        digits: [Some(v / 10), Some(v % 10)],
        qualifier: Qualifier::default(),
    }
}

fn year_of(y: i32) -> Option<Year> {
    if !(-9999..=9999).contains(&y) {
        return None;
    }
    let a = y.unsigned_abs();
    Some(Year {
        kind: YearKind::Standard {
            negative: y < 0,
            digits: [
                Some((a / 1000) as u8),
                Some((a / 100 % 10) as u8),
                Some((a / 10 % 10) as u8),
                Some((a % 10) as u8),
            ],
        },
        significant_digits: None,
        qualifier: Qualifier::default(),
    })
}

fn date_y(y: i32) -> Option<Date> {
    Some(Date {
        year: year_of(y)?,
        month: None,
        day: None,
    })
}

fn date_ym(y: i32, m: u8) -> Option<Date> {
    Some(Date {
        month: Some(quiet(m)),
        ..date_y(y)?
    })
}

fn date_ymd(y: i32, m: u8, d: u8) -> Option<Date> {
    Some(Date {
        day: Some(quiet(d)),
        ..date_ym(y, m)?
    })
}

/// `XXXX` year for the missing-year forms (N9).
fn masked_year() -> Year {
    Year {
        kind: YearKind::Standard {
            negative: false,
            digits: [None; 4],
        },
        significant_digits: None,
        qualifier: Qualifier::default(),
    }
}

/// `198X` from the three leading digits (198).
fn decade_date(prefix3: u16) -> Date {
    Date {
        year: Year {
            kind: YearKind::Standard {
                negative: false,
                digits: [
                    Some((prefix3 / 100) as u8),
                    Some((prefix3 / 10 % 10) as u8),
                    Some((prefix3 % 10) as u8),
                    None,
                ],
            },
            significant_digits: None,
            qualifier: Qualifier::default(),
        },
        month: None,
        day: None,
    }
}

/// `18XX` (or `-01XX`) from the two leading digits.
fn century_date(prefix2: u8, negative: bool) -> Date {
    Date {
        year: Year {
            kind: YearKind::Standard {
                negative,
                digits: [Some(prefix2 / 10), Some(prefix2 % 10), None, None],
            },
            significant_digits: None,
            qualifier: Qualifier::default(),
        },
        month: None,
        day: None,
    }
}

fn interval(a: Date, b: Date) -> Expr {
    Expr::Interval(Interval {
        start: IntervalEndpoint::Date(a),
        end: IntervalEndpoint::Date(b),
    })
}

// ---------------------------------------------------------------------------
// Token classifiers

fn num(s: &str) -> Option<u32> {
    if s.is_empty() || s.len() > 5 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn strip_dot(t: &str) -> &str {
    t.trim_end_matches('.')
}

/// Compare a token to a dotless table word, ignoring '.' in the token
/// ("н.э." matches "нэ", "b.c." matches "bc").
fn eq_dotless(token: &str, word: &str) -> bool {
    let mut w = word.chars();
    for c in token.chars() {
        if c == '.' {
            continue;
        }
        if w.next() != Some(c) {
            return false;
        }
    }
    w.next().is_none()
}

fn table_u8(t: &str, table: &[(&str, u8)]) -> Option<u8> {
    let t = strip_dot(t);
    table.iter().find(|(n, _)| *n == t).map(|&(_, v)| v)
}

/// "19", "19th", "19-й", or "nineteenth" as a cardinal.
fn ordinal_num(t: &str, lang: &Lang) -> Option<u32> {
    if let Some(v) = num(t) {
        return Some(v);
    }
    for suffix in lang.ordinal_suffixes {
        if let Some(v) = t.strip_suffix(suffix).and_then(num) {
            return Some(v);
        }
    }
    lang.ordinal_words
        .iter()
        .find(|(w, _)| *w == t)
        .map(|&(_, v)| v)
}

/// Roman numeral 1–99 ("xix" → 19), tolerating Cyrillic lookalike letters —
/// real Russian sources routinely type "ХІХ век" with Cyrillic Х.
fn roman_num(t: &str) -> Option<u32> {
    let mut vals = Vec::new();
    for c in t.chars() {
        vals.push(match c {
            'i' | '\u{456}' => 1, // і (Cyrillic/Ukrainian i)
            'v' => 5,
            'x' | '\u{445}' => 10,   // х
            'l' | '\u{43b}' => 50,   // л
            'c' | '\u{441}' => 100,  // с
            'd' | '\u{434}' => 500,  // д
            'm' | '\u{43c}' => 1000, // м
            _ => return None,
        });
    }
    let mut total: u32 = 0;
    for (i, &v) in vals.iter().enumerate() {
        if vals[i + 1..].iter().any(|&n| n > v) {
            total = total.checked_sub(v)?;
        } else {
            total = total.checked_add(v)?;
        }
    }
    (1..=99).contains(&total).then_some(total)
}

/// Ordinal in century position: digits, suffixed digits, ordinal words, and
/// Roman numerals where the language uses them.
fn century_ordinal(t: &str, lang: &Lang) -> Option<u32> {
    ordinal_num(t, lang).or_else(|| {
        if lang.roman_centuries {
            roman_num(t)
        } else {
            None
        }
    })
}

/// Day-of-month token: 1–2 digits with optional ordinal suffix.
fn day_num(t: &str, lang: &Lang) -> Option<u8> {
    let digits = lang
        .ordinal_suffixes
        .iter()
        .find_map(|s| t.strip_suffix(s))
        .unwrap_or(t);
    if digits.len() > 2 {
        return None;
    }
    let v = num(digits)?;
    (1..=31).contains(&v).then_some(v as u8)
}

/// Year token: 3–4 digits, nonzero.
fn year_num(t: &str) -> Option<i32> {
    if t.len() < 3 || t.len() > 4 {
        return None;
    }
    let v = num(t)?;
    (v >= 1).then_some(v as i32)
}

// ---------------------------------------------------------------------------
// Preprocessing and qualifier stripping

/// Lowercase, unify dashes/apostrophes, drop commas, collapse whitespace.
fn preprocess(input: &str) -> String {
    let mut s = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\u{2013}' | '\u{2014}' => s.push('-'),
            '\u{2019}' => s.push('\''),
            ',' => s.push(' '),
            c => s.extend(c.to_lowercase()),
        }
    }
    let mut out = String::with_capacity(s.len());
    for tok in s.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(tok);
    }
    out
}

/// Strip leading noise ("the", "в"), trailing noise ("гг.", "года"), leading
/// and trailing qualifier words, and attached approximate prefixes ("c1860",
/// "ок.1920"). Returns the remainder and the accumulated whole-expression
/// qualifier (N10).
fn strip_qualifiers(s: &str, lang: &Lang) -> (String, Qualifier) {
    let mut toks: Vec<&str> = s.split(' ').filter(|t| !t.is_empty()).collect();
    let mut q = Qualifier::default();
    'outer: loop {
        if toks.len() > 1 {
            let head = strip_dot(toks[0].trim_end_matches(':'));
            if lang.noise_leading.contains(&head) {
                toks.remove(0);
                continue;
            }
            if lang.approx_leading.contains(&head) {
                q.approximate = true;
                toks.remove(0);
                continue;
            }
            if lang.uncertain_leading.contains(&head) {
                q.uncertain = true;
                toks.remove(0);
                continue;
            }
        }
        if let Some(&t0) = toks.first() {
            for p in lang.approx_attached {
                if let Some(rest) = t0.strip_prefix(p) {
                    if rest.starts_with(|c: char| c.is_ascii_digit()) {
                        q.approximate = true;
                        toks[0] = rest;
                        continue 'outer;
                    }
                }
            }
        }
        if toks.len() > 1 {
            let tail = strip_dot(toks[toks.len() - 1]);
            if lang.noise_trailing.contains(&tail) {
                toks.pop();
                continue;
            }
            if lang.approx_trailing.contains(&tail) {
                q.approximate = true;
                toks.pop();
                continue;
            }
            if lang.uncertain_trailing.contains(&tail) {
                q.uncertain = true;
                toks.pop();
                continue;
            }
        }
        break;
    }
    (toks.join(" "), q)
}

// ---------------------------------------------------------------------------
// Eras

/// Split a trailing era phrase ("bc", "до н. э.") off the token list.
/// `Some(true)` = BC. Table order is longest-first, so "до н э" wins
/// over "н э".
fn split_era<'a>(toks: &[&'a str], lang: &Lang) -> Option<(bool, Vec<&'a str>)> {
    for (phrase, bc) in lang.eras {
        if toks.len() > phrase.len() {
            let tail = &toks[toks.len() - phrase.len()..];
            if tail.iter().zip(*phrase).all(|(t, w)| eq_dotless(t, w)) {
                return Some((*bc, toks[..toks.len() - phrase.len()].to_vec()));
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Decades

enum DecadeParse {
    /// "1860s", "1980-е" — three leading digits fixed.
    Exact(u16),
    /// "1900s" — decade `190X` or century `19XX` (N6).
    CenturyAmbig(u16, u8),
    /// "80s", "80-е" — tens digit only; century unknown (N6).
    Bare(u8),
}

fn decade_tok(t: &str, lang: &Lang) -> Option<DecadeParse> {
    let t = t.strip_prefix('\'').unwrap_or(t);
    let digits = lang
        .decade_suffixes
        .iter()
        .find_map(|s| t.strip_suffix(s))?;
    let v = num(digits)?;
    if v % 10 != 0 {
        return None;
    }
    match digits.len() {
        4 => {
            let prefix3 = (v / 10) as u16;
            if v % 100 == 0 {
                Some(DecadeParse::CenturyAmbig(prefix3, (v / 100) as u8))
            } else {
                Some(DecadeParse::Exact(prefix3))
            }
        }
        2 => Some(DecadeParse::Bare((v / 10) as u8)),
        _ => None,
    }
}

fn decade_single(parse: DecadeParse, opts: &Options) -> Single {
    match parse {
        DecadeParse::Exact(p3) => Single::One(Expr::Date(decade_date(p3)), Vec::new()),
        DecadeParse::CenturyAmbig(p3, p2) => Single::Many(vec![
            Candidate {
                expr: Expr::Date(decade_date(p3)),
                reading: format!("decade ({p3}X)"),
                notes: vec![Note::DecadeAmbiguity],
            },
            Candidate {
                expr: Expr::Date(century_date(p2, false)),
                reading: format!("century ({p2}XX)"),
                notes: vec![Note::DecadeAmbiguity],
            },
        ]),
        DecadeParse::Bare(tens) => {
            if let Some(century) = opts.default_century {
                let p3 = century / 100 * 10 + u16::from(tens);
                Single::One(
                    Expr::Date(decade_date(p3)),
                    vec![Note::DefaultCenturyApplied],
                )
            } else {
                let mk = |p3: u16| Candidate {
                    expr: Expr::Date(decade_date(p3)),
                    reading: format!("the {p3}0s"),
                    notes: vec![Note::DecadeAmbiguity],
                };
                Single::Many(vec![mk(180 + u16::from(tens)), mk(190 + u16::from(tens))])
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Centuries

/// "19th century", "19c", "XIX век", bare "xix" (Roman-numeral languages
/// only) → century number. Era is split off by the caller.
fn century_toks(toks: &[&str], lang: &Lang) -> Option<u32> {
    let word = |t: &str| lang.century_words.contains(&strip_dot(t));
    // Attached single-letter century word: "19c", "19в".
    let attached = |t: &str| -> Option<u32> {
        let t = strip_dot(t);
        let d = lang
            .century_words
            .iter()
            .filter(|w| w.chars().count() == 1)
            .find_map(|w| t.strip_suffix(w))?;
        let n = century_ordinal(d, lang)?;
        (1..=99).contains(&n).then_some(n)
    };
    let n = match toks {
        [t] => attached(t).or_else(|| {
            // A bare Roman numeral reads as a century where the language
            // writes centuries that way ("XIX—XX вв." endpoints) — but a
            // lone letter with no century word is too weak a signal.
            (lang.roman_centuries && t.chars().count() >= 2)
                .then(|| roman_num(t))
                .flatten()
        })?,
        [o, w] if word(w) => century_ordinal(o, lang)?,
        _ => return None,
    };
    (1..=99).contains(&n).then_some(n)
}

/// Astronomical year span of century `n`: 19th CE = 1801..=1900,
/// 2nd BC = -0199..=-0100, 1st BC = -0099..=0000 (year 0 exists; N2/N3).
fn century_span(n: u32, bc: bool) -> (i32, i32) {
    let n = n as i32;
    if bc {
        (-(100 * n - 1), -(100 * (n - 1)))
    } else {
        (100 * (n - 1) + 1, 100 * n)
    }
}

/// Whole century as a date/interval expression (masked when possible).
fn century_expr(n: u32, bc: bool) -> Option<(Expr, Vec<Note>)> {
    let mut notes = vec![Note::CenturyMask];
    if bc {
        // edtf-core rejects unspecified digits in negative years (Annex A's
        // mask shapes are positive-only), and the 1st century BC would need
        // year -0000 anyway — BC centuries are exact intervals (N2).
        notes.push(Note::AstronomicalYear);
        notes.push(Note::BcCenturyInterval);
        let (s, e) = century_span(n, true);
        return Some((interval(date_y(s)?, date_y(e)?), notes));
    }
    let prefix2 = u8::try_from(n - 1).ok()?;
    Some((Expr::Date(century_date(prefix2, bc)), notes))
}

/// Early/mid/late/half of a century as a decade-rounded interval (N1).
fn century_part_expr(span: Span, n: u32, bc: bool) -> Option<(Expr, Vec<Note>)> {
    let (s, e) = century_span(n, bc);
    let (a, b) = match span {
        Span::Early => (s, s + 29),
        Span::Mid => (s + 30, s + 69),
        Span::Late => (s + 70, e),
        Span::FirstHalf => (s, s + 49),
        Span::SecondHalf => (s + 50, e),
    };
    let mut notes = vec![Note::CenturyPartInterval];
    if bc {
        notes.push(Note::AstronomicalYear);
    }
    Some((interval(date_y(a)?, date_y(b)?), notes))
}

// ---------------------------------------------------------------------------
// Numeric tokens ("12/04/1985", "1914-1918", "7/2008", "1914-18")

fn numeric_sep(t: &str) -> Option<char> {
    let mut sep = None;
    for c in ['/', '-', '.'] {
        if t.contains(c) {
            if sep.is_some() {
                return None; // mixed separators
            }
            sep = Some(c);
        }
    }
    sep
}

fn dmy(d: u32, m: u32, y: i32) -> Option<Expr> {
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(Expr::Date(date_ymd(y, m as u8, d as u8)?))
}

fn numeric_token(t: &str, opts: &Options, lang: &Lang) -> Option<Single> {
    let sep = numeric_sep(t)?;
    let parts: Vec<&str> = t.split(sep).collect();
    let vals: Vec<u32> = parts.iter().map(|p| num(p)).collect::<Option<_>>()?;
    match (parts.as_slice(), vals.as_slice()) {
        // Year first is order-unambiguous: 1985/04/12.
        ([py, _, _], &[y, m, d]) if py.len() == 4 => Some(Single::One(
            dmy(d, m, y as i32)?,
            vec![Note::NumericUnambiguous],
        )),
        // Two small fields then a 4-digit year: the DMY/MDY minefield (N5).
        ([pa, pb, py], &[a, b, y]) if py.len() == 4 && pa.len() <= 2 && pb.len() <= 2 => {
            let y = y as i32;
            let day_first = dmy(a, b, y);
            let month_first = dmy(b, a, y);
            match (a > 12, b > 12) {
                (true, false) => Some(Single::One(day_first?, vec![Note::NumericUnambiguous])),
                (false, true) => Some(Single::One(month_first?, vec![Note::NumericUnambiguous])),
                (true, true) => None,
                (false, false) if a == b => {
                    Some(Single::One(day_first?, vec![Note::NumericOrderIrrelevant]))
                }
                (false, false) => {
                    let (order, note) = match (opts.numeric_order, lang.implied_numeric_order) {
                        (Some(o), _) => (Some(o), Note::NumericResolvedByOption),
                        (None, Some(o)) => (Some(o), Note::NumericResolvedByLocale),
                        (None, None) => (None, Note::NumericOrderAmbiguous),
                    };
                    match order {
                        Some(NumericOrder::DayFirst) => Some(Single::One(day_first?, vec![note])),
                        Some(NumericOrder::MonthFirst) => {
                            Some(Single::One(month_first?, vec![note]))
                        }
                        None => {
                            let cand = |e: Option<Expr>, reading: &str| {
                                e.map(|expr| Candidate {
                                    expr,
                                    reading: String::from(reading),
                                    notes: vec![Note::NumericOrderAmbiguous],
                                })
                            };
                            let readings: Vec<Candidate> = [
                                cand(day_first, "day-month-year"),
                                cand(month_first, "month-day-year"),
                            ]
                            .into_iter()
                            .flatten()
                            .collect();
                            (!readings.is_empty()).then_some(Single::Many(readings))
                        }
                    }
                }
            }
        }
        // Month and year: 7/2008, 04.1985.
        ([pa, py], &[m, y]) if pa.len() <= 2 && py.len() == 4 && (1..=12).contains(&m) => Some(
            Single::One(Expr::Date(date_ym(y as i32, m as u8)?), Vec::new()),
        ),
        // Year then small field: 1985/4, 1985-4 (a full "1985-04" is already
        // valid EDTF and never reaches the grammar).
        ([py, pb], &[y, m]) if py.len() == 4 && pb.len() <= 2 && (1..=12).contains(&m) => Some(
            Single::One(Expr::Date(date_ym(y as i32, m as u8)?), Vec::new()),
        ),
        // Hyphenated year pair or elided end year: 1914-1918, 1914-18 (N4).
        // 21..=41 collides with EDTF sub-year codes: honest ambiguity (N13).
        ([py, pb], &[y, b]) if py.len() == 4 && sep == '-' => {
            let y = y as i32;
            match pb.len() {
                4 => {
                    let e = b as i32;
                    if e > y {
                        Some(Single::One(interval(date_y(y)?, date_y(e)?), Vec::new()))
                    } else {
                        None
                    }
                }
                2 => {
                    let elided = y / 100 * 100 + b as i32;
                    if elided <= y || b <= 12 {
                        return None;
                    }
                    if (21..=41).contains(&b) {
                        Some(Single::Many(vec![
                            Candidate {
                                expr: Expr::Date(Date {
                                    month: Some(quiet(b as u8)),
                                    ..date_y(y)?
                                }),
                                reading: format!("sub-year code {b} ({})", season_name(b as u8)),
                                notes: vec![Note::SeasonRangeCollision],
                            },
                            Candidate {
                                expr: interval(date_y(y)?, date_y(elided)?),
                                reading: format!("year range {y}/{elided}"),
                                notes: vec![Note::ElidedEndYear, Note::SeasonRangeCollision],
                            },
                        ]))
                    } else {
                        Some(Single::One(
                            interval(date_y(y)?, date_y(elided)?),
                            vec![Note::ElidedEndYear],
                        ))
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// The single-expression grammar

/// Match one date-ish expression. `in_range` suppresses interval-producing
/// forms (century parts) so range endpoints stay dates (N1 scope).
fn parse_single(s: &str, opts: &Options, lang: &Lang, in_range: bool) -> Option<Single> {
    let toks: Vec<&str> = s.split(' ').filter(|t| !t.is_empty()).collect();
    if toks.is_empty() {
        return None;
    }

    // Trailing era phrase, if any ("500 bc", "xix век до н. э.").
    let (era, core_toks) = match split_era(&toks, lang) {
        Some((bc, rest)) => (Some(bc), rest),
        None => (None, toks.clone()),
    };
    let bc = era == Some(true);

    // Part-of-century / decade modifier, spaced or attached ("mid-1930s").
    if let Some((span, rest)) = split_modifier(&core_toks, lang) {
        if era.is_none() {
            if let [t] = rest.as_slice() {
                if let Some(d) = decade_tok(t, lang) {
                    // Sub-decade parts would be false precision: the modifier
                    // is dropped and recorded (N1).
                    return Some(with_note(decade_single(d, opts), Note::ModifierDropped));
                }
            }
        }
        if let Some(n) = century_toks(&rest, lang) {
            let (expr, notes) = if in_range {
                let (e, mut ns) = century_expr(n, bc)?;
                ns.push(Note::ModifierDropped);
                (e, ns)
            } else {
                century_part_expr(span, n, bc)?
            };
            return Some(Single::One(expr, notes));
        }
        return None;
    }

    if let Some(n) = century_toks(&core_toks, lang) {
        let (expr, notes) = century_expr(n, bc)?;
        return Some(Single::One(expr, notes));
    }

    // Era-qualified plain year: "500 bc" → -0499, "79 ce" → 0079 (N3).
    if let Some(bc) = era {
        if let [t] = core_toks.as_slice() {
            let v = num(t)?;
            if v >= 1 && t.len() <= if bc { 5 } else { 4 } {
                let y = i32::try_from(if bc { 1 - i64::from(v) } else { i64::from(v) }).ok()?;
                let notes = if bc {
                    vec![Note::AstronomicalYear]
                } else {
                    Vec::new()
                };
                return Some(Single::One(Expr::Date(date_y(y)?), notes));
            }
        }
        return None;
    }

    match core_toks.as_slice() {
        [t] => {
            if let Some(y) = year_num(t) {
                return Some(Single::One(Expr::Date(date_y(y)?), Vec::new()));
            }
            if let Some(d) = decade_tok(t, lang) {
                return Some(decade_single(d, opts));
            }
            if let Some(m) = table_u8(t, lang.months) {
                // "January" → XXXX-01 (N9).
                return Some(Single::One(
                    Expr::Date(Date {
                        year: masked_year(),
                        month: Some(quiet(m)),
                        day: None,
                    }),
                    vec![Note::MissingYearMasked],
                ));
            }
            numeric_token(t, opts, lang)
        }
        [a, b] => {
            // "spring 2001" / "весной 2001" / "2001 spring" (N7).
            for (st, yt) in [(a, b), (b, a)] {
                if let (Some(code), Some(y)) = (table_u8(st, lang.seasons), year_num(yt)) {
                    return Some(Single::One(
                        Expr::Date(Date {
                            month: Some(quiet(code)),
                            ..date_y(y)?
                        }),
                        vec![Note::SeasonCode],
                    ));
                }
            }
            // "June 1940" / "1940 June".
            for (mt, yt) in [(a, b), (b, a)] {
                if let (Some(m), Some(y)) = (table_u8(mt, lang.months), year_num(yt)) {
                    return Some(Single::One(Expr::Date(date_ym(y, m)?), Vec::new()));
                }
            }
            // "January 12" / "12 January" → XXXX-01-12 (N9).
            for (mt, dt) in [(a, b), (b, a)] {
                if let (Some(m), Some(d)) = (table_u8(mt, lang.months), day_num(dt, lang)) {
                    return Some(Single::One(
                        Expr::Date(Date {
                            year: masked_year(),
                            month: Some(quiet(m)),
                            day: Some(quiet(d)),
                        }),
                        vec![Note::MissingYearMasked],
                    ));
                }
            }
            None
        }
        [a, b, c] => {
            // "12 April 1985" / "April 12, 1985" / "12-го апреля 1985".
            for (dt, mt) in [(a, b), (b, a)] {
                if let (Some(d), Some(m), Some(y)) =
                    (day_num(dt, lang), table_u8(mt, lang.months), year_num(c))
                {
                    return Some(Single::One(Expr::Date(date_ymd(y, m, d)?), Vec::new()));
                }
            }
            None
        }
        _ => None,
    }
}

/// Split a leading part-of-century modifier phrase off the token list, then
/// drop any noise between it and the rest ("first half OF THE 19th century").
fn split_modifier<'a>(toks: &[&'a str], lang: &Lang) -> Option<(Span, Vec<&'a str>)> {
    let matched = lang.modifiers.iter().find_map(|(phrase, span)| {
        (toks.len() > phrase.len() && toks.iter().zip(*phrase).all(|(t, w)| strip_dot(t) == *w))
            .then(|| (*span, toks[phrase.len()..].to_vec()))
    });
    let (span, mut rest) = matched.or_else(|| {
        // Attached single-word modifier: "mid-1930s", "late-19th".
        let (head, tail_toks) = toks.split_first()?;
        let (word, tail) = head.split_once('-')?;
        let span = lang
            .modifiers
            .iter()
            .find(|(p, _)| p.len() == 1 && p[0] == word)
            .map(|&(_, s)| s)?;
        if tail.is_empty() {
            return None;
        }
        let mut out = vec![tail];
        out.extend_from_slice(tail_toks);
        Some((span, out))
    })?;
    while rest.len() > 1 && lang.noise_leading.contains(&strip_dot(rest[0])) {
        rest.remove(0);
    }
    Some((span, rest))
}

fn with_note(s: Single, note: Note) -> Single {
    match s {
        Single::One(e, mut notes) => {
            notes.push(note);
            Single::One(e, notes)
        }
        Single::Many(mut cands) => {
            for c in &mut cands {
                c.notes.push(note);
            }
            Single::Many(cands)
        }
    }
}

// ---------------------------------------------------------------------------
// Open-ended and two-sided ranges

/// Match a leading phrase from a longest-first phrase list.
fn match_head<'a>(toks: &[&'a str], phrases: &[&[&str]]) -> Option<Vec<&'a str>> {
    for phrase in phrases {
        if toks.len() > phrase.len() && toks.iter().zip(*phrase).all(|(t, w)| strip_dot(t) == *w) {
            return Some(toks[phrase.len()..].to_vec());
        }
    }
    None
}

/// "before X" → `../X`, "after X" → `X/..` (interval reading, N8).
fn parse_open(toks: &[&str], opts: &Options, lang: &Lang) -> Option<Single> {
    let (before, rest) = match_head(toks, lang.before_phrases)
        .map(|rest| (true, rest))
        .or_else(|| match_head(toks, lang.after_phrases).map(|rest| (false, rest)))?;
    let (inner, q) = strip_qualifiers(&rest.join(" "), lang);
    let single = parse_single(&inner, opts, lang, true)?;
    let wrap = |expr: Expr| -> Option<Expr> {
        let Expr::Date(mut d) = expr else { return None };
        qualify_date(&mut d, q);
        let (start, end) = if before {
            (IntervalEndpoint::Open, IntervalEndpoint::Date(d))
        } else {
            (IntervalEndpoint::Date(d), IntervalEndpoint::Open)
        };
        Some(Expr::Interval(Interval { start, end }))
    };
    map_single(single, wrap, Note::OpenInterval)
}

/// "1863 or 1864" → honest ambiguity (N14).
fn parse_or(toks: &[&str], opts: &Options, lang: &Lang) -> Option<Single> {
    let pos = toks.iter().position(|t| lang.or_words.contains(t))?;
    if pos == 0 || pos == toks.len() - 1 {
        return None;
    }
    let (left, right) = (toks[..pos].join(" "), toks[pos + 1..].join(" "));
    let mut cands = Vec::new();
    for side in [left, right] {
        let (inner, q) = strip_qualifiers(&side, lang);
        match parse_single(&inner, opts, lang, true)? {
            Single::One(mut expr, mut notes) => {
                qualify_expr(&mut expr, q);
                notes.push(Note::OrAlternatives);
                cands.push(Candidate {
                    expr,
                    reading: String::from("alternative"),
                    notes,
                });
            }
            Single::Many(_) => return None, // nested ambiguity: refuse to guess
        }
    }
    Some(Single::Many(cands))
}

/// Two-sided ranges: "from X to Y", "с X по Y", "X to Y", "X-Y".
fn parse_range(s: &str, toks: &[&str], opts: &Options, lang: &Lang) -> Option<Single> {
    let word_split = |toks: &[&str], sep: &str| -> Option<(String, String)> {
        let pos = toks.iter().position(|t| *t == sep)?;
        (pos > 0 && pos < toks.len() - 1)
            .then(|| (toks[..pos].join(" "), toks[pos + 1..].join(" ")))
    };
    for (lead, sep) in lang.range_pairs {
        let inner = match lead {
            Some(l) if toks.first() == Some(l) => &toks[1..],
            Some(_) => continue,
            None => toks,
        };
        if let Some((l, r)) = word_split(inner, sep) {
            if let Some(single) = endpoint_pair(&l, &r, opts, lang) {
                return Some(single);
            }
        }
    }
    // Hyphen split: try each '-' position left to right.
    for (i, c) in s.char_indices() {
        if c != '-' {
            continue;
        }
        let (l, r) = (s[..i].trim(), s[i + 1..].trim());
        if l.is_empty() || r.is_empty() {
            continue;
        }
        if let Some(single) = endpoint_pair(l, r, opts, lang) {
            return Some(single);
        }
    }
    None
}

/// Parse both endpoints of a range, with per-endpoint qualifiers
/// ("1856-ca. 1865" → `1856/1865~`), elided end years ("1914-18", N4), and
/// bare-ordinal left centuries ("17-19th centuries" → `16XX/18XX`).
fn endpoint_pair(left: &str, right: &str, opts: &Options, lang: &Lang) -> Option<Single> {
    let (ls, lq) = strip_qualifiers(left, lang);
    let (rs, rq) = strip_qualifiers(right, lang);
    let lres = parse_single(&ls, opts, lang, true);
    let rres = parse_single(&rs, opts, lang, true);

    let (lres, rres) = match (lres, rres) {
        (Some(l), Some(r)) => (l, r),
        // "17-19th centuries", "XVII-XIX вв.": a bare ordinal on the left
        // inherits century-ness from the right endpoint.
        (None, Some(r)) => {
            let Single::One(Expr::Date(rd), rnotes) = &r else {
                return None;
            };
            if !rnotes.contains(&Note::CenturyMask) || ls.contains(' ') {
                return None;
            }
            let n = century_ordinal(&ls, lang)?;
            let bc = matches!(rd.year.kind, YearKind::Standard { negative: true, .. });
            let (expr, ns) = century_expr(n, bc)?;
            (Single::One(expr, ns), r)
        }
        // "1914 - 18": the right side elides the left year's century (N4).
        (Some(l), None) => {
            let Single::One(Expr::Date(ld), _) = &l else {
                return None;
            };
            let y = i32::try_from(ld.year.value()?).ok()?;
            let v = num(&rs).filter(|_| rs.len() <= 2)?;
            let elided = y / 100 * 100 + v as i32;
            if elided <= y {
                return None;
            }
            let r = Single::One(Expr::Date(date_y(elided)?), vec![Note::ElidedEndYear]);
            (l, r)
        }
        (None, None) => return None,
    };

    let build = |ld: &Date, rd: &Date| -> Option<Expr> {
        let (mut ld, mut rd) = (*ld, *rd);
        qualify_date(&mut ld, lq);
        qualify_date(&mut rd, rq);
        if let (Some(a), Some(b)) = (ld.year.value(), rd.year.value()) {
            if a > b {
                return None; // reversed ranges are prose errors, not dates
            }
        }
        Some(interval(ld, rd))
    };
    match (lres, rres) {
        (Single::One(Expr::Date(ld), ln), Single::One(Expr::Date(rd), rn)) => {
            let expr = build(&ld, &rd)?;
            let mut notes = ln;
            notes.extend(rn);
            Some(Single::One(expr, notes))
        }
        (Single::One(Expr::Date(ld), ln), Single::Many(cands)) => {
            let out: Vec<Candidate> = cands
                .into_iter()
                .filter_map(|c| {
                    let Expr::Date(cd) = c.expr else { return None };
                    let expr = build(&ld, &cd)?;
                    let mut notes = ln.clone();
                    notes.extend(c.notes);
                    Some(Candidate {
                        expr,
                        reading: c.reading,
                        notes,
                    })
                })
                .collect();
            (!out.is_empty()).then_some(Single::Many(out))
        }
        (Single::Many(cands), Single::One(Expr::Date(rd), rn)) => {
            let out: Vec<Candidate> = cands
                .into_iter()
                .filter_map(|c| {
                    let Expr::Date(cd) = c.expr else { return None };
                    let expr = build(&cd, &rd)?;
                    let mut notes = c.notes;
                    notes.extend(rn.clone());
                    Some(Candidate {
                        expr,
                        reading: c.reading,
                        notes,
                    })
                })
                .collect();
            (!out.is_empty()).then_some(Single::Many(out))
        }
        // A two-sided ambiguity would be a 4-way product — refuse to guess.
        _ => None,
    }
}

/// Apply a `Single`-level transform that may reject candidates.
fn map_single(single: Single, wrap: impl Fn(Expr) -> Option<Expr>, note: Note) -> Option<Single> {
    match single {
        Single::One(expr, mut notes) => {
            let expr = wrap(expr)?;
            notes.push(note);
            Some(Single::One(expr, notes))
        }
        Single::Many(cands) => {
            let out: Vec<Candidate> = cands
                .into_iter()
                .filter_map(|mut c| {
                    c.expr = wrap(c.expr)?;
                    c.notes.push(note);
                    Some(c)
                })
                .collect();
            (!out.is_empty()).then_some(Single::Many(out))
        }
    }
}

// ---------------------------------------------------------------------------
// Qualification and rendering

fn merge(into: &mut Qualifier, q: Qualifier) {
    into.uncertain |= q.uncertain;
    into.approximate |= q.approximate;
}

/// Distribute a whole-expression qualifier over every present component;
/// uniform qualification renders as the trailing form ("1940-06~") via
/// core's canonical Display (N10).
fn qualify_date(d: &mut Date, q: Qualifier) {
    if !q.uncertain && !q.approximate {
        return;
    }
    merge(&mut d.year.qualifier, q);
    if let Some(m) = &mut d.month {
        merge(&mut m.qualifier, q);
    }
    if let Some(day) = &mut d.day {
        merge(&mut day.qualifier, q);
    }
}

fn qualify_expr(e: &mut Expr, q: Qualifier) {
    match e {
        Expr::Date(d) => qualify_date(d, q),
        Expr::Interval(iv) => {
            for ep in [&mut iv.start, &mut iv.end] {
                if let IntervalEndpoint::Date(d)
                | IntervalEndpoint::OnOrBefore(d)
                | IntervalEndpoint::OnOrAfter(d) = ep
                {
                    qualify_date(d, q);
                }
            }
        }
    }
}

/// Render through core's canonical Display and re-parse: the calendar check
/// and the "every output is valid EDTF" guarantee in one step.
fn render(expr: Expr) -> Option<(Edtf, String)> {
    let value = match expr {
        Expr::Date(d) => Edtf::Date(d),
        Expr::Interval(iv) => Edtf::Interval(iv),
    };
    let s = value.to_string();
    let reparsed = Edtf::parse(&s).ok()?;
    Some((reparsed, s))
}

fn outcome_from(single: Single, q: Qualifier, base_notes: Vec<Note>) -> Outcome {
    match single {
        Single::One(mut expr, mut notes) => {
            let distributed = q.is_qualified() && matches!(expr, Expr::Interval(_));
            qualify_expr(&mut expr, q);
            match render(expr) {
                Some((value, edtf)) => {
                    let mut all = base_notes;
                    all.append(&mut notes);
                    if distributed {
                        all.push(Note::QualifierDistributed);
                    }
                    Outcome::Normalized(Normalized {
                        edtf,
                        value,
                        notes: all,
                    })
                }
                None => Outcome::NoMatch,
            }
        }
        Single::Many(cands) => {
            let mut interps: Vec<Interpretation> = cands
                .into_iter()
                .filter_map(|mut c| {
                    qualify_expr(&mut c.expr, q);
                    let (value, edtf) = render(c.expr)?;
                    let mut notes = base_notes.clone();
                    notes.extend(c.notes);
                    Some(Interpretation {
                        edtf,
                        value,
                        reading: c.reading,
                        notes,
                    })
                })
                .collect();
            match interps.len() {
                0 => Outcome::NoMatch,
                1 => {
                    let i = interps.remove(0);
                    Outcome::Normalized(Normalized {
                        edtf: i.edtf,
                        value: i.value,
                        notes: i.notes,
                    })
                }
                _ => Outcome::Ambiguous(Ambiguous {
                    interpretations: interps,
                }),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point

pub(crate) fn run(input: &str, opts: &Options) -> Outcome {
    let lang = lang_for(opts.language);
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Outcome::NoMatch;
    }

    // Dash-unified copy of the verbatim input (EDTF is case-sensitive:
    // X/Y/E/S must survive, so no lowercasing here).
    let dashed: String = trimmed
        .chars()
        .map(|c| match c {
            '\u{2013}' | '\u{2014}' => '-',
            c => c,
        })
        .collect();

    // "1914-21" is valid EDTF (spring 1914) AND a plausible elided range —
    // the collision must surface before the passthrough swallows it (N13).
    if let Some(outcome) = collision_check(&dashed, opts, lang) {
        return outcome;
    }

    if let Ok(v) = Edtf::parse(&dashed) {
        let edtf = v.to_string();
        let value = Edtf::parse(&edtf).unwrap_or(v);
        return Outcome::Normalized(Normalized {
            edtf,
            value,
            notes: vec![Note::AlreadyValidEdtf],
        });
    }

    let mut pre = preprocess(trimmed);
    let mut q = Qualifier::default();
    if pre.ends_with('?') {
        q.uncertain = true;
        pre.pop();
        while pre.ends_with(' ') {
            pre.pop();
        }
    }
    let (rest, q2) = strip_qualifiers(&pre, lang);
    merge(&mut q, q2);
    if rest.is_empty() {
        return Outcome::NoMatch;
    }
    if lang.explicit_no_date.contains(&rest.as_str()) {
        return Outcome::NoMatch; // the form decides what "unknown" means (N12)
    }
    let toks: Vec<&str> = rest.split(' ').collect();

    if let Some(single) = parse_single(&rest, opts, lang, false) {
        return outcome_from(single, q, Vec::new());
    }
    if let Some(single) = parse_open(&toks, opts, lang) {
        return outcome_from(single, q, Vec::new());
    }
    if let Some(single) = parse_or(&toks, opts, lang) {
        return outcome_from(single, q, Vec::new());
    }
    if let Some(single) = parse_range(&rest, &toks, opts, lang) {
        return outcome_from(single, q, Vec::new());
    }
    Outcome::NoMatch
}

/// `NNNN-NN` with a sub-year code 21–41 and a plausible elided range (N13).
fn collision_check(s: &str, opts: &Options, lang: &Lang) -> Option<Outcome> {
    let b = s.as_bytes();
    if b.len() != 7 || b[4] != b'-' {
        return None;
    }
    if !b[..4].iter().all(u8::is_ascii_digit) || !b[5..].iter().all(u8::is_ascii_digit) {
        return None;
    }
    let code: u32 = s[5..].parse().ok()?;
    let year: i32 = s[..4].parse().ok()?;
    if !(21..=41).contains(&code) || year / 100 * 100 + code as i32 <= year {
        return None;
    }
    match numeric_token(s, opts, lang)? {
        many @ Single::Many(_) => Some(outcome_from(many, Qualifier::default(), Vec::new())),
        _ => None,
    }
}
