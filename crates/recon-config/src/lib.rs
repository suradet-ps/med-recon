//! HOSxP configuration store — two independent JSON files under the
//! platform config directory:
//!
//! * `connection.json` — host/port/database/user/password, stored
//!   **encrypted** (AES-256-GCM + HKDF via `encryptman`, master key in the
//!   OS keychain via `encryptman-keyring`). Credentials never touch disk
//!   in plaintext; the keychain gives per-user key isolation.
//! * `settings.json` — non-secret site settings: `site_name`,
//!   `history_days`, and the operator-configured current-medication list
//!   (`current_med_codes`). Plain readable JSON so operators can back up
//!   or edit it directly.
//!
//! On first open, a legacy single-file config (`site-config.json`, the
//! pre-split encrypted blob) is migrated into the two files and renamed to
//! `site-config.json.bak`.

use std::fs;
use std::path::{Path, PathBuf};

use encryptman_keyring::Vault;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Keyring service name — shared by all Recon users on this machine.
const KEYRING_SERVICE: &str = "Recon";

/// Encrypted connection settings file name.
pub const CONNECTION_FILE: &str = "connection.json";
/// Plain site settings file name.
pub const SETTINGS_FILE: &str = "settings.json";
/// Legacy pre-split config file name (migrated on first open).
const LEGACY_FILE: &str = "site-config.json";
/// Backup name for the migrated legacy file.
const LEGACY_BACKUP_FILE: &str = "site-config.json.bak";

/// HOSxP connection settings — the encrypted half of the store.
///
/// `PartialEq` is intentionally not derived: the password is a
/// [`SecretString`] and must not be compared/logged incidentally.
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    /// Hostname or IP.
    pub host: String,
    /// TCP port.
    pub port: u16,
    /// HOSxP database name.
    pub database: String,
    /// Database user.
    pub user: String,
    /// Database password (never serialized in plaintext).
    pub password: SecretString,
}

/// Plaintext payload — the shape written inside the encrypted blob.
#[derive(Debug, Serialize, Deserialize)]
struct ConnectionConfigRaw {
    host: String,
    port: u16,
    database: String,
    user: String,
    password: String,
}

/// Non-secret site settings — the plain JSON half of the store.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SiteSettings {
    /// Human-readable site label (e.g. "โรงพยาบาลสมมติ"); shown on the
    /// exported report when set.
    pub site_name: String,
    /// History window in days.
    pub history_days: u32,
    /// `icode`s of the operator-configured current medications (from
    /// `drugitems`). Drives the BPMH active/lapsed split: only drugs on
    /// this list are shown as ยาที่คาดว่ายังใช้อยู่.
    pub current_med_codes: Vec<String>,
}

impl Default for SiteSettings {
    fn default() -> Self {
        Self {
            site_name: String::new(),
            history_days: 730,
            current_med_codes: Vec::new(),
        }
    }
}

/// File wrapper around the encrypted blob.
#[derive(Debug, Serialize, Deserialize)]
struct EncryptedFile {
    version: u32,
    ciphertext: String,
}

/// Errors produced by the config store.
#[derive(Debug, Error)]
pub enum Error {
    /// The OS keychain is unavailable or rejected the operation.
    #[error("keychain error: {0}")]
    Keychain(#[from] encryptman_keyring::Error),

    /// Encryption/decryption failed.
    #[error("crypto error: {0}")]
    Crypto(#[from] encryptman::CryptoError),

    /// The config file could not be read/written.
    #[error("config file error at {path}: {source}")]
    Io {
        /// Path that failed.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// The stored payload is not valid JSON.
    #[error("config file is not valid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),

    /// The vault (keychain) rejected the operation.
    #[error("vault error: {0}")]
    Vault(String),

    /// The stored payload uses an unsupported format version.
    #[error("unsupported config version: {0}")]
    UnsupportedVersion(u32),

    /// No connection config file exists.
    #[error("no connection configuration saved")]
    NoConfig,
}

/// Result alias for the config store.
pub type Result<T> = std::result::Result<T, Error>;

/// Abstraction over the master-key vault so the store is testable without
/// touching the OS keychain.
pub trait SecretVault: Send + Sync {
    /// Encrypt a plaintext string.
    fn encrypt(
        &self,
        plaintext: &str,
    ) -> std::result::Result<String, Box<dyn std::error::Error + Send + Sync>>;
    /// Decrypt a ciphertext string.
    fn decrypt(
        &self,
        ciphertext: &str,
    ) -> std::result::Result<String, Box<dyn std::error::Error + Send + Sync>>;
}

/// Default vault: OS keychain-backed master key.
pub struct KeyringVault {
    vault: Vault,
}

impl KeyringVault {
    /// Open (or create on first use) the OS keychain vault.
    pub fn new() -> Result<Self> {
        Ok(Self {
            vault: Vault::new(KEYRING_SERVICE)?,
        })
    }
}

impl SecretVault for KeyringVault {
    fn encrypt(
        &self,
        plaintext: &str,
    ) -> std::result::Result<String, Box<dyn std::error::Error + Send + Sync>> {
        self.vault.encrypt(plaintext).map_err(Box::from)
    }

    fn decrypt(
        &self,
        ciphertext: &str,
    ) -> std::result::Result<String, Box<dyn std::error::Error + Send + Sync>> {
        self.vault.decrypt(ciphertext).map_err(Box::from)
    }
}

/// Configuration store bound to a directory, managing two JSON files:
/// [`CONNECTION_FILE`] (encrypted) and [`SETTINGS_FILE`] (plain).
pub struct ConfigStore {
    vault: Box<dyn SecretVault>,
    dir: PathBuf,
}

impl ConfigStore {
    /// Default store: keyring vault + platform config directory.
    ///
    /// Migrates a legacy single-file config when present.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Keychain`] when the OS keychain is unavailable.
    pub fn open() -> Result<Self> {
        let vault = KeyringVault::new()?;
        let store = Self::with_vault(Box::new(vault), default_config_dir());
        store.migrate_legacy()?;
        Ok(store)
    }

    /// Store with an explicit vault and directory (used by tests and
    /// embedded deployments).
    pub fn with_vault(vault: Box<dyn SecretVault>, dir: PathBuf) -> Self {
        Self { vault, dir }
    }

    /// Absolute path of the encrypted connection file.
    pub fn connection_path(&self) -> PathBuf {
        self.dir.join(CONNECTION_FILE)
    }

    /// Absolute path of the plain site-settings file.
    pub fn settings_path(&self) -> PathBuf {
        self.dir.join(SETTINGS_FILE)
    }

    /// Whether a connection config has been saved.
    pub fn connection_exists(&self) -> bool {
        self.connection_path().exists()
    }

    /// Whether site settings have been saved (absent settings fall back to
    /// [`SiteSettings::default`]).
    pub fn settings_exists(&self) -> bool {
        self.settings_path().exists()
    }

    /// Load and decrypt the connection config.
    ///
    /// Returns [`Error::NoConfig`] when none has been saved yet.
    pub fn load_connection(&self) -> Result<ConnectionConfig> {
        let raw = read_optional(&self.connection_path())?
            .ok_or(Error::NoConfig)?;
        let file: EncryptedFile = serde_json::from_str(&raw)?;
        if file.version != 1 {
            return Err(Error::UnsupportedVersion(file.version));
        }
        let plaintext = self
            .vault
            .decrypt(&file.ciphertext)
            .map_err(|e| Error::Vault(e.to_string()))?;
        let raw_config: ConnectionConfigRaw = serde_json::from_str(&plaintext)?;
        Ok(raw_config.into())
    }

    /// Encrypt and persist the connection config.
    pub fn save_connection(&self, config: &ConnectionConfig) -> Result<()> {
        let raw_config: ConnectionConfigRaw = config.clone().into();
        let plaintext = serde_json::to_string(&raw_config)?;
        let ciphertext = self
            .vault
            .encrypt(&plaintext)
            .map_err(|e| Error::Vault(e.to_string()))?;
        let file = EncryptedFile {
            version: 1,
            ciphertext,
        };
        write_json(&self.connection_path(), &file)
    }

    /// Load the site settings; returns [`SiteSettings::default`] when no
    /// settings file exists yet.
    pub fn load_settings(&self) -> Result<SiteSettings> {
        let path = self.settings_path();
        let raw = match read_optional(&path)? {
            Some(raw) => raw,
            None => return Ok(SiteSettings::default()),
        };
        if raw.trim().is_empty() {
            return Ok(SiteSettings::default());
        }
        Ok(serde_json::from_str(&raw)?)
    }

    /// Persist the site settings as plain JSON.
    pub fn save_settings(&self, settings: &SiteSettings) -> Result<()> {
        write_json(&self.settings_path(), settings)
    }

    /// Remove both config files (and any legacy backup). The keyring key is
    /// kept so that future saves are encrypted under the same master key.
    pub fn clear(&self) -> Result<()> {
        remove_optional(&self.connection_path())?;
        remove_optional(&self.settings_path())?;
        remove_optional(&self.dir.join(LEGACY_BACKUP_FILE))?;
        Ok(())
    }

    /// Split a legacy single-file `site-config.json` (pre-split format)
    /// into the two new files and rename it to `site-config.json.bak`.
    ///
    /// No-op when no legacy file exists. When both new files already exist
    /// the legacy file is archived untouched (never overwrites current
    /// data); otherwise each missing file is backfilled from the legacy
    /// payload.
    fn migrate_legacy(&self) -> Result<()> {
        let legacy = self.dir.join(LEGACY_FILE);
        if !legacy.exists() {
            return Ok(());
        }

        if self.connection_exists() && self.settings_exists() {
            return archive_legacy(&legacy);
        }

        let raw = fs::read_to_string(&legacy).map_err(|e| map_io(e, &legacy))?;
        let file: EncryptedFile = serde_json::from_str(&raw)?;
        if file.version != 1 {
            return Err(Error::UnsupportedVersion(file.version));
        }
        let plaintext = self
            .vault
            .decrypt(&file.ciphertext)
            .map_err(|e| Error::Vault(e.to_string()))?;
        let legacy_config: LegacySiteConfig = serde_json::from_str(&plaintext)?;

        if !self.connection_exists() {
            self.save_connection(&legacy_config.connection())?;
        }
        if !self.settings_exists() {
            self.save_settings(&legacy_config.settings())?;
        }

        archive_legacy(&legacy)
    }
}

/// Rename the legacy file to [`LEGACY_BACKUP_FILE`].
fn archive_legacy(legacy: &Path) -> Result<()> {
    let backup = legacy
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(LEGACY_BACKUP_FILE);
    fs::rename(legacy, &backup).map_err(|e| map_io(e, legacy))
}

/// The legacy (pre-split) config payload, used only for migration.
#[derive(Debug, Deserialize)]
struct LegacySiteConfig {
    site_name: String,
    host: String,
    port: u16,
    database: String,
    user: String,
    password: String,
    history_days: u32,
    #[serde(default)]
    current_med_codes: Vec<String>,
}

impl LegacySiteConfig {
    fn connection(&self) -> ConnectionConfig {
        ConnectionConfig {
            host: self.host.clone(),
            port: self.port,
            database: self.database.clone(),
            user: self.user.clone(),
            password: SecretString::from(self.password.clone()),
        }
    }

    fn settings(&self) -> SiteSettings {
        SiteSettings {
            site_name: self.site_name.clone(),
            history_days: self.history_days,
            current_med_codes: self.current_med_codes.clone(),
        }
    }
}

impl From<ConnectionConfig> for ConnectionConfigRaw {
    fn from(c: ConnectionConfig) -> Self {
        Self {
            host: c.host,
            port: c.port,
            database: c.database,
            user: c.user,
            password: c.password.expose_secret().to_string(),
        }
    }
}

impl From<ConnectionConfigRaw> for ConnectionConfig {
    fn from(r: ConnectionConfigRaw) -> Self {
        Self {
            host: r.host,
            port: r.port,
            database: r.database,
            user: r.user,
            password: SecretString::from(r.password),
        }
    }
}

/// Platform config directory for the app (`~/Library/Application Support/Recon`
/// on macOS, `~/.config/Recon` on Linux, `%APPDATA%\Recon` on Windows).
fn default_config_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "Recon")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn map_io(source: std::io::Error, path: &Path) -> Error {
    Error::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Read a file, mapping a missing file to `Ok(None)`.
fn read_optional(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(raw) => Ok(Some(raw)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(map_io(e, path)),
    }
}

/// Remove a file, ignoring a missing file.
fn remove_optional(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(map_io(e, path)),
    }
}

/// Serialize `value` as pretty JSON and write it (creating the parent
/// directory), ensuring atomic-ish replacement via a temp file rename.
fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| map_io(e, path))?;
    }
    fs::write(path, json).map_err(|e| map_io(e, path))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Test vault keeping a master key in memory.
    struct InMemoryVault {
        key: encryptman::MasterKey,
    }

    impl InMemoryVault {
        fn new() -> Self {
            Self {
                key: encryptman::MasterKey::generate().unwrap(),
            }
        }
    }

    impl SecretVault for InMemoryVault {
        fn encrypt(
            &self,
            plaintext: &str,
        ) -> std::result::Result<String, Box<dyn std::error::Error + Send + Sync>> {
            Ok(encryptman::encrypt(&self.key, plaintext)?)
        }

        fn decrypt(
            &self,
            ciphertext: &str,
        ) -> std::result::Result<String, Box<dyn std::error::Error + Send + Sync>> {
            Ok(encryptman::decrypt(&self.key, ciphertext)?)
        }
    }

    /// Tests touch a real temporary directory — serialized only.
    static DIR_LOCK: Mutex<()> = Mutex::new(());

    fn sample_connection() -> ConnectionConfig {
        ConnectionConfig {
            host: "10.0.0.5".into(),
            port: 3306,
            database: "hos".into(),
            user: "recon_ro".into(),
            password: SecretString::from("sup3r-s3cret"),
        }
    }

    fn sample_settings() -> SiteSettings {
        SiteSettings {
            site_name: "รพ.ทดสอบ".into(),
            history_days: 730,
            current_med_codes: vec!["P1".into(), "M1".into()],
        }
    }

    fn assert_connection_eq(a: &ConnectionConfig, b: &ConnectionConfig) {
        assert_eq!(a.host, b.host);
        assert_eq!(a.port, b.port);
        assert_eq!(a.database, b.database);
        assert_eq!(a.user, b.user);
        assert_eq!(a.password.expose_secret(), b.password.expose_secret());
    }

    fn store_in(dir: &tempfile::TempDir) -> ConfigStore {
        ConfigStore::with_vault(Box::new(InMemoryVault::new()), dir.path().to_path_buf())
    }

    #[test]
    fn connection_roundtrip() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);

        assert!(matches!(store.load_connection(), Err(Error::NoConfig)));
        assert!(!store.connection_exists());

        store.save_connection(&sample_connection()).unwrap();
        assert!(store.connection_exists());
        assert_connection_eq(&store.load_connection().unwrap(), &sample_connection());
    }

    #[test]
    fn settings_roundtrip_and_are_plain_json_on_disk() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);

        // Missing settings file loads as defaults.
        assert_eq!(store.load_settings().unwrap(), SiteSettings::default());
        assert!(!store.settings_exists());

        store.save_settings(&sample_settings()).unwrap();
        assert!(store.settings_exists());
        assert_eq!(store.load_settings().unwrap(), sample_settings());

        // Non-secret by design: the file is human-readable JSON.
        let on_disk = fs::read_to_string(store.settings_path()).unwrap();
        assert!(on_disk.contains("รพ.ทดสอบ"));
        assert!(on_disk.contains("currentMedCodes"));
    }

    #[test]
    fn settings_missing_field_defaults() {
        // Settings saved before `current_med_codes` existed still load.
        let raw: SiteSettings = serde_json::from_str(
            r#"{"siteName":"x","historyDays":730}"#,
        )
        .unwrap();
        assert_eq!(raw, SiteSettings {
            site_name: "x".into(),
            history_days: 730,
            current_med_codes: vec![],
        });
    }

    #[test]
    fn connection_disk_contains_no_plaintext_credentials() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        store.save_connection(&sample_connection()).unwrap();

        let on_disk = fs::read_to_string(store.connection_path()).unwrap();
        assert!(
            !on_disk.contains("sup3r-s3cret"),
            "plaintext password on disk!"
        );
        assert!(!on_disk.contains("10.0.0.5"), "plaintext host on disk!");
        assert!(on_disk.contains("ciphertext"));
    }

    #[test]
    fn wrong_vault_cannot_decrypt_connection() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        store.save_connection(&sample_connection()).unwrap();

        let other = ConfigStore::with_vault(
            Box::new(InMemoryVault::new()),
            dir.path().to_path_buf(),
        );
        assert!(
            other.load_connection().is_err(),
            "decrypt with wrong key must fail"
        );
    }

    #[test]
    fn clear_removes_both_files() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        store.save_connection(&sample_connection()).unwrap();
        store.save_settings(&sample_settings()).unwrap();

        store.clear().unwrap();
        assert!(!store.connection_path().exists());
        assert!(!store.settings_path().exists());
        assert!(matches!(store.load_connection(), Err(Error::NoConfig)));
    }

    #[test]
    fn unicode_and_secrets_roundtrip() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let mut cfg = sample_connection();
        cfg.password = SecretString::from("พาสเวิร์ด! 🔐 x$y%");
        store.save_connection(&cfg).unwrap();
        assert_connection_eq(&store.load_connection().unwrap(), &cfg);
    }

    #[test]
    fn unsupported_version_rejected() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        fs::write(
            &store.connection_path(),
            r#"{"version": 99, "ciphertext": "abc"}"#,
        )
        .unwrap();
        assert!(matches!(
            store.load_connection(),
            Err(Error::UnsupportedVersion(99))
        ));
    }

    #[test]
    fn legacy_config_migrates_to_two_files() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let vault = Box::new(InMemoryVault::new());
        let legacy_path = dir.path().join(LEGACY_FILE);
        let legacy_payload = serde_json::json!({
            "site_name": "รพ.เดิม",
            "host": "192.168.1.7",
            "port": 3306,
            "database": "hos",
            "user": "recon_ro",
            "password": "legacy-pass",
            "history_days": 365,
            "current_med_codes": ["P1"]
        })
        .to_string();
        let ciphertext = vault
            .encrypt(&legacy_payload)
            .expect("invariant: test vault encrypts");
        let blob = serde_json::to_string_pretty(&EncryptedFile {
            version: 1,
            ciphertext,
        })
        .unwrap();
        fs::write(&legacy_path, blob).unwrap();

        let store = ConfigStore::with_vault(vault, dir.path().to_path_buf());
        store.migrate_legacy().unwrap();

        let conn = store.load_connection().unwrap();
        assert_eq!(conn.host, "192.168.1.7");
        assert_eq!(conn.password.expose_secret(), "legacy-pass");
        let settings = store.load_settings().unwrap();
        assert_eq!(settings.site_name, "รพ.เดิม");
        assert_eq!(settings.history_days, 365);
        assert_eq!(settings.current_med_codes, vec!["P1".to_string()]);

        // Legacy file renamed, not deleted; nothing plaintext remains.
        assert!(!legacy_path.exists());
        assert!(dir.path().join(LEGACY_BACKUP_FILE).exists());
        let backup = fs::read_to_string(dir.path().join(LEGACY_BACKUP_FILE)).unwrap();
        assert!(!backup.contains("legacy-pass"));
        assert!(!fs::read_to_string(store.connection_path())
            .unwrap()
            .contains("legacy-pass"));
    }

    #[test]
    fn migrate_legacy_keeps_existing_files() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        store.save_connection(&sample_connection()).unwrap();
        store.save_settings(&sample_settings()).unwrap();

        // A stale legacy file appears later — must be archived, never
        // decrypted or allowed to overwrite current data.
        let legacy = dir.path().join(LEGACY_FILE);
        fs::write(&legacy, "garbage").unwrap();
        store.migrate_legacy().unwrap();

        assert!(!legacy.exists());
        assert!(dir.path().join(LEGACY_BACKUP_FILE).exists());
        assert_connection_eq(&store.load_connection().unwrap(), &sample_connection());
        assert_eq!(store.load_settings().unwrap(), sample_settings());
    }
}
