mod auth;
mod commands;
mod errors;
mod instances;
mod minecraft;
mod models;
mod storage;

use commands::game::*;
use commands::updates::*;
use commands::*;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(GameJobs::default())
        .manage(auth::accounts::AccountJobs::default())
        .setup(|app| {
            let mut store = storage::Store::open(app.path().app_local_data_dir()?)?;
            clean_staged_installers(&store.paths.cache);
            let appender = tracing_appender::rolling::Builder::new()
                .rotation(tracing_appender::rolling::Rotation::DAILY)
                .filename_prefix("launcher.log")
                .max_log_files(7)
                .build(&store.paths.logs)?;
            let (writer, guard) = tracing_appender::non_blocking(appender);
            let _ = tracing_subscriber::fmt()
                .json()
                .with_ansi(false)
                .with_writer(writer)
                .try_init();
            if let Err(error) = store.relocate_rocket_instances() {
                tracing::warn!(%error, "Could not finish organizing Rocket Pack instances");
            }
            tracing::info!(
                event = "launcher_started",
                version = env!("CARGO_PKG_VERSION")
            );
            app.manage(LauncherState {
                store: std::sync::Mutex::new(store),
                catalog_lock: tokio::sync::Mutex::new(()),
                _log_guard: guard,
            });
            Ok(())
        });

    #[cfg(debug_assertions)]
    let app = app.invoke_handler(tauri::generate_handler![
        get_snapshot,
        save_settings,
        create_instance,
        update_instance,
        select_instance,
        set_instance_favorite,
        read_instance_log,
        diagnose_instance,
        get_accounts,
        create_offline_account,
        rename_offline_profile,
        microsoft_sign_in,
        set_offline_mode,
        select_account,
        sign_out_account,
        create_local_test_account,
        remove_instance,
        duplicate_instance,
        list_installed_mods,
        set_instance_mod_enabled,
        open_instance_mods_folder,
        open_instance_folder,
        rocket_client_status,
        install_rocket_client,
        get_versions,
        get_fabric_loaders,
        get_forge_loaders,
        get_fabric_game_versions,
        read_logs,
        check_game,
        install_game,
        download_status,
        ignite_game,
        abort_game,
        search_modrinth,
        modrinth_gallery,
        search_curseforge,
        curseforge_key_saved,
        set_curseforge_key,
        curseforge_versions,
        install_curseforge,
        search_modpacks,
        modpack_details,
        install_modpack,
        modrinth_versions,
        install_modrinth,
        saved_servers,
        save_server,
        remove_saved_server,
        ping_server,
        multiplayer_addons,
        install_multiplayer_addon,
        install_rocket_pack,
        check_launcher_update,
        install_launcher_update,
    ]);

    #[cfg(not(debug_assertions))]
    let app = app.invoke_handler(tauri::generate_handler![
        get_snapshot,
        save_settings,
        create_instance,
        update_instance,
        select_instance,
        set_instance_favorite,
        read_instance_log,
        diagnose_instance,
        get_accounts,
        create_offline_account,
        rename_offline_profile,
        microsoft_sign_in,
        set_offline_mode,
        select_account,
        sign_out_account,
        remove_instance,
        duplicate_instance,
        list_installed_mods,
        set_instance_mod_enabled,
        open_instance_mods_folder,
        open_instance_folder,
        rocket_client_status,
        install_rocket_client,
        get_versions,
        get_fabric_loaders,
        get_forge_loaders,
        get_fabric_game_versions,
        read_logs,
        check_game,
        install_game,
        download_status,
        ignite_game,
        abort_game,
        search_modrinth,
        modrinth_gallery,
        search_curseforge,
        curseforge_key_saved,
        set_curseforge_key,
        curseforge_versions,
        install_curseforge,
        search_modpacks,
        modpack_details,
        install_modpack,
        modrinth_versions,
        install_modrinth,
        saved_servers,
        save_server,
        remove_saved_server,
        ping_server,
        multiplayer_addons,
        install_multiplayer_addon,
        install_rocket_pack,
        check_launcher_update,
        install_launcher_update,
    ]);

    app.run(tauri::generate_context!())
        .expect("Rocket Launcher could not start. Check launcher.json and filesystem permissions in the application data directory.");
}
