//! `edtf` — command-line EDTF (ISO 8601-2:2019 Annex A) validator.
//!
//! ```text
//! edtf validate <EXPR>...   exit 0 if all valid; prints one line per input
//! edtf canonical <EXPR>...  print the spec-preferred form of each input
//! edtf level <EXPR>...      print the conformance level (0/1/2) of each input
//! edtf info <EXPR>...       print a JSON summary of each input
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

COMMANDS:
    validate    check inputs; exit 0 only if every input is valid EDTF
    canonical   print the spec-preferred form of each valid input
    level       print the minimum conformance level (0, 1 or 2)
    info        print a JSON summary (kind, precision, bounds, flags)

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
        Some(cmd @ ("validate" | "canonical" | "level" | "info")) => run(cmd, &args[1..]),
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

/// Handle one input; returns false if it was invalid.
fn process(cmd: &str, input: &str, out: &mut impl Write) -> bool {
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
