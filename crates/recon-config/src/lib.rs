//! Encrypted HOSxP site configuration.
//!
//! Connection credentials (including the database password) are stored on
//! disk **encrypted**: the plaintext JSON payload is encrypted with
//! `encryptman` (AES-256-GCM + HKDF) under a master key held in the OS
//! keychain via `encryptman-keyring`. No credential ever touches disk in
//! plaintext, and the keychain gives per-user key isolation.

use std::fs;
use std::path::{Path, PathBuf};

use encryptman_keyring::Vault;
use recon_core::DateEra;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Keyring service name — shared by all Recon users on this machine.
const KEYRING_SERVICE: &str = "org.recon.desktop";

/// HOSxP connection settings owned by the store.
///
/// `PartialEq` is intentionally not derived: the password is a
/// [`SecretString`] and must not be compared/logged incidentally.
#[derive(Debug, Clone)]
pub struct SiteConfig {
    /// Human-readable site label (e.g. "โรงพยาบาลสมมติ").
    pub site_name: String,
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
    /// Date era of the site's HOSxP date columns.
    pub era: DateEra,
    /// History window in days.
    pub history_days: u32,
    /// Read `medusage` sig data (verify schema before enabling).
    pub use_medusage_sig: bool,
}

/// Plaintext payload — the shape written to disk inside the encrypted blob.
#[derive(Debug, Serialize, Deserialize)]
struct SiteConfigRaw {
    site_name: String,
    host: String,
    port: u16,
    database: String,
    user: String,
    password: String,
    era: DateEra,
    history_days: u32,
    use_medusage_sig: bool,
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

    /// No config file exists.
    #[error("no configuration saved")]
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

/// Encrypted config store bound to a file path.
pub struct ConfigStore {
    vault: Box<dyn SecretVault>,
    path: PathBuf,
}

impl ConfigStore {
    /// Default store: keyring vault + platform config directory.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Keychain`] when the OS keychain is unavailable.
    pub fn open() -> Result<Self> {
        let vault = KeyringVault::new()?;
        let path = default_config_path();
        Ok(Self {
            vault: Box::new(vault),
            path,
        })
    }

    /// Store with an explicit vault and path (used by tests and embedded
    /// deployments).
    pub fn with_vault(vault: Box<dyn SecretVault>, path: PathBuf) -> Self {
        Self { vault, path }
    }

    /// Absolute path of the config file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether a config file already exists.
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Load and decrypt the site configuration.
    ///
    /// Returns [`Error::NoConfig`] when no config has been saved yet.
    pub fn load(&self) -> Result<SiteConfig> {
        let raw = match fs::read_to_string(&self.path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Error::NoConfig),
            Err(e) => return Err(map_io(e, &self.path)),
        };
        if raw.trim().is_empty() {
            return Err(Error::NoConfig);
        }
        let file: EncryptedFile = serde_json::from_str(&raw)?;
        if file.version != 1 {
            return Err(Error::UnsupportedVersion(file.version));
        }
        let plaintext = self
            .vault
            .decrypt(&file.ciphertext)
            .map_err(|e| Error::Vault(e.to_string()))?;
        let raw_config: SiteConfigRaw = serde_json::from_str(&plaintext)?;
        Ok(raw_config.into())
    }

    /// Encrypt and persist the site configuration.
    pub fn save(&self, config: &SiteConfig) -> Result<()> {
        let raw_config: SiteConfigRaw = config.clone().into();
        let plaintext = serde_json::to_string(&raw_config)?;
        let ciphertext = self
            .vault
            .encrypt(&plaintext)
            .map_err(|e| Error::Vault(e.to_string()))?;
        let file = EncryptedFile {
            version: 1,
            ciphertext,
        };
        let json = serde_json::to_string_pretty(&file)?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| map_io(e, &self.path))?;
        }
        fs::write(&self.path, json).map_err(|e| map_io(e, &self.path))?;
        Ok(())
    }

    /// Remove the config file. The keyring key is kept so that future
    /// config saves are encrypted under the same master key.
    pub fn clear(&self) -> Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(map_io(e, &self.path)),
        }
    }
}

impl From<SiteConfig> for SiteConfigRaw {
    fn from(c: SiteConfig) -> Self {
        Self {
            site_name: c.site_name,
            host: c.host,
            port: c.port,
            database: c.database,
            user: c.user,
            password: c.password.expose_secret().to_string(),
            era: c.era,
            history_days: c.history_days,
            use_medusage_sig: c.use_medusage_sig,
        }
    }
}

impl From<SiteConfigRaw> for SiteConfig {
    fn from(r: SiteConfigRaw) -> Self {
        Self {
            site_name: r.site_name,
            host: r.host,
            port: r.port,
            database: r.database,
            user: r.user,
            password: SecretString::from(r.password),
            era: r.era,
            history_days: r.history_days,
            use_medusage_sig: r.use_medusage_sig,
        }
    }
}

/// Platform config directory for the app (`~/Library/Application Support/...`
/// on macOS, XDG on Linux, AppData on Windows).
fn default_config_path() -> PathBuf {
    let dir = directories::ProjectDirs::from("org", "recon", "Recon")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    dir.join("site-config.json")
}

fn map_io(source: std::io::Error, path: &Path) -> Error {
    Error::Io {
        path: path.to_path_buf(),
        source,
    }
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

    /// Round-trip through a real temporary directory, serialized tests only.
    static DIR_LOCK: Mutex<()> = Mutex::new(());

    fn sample_config() -> SiteConfig {
        SiteConfig {
            site_name: "รพ.ทดสอบ".into(),
            host: "10.0.0.5".into(),
            port: 3306,
            database: "hos".into(),
            user: "recon_ro".into(),
            password: SecretString::from("sup3r-s3cret"),
            era: DateEra::Christian,
            history_days: 730,
            use_medusage_sig: false,
        }
    }

    fn assert_config_eq(a: &SiteConfig, b: &SiteConfig) {
        assert_eq!(a.site_name, b.site_name);
        assert_eq!(a.host, b.host);
        assert_eq!(a.port, b.port);
        assert_eq!(a.database, b.database);
        assert_eq!(a.user, b.user);
        assert_eq!(a.password.expose_secret(), b.password.expose_secret());
        assert_eq!(a.era, b.era);
        assert_eq!(a.history_days, b.history_days);
        assert_eq!(a.use_medusage_sig, b.use_medusage_sig);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::with_vault(
            Box::new(InMemoryVault::new()),
            dir.path().join("site-config.json"),
        );

        assert!(matches!(store.load(), Err(Error::NoConfig)));
        assert!(!store.exists());

        store.save(&sample_config()).unwrap();
        assert!(store.exists());

        let loaded = store.load().unwrap();
        assert_config_eq(&loaded, &sample_config());
    }

    #[test]
    fn disk_contains_no_plaintext_credentials() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("site-config.json");
        let store = ConfigStore::with_vault(Box::new(InMemoryVault::new()), path.clone());
        store.save(&sample_config()).unwrap();

        let on_disk = fs::read_to_string(&path).unwrap();
        assert!(
            !on_disk.contains("sup3r-s3cret"),
            "plaintext password on disk!"
        );
        assert!(!on_disk.contains("10.0.0.5"), "plaintext host on disk!");
        assert!(on_disk.contains("ciphertext"));
    }

    #[test]
    fn wrong_vault_cannot_decrypt() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("site-config.json");
        let store = ConfigStore::with_vault(Box::new(InMemoryVault::new()), path.clone());
        store.save(&sample_config()).unwrap();

        let other = ConfigStore::with_vault(Box::new(InMemoryVault::new()), path.clone());
        assert!(other.load().is_err(), "decrypt with wrong key must fail");
    }

    #[test]
    fn clear_removes_file() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("site-config.json");
        let store = ConfigStore::with_vault(Box::new(InMemoryVault::new()), path.clone());
        store.save(&sample_config()).unwrap();
        store.clear().unwrap();
        assert!(!path.exists());
        assert!(matches!(store.load(), Err(Error::NoConfig)));
    }

    #[test]
    fn unicode_and_secrets_roundtrip() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::with_vault(
            Box::new(InMemoryVault::new()),
            dir.path().join("site-config.json"),
        );
        let mut cfg = sample_config();
        cfg.password = SecretString::from("พาสเวิร์ด! 🔐 x$y%");
        store.save(&cfg).unwrap();
        assert_config_eq(&store.load().unwrap(), &cfg);
    }

    #[test]
    fn unsupported_version_rejected() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("site-config.json");
        fs::write(&path, r#"{"version": 99, "ciphertext": "abc"}"#).unwrap();
        let store = ConfigStore::with_vault(Box::new(InMemoryVault::new()), path);
        assert!(matches!(store.load(), Err(Error::UnsupportedVersion(99))));
    }
}
