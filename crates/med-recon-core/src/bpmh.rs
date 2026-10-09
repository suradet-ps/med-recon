//! Best Possible Medication History (BPMH) aggregation engine.
//!
//! Raw dispensing events ([`Dispense`]) are merged by drug code and
//! directions for use (sig) into [`MedicationItem`]s, days supply is
//! derived from sig data, and each item is labelled `active` or `lapsed`
//! against the **operator-configured list of current medications**
//! (`current_codes`): a drug on the list is active no matter when it was
//! last dispensed, anything else is lapsed. The list
//! is curated by the pharmacist/site staff (set in the app's settings), not
//! inferred from dispensing recency.
//!
//! This is one source among several in a real BPMH workflow - the UI must
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
/// Dedup key is the drug `icode` **plus** its sig (directions for use):
/// events merge only when both match, so a drug ordered with different
/// sigs stays visible as separate items instead of one sig being silently
/// dropped. Events without sig data fold into the most recently dispensed
/// sig group (missing sig is missing data, not a different order); a drug
/// with no sig data at all keeps a single group.
///
/// Items are sorted by most recent dispense, newest first. The
/// name/strength/units of the most recent event win. When several events in
/// a group share the same dispense date (duplicate rows on one `vstdate`),
/// the event with the highest dispensed quantity wins - it is the
/// representative event for that date.
///
/// `current_codes` is the operator-configured current-medication list: an
/// `icode` present in the set is labelled `Active`, every other dispensed
/// drug `Lapsed`.
pub fn aggregate_medications(
    dispenses: &[Dispense],
    reference_date: chrono::NaiveDate,
    current_codes: &HashSet<String>,
) -> Vec<MedicationItem> {
    let mut by_icode: BTreeMap<&str, Vec<&Dispense>> = BTreeMap::new();
    for d in dispenses {
        by_icode.entry(d.icode.as_str()).or_default().push(d);
    }

    let mut items: Vec<MedicationItem> = by_icode
        .into_values()
        .flat_map(split_by_sig)
        .map(|group| build_item(group, reference_date, current_codes))
        .collect();

    items.sort_by_key(|a| std::cmp::Reverse(a.last_dispense));
    items
}

/// Grouping key for one order's directions for use.
///
/// Dispensing events merge only when their sig matches. Floating point
/// fields are compared by bit pattern, which gives the key a total and
/// deterministic order (a plain `f64` comparison is only partial).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SigKey {
    dose_bits: Option<u64>,
    frequency_bits: Option<u64>,
    note: Option<String>,
}

impl SigKey {
    /// Key for a dispense event's sig; a missing sig maps to
    /// [`SigKey::unknown`].
    fn of(sig: Option<&Sig>) -> Self {
        match sig {
            Some(sig) => Self {
                dose_bits: sig.dose_per_admin.map(f64::to_bits),
                frequency_bits: sig.frequency_per_day.map(f64::to_bits),
                note: sig.note.clone(),
            },
            None => Self::unknown(),
        }
    }

    /// Whether the key carries no sig information at all.
    fn is_unknown(&self) -> bool {
        self.dose_bits.is_none() && self.frequency_bits.is_none() && self.note.is_none()
    }

    fn unknown() -> Self {
        Self {
            dose_bits: None,
            frequency_bits: None,
            note: None,
        }
    }
}

/// Split one drug's events into groups sharing the same sig.
///
/// Different sigs are different orders, so they stay separate. Events
/// without sig data fold into the most recently dispensed sig-bearing
/// group; when the drug has no sig data at all they form the single group.
fn split_by_sig(events: Vec<&Dispense>) -> Vec<Vec<&Dispense>> {
    let mut by_sig: BTreeMap<SigKey, Vec<&Dispense>> = BTreeMap::new();
    for d in events {
        by_sig
            .entry(SigKey::of(d.sig.as_ref()))
            .or_default()
            .push(d);
    }

    if let Some(target) = most_recent_sig_key(&by_sig)
        && let Some(without_sig) = by_sig.remove(&SigKey::unknown())
    {
        by_sig.entry(target).or_default().extend(without_sig);
    }

    by_sig.into_values().collect()
}

/// The sig key with the most recent dispense among the sig-bearing groups
/// (`None` when every group is the no-sig group).
fn most_recent_sig_key(by_sig: &BTreeMap<SigKey, Vec<&Dispense>>) -> Option<SigKey> {
    by_sig
        .iter()
        .filter(|(key, _)| !key.is_unknown())
        .max_by_key(|(_, events)| events.iter().map(|d| d.date).max())
        .map(|(key, _)| key.clone())
}

/// Build one medication item from the events of a single
/// `(icode, sig)` group.
///
/// The group's sig (shared by all sig-bearing events in it) also backs the
/// days supply, so a newer no-sig event cannot leave the row showing
/// directions without a supply estimate.
fn build_item(
    mut events: Vec<&Dispense>,
    reference_date: chrono::NaiveDate,
    current_codes: &HashSet<String>,
) -> MedicationItem {
    events.sort_by(|a, b| {
        a.date.cmp(&b.date).then(
            a.qty
                .partial_cmp(&b.qty)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
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

    let sig = events.iter().find_map(|d| d.sig.clone());
    let days_supply = sig.as_ref().and_then(|sig| days_supply(latest.qty, sig));
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
        last_qty: latest.qty,
        total_qty,
        visit_count: visit_ids.len() as u32,
        sources: sources.into_keys().collect(),
        last_source: latest.source,
        days_supply,
        sig,
        appointment_date: latest.appointment,
        status,
        days_since_last_dispense: (reference_date - latest.date).num_days(),
    }
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
            appointment: None,
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
                date(2020, 1, 1), // long ago - still active if configured
            ),
        ];
        let items = aggregate_medications(&dispenses, date(2026, 4, 1), &codes(&["A1", "C3"]));
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
        assert_eq!(a1.last_qty, 20.0); // quantity of the most recent event
        assert_eq!(a1.visit_count, 2);
        assert_eq!(a1.first_dispense, date(2026, 1, 1));
        assert_eq!(a1.last_dispense, date(2026, 2, 1));
        assert_eq!(a1.last_source, EncounterSource::Opd); // latest event is OPD
        let b2 = items.iter().find(|i| i.icode == "B2").unwrap();
        assert_eq!(b2.last_source, EncounterSource::Ipd);
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
    fn aggregate_same_date_duplicates_pick_highest_qty() {
        let dispenses = vec![
            dispense("A1", 10.0, "vn1", EncounterSource::Opd, date(2026, 1, 1)),
            dispense("A1", 30.0, "vn2", EncounterSource::Opd, date(2026, 1, 1)),
            dispense("A1", 20.0, "vn3", EncounterSource::Opd, date(2026, 1, 1)),
        ];
        let items = aggregate_medications(&dispenses, date(2026, 1, 2), &empty_codes());
        let a1 = &items[0];
        assert_eq!(a1.last_qty, 30.0); // highest qty among same-date duplicates
        assert_eq!(a1.last_dispense, date(2026, 1, 1));
        assert_eq!(a1.visit_count, 3);
        assert_eq!(a1.total_qty, 60.0);
    }

    #[test]
    fn aggregate_later_dates_still_win_over_same_date_qty() {
        let dispenses = vec![
            dispense("A1", 90.0, "vn1", EncounterSource::Opd, date(2026, 1, 1)),
            dispense("A1", 10.0, "vn2", EncounterSource::Opd, date(2026, 3, 1)),
        ];
        let items = aggregate_medications(&dispenses, date(2026, 4, 1), &empty_codes());
        let a1 = &items[0];
        assert_eq!(a1.last_qty, 10.0); // later date wins regardless of qty
        assert_eq!(a1.last_dispense, date(2026, 3, 1));
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
        assert_eq!(a1.last_source, EncounterSource::Ipd); // latest event is IPD
        assert_eq!(a1.status, MedicationStatus::Active);
        assert_eq!(a1.days_since_last_dispense, 1);
    }

    #[test]
    fn aggregate_splits_same_icode_with_different_sigs() {
        let dispenses = vec![
            Dispense {
                sig: Some(sig(1.0, 3.0)),
                ..dispense("A1", 30.0, "vn1", EncounterSource::Opd, date(2026, 1, 1))
            },
            Dispense {
                sig: Some(sig(1.0, 1.0)),
                ..dispense("A1", 30.0, "vn1", EncounterSource::Opd, date(2026, 1, 1))
            },
        ];
        let items = aggregate_medications(&dispenses, date(2026, 1, 2), &empty_codes());
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| i.icode == "A1" && i.visit_count == 1));
        assert!(items.iter().any(|i| i.days_supply == Some(10)));
        assert!(items.iter().any(|i| i.days_supply == Some(30)));
    }

    #[test]
    fn aggregate_folds_missing_sig_into_most_recent_sig_group() {
        let dispenses = vec![
            dispense("A1", 10.0, "vn1", EncounterSource::Opd, date(2026, 1, 1)),
            Dispense {
                sig: Some(sig(1.0, 3.0)),
                ..dispense("A1", 30.0, "vn2", EncounterSource::Opd, date(2026, 2, 1))
            },
            Dispense {
                sig: Some(sig(2.0, 1.0)),
                ..dispense("A1", 20.0, "vn3", EncounterSource::Opd, date(2026, 3, 1))
            },
        ];
        let items = aggregate_medications(&dispenses, date(2026, 3, 2), &empty_codes());
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].last_dispense, date(2026, 3, 1));
        assert_eq!(items[0].visit_count, 2); // the no-sig event folds in here
        assert_eq!(
            items[0].sig.as_ref().and_then(|s| s.frequency_per_day),
            Some(1.0)
        );
        assert_eq!(items[1].visit_count, 1);
        assert_eq!(
            items[1].sig.as_ref().and_then(|s| s.frequency_per_day),
            Some(3.0)
        );
    }

    #[test]
    fn aggregate_days_supply_uses_group_sig_when_latest_event_has_none() {
        let dispenses = vec![
            Dispense {
                sig: Some(sig(1.0, 2.0)),
                ..dispense("A1", 30.0, "vn1", EncounterSource::Opd, date(2026, 1, 1))
            },
            // The later no-sig order folds into the group above and becomes
            // the latest event.
            dispense("A1", 40.0, "vn2", EncounterSource::Opd, date(2026, 2, 1)),
        ];
        let items = aggregate_medications(&dispenses, date(2026, 2, 2), &empty_codes());
        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.last_qty, 40.0);
        assert!(item.sig.is_some());
        assert_eq!(item.days_supply, Some(20)); // 40 / (1 x 2)
    }
}
