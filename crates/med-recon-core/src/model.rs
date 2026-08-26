//! Core domain types shared across the Med Recon workspace.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// A patient as returned by the identity search.
///
/// PHI: handle with care. Never log this struct; use [`crate::redact`] first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatientSummary {
    /// Hospital number - the cross-visit join key.
    pub hn: String,
    /// National ID, if present in the source system.
    pub cid: Option<String>,
    /// Title (คำนำหน้า), e.g. นาย / นาง / Mr / Mrs.
    pub title: Option<String>,
    /// First name.
    pub first_name: String,
    /// Last name.
    pub last_name: String,
    /// Birth date, if known.
    pub birthday: Option<NaiveDate>,
}

impl PatientSummary {
    /// Display name: title + first + last, whitespace-collapsed.
    pub fn display_name(&self) -> String {
        [
            self.title.as_deref(),
            Some(self.first_name.as_str()),
            Some(self.last_name.as_str()),
        ]
        .into_iter()
        .flatten()
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ")
    }
}

/// Where a dispensing event came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EncounterSource {
    /// OPD order/dispense (`opitemrece`, vn-keyed).
    Opd,
    /// IPD order/dispense (`opitemrece`, an-keyed).
    Ipd,
}

/// Directions-for-use (sig) fields carried by a dispensing event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sig {
    /// Dose per administration, in the drug's own units (e.g. 1 tablet).
    pub dose_per_admin: Option<f64>,
    /// Times per day the dose is taken.
    pub frequency_per_day: Option<f64>,
    /// Raw sig note text, if any (e.g. "หลังอาหารเช้า", "take with food").
    pub note: Option<String>,
}

/// One raw dispensing event from the source system.
///
/// This is the unit fed into [`crate::bpmh::aggregate_medications`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dispense {
    /// Hospital number of the patient.
    pub hn: String,
    /// Visit id: `vn` for OPD, `an` for IPD.
    pub visit_id: String,
    /// Encounter source.
    pub source: EncounterSource,
    /// Drug master code.
    pub icode: String,
    /// Display name of the drug.
    pub drug_name: String,
    /// Strength text, e.g. "500 mg".
    pub strength: Option<String>,
    /// Units text, e.g. "เม็ด" / "tablet".
    pub units: Option<String>,
    /// Quantity dispensed at this event.
    pub qty: f64,
    /// Dispense date.
    pub date: NaiveDate,
    /// Sig / directions for use, if available.
    pub sig: Option<Sig>,
    /// Next appointment date for this visit (`oapp.nextdate`), if any.
    /// OPD visits only - IPD rows carry an admission number and have no
    /// direct `oapp` row.
    pub appointment: Option<NaiveDate>,
}

/// Whether a medication is considered part of the current regimen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MedicationStatus {
    /// Last dispense + derived days supply covers today (within grace period).
    Active,
    /// No longer within the covered window.
    Lapsed,
}

/// A deduplicated BPMH entry for one drug.
///
/// All dispensing events sharing an `icode` are merged into a single item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MedicationItem {
    /// Drug master code - the dedup key.
    pub icode: String,
    /// Display name of the drug.
    pub drug_name: String,
    /// Strength text, if any.
    pub strength: Option<String>,
    /// Units text, if any.
    pub units: Option<String>,
    /// Most recent dispense date.
    pub last_dispense: NaiveDate,
    /// Earliest dispense date in the history.
    pub first_dispense: NaiveDate,
    /// Quantity of the most recent dispensing event (shown next to the
    /// dispense date; not the lifetime total).
    pub last_qty: f64,
    /// Sum of quantities across all events.
    pub total_qty: f64,
    /// Number of distinct visits that dispensed this drug.
    pub visit_count: u32,
    /// Sources this drug was seen in.
    pub sources: Vec<EncounterSource>,
    /// Source of the most recent dispense event - the row-level anchor:
    /// date/qty/sig all come from the latest event, so the provenance badge
    /// follows the same event.
    pub last_source: EncounterSource,
    /// Derived days supply from the most recent event's sig.
    pub days_supply: Option<u32>,
    /// Sig from the most recent event.
    pub sig: Option<Sig>,
    /// Next appointment date (`oapp.nextdate`) of the most recent event's
    /// visit, if any.
    pub appointment_date: Option<NaiveDate>,
    /// Active/lapsed inference.
    pub status: MedicationStatus,
    /// Days between `last_dispense` and the status inference date.
    pub days_since_last_dispense: i64,
}

/// One allergy / adverse drug reaction record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllergyRecord {
    /// Agent - free-text or drug code, depending on site configuration.
    pub agent: String,
    /// Reported symptom(s).
    pub symptom: Option<String>,
    /// Date the reaction was reported (`opd_allergy.report_date`).
    pub report_date: Option<NaiveDate>,
    /// Free-text note.
    pub note: Option<String>,
    /// Reporter name/title.
    pub reporter: Option<String>,
}

/// A single visit (encounter) summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisitSummary {
    /// Visit id: `vn` for OPD, `an` for IPD.
    pub visit_id: String,
    /// Encounter source.
    pub source: EncounterSource,
    /// Visit date.
    pub date: NaiveDate,
    /// `main_dep` (OPD) or `ward` (IPD) label.
    pub department: Option<String>,
}

/// OPD screening record (`opdscreen`) - chief complaint and physical exam
/// text, keyed by the visit id (`vn`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpdScreenRecord {
    /// OPD visit id.
    pub vn: String,
    /// Screening date.
    pub vstdate: NaiveDate,
    /// Chief complaint (`cc`).
    pub cc: Option<String>,
    /// Physical examination text (`pe`).
    pub pe: Option<String>,
}

/// The full cross-visit history for one patient.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatientHistory {
    /// Patient identity.
    pub patient: PatientSummary,
    /// BPMH medication list, sorted by most recent dispense first.
    pub medications: Vec<MedicationItem>,
    /// Allergy / ADR records.
    pub allergies: Vec<AllergyRecord>,
    /// Visit history, most recent first.
    pub visits: Vec<VisitSummary>,
    /// OPD screening records (CC/PE), most recent first.
    pub screen_records: Vec<OpdScreenRecord>,
    /// Past medical history (`opdscreen.pmh`), free text. When several
    /// records exist, the latest `vstdate` wins - cumulative history, so it
    /// is **not** bounded by the history window.
    pub pmh: Option<String>,
    /// Data-completeness warnings (e.g. a HOSxP table missing on this site,
    /// so part of the history was skipped). Shown to the user verbatim.
    pub warnings: Vec<String>,
}
