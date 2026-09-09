//! Med Recon desktop shell (Tauri 2 backend).

pub mod commands;
pub mod pdf;
pub mod report;
pub mod state;

use state::AppState;
use tauri::Manager;

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
                if let Err(e) = window.center() {
                    tracing::error!("failed to center main window: {e}");
                }
                if let Err(e) = window.show() {
                    tracing::error!("failed to show main window: {e}");
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
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
