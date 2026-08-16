//! Tauri command adapters.
//!
//! Errors cross the IPC as a typed [`CommandError`] (kind + Thai message)
//! so the frontend can decide presentation from the kind — e.g. raise the
//! connection banner — instead of matching on message text. Crates stay
//! English and typed; this layer is the only place where errors are
//! translated for the UI.

use std::time::{Duration, Instant};

use recon_config::SiteConfig;
use recon_core::{DateEra, PatientHistory, PatientSummary};
use recon_hosxp::{HosxpClient, HosxpConfig};
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

impl recon_config::SecretVault for EphemeralVault {
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
    eprintln!("[recon] {context} failed: {detail:?}");
    CommandError::new(kind, message)
}

/// Maps a repository error to a typed command error. `action` is the Thai
/// verb phrase for the Query variant ("ค้นหาผู้ป่วย" etc.).
fn map_repo_error(err: recon_hosxp::Error, action: &'static str) -> CommandError {
    eprintln!("[recon] {action} failed: {err:?}");
    match err {
        recon_hosxp::Error::Connect { .. } => {
            CommandError::new(CommandErrorKind::Connection, "เชื่อมต่อฐานข้อมูล HOSxP ไม่สำเร็จ")
        }
        recon_hosxp::Error::Database(sqlx::Error::PoolTimedOut)
        | recon_hosxp::Error::Database(sqlx::Error::PoolClosed)
        | recon_hosxp::Error::Database(sqlx::Error::Io(_)) => {
            CommandError::new(CommandErrorKind::Connection, "เชื่อมต่อฐานข้อมูล HOSxP ไม่สำเร็จ")
        }
        recon_hosxp::Error::Database(sqlx::Error::Database(db)) => {
            CommandError::new(CommandErrorKind::Query, format!("{action}ไม่สำเร็จ ({db})"))
        }
        recon_hosxp::Error::ReadOnlyViolation(_) => {
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
    /// Human-readable site label.
    pub site_name: String,
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
    /// Date era of the site's HOSxP date columns.
    pub era: DateEra,
    /// History window in days.
    pub history_days: u32,
    /// Read `medusage` sig data (verify schema before enabling).
    pub use_medusage_sig: bool,
}

impl From<ConnectionInput> for SiteConfig {
    fn from(i: ConnectionInput) -> Self {
        Self {
            site_name: i.site_name,
            host: i.host,
            port: i.port,
            database: i.database,
            user: i.user,
            password: SecretString::from(i.password),
            era: i.era,
            history_days: i.history_days,
            use_medusage_sig: i.use_medusage_sig,
        }
    }
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

/// Build the connection config for the HOSxP client from the stored site
/// config.
fn to_hosxp_config(cfg: &SiteConfig) -> HosxpConfig {
    HosxpConfig {
        host: cfg.host.clone(),
        port: cfg.port,
        database: cfg.database.clone(),
        user: cfg.user.clone(),
        password: cfg.password.clone(),
        era: cfg.era,
        history_days: cfg.history_days,
        use_medusage_sig: cfg.use_medusage_sig,
    }
}

/// Resolve a usable HOSxP client, connecting from the saved config if
/// needed. `action` labels the user-facing error.
async fn client(state: &AppState, action: &'static str) -> Result<HosxpClient, CommandError> {
    let guard = state.client.read().await;
    if let Some(client) = guard.as_ref() {
        return Ok(client.clone());
    }

    let cfg = state.store.load().map_err(|_| {
        CommandError::new(CommandErrorKind::NotConfigured, "ยังไม่ได้ตั้งค่าการเชื่อมต่อ HOSxP")
    })?;
    let client = connect_client(to_hosxp_config(&cfg), action).await?;

    *state.client.write().await = Some(client.clone());
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
    /// Date era.
    pub era: DateEra,
    /// History window in days.
    pub history_days: u32,
    /// Whether `medusage` sig reading is enabled.
    pub use_medusage_sig: bool,
}

/// Report the current configuration status (password never included).
#[tauri::command]
pub async fn get_app_status(state: State<'_, AppState>) -> Result<AppStatus, CommandError> {
    match state.store.load() {
        Ok(cfg) => Ok(AppStatus {
            configured: true,
            site_name: Some(cfg.site_name),
            host: Some(cfg.host),
            database: Some(cfg.database),
            user: Some(cfg.user),
            era: cfg.era,
            history_days: cfg.history_days,
            use_medusage_sig: cfg.use_medusage_sig,
        }),
        Err(recon_config::Error::NoConfig) => Ok(AppStatus {
            configured: false,
            site_name: None,
            host: None,
            database: None,
            user: None,
            era: DateEra::Christian,
            history_days: 730,
            use_medusage_sig: false,
        }),
        Err(e) => Err(dev_log(
            "get_app_status",
            &e,
            CommandErrorKind::Query,
            "อ่านการตั้งค่าไม่สำเร็จ",
        )),
    }
}

/// Whether stored settings exist.
#[tauri::command]
pub async fn is_configured(state: State<'_, AppState>) -> Result<bool, CommandError> {
    Ok(state.store.exists())
}

/// Live connection health for the top-bar status dot.
#[tauri::command]
pub async fn connection_health(
    state: State<'_, AppState>,
) -> Result<ConnectionHealth, CommandError> {
    if !state.store.exists() {
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

/// Save the site configuration (encrypted at rest) and connect.
#[tauri::command]
pub async fn save_site_config(
    state: State<'_, AppState>,
    config: ConnectionInput,
) -> Result<(), CommandError> {
    let site_config: SiteConfig = config.into();
    let hosxp_config = to_hosxp_config(&site_config);

    // Connect before persisting so a bad password never gets saved.
    let client = connect_client(hosxp_config, "บันทึกการตั้งค่า").await?;

    // Encrypting for disk touches the OS keychain, which can stall on a
    // permission dialog with unsigned dev binaries — cap it as well.
    let saved =
        tokio::time::timeout(COMMAND_TIMEOUT, async { state.store.save(&site_config) }).await;
    match saved {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            return Err(dev_log(
                "save_site_config",
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

    *state.client.write().await = Some(client);
    tracing::info!(
        site = %site_config.site_name,
        host = %site_config.host,
        "site configuration saved and connected"
    );
    Ok(())
}

/// Validate connectivity against the given settings (without saving) or the
/// saved configuration when no settings are provided.
#[tauri::command]
pub async fn test_connection(
    state: State<'_, AppState>,
    config: Option<ConnectionInput>,
) -> Result<ConnectionTestResult, CommandError> {
    let cfg = match config {
        Some(input) => SiteConfig::from(input),
        None => state.store.load().map_err(|_| {
            CommandError::new(CommandErrorKind::NotConfigured, "ยังไม่ได้ตั้งค่าการเชื่อมต่อ HOSxP")
        })?,
    };

    let started = Instant::now();
    let client = connect_client(to_hosxp_config(&cfg), "ทดสอบการเชื่อมต่อ").await?;
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
    let history = client
        .load_history(&hn)
        .await
        .map_err(|e| map_repo_error(e, "โหลดประวัติ"))?;
    tracing::debug!(hn = %recon_core::redact_hn(&hn), "patient history loaded");
    Ok(history)
}

/// Export a printable HTML medication history report for a patient.
#[tauri::command]
pub async fn export_report(state: State<'_, AppState>, hn: String) -> Result<String, CommandError> {
    let client = client(&state, "ส่งออกรายงาน").await?;
    let history = client
        .load_history(&hn)
        .await
        .map_err(|e| map_repo_error(e, "ส่งออกรายงาน"))?;

    let site_name = state
        .store
        .load()
        .ok()
        .map(|c| c.site_name)
        .unwrap_or_default();

    let html = crate::report::build_report(&history, &site_name);
    let path = rfd::AsyncFileDialog::new()
        .set_title("Export medication history report")
        .set_file_name(format!("recon-report-{hn}.html"))
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
    tracing::debug!(hn = %recon_core::redact_hn(&hn), "report exported");
    Ok(path.display().to_string())
}
