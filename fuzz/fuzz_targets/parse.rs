// SPDX-FileCopyrightText: Copyright (c) the edtf contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Coverage-guided never-panic target: `Edtf::parse` must return, not crash,
//! on arbitrary UTF-8. The deterministic harness in
//! `crates/edtf-core/tests/fuzz.rs` guards every CI push; this one digs
//! deeper on a schedule (see .github/workflows/fuzz.yml).

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = core::str::from_utf8(data) {
        drop(edtf_core::Edtf::parse(s));
    }
});
