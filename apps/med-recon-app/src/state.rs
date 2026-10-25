//! Shared application state for the Tauri backend.

use med_recon_config::ConfigStore;
use med_recon_core::SessionUser;
use med_recon_hosxp::HosxpClient;
use std::sync::atomic::AtomicU64;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock};

/// Consecutive failed login attempts and the current lockout deadline.
#[derive(Debug, Default)]
pub struct LoginThrottle {
    /// Number of consecutive failed authentication attempts.
    pub failed_attempts: u32,
    /// When the current lockout ends, if the threshold was reached.
    pub locked_until: Option<Instant>,
}

/// Backend state managed by Tauri.
pub struct AppState {
    /// Encrypted config store (keyring-backed).
    pub store: ConfigStore,
    /// Lazily-created HOSxP client for the saved site config.
    pub client: RwLock<Option<HosxpClient>>,
    /// Cached connection health, refreshed by `connection_health`.
    pub health: RwLock<crate::commands::ConnectionHealth>,
    /// Signed-in HOSxP officer for this app process. Memory only - a restart
    /// requires signing in again.
    pub session: RwLock<Option<SessionUser>>,
    /// Bumped whenever the session is cleared or the connection changes, so
    /// an in-flight login cannot restore a stale session.
    pub session_epoch: AtomicU64,
    /// Consecutive failed login attempts, used to throttle password guessing.
    pub login_throttle: Mutex<LoginThrottle>,
}

impl AppState {
    /// Open the default config store.
    ///
    /// Falls back to an in-memory store if the OS keychain is unavailable
    /// (headless environments) so the app still boots.
    pub fn new() -> Self {
        let store = match ConfigStore::open() {
            Ok(store) => store,
            Err(e) => {
                tracing::warn!("keychain unavailable, falling back to in-memory store: {e}");
                ConfigStore::with_vault(
                    Box::new(crate::commands::EphemeralVault::default()),
                    std::path::PathBuf::new(),
                )
            }
        };
        Self {
            store,
            client: RwLock::new(None),
            health: RwLock::new(crate::commands::ConnectionHealth::Unconfigured),
            session: RwLock::new(None),
            session_epoch: AtomicU64::new(0),
            login_throttle: Mutex::new(LoginThrottle::default()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
