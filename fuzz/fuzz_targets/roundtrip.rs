//! Coverage-guided round-trip property: anything the parser accepts must
//! render to a canonical form that reparses to the same value.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = core::str::from_utf8(data) else {
        return;
    };
    if let Ok(parsed) = edtf_core::Edtf::parse(s) {
        // Everything accepted must also have total (panic-free) bounds.
        let _bounds = parsed.bounds();
        let rendered = parsed.to_string();
        let reparsed = edtf_core::Edtf::parse(&rendered).unwrap_or_else(|e| {
            panic!("accepted {s:?}, but canonical form {rendered:?} fails to reparse: {e}")
        });
        assert_eq!(
            parsed, reparsed,
            "round trip changed meaning for {s:?} (canonical {rendered:?})"
        );
    }
});
