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
//! reject unparseable BE Feb-29 rows, and the query cutoff never depends on
//! that single day.

use chrono::{Datelike, NaiveDate};

/// Date era used by the connected HOSxP site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DateEra {
    /// ค.ศ. — Gregorian (default; most HOSxP sites store this).
    Christian,
    /// พ.ศ. — Buddhist era (site-specific, confirm before enabling).
    Buddhist,
}

/// Year offset between the two calendars.
const YEAR_OFFSET: i32 = 543;

/// Convert a Buddhist-era date (พ.ศ.) to Christian era (ค.ศ.).
///
/// Cannot fail for any date representable as a `NaiveDate`, because a BE
/// date in Gregorian representation only reaches Feb 29 when the CE target
/// year is a leap year.
pub fn buddhist_to_christian(date: NaiveDate) -> NaiveDate {
    let ce_year = date.year() - YEAR_OFFSET;
    date.with_year(ce_year)
        .expect("invariant: Buddhist-era Feb 29 only exists for leap CE years")
}

/// Convert a Christian-era date (ค.ศ.) to Buddhist era (พ.ศ.).
///
/// A CE leap day (Feb 29) clamps to BE Feb 28 — see module docs.
pub fn christian_to_buddhist(date: NaiveDate) -> NaiveDate {
    let be_year = date.year() + YEAR_OFFSET;
    match date.with_year(be_year) {
        Some(d) => d,
        None => date
            .with_day(28)
            .and_then(|d| d.with_year(be_year))
            .expect("invariant: Feb 28 exists in every year"),
    }
}

/// Convert a site date to the era used internally (Christian era).
pub fn to_internal(date: NaiveDate, site_era: DateEra) -> NaiveDate {
    match site_era {
        DateEra::Christian => date,
        DateEra::Buddhist => buddhist_to_christian(date),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn be_to_ce_plain_date() {
        assert_eq!(
            buddhist_to_christian(NaiveDate::from_ymd_opt(2567, 5, 10).unwrap()),
            NaiveDate::from_ymd_opt(2024, 5, 10).unwrap()
        );
    }

    #[test]
    fn ce_to_be_plain_date() {
        assert_eq!(
            christian_to_buddhist(NaiveDate::from_ymd_opt(2024, 5, 10).unwrap()),
            NaiveDate::from_ymd_opt(2567, 5, 10).unwrap()
        );
    }

    #[test]
    fn ce_leap_day_clamps_to_be_feb_28() {
        // CE 2024-02-29 is BE 2567-02-29 (Thai leap day), which has no
        // proleptic Gregorian representation — documented clamp.
        assert_eq!(
            christian_to_buddhist(NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()),
            NaiveDate::from_ymd_opt(2567, 2, 28).unwrap()
        );
    }

    #[test]
    fn roundtrip_preserves_most_dates() {
        for d in [
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(1970, 6, 15).unwrap(),
            NaiveDate::from_ymd_opt(2099, 12, 31).unwrap(),
            NaiveDate::from_ymd_opt(2024, 3, 1).unwrap(),
            NaiveDate::from_ymd_opt(2025, 2, 28).unwrap(),
        ] {
            let be = christian_to_buddhist(d);
            assert_eq!(buddhist_to_christian(be), d, "roundtrip failed for {d}");
        }
    }

    #[test]
    fn to_internal_passes_through_ce() {
        let d = NaiveDate::from_ymd_opt(2024, 5, 10).unwrap();
        assert_eq!(to_internal(d, DateEra::Christian), d);
        assert_eq!(
            to_internal(
                NaiveDate::from_ymd_opt(2567, 5, 10).unwrap(),
                DateEra::Buddhist
            ),
            d
        );
    }
}
