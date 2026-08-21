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
///
/// When `history_days` is `Some(n)`, the backend uses `n` days instead of
/// the configured default — this powers the per-query history window UI.
pub async fn load_history(hn: &str, history_days: Option<u32>) -> Result<PatientHistory, ApiError> {
    invoke_raw(
        "load_history",
        serde_json::json!({ "hn": hn, "historyDays": history_days }),
    )
    .await
}

/// Load the patient's photo (`patient_image` BLOB) as a `data:` URL;
/// `None` when the site has no photo on file.
pub async fn load_patient_image(hn: &str) -> Result<Option<String>, ApiError> {
    call_string_arg("load_patient_image", "hn", hn).await
}

/// Export a printable HTML report; returns the saved path.
///
/// `labels` carries every user-visible report string — Thai-only, resolved
/// once by [`report_labels`]. The backend never hard-codes text.
pub async fn export_report(hn: &str, labels: &ReportLabels) -> Result<String, ApiError> {
    invoke_raw(
        "export_report",
        serde_json::json!({ "hn": hn, "labels": labels }),
    )
    .await
}

/// Capture the current webview content as a PNG (Windows WebView2 DevTools
/// Protocol re-rasterization); returns the saved path. `base_name` is the
/// suggested file name stem — the backend appends a timestamp and `.png`.
/// `scale` is the display's `devicePixelRatio`, so the shot matches the
/// screen's physical resolution instead of the DPI-scaled logical one.
pub async fn capture_screenshot(base_name: &str, scale: f64) -> Result<String, ApiError> {
    invoke_raw(
        "capture_screenshot",
        serde_json::json!({ "baseName": base_name, "scale": scale }),
    )
    .await
}

/// Every user-visible string in the exported report, fixed Thai — the UI
/// is Thai-only. Mirrors the backend `ReportLabels` shape.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportLabels {
    pub html_lang: &'static str,
    pub heading: &'static str,
    pub generated: &'static str,
    pub site_default: &'static str,
    pub title: &'static str,
    pub disclaimer: &'static str,
    pub section_patient: &'static str,
    pub section_allergy: &'static str,
    pub section_active: &'static str,
    pub section_lapsed: &'static str,
    pub section_visits: &'static str,
    pub col_date: &'static str,
    pub col_type: &'static str,
    pub col_dept: &'static str,
    pub col_visit: &'static str,
    pub last_dispensed: &'static str,
    pub dispenses: &'static str,
    pub total: &'static str,
    pub supply: &'static str,
    pub freq_per_day: &'static str,
    pub reported_on: &'static str,
    pub by: &'static str,
    pub note: &'static str,
    pub warnings_title: &'static str,
    pub footer_phi: &'static str,
}

/// The fixed Thai report labels.
pub fn report_labels() -> ReportLabels {
    ReportLabels {
        html_lang: "th",
        heading: "ประวัติยาและการใช้ยา — Med Recon",
        generated: "สร้างเมื่อ {date}",
        site_default: "สถานบริการ",
        title: "ประวัติยา {name} ({hn})",
        disclaimer: "⚠️ เอกสารนี้สร้างจากข้อมูลการจ่ายยา (dispensing) ใน HOSxP ซึ่งเป็นเพียงแหล่งข้อมูลหนึ่งในหลายแหล่ง สำหรับ Best Possible Medication History (BPMH) ยังไม่ถือว่าเป็นรายการยาที่สมบูรณ์หรือได้รับการยืนยัน ควรสอบทานร่วมกับผู้ป่วย/ญาติก่อนนำไปใช้ทางคลินิก",
        section_patient: "ข้อมูลผู้ป่วย",
        section_allergy: "แพ้ยา / อาการไม่พึงประสงค์ ({n})",
        section_active: "ยาที่ผู้ป่วยเคยได้รับ ({n})",
        section_lapsed: "ยาที่ผู้ป่วยเคยได้รับ (ยาตามอาการ) ({n})",
        section_visits: "ประวัติการเข้ารับบริการ ({n})",
        col_date: "วันที่",
        col_type: "ประเภท",
        col_dept: "แผนก / หอผู้ป่วย",
        col_visit: "รหัส visit",
        last_dispensed: "ครั้งล่าสุด",
        dispenses: "dispense {n} ครั้ง",
        total: "รวม",
        supply: "supply ≈ {n} วัน",
        freq_per_day: "/วัน",
        reported_on: "รายงานเมื่อ {date}",
        by: "โดย {name}",
        note: "หมายเหตุ: {note}",
        warnings_title: "คำเตือนความครบถ้วนของข้อมูล",
        footer_phi: "ข้อมูลนี้เป็นข้อมูลสุขภาพส่วนบุคคล (PHI) ต้องจัดเก็บและส่งต่อตามระเบียบปฏิบัติด้านการคุ้มครองข้อมูลส่วนบุคคล",
    }
}
