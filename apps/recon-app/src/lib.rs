//! Recon desktop shell (Tauri 2 backend).

pub mod commands;
pub mod report;
pub mod state;

use state::AppState;

/// Start the Tauri application.
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "recon_app=info,recon_hosxp=info,recon_config=info".into()),
        )
        .init();

    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::get_app_status,
            commands::is_configured,
            commands::connection_health,
            commands::save_site_config,
            commands::test_connection,
            commands::clear_site_config,
            commands::search_patients,
            commands::load_history,
            commands::export_report,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Recon");
}
