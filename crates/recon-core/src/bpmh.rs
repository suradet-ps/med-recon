//! Best Possible Medication History (BPMH) aggregation engine.
//!
//! Raw dispensing events ([`Dispense`]) are merged by drug code into
//! [`MedicationItem`]s, days supply is derived from sig data, and each item
//! is labelled `active` or `lapsed` against the **operator-configured list
//! of current medications** (`current_codes`): a drug on the list is active
//! no matter when it was last dispensed, anything else is lapsed. The list
//! is curated by the pharmacist/site staff (set in the app's settings), not
//! inferred from dispensing recency.
//!
//! This is one source among several in a real BPMH workflow — the UI must
//! never present the output as a complete or verified medication list.

use std::collections::{BTreeMap, HashSet};

use crate::model::{Dispense, EncounterSource, MedicationItem, MedicationStatus, Sig};

/// Derive days supply from quantity and sig data.
///
/// `days = qty / (dose_per_admin × frequency_per_day)`, rounded up so the
/// remaining supply estimate never understates.
///
/// Returns `None` when the sig is missing or does not carry both a dose and a
/// frequency.
pub fn days_supply(qty: f64, sig: &Sig) -> Option<u32> {
    let dose = sig.dose_per_admin?;
    let freq = sig.frequency_per_day?;
    if dose <= 0.0 || freq <= 0.0 || qty <= 0.0 {
        return None;
    }
    let days = qty / (dose * freq);
    Some(days.ceil() as u32)
}

/// Merge all dispensing events for a patient into a BPMH medication list.
///
/// Dedup key is the drug `icode`. Items are sorted by most recent dispense,
/// newest first. The sig/name/strength/units of the most recent event win.
///
/// `current_codes` is the operator-configured current-medication list: an
/// `icode` present in the set is labelled `Active`, every other dispensed
/// drug `Lapsed`.
pub fn aggregate_medications(
    dispenses: &[Dispense],
    reference_date: chrono::NaiveDate,
    current_codes: &HashSet<String>,
) -> Vec<MedicationItem> {
    let mut groups: BTreeMap<&str, Vec<&Dispense>> = BTreeMap::new();
    for d in dispenses {
        groups.entry(d.icode.as_str()).or_default().push(d);
    }

    let mut items: Vec<MedicationItem> = groups
        .into_values()
        .map(|mut events| {
            events.sort_by_key(|d| d.date);
            let latest = *events
                .last()
                .expect("invariant: group always has at least one event");
            let earliest = *events
                .first()
                .expect("invariant: group always has at least one event");

            let mut total_qty = 0.0;
            let mut visit_ids = BTreeMap::new();
            let mut sources = BTreeMap::new();
            for d in &events {
                total_qty += d.qty;
                visit_ids.entry(&d.visit_id).or_insert(());
                sources.entry(d.source).or_insert(());
            }

            let days_supply = latest
                .sig
                .as_ref()
                .and_then(|sig| days_supply(latest.qty, sig));
            let status = if current_codes.contains(latest.icode.as_str()) {
                MedicationStatus::Active
            } else {
                MedicationStatus::Lapsed
            };

            MedicationItem {
                icode: latest.icode.clone(),
                drug_name: latest.drug_name.clone(),
                strength: latest.strength.clone(),
                units: latest.units.clone(),
                last_dispense: latest.date,
                first_dispense: earliest.date,
                total_qty,
                visit_count: visit_ids.len() as u32,
                sources: sources.into_keys().collect(),
                days_supply,
                sig: latest.sig.clone(),
                status,
                days_since_last_dispense: (reference_date - latest.date).num_days(),
            }
        })
        .collect();

    items.sort_by_key(|a| std::cmp::Reverse(a.last_dispense));
    items
}

/// Helper for tests and UI labels: whether any source is IPD.
pub fn has_ipd_source(item: &MedicationItem) -> bool {
    item.sources.contains(&EncounterSource::Ipd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn empty_codes() -> HashSet<String> {
        HashSet::new()
    }

    fn codes(icodes: &[&str]) -> HashSet<String> {
        icodes.iter().map(|s| s.to_string()).collect()
    }

    fn sig(dose: f64, freq: f64) -> Sig {
        Sig {
            dose_per_admin: Some(dose),
            frequency_per_day: Some(freq),
            note: None,
        }
    }

    fn dispense(
        icode: &str,
        qty: f64,
        visit_id: &str,
        source: EncounterSource,
        date: NaiveDate,
    ) -> Dispense {
        Dispense {
            hn: "0001".into(),
            visit_id: visit_id.into(),
            source,
            icode: icode.into(),
            drug_name: icode.into(),
            strength: None,
            units: None,
            qty,
            date,
            sig: None,
        }
    }

    #[test]
    fn days_supply_derives_from_dose_and_frequency() {
        let d = days_supply(30.0, &sig(1.0, 3.0));
        assert_eq!(d, Some(10));
    }

    #[test]
    fn days_supply_rounds_up_fractional_supply() {
        assert_eq!(days_supply(10.0, &sig(1.5, 2.0)), Some(4)); // 3.33 -> 4
    }

    #[test]
    fn days_supply_missing_sig_is_none() {
        assert_eq!(
            days_supply(
                30.0,
                &Sig {
                    dose_per_admin: None,
                    frequency_per_day: None,
                    note: None
                }
            ),
            None
        );
        assert_eq!(days_supply(30.0, &sig(0.0, 3.0)), None);
        assert_eq!(days_supply(0.0, &sig(1.0, 3.0)), None);
    }

    #[test]
    fn status_follows_configured_current_codes() {
        let dispenses = vec![
            dispense("A1", 30.0, "vn1", EncounterSource::Opd, date(2026, 1, 1)),
            dispense("B2", 10.0, "vn2", EncounterSource::Opd, date(2026, 3, 1)),
            dispense(
                "C3",
                10.0,
                "vn3",
                EncounterSource::Opd,
                date(2020, 1, 1), // long ago — still active if configured
            ),
        ];
        let items = aggregate_medications(
            &dispenses,
            date(2026, 4, 1),
            &codes(&["A1", "C3"]),
        );
        let status_of = |icode: &str| {
            items
                .iter()
                .find(|i| i.icode == icode)
                .map(|i| i.status)
                .unwrap()
        };
        assert_eq!(status_of("A1"), MedicationStatus::Active);
        assert_eq!(status_of("C3"), MedicationStatus::Active); // configured, old dispense
        assert_eq!(status_of("B2"), MedicationStatus::Lapsed); // dispensed but not configured
    }

    #[test]
    fn empty_configuration_marks_everything_lapsed() {
        let dispenses = vec![dispense(
            "A1",
            30.0,
            "vn1",
            EncounterSource::Opd,
            date(2026, 1, 1),
        )];
        let items = aggregate_medications(&dispenses, date(2026, 1, 2), &empty_codes());
        assert_eq!(items[0].status, MedicationStatus::Lapsed);
    }

    #[test]
    fn aggregate_empty_input_is_empty() {
        assert!(aggregate_medications(&[], date(2026, 1, 1), &empty_codes()).is_empty());
    }

    #[test]
    fn aggregate_dedups_by_icode_and_merges_events() {
        let dispenses = vec![
            dispense("A1", 10.0, "vn1", EncounterSource::Opd, date(2026, 1, 1)),
            dispense("A1", 20.0, "vn2", EncounterSource::Opd, date(2026, 2, 1)),
            dispense("B2", 5.0, "an1", EncounterSource::Ipd, date(2026, 3, 1)),
        ];
        let items = aggregate_medications(&dispenses, date(2026, 4, 1), &empty_codes());
        assert_eq!(items.len(), 2);

        let a1 = items.iter().find(|i| i.icode == "A1").unwrap();
        assert_eq!(a1.total_qty, 30.0);
        assert_eq!(a1.visit_count, 2);
        assert_eq!(a1.first_dispense, date(2026, 1, 1));
        assert_eq!(a1.last_dispense, date(2026, 2, 1));
    }

    #[test]
    fn aggregate_sorts_most_recent_first() {
        let dispenses = vec![
            dispense("Old", 1.0, "v1", EncounterSource::Opd, date(2025, 1, 1)),
            dispense("New", 1.0, "v2", EncounterSource::Opd, date(2026, 1, 1)),
        ];
        let items = aggregate_medications(&dispenses, date(2026, 1, 15), &empty_codes());
        assert_eq!(items[0].icode, "New");
        assert_eq!(items[1].icode, "Old");
    }

    #[test]
    fn aggregate_uses_latest_sig_and_sources() {
        let dispenses = vec![
            dispense("A1", 30.0, "vn1", EncounterSource::Opd, date(2026, 1, 1)),
            Dispense {
                sig: Some(sig(1.0, 3.0)),
                ..dispense("A1", 90.0, "an1", EncounterSource::Ipd, date(2026, 3, 1))
            },
        ];
        let items = aggregate_medications(&dispenses, date(2026, 3, 2), &codes(&["A1"]));
        let a1 = &items[0];
        assert_eq!(a1.days_supply, Some(30));
        assert_eq!(a1.sources, vec![EncounterSource::Opd, EncounterSource::Ipd]);
        assert_eq!(a1.status, MedicationStatus::Active);
        assert_eq!(a1.days_since_last_dispense, 1);
    }
}
