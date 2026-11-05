mod auth;
mod commands;
mod errors;
mod instances;
mod minecraft;
mod models;
mod storage;

use commands::game::*;
use commands::*;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(GameJobs::default())
        .setup(|app| {
            let store = storage::Store::open(app.path().app_local_data_dir()?)?;
            let appender = tracing_appender::rolling::Builder::new()
                .rotation(tracing_appender::rolling::Rotation::DAILY)
                .filename_prefix("launcher.log").max_log_files(7)
                .build(&store.paths.logs)?;
            let (writer, guard) = tracing_appender::non_blocking(appender);
            let _ = tracing_subscriber::fmt().json().with_ansi(false).with_writer(writer).try_init();
            tracing::info!(event = "launcher_started", version = env!("CARGO_PKG_VERSION"));
            app.manage(LauncherState { store: std::sync::Mutex::new(store), catalog_lock: tokio::sync::Mutex::new(()), _log_guard: guard });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot, save_settings, create_instance, update_instance, select_instance,
            remove_instance, duplicate_instance, list_installed_mods, get_versions, read_logs,
            check_game, install_game, download_status, ignite_game,
        ])
        .run(tauri::generate_context!())
        .expect("Rocket Launcher could not start. Check launcher.json and filesystem permissions in the application data directory.");
}
