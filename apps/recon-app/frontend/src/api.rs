//! Frontend API layer — the only place the webview talks to the Tauri
//! backend.
//!
//! No hosts, no credentials, no SQL live here — every call is a thin
//! `invoke` to a Rust command, which owns all connection concerns.
//!
//! Errors cross the IPC as a typed [`ApiError`] (kind + Thai message):
//! components switch on `kind` (e.g. to raise the connection banner) and
//! display `message` verbatim.

use recon_core::{PatientHistory, PatientSummary};
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
    fn from_bridge(err: recon_bridge::BridgeError) -> Self {
        // The bridge passes the backend's serialized CommandError as the
        // message; parse it back into the typed shape when possible.
        if let recon_bridge::BridgeError::Command(text) = &err
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
    recon_bridge::invoke::<T>(cmd, args)
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

/// Export a printable HTML report; returns the saved path.
pub async fn export_report(hn: &str) -> Result<String, ApiError> {
    call_string_arg("export_report", "hn", hn).await
}
