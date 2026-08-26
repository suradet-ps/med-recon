//! Med Recon frontend library (Leptos 0.8, CSR).
//!
//! Two-panel desktop layout (AllerX-style): sidebar (search) + main canvas
//! (complete medication history). On launch the app checks for stored
//! connection settings; if absent, the settings dialog opens automatically.
//! The top-bar status dot is driven by polling the backend's live
//! `connection_health`.

use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;

use crate::components::history_canvas::HistoryCanvas;
use crate::components::patient_card::PatientCard;
use crate::components::patient_search::PatientSearch;
use crate::components::pmh_card::PmhCard;
use crate::components::settings_modal::SettingsModal;
use crate::components::top_bar::TopBar;
use crate::state::{AppState, ConnectionHealth};

/// How often the frontend polls the backend's live health state.
const HEALTH_POLL_INTERVAL: Duration = Duration::from_secs(30);

/// Mounts the app into the document body.
pub fn run() {
    leptos::mount::mount_to_body(|| view! { <App /> });
}

/// Two-panel desktop layout.
#[component]
fn App() -> impl IntoView {
    let state = AppState::new();

    // First-run check: no stored settings → open the settings dialog.
    spawn_local(async move {
        match crate::api::is_configured().await {
            Ok(configured) => {
                state.configured.set(configured);
                if !configured {
                    state.settings_open.set(true);
                }
            }
            Err(_) => {
                state.settings_open.set(true);
            }
        }
    });

    // Mirror the configured history window so the segmented control can
    // label the "ค่าเริ่มต้น" segment with the real default, and the site
    // name for the top-bar brand line.
    let default_state = state;
    spawn_local(async move {
        if let Ok(settings) = crate::api::get_site_settings().await {
            default_state
                .default_history_days
                .set(settings.history_days);
            default_state.site_name.set(settings.site_name);
        }
    });

    // Poll the backend's live reachability - the status dot must reflect a
    // dead database within seconds, not "config exists".
    let poll_state = state;
    spawn_local(async move {
        loop {
            match crate::api::connection_health().await {
                Ok(health) => poll_state.health.set(health),
                Err(_) => poll_state.health.set(ConnectionHealth::Disconnected),
            }
            // Sleep via a JS setTimeout promise - plain `set_timeout` is
            // fire-and-forget and cannot be awaited.
            let delay = js_sys::Promise::new(&mut |resolve, _reject| {
                let f: &js_sys::Function = resolve.unchecked_ref();
                let _ = web_sys::window()
                    .expect("invariant: Tauri webview window")
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        f,
                        HEALTH_POLL_INTERVAL.as_millis() as i32,
                    );
            });
            wasm_bindgen_futures::JsFuture::from(delay)
                .await
                .expect("invariant: setTimeout promise resolves");
        }
    });

    view! {
        <div class="app">
            <TopBar state=state />
            <div class="app__body">
                <aside class="sidebar">
                    <PatientSearch state=state />
                    <PatientCard state=state />
                    <PmhCard state=state />
                </aside>
                <HistoryCanvas state=state />
            </div>
            <SettingsModal state=state />
        </div>
    }
}
