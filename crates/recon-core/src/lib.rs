//! Pure domain layer for Recon — no I/O, no database knowledge.
//!
//! Contains the Best Possible Medication History (BPMH) model and the
//! aggregation/inference engine that turns raw dispensing events from HOSxP
//! into a deduplicated, status-labelled medication list. Also provides date
//! era normalization (auto-detected พ.ศ. / ค.ศ. per value) and PHI
//! redaction helpers used across the application.

pub mod bpmh;
pub mod eras;
pub mod model;
pub mod query_kind;
pub mod redact;

pub use bpmh::{aggregate_medications, days_supply, infer_status};
pub use eras::{
    buddhist_to_christian, is_buddhist_era_year, normalize_date, BUDDHIST_ERA_YEAR_THRESHOLD,
};
pub use model::{
    AllergyRecord, Dispense, EncounterSource, MedicationItem, MedicationStatus, PatientHistory,
    PatientSummary, Sig, VisitSummary,
};
pub use query_kind::{QueryKind, detect_query_kind};
pub use redact::{redact_cid, redact_hn};
