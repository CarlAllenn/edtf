//! Pinned oracle corpus of historically attested O.S./N.S. pairs
//! (issue #21's test ceremony): Russian revolution dates, Orthodox feasts,
//! the 1582 reform week, and well-documented biographical dates.
//!
//! Dual-dated English sources (Washington, Newton) are recorded here with
//! their *astronomical* Julian year — resolving "11 February 1731/32" to
//! 1732 is the caller's job (the 25-March year start is a documented
//! non-goal), and these rows pin what the arithmetic must do *after* that
//! resolution.

use edtf_calendars::{gregorian_to_julian, julian_to_gregorian, JulianDate};
use edtf_core::BoundDate;

/// (Julian y, m, d, Gregorian y, m, d, what it is).
type Case = (i64, u8, u8, i64, u8, u8, &'static str);

const ATTESTED: &[Case] = &[
    (
        1582,
        10,
        4,
        1582,
        10,
        14,
        "last day before the reform (Julian Thursday 4 Oct 1582)",
    ),
    (
        1582,
        10,
        5,
        1582,
        10,
        15,
        "first reformed day (Gregorian Friday 15 Oct 1582)",
    ),
    (
        1616,
        4,
        23,
        1616,
        5,
        3,
        "Shakespeare's death (England still O.S.)",
    ),
    (1709, 6, 27, 1709, 7, 8, "Battle of Poltava"),
    (
        1727,
        3,
        20,
        1727,
        3,
        31,
        "Newton's death (dual-dated 1726/27 O.S.)",
    ),
    (
        1732,
        2,
        11,
        1732,
        2,
        22,
        "Washington's birth (dual-dated 1731/32 O.S.)",
    ),
    (
        1917,
        2,
        23,
        1917,
        3,
        8,
        "February Revolution (International Women's Day)",
    ),
    (
        1917,
        10,
        25,
        1917,
        11,
        7,
        "October Revolution (in November, N.S.)",
    ),
    (
        1917,
        12,
        25,
        1918,
        1,
        7,
        "Orthodox Christmas 1917: next Gregorian year",
    ),
    (
        1918,
        1,
        31,
        1918,
        2,
        13,
        "Russia's last Julian day (Sovnarkom decree)",
    ),
    (
        2000,
        12,
        25,
        2001,
        1,
        7,
        "Orthodox Christmas today: 7 January",
    ),
    (
        2100,
        12,
        25,
        2101,
        1,
        8,
        "Orthodox Christmas after 2100: drifts to 8 January",
    ),
    (
        -4712,
        1,
        1,
        -4713,
        11,
        24,
        "Julian day zero (astronomical epoch)",
    ),
];

#[test]
fn attested_pairs_convert_both_ways() {
    for &(jy, jm, jd, gy, gm, gd, what) in ATTESTED {
        let julian = JulianDate {
            year: jy,
            month: jm,
            day: jd,
        };
        let gregorian = BoundDate {
            year: gy,
            month: gm,
            day: gd,
        };
        assert_eq!(
            julian_to_gregorian(julian),
            Ok(gregorian),
            "{what}: {julian} O.S. must be {gregorian} N.S."
        );
        assert_eq!(
            gregorian_to_julian(gregorian),
            Ok(julian),
            "{what}: {gregorian} N.S. must be {julian} O.S."
        );
    }
}
