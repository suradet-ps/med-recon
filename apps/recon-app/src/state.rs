//! Shared application state for the Tauri backend.

use recon_config::ConfigStore;
use recon_hosxp::HosxpClient;
use tokio::sync::RwLock;

/// Backend state managed by Tauri.
pub struct AppState {
    /// Encrypted config store (keyring-backed).
    pub store: ConfigStore,
    /// Lazily-created HOSxP client for the saved site config.
    pub client: RwLock<Option<HosxpClient>>,
    /// Cached connection health, refreshed by `connection_health`.
    pub health: RwLock<crate::commands::ConnectionHealth>,
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
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
