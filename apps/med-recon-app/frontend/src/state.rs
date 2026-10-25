//! Client-side application state (signals shared between components).

use std::collections::HashSet;

use leptos::prelude::*;
use med_recon_core::{PatientHistory, PatientSummary, SessionUser};

/// Shown when a history response does not belong to the selected patient.
/// This is a safety net; the backend should always echo the requested HN.
pub const HISTORY_PATIENT_MISMATCH: &str = "ข้อมูลประวัติที่ได้รับไม่ตรงกับผู้ป่วยที่เลือก กรุณาลองใหม่";

/// Live HOSxP reachability, mirrored from the backend's
/// `connection_health` command - drives the top-bar status dot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionHealth {
    /// No stored settings - the settings dialog is the flow.
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
    /// Signed-in HOSxP officer, mirrored from the backend session.
    pub session: RwSignal<Option<SessionUser>>,
    /// True once the startup probe (configuration + session) finished.
    pub auth_checked: RwSignal<bool>,
    /// Whether the connection settings dialog is open.
    pub settings_open: RwSignal<bool>,
    /// Whether the user manual (คู่มือการใช้งาน) dialog is open.
    pub help_open: RwSignal<bool>,
    /// Polled live reachability - top-bar dot source.
    pub health: RwSignal<ConnectionHealth>,
    /// Search input text - shared so the patient card can reset it.
    pub search_query: RwSignal<String>,
    /// Selected patient; `None` until one is picked from search results.
    pub patient: RwSignal<Option<PatientSummary>>,
    /// Selected patient's photo as a `data:` URL; `None` until loaded or
    /// when the site has no photo on file.
    pub patient_photo: RwSignal<Option<String>>,
    /// Loaded cross-visit history for the selected patient.
    pub history: RwSignal<Option<PatientHistory>>,
    /// Whether a history load is in flight.
    pub history_loading: RwSignal<bool>,
    /// `history_loading`, delayed by a short threshold - what the loading
    /// indicators bind to, so a fast load never flashes them.
    pub history_loading_visible: RwSignal<bool>,
    /// Last history-load error message, if any.
    pub history_error: RwSignal<Option<String>>,
    /// Per-query history window override (days). `None` = use the
    /// configured default from settings; `Some(n)` overrides with `n` days.
    pub history_days_override: RwSignal<Option<u32>>,
    /// Configured default history window (days), mirrored from site settings.
    /// Drives the "ค่าเริ่มต้น" segment label so the operator always sees
    /// what the default window resolves to.
    pub default_history_days: RwSignal<u32>,
    /// Site name (สถานบริการ), mirrored from site settings and shown in the
    /// top-bar brand line in place of a hardcoded label.
    pub site_name: RwSignal<String>,
    /// Bumped only when the operator changes the window via the segmented
    /// control - not when a new patient resets the override. The history
    /// re-fetch effect keys off this so programmatic resets don't trigger a
    /// second fetch alongside the patient-search fetch.
    pub window_epoch: RwSignal<u32>,
    /// Monotonic token for the latest history load. Bumped when a patient
    /// is picked or cleared and when the window changes, so a slower
    /// response from an older request can never overwrite the current one.
    pub history_request_epoch: RwSignal<u64>,
    /// Pending debounced window-change fetch. Kept here so picking or
    /// clearing a patient can cancel it before it starts for the previous
    /// selection.
    pub history_debounce: RwSignal<Option<leptos::prelude::TimeoutHandle>>,
    /// Row keys the operator has dismissed in the current session - a
    /// "หยุดใช้แล้ว" review aid only: dismissed rows disappear from the
    /// table. Cleared on every fresh history load (new patient, window
    /// change), which also restores the hidden rows.
    pub dismissed_meds: RwSignal<HashSet<String>>,
}

impl AppState {
    /// Fresh state for a new app session.
    pub fn new() -> Self {
        Self {
            configured: RwSignal::new(false),
            session: RwSignal::new(None),
            auth_checked: RwSignal::new(false),
            settings_open: RwSignal::new(false),
            help_open: RwSignal::new(false),
            health: RwSignal::new(ConnectionHealth::Unconfigured),
            search_query: RwSignal::new(String::new()),
            patient: RwSignal::new(None),
            patient_photo: RwSignal::new(None),
            history: RwSignal::new(None),
            history_loading: RwSignal::new(false),
            history_loading_visible: RwSignal::new(false),
            history_error: RwSignal::new(None),
            history_days_override: RwSignal::new(None),
            default_history_days: RwSignal::new(730),
            site_name: RwSignal::new(String::new()),
            window_epoch: RwSignal::new(0),
            history_request_epoch: RwSignal::new(0),
            history_debounce: RwSignal::new(None),
            dismissed_meds: RwSignal::new(HashSet::new()),
        }
    }

    /// Start a new patient-scoped history load and return its token.
    ///
    /// Any response carrying an older token must be discarded.
    pub fn begin_history_request(&self) -> u64 {
        let token = self.history_request_epoch.get_untracked().wrapping_add(1);
        self.history_request_epoch.set(token);
        token
    }

    /// Whether `token` still identifies the latest history load.
    pub fn is_current_history_request(&self, token: u64) -> bool {
        self.history_request_epoch.get_untracked() == token
    }

    /// Cancel the pending debounced window-change fetch, if any.
    pub fn cancel_pending_history_fetch(&self) {
        if let Some(handle) = self.history_debounce.get_untracked() {
            handle.clear();
        }
        self.history_debounce.set(None);
    }

    /// Completes a successful login and opens the search screen.
    pub fn finish_login(&self, user: SessionUser) {
        self.session.set(Some(user));
        self.settings_open.set(false);
    }

    /// Clears the session and every patient-scoped signal (backend logout is
    /// performed by the caller). In-flight history responses are invalidated
    /// so PHI from the previous session cannot reappear after sign-out.
    pub fn clear_session(&self) {
        self.session.set(None);
        self.search_query.set(String::new());
        self.patient.set(None);
        self.patient_photo.set(None);
        self.history.set(None);
        self.history_loading.set(false);
        self.history_loading_visible.set(false);
        self.history_error.set(None);
        self.history_days_override.set(None);
        self.cancel_pending_history_fetch();
        self.history_request_epoch.update(|epoch| {
            *epoch = epoch.wrapping_add(1);
        });
        self.dismissed_meds.set(HashSet::new());
        self.settings_open.set(false);
        self.help_open.set(false);
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
