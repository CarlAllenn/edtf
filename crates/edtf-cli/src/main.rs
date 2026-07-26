//! `edtf` — command-line EDTF (ISO 8601-2:2019 Annex A) validator.
//!
//! ```text
//! edtf validate <EXPR>...     exit 0 if all valid; prints one line per input
//! edtf canonical <EXPR>...    print the spec-preferred form of each input
//! edtf level <EXPR>...        print the conformance level (0/1/2) of each input
//! edtf info <EXPR>...         print a JSON summary of each input
//! edtf relation <A> <B>       three-valued temporal relation between A and B
//! edtf from-julian <DATE>...  Julian (O.S.) date to proleptic-Gregorian EDTF
//! ```
//!
//! Pass `-` (or pipe with no arguments) to read newline-separated
//! expressions from stdin — handy for validating whole data files:
//! `cut -f3 dates.tsv | edtf validate -`.

use edtf_core::{Bound, Edtf};
use std::io::{BufRead, Write};
use std::process::ExitCode;

const USAGE: &str = "\
edtf — EDTF (ISO 8601-2:2019 Annex A) validator, levels 0-2

USAGE:
    edtf <COMMAND> [EXPR]...      operate on arguments
    edtf <COMMAND> -              read newline-separated inputs from stdin
    edtf <COMMAND>                (with piped stdin) same as '-'
    edtf relation <A> <B>         compare exactly two expressions

COMMANDS:
    validate     check inputs; exit 0 only if every input is valid EDTF
    canonical    print the spec-preferred form of each valid input
    level        print the minimum conformance level (0, 1 or 2)
    info         print a JSON summary (kind, precision, bounds, flags)
    relation     three-valued temporal relation between two expressions
        (e.g. 'definitely before', 'possibly overlaps, ...')
    from-julian  convert proleptic Julian (Old Style) dates to Gregorian
        EDTF; input Y, Y-MM or Y-MM-DD (astronomical years). Year/month
        precision prints an interval, never a bare 'converted' year —
        Julian 1917 is 1917-01-14/1918-01-13

OPTIONS:
    -h, --help       show this help
    -V, --version    show version
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("-h" | "--help" | "help") => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some("-V" | "--version") => {
            println!("edtf {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some(cmd @ ("validate" | "canonical" | "level" | "info" | "from-julian")) => {
            run(cmd, &args[1..])
        }
        Some("relation") => relation(&args[1..]),
        Some(other) => {
            eprintln!("edtf: unknown command {other:?}\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

fn run(cmd: &str, rest: &[String]) -> ExitCode {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut ok = true;
    let mut handle = |input: &str| {
        if !process(cmd, input, &mut out) {
            ok = false;
        }
    };
    if rest.is_empty() || (rest.len() == 1 && rest[0] == "-") {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("edtf: stdin: {e}");
                    return ExitCode::FAILURE;
                }
            };
            if !line.is_empty() {
                handle(&line);
            }
        }
    } else {
        for arg in rest {
            handle(arg);
        }
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// `edtf relation <A> <B>`: print the three-valued temporal relation.
fn relation(rest: &[String]) -> ExitCode {
    let [a, b] = rest else {
        eprintln!("edtf: relation takes exactly two expressions\n\n{USAGE}");
        return ExitCode::FAILURE;
    };
    let parse = |s: &String| match Edtf::parse(s) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("{s}: {e}");
            None
        }
    };
    let (Some(a), Some(b)) = (parse(a), parse(b)) else {
        return ExitCode::FAILURE;
    };
    println!("{}", a.relation(&b));
    ExitCode::SUCCESS
}

/// Handle one input; returns false if it was invalid.
fn process(cmd: &str, input: &str, out: &mut impl Write) -> bool {
    if cmd == "from-julian" {
        return from_julian(input, out);
    }
    let parsed = match Edtf::parse(input) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{input}: {e}");
            return false;
        }
    };
    let line = match cmd {
        "validate" => format!("{input}: ok (level {})", parsed.level()),
        "canonical" => parsed.to_string(),
        "level" => parsed.level().to_string(),
        "info" => info_json(input, &parsed),
        _ => unreachable!("commands are pre-validated"),
    };
    writeln!(out, "{line}").is_ok()
}

/// Handle one `from-julian` input; returns false if it was invalid.
fn from_julian(input: &str, out: &mut impl Write) -> bool {
    let Some((year, month, day)) = parse_julian_parts(input) else {
        eprintln!("{input}: expected a Julian date as Y, Y-MM or Y-MM-DD");
        return false;
    };
    match edtf_calendars::convert(year, month, day) {
        Ok(edtf_calendars::Converted::Day(d)) => writeln!(out, "{d}").is_ok(),
        Ok(edtf_calendars::Converted::Span { earliest, latest }) => {
            writeln!(out, "{earliest}/{latest}").is_ok()
        }
        Err(e) => {
            eprintln!("{input}: {e}");
            false
        }
    }
}

/// Split `Y`, `Y-MM` or `Y-MM-DD` (astronomical year, may be negative)
/// into numeric parts. Returns None on any other shape.
fn parse_julian_parts(input: &str) -> Option<(i64, Option<u8>, Option<u8>)> {
    let (negative, rest) = match input.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, input),
    };
    let mut parts = rest.split('-');
    let year_digits = parts.next()?;
    if year_digits.is_empty() || !year_digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let year: i64 = year_digits.parse().ok()?;
    let year = if negative { -year } else { year };
    let two_digit = |s: Option<&str>| match s {
        None => Some(None),
        Some(s) if s.len() == 2 && s.bytes().all(|b| b.is_ascii_digit()) => {
            Some(Some(s.parse::<u8>().ok()?))
        }
        Some(_) => None,
    };
    let month = two_digit(parts.next())?;
    let day = two_digit(parts.next())?;
    if parts.next().is_some() {
        return None;
    }
    Some((year, month, day))
}

fn bound_value(b: Bound) -> serde_json::Value {
    match b {
        Bound::Date(d) => serde_json::Value::String(d.to_string()),
        Bound::NegativeInfinity => serde_json::Value::String("-infinity".into()),
        Bound::PositiveInfinity => serde_json::Value::String("infinity".into()),
        Bound::Unknown => serde_json::Value::Null,
    }
}

fn info_json(input: &str, parsed: &Edtf) -> String {
    let bounds = parsed.bounds();
    let (kind, precision) = match parsed {
        Edtf::Date(d) => ("date", Some(precision_str(d))),
        Edtf::DateTime(dt) => ("datetime", Some(precision_str(&dt.date))),
        Edtf::Interval(_) => ("interval", None),
        Edtf::Set(_) => ("set", None),
    };
    serde_json::json!({
        "input": input,
        "canonical": parsed.to_string(),
        "level": parsed.level(),
        "kind": kind,
        "precision": precision,
        "earliest": bound_value(bounds.earliest),
        "latest": bound_value(bounds.latest),
        "uncertain": parsed.is_uncertain(),
        "approximate": parsed.is_approximate(),
        "unspecified": parsed.has_unspecified(),
    })
    .to_string()
}

fn precision_str(d: &edtf_core::Date) -> &'static str {
    match d.precision() {
        edtf_core::Precision::Year => "year",
        edtf_core::Precision::Month => "month",
        edtf_core::Precision::Season => "season",
        edtf_core::Precision::Day => "day",
    }
}
