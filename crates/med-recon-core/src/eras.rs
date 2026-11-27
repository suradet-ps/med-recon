//! Date era conversion between the Buddhist (พ.ศ.) and Christian (ค.ศ.)
//! calendars.
//!
//! The Thai calendar is the Gregorian calendar with the year offset by +543
//! and identical leap rules, so conversion preserves month and day:
//! `(be_year, m, d)` ↔ `(be_year − 543, m, d)`.
//!
//! One caveat: Thai leap days (BE Feb 29, e.g. 2567-02-29 = CE 2024-02-29)
//! do not exist in Rust's proleptic Gregorian representation, so converting
//! a CE leap day to BE clamps to Feb 28. In practice HOSxP drivers already
//! reject unparseable BE Feb-29 rows.
//!
//! The site era is **not configured**: each date value read from HOSxP is
//! normalized individually by its year. Years ≥ 2500 (BE 2500 = CE 1957)
//! are Buddhist-era and converted to Christian era; anything else is
//! already Christian-era. This tolerates sites storing either era - even
//! mixed within one database.

use time::{Date, Month};

/// Year offset between the two calendars.
const YEAR_OFFSET: i32 = 543;

/// Construct a date from calendar components.
///
/// Returns `None` when the year, month, or day is out of range.
pub fn date_from_ymd(year: i32, month: u8, day: u8) -> Option<Date> {
    Date::from_calendar_date(year, Month::try_from(month).ok()?, day).ok()
}

/// First year that is unambiguously Buddhist era for patient records.
///
/// BE 2500 = CE 1957; no plausible patient record predates CE 1957, so any
/// stored year ≥ 2500 must be Buddhist era.
pub const BUDDHIST_ERA_YEAR_THRESHOLD: i32 = 2500;

/// Whether a stored HOSxP year is Buddhist era (พ.ศ.).
pub fn is_buddhist_era_year(year: i32) -> bool {
    year >= BUDDHIST_ERA_YEAR_THRESHOLD
}

/// Convert a Buddhist-era date (พ.ศ.) to Christian era (ค.ศ.).
///
/// Cannot fail for any date representable as a `time::Date`, because a BE
/// date in Gregorian representation only reaches Feb 29 when the CE target
/// year is a leap year.
pub fn buddhist_to_christian(date: Date) -> Date {
    let ce_year = date.year() - YEAR_OFFSET;
    date.replace_year(ce_year)
        .expect("invariant: Buddhist-era Feb 29 only exists for leap CE years")
}

/// Convert a HOSxP date value to Christian era by detecting its era from
/// the stored year (see [`is_buddhist_era_year`]).
pub fn normalize_date(date: Date) -> Date {
    if is_buddhist_era_year(date.year()) {
        buddhist_to_christian(date)
    } else {
        date
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn be_year_threshold_boundaries() {
        assert!(is_buddhist_era_year(2500));
        assert!(is_buddhist_era_year(2567));
        assert!(!is_buddhist_era_year(2499));
        assert!(!is_buddhist_era_year(2026));
        assert!(!is_buddhist_era_year(1970));
    }

    #[test]
    fn be_to_ce_plain_date() {
        assert_eq!(
            buddhist_to_christian(date_from_ymd(2567, 5, 10).unwrap()),
            date_from_ymd(2024, 5, 10).unwrap()
        );
    }

    #[test]
    fn normalize_detects_both_eras_per_value() {
        assert_eq!(
            normalize_date(date_from_ymd(2567, 5, 10).unwrap()),
            date_from_ymd(2024, 5, 10).unwrap()
        );
        assert_eq!(
            normalize_date(date_from_ymd(2024, 5, 10).unwrap()),
            date_from_ymd(2024, 5, 10).unwrap()
        );
        assert_eq!(
            normalize_date(date_from_ymd(2499, 1, 1).unwrap()),
            date_from_ymd(2499, 1, 1).unwrap()
        );
    }
}
