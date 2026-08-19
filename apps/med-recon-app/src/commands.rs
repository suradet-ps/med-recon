//! Tauri command adapters.
//!
//! Errors cross the IPC as a typed [`CommandError`] (kind + Thai message)
//! so the frontend can decide presentation from the kind — e.g. raise the
//! connection banner — instead of matching on message text. Crates stay
//! English and typed; this layer is the only place where errors are
//! translated for the UI.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use med_recon_config::{ConnectionConfig, SiteSettings};
use med_recon_core::{PatientHistory, PatientSummary};
use med_recon_hosxp::{HosxpClient, HosxpConfig};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

/// Ephemeral vault used when the OS keychain is unavailable (headless
/// environments). Config saved through it is never persisted to disk.
#[derive(Default)]
pub struct EphemeralVault {
    key: std::sync::Mutex<Option<encryptman::MasterKey>>,
}

impl med_recon_config::SecretVault for EphemeralVault {
    fn encrypt(&self, plaintext: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut guard = self.key.lock().expect("invariant: ephemeral key lock");
        let key = guard.get_or_insert_with(|| {
            encryptman::MasterKey::generate().expect("invariant: OS rng available")
        });
        Ok(encryptman::encrypt(key, plaintext)?)
    }

    fn decrypt(
        &self,
        ciphertext: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let guard = self.key.lock().expect("invariant: ephemeral key lock");
        let key = guard
            .as_ref()
            .ok_or_else(|| Box::new(std::io::Error::other("no master key available")))?;
        Ok(encryptman::decrypt(key, ciphertext)?)
    }
}

/// Failure class of a command — the frontend switches on this for
/// presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandErrorKind {
    /// No connection settings stored.
    NotConfigured,
    /// HOSxP could not be reached.
    Connection,
    /// The read-only guard rejected a statement — an internal error.
    Guard,
    /// The statement failed server-side.
    Query,
}

/// User-facing command error: a machine-readable kind plus the Thai message
/// shown verbatim (PII-free — parameter values never appear).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    /// Failure class.
    pub kind: CommandErrorKind,
    /// User-facing message (Thai).
    pub message: String,
}

impl CommandError {
    fn new(kind: CommandErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Logs the underlying cause for developers (PII-free) and returns the
/// user-facing Thai error.
fn dev_log(
    context: &str,
    detail: &impl std::fmt::Debug,
    kind: CommandErrorKind,
    message: &'static str,
) -> CommandError {
    eprintln!("[med-recon] {context} failed: {detail:?}");
    CommandError::new(kind, message)
}

/// Maps a repository error to a typed command error. `action` is the Thai
/// verb phrase for the Query variant ("ค้นหาผู้ป่วย" etc.).
fn map_repo_error(err: med_recon_hosxp::Error, action: &'static str) -> CommandError {
    eprintln!("[med-recon] {action} failed: {err:?}");
    match err {
        med_recon_hosxp::Error::Connect { .. } => {
            CommandError::new(CommandErrorKind::Connection, "เชื่อมต่อฐานข้อมูล HOSxP ไม่สำเร็จ")
        }
        med_recon_hosxp::Error::Database(sqlx::Error::PoolTimedOut)
        | med_recon_hosxp::Error::Database(sqlx::Error::PoolClosed)
        | med_recon_hosxp::Error::Database(sqlx::Error::Io(_)) => {
            CommandError::new(CommandErrorKind::Connection, "เชื่อมต่อฐานข้อมูล HOSxP ไม่สำเร็จ")
        }
        med_recon_hosxp::Error::Database(sqlx::Error::Database(db)) => {
            CommandError::new(CommandErrorKind::Query, format!("{action}ไม่สำเร็จ ({db})"))
        }
        med_recon_hosxp::Error::ReadOnlyViolation(_) => {
            CommandError::new(CommandErrorKind::Guard, "ระบบความปลอดภัยของแอปปฏิเสธคำสั่งนี้")
        }
        other => CommandError::new(
            CommandErrorKind::Query,
            format!("{action}ไม่สำเร็จ ({other})"),
        ),
    }
}

/// Connection settings submitted from the settings dialog. The password
/// lives in this struct only for the duration of the command call.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInput {
    /// Hostname or IP.
    pub host: String,
    /// TCP port.
    pub port: u16,
    /// HOSxP database name.
    pub database: String,
    /// Database user.
    pub user: String,
    /// Database password.
    pub password: String,
}

impl From<ConnectionInput> for ConnectionConfig {
    fn from(i: ConnectionInput) -> Self {
        Self {
            host: i.host,
            port: i.port,
            database: i.database,
            user: i.user,
            password: SecretString::from(i.password),
        }
    }
}

/// Site settings submitted from the settings dialog (non-secret).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteSettingsInput {
    /// Human-readable site label.
    pub site_name: String,
    /// History window in days.
    pub history_days: u32,
    /// Current-medication `icode`s (from the drug picker).
    pub current_med_codes: Vec<String>,
}

impl From<SiteSettingsInput> for SiteSettings {
    fn from(i: SiteSettingsInput) -> Self {
        Self {
            site_name: i.site_name,
            history_days: i.history_days.clamp(30, 3650),
            current_med_codes: i.current_med_codes,
        }
    }
}

/// Load the operator-configured current-medication `icode`s from the site
/// settings file (defaults to empty when no settings were saved yet).
fn configured_med_codes(state: &AppState) -> HashSet<String> {
    state
        .store
        .load_settings()
        .map(|s| s.current_med_codes.into_iter().collect())
        .unwrap_or_default()
}

/// Drug master entry as returned to the settings UI (never PHI — drug
/// metadata only).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DrugInfo {
    /// Drug master code.
    pub icode: String,
    /// Drug display name.
    pub name: String,
    /// Strength text, if any.
    pub strength: Option<String>,
    /// Units text, if any.
    pub units: Option<String>,
}

impl From<med_recon_hosxp::DrugItem> for DrugInfo {
    fn from(d: med_recon_hosxp::DrugItem) -> Self {
        Self {
            icode: d.icode,
            name: d.name,
            strength: d.strength,
            units: d.units,
        }
    }
}

/// Search the drug master (`drugitems`) by name or code for the
/// current-medication settings.
#[tauri::command]
pub async fn search_drugs(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<DrugInfo>, CommandError> {
    let client = client(&state, "ค้นหายา").await?;
    let results = client
        .search_drugs(&query)
        .await
        .map_err(|e| map_repo_error(e, "ค้นหายา"))?;
    tracing::debug!(
        query_len = query.len(),
        count = results.len(),
        "drug search"
    );
    Ok(results.into_iter().map(DrugInfo::from).collect())
}

/// The operator-configured current medications, resolved to names.
#[tauri::command]
pub async fn get_current_meds(state: State<'_, AppState>) -> Result<Vec<DrugInfo>, CommandError> {
    let codes: Vec<String> = configured_med_codes(&state).into_iter().collect();
    if codes.is_empty() {
        return Ok(Vec::new());
    }
    let client = client(&state, "อ่านรายการยา").await?;
    let results = client
        .load_drugs_by_codes(&codes)
        .await
        .map_err(|e| map_repo_error(e, "อ่านรายการยา"))?;
    Ok(results.into_iter().map(DrugInfo::from).collect())
}

/// Live HOSxP reachability, polled by the frontend — drives the top-bar
/// status dot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionHealth {
    /// No stored settings — the settings dialog is the flow.
    Unconfigured,
    /// A ping succeeded recently.
    Connected,
    /// HOSxP could not be reached.
    Disconnected,
}

/// Result of the backend's `SELECT 1` smoke test.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTestResult {
    /// Round-trip latency of the ping.
    pub latency_ms: u64,
}

/// Overall timeout for a connect+ping round trip. The pool already has its
/// own connect/acquire timeouts; this guards the remaining steps (ping,
/// keychain access) so the UI never waits forever.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(15);

/// Connects a client with a wall-clock timeout, mapping timeouts to a
/// user-facing connection error instead of hanging the command.
async fn connect_client(
    cfg: HosxpConfig,
    action: &'static str,
) -> Result<HosxpClient, CommandError> {
    match tokio::time::timeout(COMMAND_TIMEOUT, HosxpClient::connect(cfg)).await {
        Ok(Ok(client)) => Ok(client),
        Ok(Err(e)) => Err(map_repo_error(e, action)),
        Err(_) => Err(CommandError::new(
            CommandErrorKind::Connection,
            format!("{action}หมดเวลา — ตรวจสอบ Host/Port และเครือข่าย"),
        )),
    }
}

/// Build the connection config for the HOSxP client from the stored
/// connection config plus the history window from the site settings.
fn to_hosxp_config(conn: &ConnectionConfig, history_days: u32) -> HosxpConfig {
    HosxpConfig {
        host: conn.host.clone(),
        port: conn.port,
        database: conn.database.clone(),
        user: conn.user.clone(),
        password: conn.password.clone(),
        history_days,
    }
}

/// Load the stored connection config, mapping a missing file to the
/// user-facing NotConfigured error.
fn stored_connection(state: &AppState) -> Result<ConnectionConfig, CommandError> {
    state.store.load_connection().map_err(|_| {
        CommandError::new(CommandErrorKind::NotConfigured, "ยังไม่ได้ตั้งค่าการเชื่อมต่อ HOSxP")
    })
}

/// Resolve a usable HOSxP client, connecting from the saved config if
/// needed. `action` labels the user-facing error.
///
/// The cache lock is only held for the short cache lookup/store — the slow
/// work (keychain decrypt, TCP connect) happens with no lock held, so a
/// slow/failed connect can never block other commands that need the lock.
async fn client(state: &AppState, action: &'static str) -> Result<HosxpClient, CommandError> {
    {
        let guard = state.client.read().await;
        if let Some(client) = guard.as_ref() {
            return Ok(client.clone());
        }
    }

    let conn = stored_connection(state)?;
    let settings = state
        .store
        .load_settings()
        .map_err(|e| dev_log("client", &e, CommandErrorKind::Query, "อ่านการตั้งค่าไม่สำเร็จ"))?;
    let client = connect_client(to_hosxp_config(&conn, settings.history_days), action).await?;

    // Another command may have cached a client while we connected; keep the
    // first writer's client to avoid dropping an in-use pool.
    let mut slot = state.client.write().await;
    if slot.is_none() {
        *slot = Some(client.clone());
    }
    Ok(client)
}

/// Non-sensitive summary of the saved configuration (never includes the
/// password).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    /// Whether a config has been saved.
    pub configured: bool,
    /// Site label, if configured.
    pub site_name: Option<String>,
    /// Host, if configured.
    pub host: Option<String>,
    /// Database name, if configured.
    pub database: Option<String>,
    /// Database user, if configured.
    pub user: Option<String>,
    /// History window in days.
    pub history_days: u32,
}

/// Report the current configuration status (password never included).
#[tauri::command]
pub async fn get_app_status(state: State<'_, AppState>) -> Result<AppStatus, CommandError> {
    let settings = state.store.load_settings().map_err(|e| {
        dev_log(
            "get_app_status",
            &e,
            CommandErrorKind::Query,
            "อ่านการตั้งค่าไม่สำเร็จ",
        )
    })?;
    match state.store.load_connection() {
        Ok(conn) => Ok(AppStatus {
            configured: true,
            site_name: Some(settings.site_name),
            host: Some(conn.host),
            database: Some(conn.database),
            user: Some(conn.user),
            history_days: settings.history_days,
        }),
        Err(med_recon_config::Error::NoConfig) => Ok(AppStatus {
            configured: false,
            site_name: Some(settings.site_name),
            host: None,
            database: None,
            user: None,
            history_days: settings.history_days,
        }),
        Err(e) => Err(dev_log(
            "get_app_status",
            &e,
            CommandErrorKind::Query,
            "อ่านการตั้งค่าไม่สำเร็จ",
        )),
    }
}

/// Whether connection settings exist.
#[tauri::command]
pub async fn is_configured(state: State<'_, AppState>) -> Result<bool, CommandError> {
    Ok(state.store.connection_exists())
}

/// Live connection health for the top-bar status dot.
#[tauri::command]
pub async fn connection_health(
    state: State<'_, AppState>,
) -> Result<ConnectionHealth, CommandError> {
    if !state.store.connection_exists() {
        return Ok(ConnectionHealth::Unconfigured);
    }
    match client(&state, "ตรวจสอบการเชื่อมต่อ").await {
        Ok(c) => match c.ping().await {
            Ok(()) => {
                *state.health.write().await = ConnectionHealth::Connected;
                Ok(ConnectionHealth::Connected)
            }
            Err(_) => {
                *state.health.write().await = ConnectionHealth::Disconnected;
                Ok(ConnectionHealth::Disconnected)
            }
        },
        Err(e) => match e.kind {
            CommandErrorKind::NotConfigured => Ok(ConnectionHealth::Unconfigured),
            _ => Ok(ConnectionHealth::Disconnected),
        },
    }
}

/// Save the HOSxP connection config (encrypted at rest) and connect.
#[tauri::command]
pub async fn save_connection(
    state: State<'_, AppState>,
    config: ConnectionInput,
) -> Result<(), CommandError> {
    let connection: ConnectionConfig = config.into();
    let settings = state.store.load_settings().map_err(|e| {
        dev_log(
            "save_connection",
            &e,
            CommandErrorKind::Query,
            "อ่านการตั้งค่าไม่สำเร็จ",
        )
    })?;
    let hosxp_config = to_hosxp_config(&connection, settings.history_days);

    // Connect before persisting so a bad password never gets saved.
    let client = connect_client(hosxp_config, "บันทึกการตั้งค่า").await?;

    // Encrypting for disk touches the OS keychain, which can stall on a
    // permission dialog with unsigned dev binaries — cap it as well.
    let saved = tokio::time::timeout(COMMAND_TIMEOUT, async {
        state.store.save_connection(&connection)
    })
    .await;
    match saved {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            return Err(dev_log(
                "save_connection",
                &e,
                CommandErrorKind::Query,
                "บันทึกการตั้งค่าไม่สำเร็จ",
            ));
        }
        Err(_) => {
            return Err(CommandError::new(
                CommandErrorKind::Query,
                "บันทึกการตั้งค่าไม่สำเร็จ — การเข้าถึง Keychain ใช้เวลานานเกินไป",
            ));
        }
    }

    // Replace the cached client under a short lock; the previous pool is
    // closed after the lock is released so no in-flight query is disturbed.
    let old = {
        let mut slot = state.client.write().await;
        slot.replace(client.clone())
    };
    if let Some(old) = old {
        old.disconnect().await;
    }
    tracing::info!(
        host = %connection.host,
        "connection saved and connected"
    );
    Ok(())
}

/// Non-secret summary of the saved connection (the password is never
/// returned — it cannot be restored into the form).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    /// Hostname or IP.
    pub host: String,
    /// TCP port.
    pub port: u16,
    /// HOSxP database name.
    pub database: String,
    /// Database user.
    pub user: String,
}

/// The saved HOSxP connection, without the password — used to pre-fill the
/// settings form so re-saving does not require retyping everything.
#[tauri::command]
pub async fn get_connection(state: State<'_, AppState>) -> Result<ConnectionInfo, CommandError> {
    let conn = stored_connection(&state)?;
    Ok(ConnectionInfo {
        host: conn.host,
        port: conn.port,
        database: conn.database,
        user: conn.user,
    })
}

/// The non-secret site settings (site name, history window, current
/// medication list) — independent of the connection config.
#[tauri::command]
pub async fn get_site_settings(state: State<'_, AppState>) -> Result<SiteSettings, CommandError> {
    state.store.load_settings().map_err(|e| {
        dev_log(
            "get_site_settings",
            &e,
            CommandErrorKind::Query,
            "อ่านการตั้งค่าไม่สำเร็จ",
        )
    })
}

/// Save the non-secret site settings as plain JSON (`settings.json`).
#[tauri::command]
pub async fn save_site_settings(
    state: State<'_, AppState>,
    settings: SiteSettingsInput,
) -> Result<(), CommandError> {
    let settings: SiteSettings = settings.into();
    state.store.save_settings(&settings).map_err(|e| {
        dev_log(
            "save_site_settings",
            &e,
            CommandErrorKind::Query,
            "บันทึกการตั้งค่าไม่สำเร็จ",
        )
    })
}

/// Validate connectivity against the given settings (without saving) or the
/// saved configuration when no settings are provided.
#[tauri::command]
pub async fn test_connection(
    state: State<'_, AppState>,
    config: Option<ConnectionInput>,
) -> Result<ConnectionTestResult, CommandError> {
    let (conn, history_days) = match config {
        Some(input) => (ConnectionConfig::from(input), 730),
        None => {
            let conn = stored_connection(&state)?;
            let settings = state.store.load_settings().map_err(|e| {
                dev_log(
                    "test_connection",
                    &e,
                    CommandErrorKind::Query,
                    "อ่านการตั้งค่าไม่สำเร็จ",
                )
            })?;
            (conn, settings.history_days)
        }
    };

    let started = Instant::now();
    let client = connect_client(to_hosxp_config(&conn, history_days), "ทดสอบการเชื่อมต่อ").await?;
    let ping = tokio::time::timeout(COMMAND_TIMEOUT, client.ping()).await;
    match ping {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(map_repo_error(e, "ทดสอบการเชื่อมต่อ")),
        Err(_) => {
            return Err(CommandError::new(
                CommandErrorKind::Connection,
                "ทดสอบการเชื่อมต่อหมดเวลา — ตรวจสอบ Host/Port และเครือข่าย",
            ));
        }
    }
    client.disconnect().await;

    Ok(ConnectionTestResult {
        latency_ms: started.elapsed().as_millis() as u64,
    })
}

/// Remove the saved configuration and drop the connection pool.
#[tauri::command]
pub async fn clear_site_config(state: State<'_, AppState>) -> Result<(), CommandError> {
    *state.client.write().await = None;
    state.store.clear().map_err(|e| {
        dev_log(
            "clear_site_config",
            &e,
            CommandErrorKind::Query,
            "ลบการตั้งค่าไม่สำเร็จ",
        )
    })
}

/// Search patients by CID, HN, or name.
#[tauri::command]
pub async fn search_patients(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<PatientSummary>, CommandError> {
    let client = client(&state, "ค้นหาผู้ป่วย").await?;
    let results = client
        .search_patients(&query)
        .await
        .map_err(|e| map_repo_error(e, "ค้นหาผู้ป่วย"))?;
    tracing::debug!(
        query_len = query.len(),
        count = results.len(),
        "patient search"
    );
    Ok(results)
}

/// Load the full cross-visit medication + allergy history for a patient.
#[tauri::command]
pub async fn load_history(
    state: State<'_, AppState>,
    hn: String,
) -> Result<PatientHistory, CommandError> {
    let client = client(&state, "โหลดประวัติ").await?;
    let current_codes = configured_med_codes(&state);
    let history = client
        .load_history(&hn, &current_codes)
        .await
        .map_err(|e| map_repo_error(e, "โหลดประวัติ"))?;
    tracing::debug!(hn = %med_recon_core::redact_hn(&hn), "patient history loaded");
    Ok(history)
}

/// Export a printable HTML medication history report for a patient.
#[tauri::command]
pub async fn export_report(
    state: State<'_, AppState>,
    hn: String,
    labels: crate::report::ReportLabels,
) -> Result<String, CommandError> {
    let client = client(&state, "พิมพ์ประวัติการได้รับยา").await?;
    let current_codes = configured_med_codes(&state);
    let history = client
        .load_history(&hn, &current_codes)
        .await
        .map_err(|e| map_repo_error(e, "พิมพ์ประวัติการได้รับยา"))?;

    let site_name = state
        .store
        .load_settings()
        .map(|s| s.site_name)
        .unwrap_or_default();

    let html = crate::report::build_report(&history, &site_name, &labels);
    let path = rfd::AsyncFileDialog::new()
        .set_title("Export medication history report")
        .set_file_name(format!("med-recon-report-{hn}.html"))
        .save_file()
        .await
        .map(|handle| handle.path().to_path_buf())
        .ok_or_else(|| CommandError::new(CommandErrorKind::Query, "ยกเลิกการส่งออก"))?;

    std::fs::write(&path, html).map_err(|e| {
        dev_log(
            "export_report",
            &e,
            CommandErrorKind::Query,
            "เขียนไฟล์รายงานไม่สำเร็จ",
        )
    })?;
    tracing::debug!(hn = %med_recon_core::redact_hn(&hn), "report exported");
    Ok(path.display().to_string())
}
