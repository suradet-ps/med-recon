//! Frontend API layer — the only place the webview talks to the Tauri
//! backend.
//!
//! No hosts, no credentials, no SQL live here — every call is a thin
//! `invoke` to a Rust command, which owns all connection concerns.
//!
//! Errors cross the IPC as a typed [`ApiError`] (kind + Thai message):
//! components switch on `kind` (e.g. to raise the connection banner) and
//! display `message` verbatim.

use med_recon_core::{PatientHistory, PatientSummary};
use serde::{Deserialize, Serialize};

use crate::state::ConnectionHealth;

/// Failure class of a backend command — mirrors the Rust
/// `CommandErrorKind` (camelCase over the wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApiErrorKind {
    /// No connection settings stored.
    NotConfigured,
    /// HOSxP could not be reached.
    Connection,
    /// The read-only guard rejected a statement — an internal error.
    Guard,
    /// The statement failed server-side.
    Query,
}

/// A backend command failure: machine-readable kind + the Thai message to
/// show verbatim. Never decide presentation by matching message text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    /// Failure class.
    pub kind: ApiErrorKind,
    /// User-facing message (Thai).
    pub message: String,
}

impl ApiError {
    fn from_bridge(err: med_recon_bridge::BridgeError) -> Self {
        // The bridge passes the backend's serialized CommandError as the
        // message; parse it back into the typed shape when possible.
        if let med_recon_bridge::BridgeError::Command(text) = &err
            && let Ok(typed) = serde_json::from_str::<ApiError>(text)
        {
            return typed;
        }
        ApiError {
            kind: ApiErrorKind::Query,
            message: err.to_string(),
        }
    }
}

/// Plaintext connection settings, typed by the operator in the settings
/// dialog. The Rust side encrypts it before anything touches disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInput {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: String,
}

/// Non-secret summary of the saved connection (password never returned) —
/// used to pre-fill the settings form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
}

/// Non-secret site settings — stored as plain JSON on disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteSettings {
    pub site_name: String,
    pub history_days: u32,
    pub current_med_codes: Vec<String>,
}

/// Result of the backend's `SELECT 1` smoke test.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTestResult {
    pub latency_ms: u64,
}

/// A drug master entry (`drugitems`) returned to the current-medication
/// settings picker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DrugInfo {
    pub icode: String,
    pub name: String,
    pub strength: Option<String>,
    pub units: Option<String>,
}

async fn invoke_raw<T>(cmd: &str, args: impl Serialize) -> Result<T, ApiError>
where
    T: serde::de::DeserializeOwned,
{
    med_recon_bridge::invoke::<T>(cmd, args)
        .await
        .map_err(ApiError::from_bridge)
}

/// Calls a Tauri command with no arguments.
async fn call_empty<T>(cmd: &str) -> Result<T, ApiError>
where
    T: serde::de::DeserializeOwned,
{
    invoke_raw(cmd, serde_json::json!({})).await
}

/// Calls a Tauri command with a single string argument — the arg name must
/// equal the command's Rust parameter name.
async fn call_string_arg<T>(cmd: &str, arg_name: &str, value: &str) -> Result<T, ApiError>
where
    T: serde::de::DeserializeOwned,
{
    invoke_raw(cmd, serde_json::json!({ arg_name: value })).await
}

/// Calls a Tauri command with a serializable argument object.
async fn call_struct_arg<T>(cmd: &str, arg_name: &str, arg: &impl Serialize) -> Result<T, ApiError>
where
    T: serde::de::DeserializeOwned,
{
    invoke_raw(cmd, serde_json::json!({ arg_name: arg })).await
}

/// Whether stored settings exist.
pub async fn is_configured() -> Result<bool, ApiError> {
    call_empty("is_configured").await
}

/// Live connection health for the top-bar status dot.
pub async fn connection_health() -> Result<ConnectionHealth, ApiError> {
    call_empty("connection_health").await
}

/// Save the site connection config (encrypted at rest) and connect.
pub async fn save_connection(config: &ConnectionInput) -> Result<(), ApiError> {
    call_struct_arg("save_connection", "config", config).await
}

/// The saved HOSxP connection (without the password) — pre-fills the form.
pub async fn get_connection() -> Result<ConnectionInfo, ApiError> {
    call_empty("get_connection").await
}

/// Load the non-secret site settings (site name, history window, current
/// medication list).
pub async fn get_site_settings() -> Result<SiteSettings, ApiError> {
    call_empty("get_site_settings").await
}

/// Save the non-secret site settings as plain JSON.
pub async fn save_site_settings(settings: &SiteSettings) -> Result<(), ApiError> {
    call_struct_arg("save_site_settings", "settings", settings).await
}

/// Test connectivity without saving.
pub async fn test_connection(
    config: Option<&ConnectionInput>,
) -> Result<ConnectionTestResult, ApiError> {
    let args = config.cloned();
    call_struct_arg("test_connection", "config", &args).await
}

/// Search patients by CID, HN, or name.
pub async fn search_patients(query: &str) -> Result<Vec<PatientSummary>, ApiError> {
    call_string_arg("search_patients", "query", query).await
}

/// Search the drug master (`drugitems`) by name or code.
pub async fn search_drugs(query: &str) -> Result<Vec<DrugInfo>, ApiError> {
    call_string_arg("search_drugs", "query", query).await
}

/// The operator-configured current medications, resolved to names.
pub async fn get_current_meds() -> Result<Vec<DrugInfo>, ApiError> {
    call_empty("get_current_meds").await
}

/// Load the full medication + allergy history for a patient.
pub async fn load_history(hn: &str) -> Result<PatientHistory, ApiError> {
    call_string_arg("load_history", "hn", hn).await
}

/// Load the patient's photo (`patient_image` BLOB) as a `data:` URL;
/// `None` when the site has no photo on file.
pub async fn load_patient_image(hn: &str) -> Result<Option<String>, ApiError> {
    call_string_arg("load_patient_image", "hn", hn).await
}

/// Export a printable HTML report; returns the saved path.
///
/// `labels` carries every user-visible report string, resolved from the i18n
/// tokens in the current UI language — the backend never hard-codes text.
pub async fn export_report(hn: &str, labels: &ReportLabels) -> Result<String, ApiError> {
    invoke_raw(
        "export_report",
        serde_json::json!({ "hn": hn, "labels": labels }),
    )
    .await
}

/// Capture the current webview content as a PNG (Windows WebView2
/// `CapturePreview`); returns the saved path. `base_name` is the suggested
/// file name stem — the backend appends a timestamp and `.png`.
pub async fn capture_screenshot(base_name: &str) -> Result<String, ApiError> {
    invoke_raw(
        "capture_screenshot",
        serde_json::json!({ "baseName": base_name }),
    )
    .await
}

/// Every user-visible string in the exported report, resolved from i18n
/// tokens by the frontend. Mirrors the backend `ReportLabels` shape.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportLabels {
    pub html_lang: String,
    pub heading: String,
    pub generated: String,
    pub site_default: String,
    pub title: String,
    pub disclaimer: String,
    pub section_patient: String,
    pub section_allergy: String,
    pub section_active: String,
    pub section_lapsed: String,
    pub section_visits: String,
    pub col_date: String,
    pub col_type: String,
    pub col_dept: String,
    pub col_visit: String,
    pub last_dispensed: String,
    pub dispenses: String,
    pub total: String,
    pub supply: String,
    pub freq_per_day: String,
    pub reported_on: String,
    pub by: String,
    pub note: String,
    pub warnings_title: String,
    pub footer_phi: String,
}

/// Resolve all report labels from the i18n tokens for a language.
pub fn report_labels(lang: crate::i18n::Lang) -> ReportLabels {
    use crate::i18n::tr;
    let t = |k: &str| tr(lang, k).to_string();
    ReportLabels {
        html_lang: t("report.html_lang"),
        heading: t("report.heading"),
        generated: t("report.generated"),
        site_default: t("report.site_default"),
        title: t("report.title"),
        disclaimer: t("report.disclaimer"),
        section_patient: t("report.section.patient"),
        section_allergy: t("report.section.allergy"),
        section_active: t("canvas.active"),
        section_lapsed: t("canvas.lapsed"),
        section_visits: t("report.section.visits"),
        col_date: t("visit.date"),
        col_type: t("visit.type"),
        col_dept: t("visit.department"),
        col_visit: t("visit.id"),
        last_dispensed: t("report.last"),
        dispenses: t("report.dispenses"),
        total: t("report.total"),
        supply: t("report.supply"),
        freq_per_day: t("report.freq_per_day"),
        reported_on: t("report.reported_on"),
        by: t("report.by"),
        note: t("report.note"),
        warnings_title: t("canvas.warnings"),
        footer_phi: t("report.footer_phi"),
    }
}
