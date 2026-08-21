// SPDX-FileCopyrightText: Copyright (c) the edtf contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

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

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a CLI's job is printing: stdout is output, stderr is diagnostics"
)]
#![expect(
    clippy::min_ident_chars,
    reason = "the test bodies use the same y/m/d date-component names as the code they exercise"
)]
#![expect(
    clippy::single_call_fn,
    reason = "a named helper used once is extraction for readability, which is the opposite of a defect; several are also the named steps the module docs describe"
)]
#![expect(
    clippy::shadow_reuse,
    reason = "the algorithms shadow deliberately — the JDN forms rebind y and m as the published derivation does, step by step, and renaming each step would break the correspondence to the source"
)]
#![expect(
    clippy::indexing_slicing,
    reason = "each index is preceded by the bounds check that justifies it, or indexes a fixed-size array whose length is in its type"
)]
#![expect(
    clippy::absolute_paths,
    reason = "a one-use std path written in full at the call site is clearer than an import that only appears once"
)]
#![expect(
    clippy::pattern_type_mismatch,
    reason = "matching through a reference without restating & at every level is the idiomatic form the rest of this crate uses"
)]
#![expect(
    clippy::arithmetic_side_effects,
    reason = "every flagged operation is bounded where it stands: slice indices by a length guard on the line above, digit values to 0-9 by the match arm that binds them, and the JDN forms by being computed in i128 so any i64 year fits. The operations that genuinely could leave range already use checked_/saturating_ and return an error rather than wrapping"
)]
#![expect(
    clippy::unreachable,
    reason = "an unreachable! whose comment names the caller-side check that makes it unreachable — a deliberate assertion of an invariant, not an unhandled case"
)]
#![expect(
    clippy::use_debug,
    reason = "the debug rendering IS the CLI's diagnostic output for an internal value"
)]

use std::{
    io::{BufRead as _, Write},
    process::ExitCode,
};

use edtf_core::{Bound, Edtf};

/// The `--help` text, and the usage line every argument error prints.
const USAGE: &str = "\
edtf \u{2014} EDTF (ISO 8601-2:2019 Annex A) validator, levels 0-2

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
        precision prints an interval, never a bare 'converted' year \u{2014}
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

/// Dispatch one command with its arguments and return the process exit code.
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
    let (negative, rest) = input
        .strip_prefix('-')
        .map_or((false, input), |rest| (true, rest));
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

/// Render a bound as JSON: a date string, or `null` for an unknown bound.
fn bound_value(b: Bound) -> serde_json::Value {
    match b {
        Bound::Date(d) => serde_json::Value::String(d.to_string()),
        Bound::NegativeInfinity => serde_json::Value::String("-infinity".into()),
        Bound::PositiveInfinity => serde_json::Value::String("infinity".into()),
        Bound::Unknown => serde_json::Value::Null,
    }
}

/// The `info` command's JSON object for one parsed expression.
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

/// The precision of a date as the lowercase word the JSON surface uses.
fn precision_str(d: &edtf_core::Date) -> &'static str {
    match d.precision() {
        edtf_core::Precision::Year => "year",
        edtf_core::Precision::Month => "month",
        edtf_core::Precision::Season => "season",
        edtf_core::Precision::Day => "day",
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::missing_panics_doc,
        reason = "a test asserts by panicking; that is the failure signal, so there is no caller to warn"
    )]
    #![expect(
        clippy::inline_modules,
        reason = "unit tests live beside the code they exercise; a separate file would put the invariants further from what they constrain"
    )]
    #![allow(
        clippy::unwrap_used,
        reason = "test code: a panic here is the failure signal, not a crash path"
    )]

    use super::*;

    /// Drive the dispatch with a sink of our own instead of the process's
    /// stdout, and report both halves of its contract: what it wrote, and
    /// whether it called the input valid.
    fn dispatch(cmd: &str, input: &str) -> (bool, String) {
        let mut out = Vec::new();
        let ok = process(cmd, input, &mut out);
        (ok, String::from_utf8(out).unwrap())
    }

    /// `tests/cli.rs` pins the same outputs through the real binary; these go
    /// through the same code with an injected sink, which is what pins that
    /// `process` writes where it is told rather than to stdout.
    #[test]
    fn every_command_writes_to_the_supplied_sink() {
        assert_eq!(
            dispatch("validate", "1985-04-12"),
            (true, "1985-04-12: ok (level 0)\n".to_owned())
        );
        assert_eq!(
            dispatch("canonical", "?2004-?06-?11"),
            (true, "2004-06-11?\n".to_owned())
        );
        assert_eq!(dispatch("level", "156X-12-25"), (true, "2\n".to_owned()));
        // `from-julian` is dispatched before parsing, since its input is a
        // Julian date rather than an EDTF expression.
        assert_eq!(
            dispatch("from-julian", "1917-10-25"),
            (true, "1917-11-07\n".to_owned())
        );
        let (ok, json) = dispatch("info", "1985");
        assert!(ok);
        let v: serde_json::Value = serde_json::from_str(json.trim()).unwrap();
        assert_eq!(v["kind"], "date");
    }

    /// An input that fails to parse is refused before the dispatch, so
    /// nothing at all reaches the sink — the diagnostic goes to stderr.
    #[test]
    fn parse_failure_writes_nothing() {
        assert_eq!(dispatch("validate", "1985-02-30"), (false, String::new()));
    }

    /// The final arm guards an invariant, loudly.
    ///
    /// `main` only ever hands `process` a command it has already matched, so
    /// a command that slipped through must abort rather than print a line
    /// for a request nobody made.
    /// A subprocess can never reach it — `main` rejects unknown commands.
    #[test]
    #[should_panic(expected = "commands are pre-validated")]
    fn unroutable_command_panics_rather_than_printing() {
        // The input parses, so control really does reach the dispatch.
        dispatch("frobnicate", "1985-04-12");
    }
}
