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

use crate::components::help_modal::HelpModal;
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

    // The window starts hidden; reveal it once the shell has mounted, so the
    // user never sees a white WebView flash.
    let root_ref = NodeRef::<leptos::html::Div>::new();
    root_ref.on_load(move |_| {
        spawn_local(async move {
            let _ = crate::api::show_main_window().await;
        });
    });

    // Loading indicators appear only after a delay: a load that finishes
    // sooner shows nothing at all, because a flashing dim/spinner reads as
    // jank. The threshold sits above the history fetch's own 300 ms
    // debounce, so a quick window change never flashes anything either.
    // The generation guard cancels a pending timer when the load ends or a
    // new one starts.
    let load_generation = RwSignal::new(0_u64);
    let delay_state = state;
    Effect::new(move |_| {
        if delay_state.history_loading.get() {
            let generation = load_generation.get_untracked().wrapping_add(1);
            load_generation.set(generation);
            set_timeout(
                move || {
                    if load_generation.get_untracked() == generation
                        && delay_state.history_loading.get_untracked()
                    {
                        delay_state.history_loading_visible.set(true);
                    }
                },
                Duration::from_millis(700),
            );
        } else {
            load_generation.update(|generation| *generation = generation.wrapping_add(1));
            delay_state.history_loading_visible.set(false);
        }
    });

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
        <div class="app" node_ref=root_ref>
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
            <HelpModal state=state />

            // Screen-reader narration for the async states (WCAG 4.1.3);
            // errors are announced by the canvas' role="alert" panel.
            <div class="sr-only" role="status" aria-live="polite">
                {move || {
                    if state.history_loading_visible.get() {
                        "กำลังโหลดประวัติยา".to_string()
                    } else if state.history.with(|history| history.is_some()) {
                        "โหลดประวัติยาแล้ว".to_string()
                    } else {
                        String::new()
                    }
                }}
            </div>
        </div>
    }
}
