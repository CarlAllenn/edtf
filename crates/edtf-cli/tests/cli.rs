//! End-to-end tests of the `edtf` binary via std::process.

use std::io::Write;
use std::process::{Command, Stdio};

fn edtf(args: &[&str]) -> (String, String, bool) {
    let out = Command::new(env!("CARGO_BIN_EXE_edtf"))
        .args(args)
        .output()
        .expect("binary runs");
    (
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
        out.status.success(),
    )
}

#[test]
fn validate_mixed_inputs() {
    let (stdout, stderr, ok) = edtf(&["validate", "1985-04-12", "1985-02-30"]);
    assert!(!ok, "one invalid input must fail the run");
    assert!(stdout.contains("1985-04-12: ok (level 0)"));
    assert!(stderr.contains("1985-02-30"));
    assert!(stderr.contains("offset 8"));
}

#[test]
fn canonicalizes() {
    let (stdout, _, ok) = edtf(&["canonical", "?2004-?06-?11"]);
    assert!(ok);
    assert_eq!(stdout.trim(), "2004-06-11?");
}

#[test]
fn levels() {
    let (stdout, _, ok) = edtf(&["level", "156X-12-25"]);
    assert!(ok);
    assert_eq!(stdout.trim(), "2");
}

#[test]
fn info_is_json() {
    let (stdout, _, ok) = edtf(&["info", "1985-04-12/.."]);
    assert!(ok);
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).expect("valid JSON");
    assert_eq!(v["kind"], "interval");
    assert_eq!(v["earliest"], "1985-04-12");
    assert_eq!(v["latest"], "infinity");
}

#[test]
fn reads_stdin() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_edtf"))
        .args(["validate", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("binary runs");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"1985\n2001-21\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("1985: ok (level 0)"));
    assert!(stdout.contains("2001-21: ok (level 1)"));
}

#[test]
fn help_and_version() {
    let (stdout, _, ok) = edtf(&["--help"]);
    assert!(ok);
    assert!(stdout.contains("USAGE"));
    let (stdout, _, ok) = edtf(&["--version"]);
    assert!(ok);
    assert!(stdout.starts_with("edtf "));
}
