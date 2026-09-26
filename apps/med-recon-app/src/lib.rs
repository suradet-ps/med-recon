//! Med Recon desktop shell (Tauri 2 backend).

pub mod commands;
pub mod pdf;
pub mod report;
pub mod state;

use state::AppState;
use std::time::Duration;
use tauri::{Manager, WebviewWindow};

/// Safety net: how long to wait for the frontend's `show_main_window` call
/// before revealing the window anyway, so a broken UI can never leave an
/// invisible app running.
const WINDOW_SHOW_FALLBACK: Duration = Duration::from_secs(5);

/// Reveal the main window. The frontend calls this once it has mounted, so
/// the user never sees a white WebView flash - the window appears with the
/// UI already rendered.
#[tauri::command]
fn show_main_window(window: WebviewWindow) {
    let _ = window.show();
    let _ = window.set_focus();
}

/// Start the Tauri application.
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "med_recon_app=info,med_recon_hosxp=info,med_recon_config=info".into()
            }),
        )
        .init();

    tauri::Builder::default()
        .manage(AppState::new())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(WINDOW_SHOW_FALLBACK).await;
                    let _ = window.show();
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            show_main_window,
            commands::get_app_status,
            commands::is_configured,
            commands::connection_health,
            commands::save_connection,
            commands::get_connection,
            commands::get_site_settings,
            commands::save_site_settings,
            commands::test_connection,
            commands::clear_site_config,
            commands::search_patients,
            commands::search_drugs,
            commands::get_current_meds,
            commands::load_history,
            commands::load_patient_image,
            commands::export_report,
            commands::capture_screenshot,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Med Recon");
}
