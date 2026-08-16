//! Client-side application state (signals shared between components).

use leptos::prelude::*;
use recon_core::{PatientHistory, PatientSummary};

use crate::i18n::Lang;

/// Live HOSxP reachability, mirrored from the backend's
/// `connection_health` command — drives the top-bar status dot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionHealth {
    /// No stored settings — the settings dialog is the flow.
    Unconfigured,
    /// A ping succeeded recently.
    Connected,
    /// HOSxP could not be reached.
    Disconnected,
}

/// Shared state for the single-page flow. `Copy` because all fields
/// are copyable signals.
#[derive(Debug, Clone, Copy)]
pub struct AppState {
    /// Whether encrypted HOSxP connection settings exist on this machine.
    pub configured: RwSignal<bool>,
    /// Whether the connection settings dialog is open.
    pub settings_open: RwSignal<bool>,
    /// Polled live reachability — top-bar dot source.
    pub health: RwSignal<ConnectionHealth>,
    /// Selected patient; `None` until one is picked from search results.
    pub patient: RwSignal<Option<PatientSummary>>,
    /// Loaded cross-visit history for the selected patient.
    pub history: RwSignal<Option<PatientHistory>>,
    /// Whether a history load is in flight.
    pub history_loading: RwSignal<bool>,
    /// Last history-load error message, if any.
    pub history_error: RwSignal<Option<String>>,
    /// Current UI language.
    pub lang: RwSignal<Lang>,
}

impl AppState {
    /// Fresh state for a new app session.
    pub fn new() -> Self {
        Self {
            configured: RwSignal::new(false),
            settings_open: RwSignal::new(false),
            health: RwSignal::new(ConnectionHealth::Unconfigured),
            patient: RwSignal::new(None),
            history: RwSignal::new(None),
            history_loading: RwSignal::new(false),
            history_error: RwSignal::new(None),
            lang: RwSignal::new(Lang::from_storage()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
