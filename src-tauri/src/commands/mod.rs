use crate::{
    auth::accounts::{self, AccountJobs, Accounts, AccountsSummary},
    errors::{AppError, Result},
    minecraft::{
        fabric::{self, FabricLoaderVersion},
        versions::{self, VersionCatalog},
    },
    models::*,
    storage::Store,
};
use std::{
    io::{Read, Seek, SeekFrom},
    sync::{Mutex, MutexGuard},
};
use tauri::State;

pub struct LauncherState {
    pub store: Mutex<Store>,
    pub catalog_lock: tokio::sync::Mutex<()>,
    pub _log_guard: tracing_appender::non_blocking::WorkerGuard,
}

impl LauncherState {
    fn store(&self) -> Result<MutexGuard<'_, Store>> {
        self.store.lock().map_err(|_| {
            AppError::new(
                "STATE",
                "Launcher state is unavailable. Restart the application.",
            )
        })
    }
}

#[tauri::command]
pub fn get_accounts(state: State<'_, LauncherState>) -> Result<AccountsSummary> {
    let data = state.store()?.paths.data.clone();
    Ok(Accounts::read(&data)?.summary())
}

#[tauri::command]
pub async fn create_offline_account(state: State<'_, LauncherState>, jobs: State<'_, AccountJobs>, name: String) -> Result<AccountsSummary> {
    let _guard = jobs.0.try_lock().map_err(|_| accounts::busy())?;
    let data = state.store()?.paths.data.clone();
    Ok(accounts::create_offline_account(&data, name)?.summary())
}

#[tauri::command]
pub fn rename_offline_profile(state: State<'_, LauncherState>, jobs: State<'_, AccountJobs>, id: String, name: String) -> Result<AccountsSummary> {
    let _guard = jobs.0.try_lock().map_err(|_| accounts::busy())?;
    let data = state.store()?.paths.data.clone();
    Ok(accounts::rename_offline(&data, id, name)?.summary())
}

#[tauri::command]
pub async fn microsoft_sign_in(
    state: State<'_, LauncherState>,
    jobs: State<'_, AccountJobs>,
) -> Result<AccountsSummary> {
    let _guard = jobs.0.try_lock().map_err(|_| accounts::busy())?;
    let data = state.store()?.paths.data.clone();
    Ok(accounts::sign_in(&data).await?.summary())
}

#[tauri::command]
pub async fn select_account(
    state: State<'_, LauncherState>,
    jobs: State<'_, AccountJobs>,
    id: String,
) -> Result<AccountsSummary> {
    let _guard = jobs.0.try_lock().map_err(|_| accounts::busy())?;
    let data = state.store()?.paths.data.clone();
    Ok(accounts::select(&data, id)?.summary())
}

#[tauri::command]
pub fn sign_out_account(
    state: State<'_, LauncherState>,
    jobs: State<'_, AccountJobs>,
    id: String,
) -> Result<AccountsSummary> {
    let _guard = jobs.0.try_lock().map_err(|_| accounts::busy())?;
    let data = state.store()?.paths.data.clone();
    Ok(accounts::sign_out(&data, id)?.summary())
}

#[cfg(debug_assertions)]
#[tauri::command]
pub async fn create_local_test_account(
    state: State<'_, LauncherState>,
    jobs: State<'_, AccountJobs>,
) -> Result<AccountsSummary> {
    let _guard = jobs.0.try_lock().map_err(|_| accounts::busy())?;
    let data = state.store()?.paths.data.clone();
    Ok(accounts::create_test_account(&data)?.summary())
}

#[tauri::command]
pub fn set_offline_mode(
    state: State<'_, LauncherState>,
    jobs: State<'_, AccountJobs>,
    offline: bool,
) -> Result<AccountsSummary> {
    let _guard = jobs.0.try_lock().map_err(|_| accounts::busy())?;
    let data = state.store()?.paths.data.clone();
    Ok(accounts::set_offline(&data, offline)?.summary())
}

#[tauri::command]
pub fn get_snapshot(state: State<'_, LauncherState>) -> Result<AppSnapshot> {
    Ok(state.store()?.snapshot())
}

#[tauri::command]
pub fn save_settings(state: State<'_, LauncherState>, settings: Settings) -> Result<AppSnapshot> {
    state.store()?.save_settings(settings)
}

#[tauri::command]
pub fn create_instance(
    state: State<'_, LauncherState>,
    input: InstanceInput,
) -> Result<AppSnapshot> {
    let mut store = state.store()?;
    let kind = versions::version_type(&store.paths.cache.join("versions.json"), &input.version)?;
    store.create_instance(input, &kind)
}

#[tauri::command]
pub fn update_instance(
    state: State<'_, LauncherState>,
    jobs: State<'_, game::GameJobs>,
    id: String,
    input: InstanceInput,
) -> Result<AppSnapshot> {
    let _guard = jobs.lock_instance_edit(&id)?;
    let mut store = state.store()?;
    let original = store.find_instance(&id)?;
    let kind = if original.version == input.version {
        original.version_type.clone()
    } else {
        versions::version_type(&store.paths.cache.join("versions.json"), &input.version)?
    };
    store.update_instance(&id, input, &kind)
}

#[tauri::command]
pub fn select_instance(state: State<'_, LauncherState>, id: String) -> Result<AppSnapshot> {
    state.store()?.select_instance(&id)
}

#[tauri::command]
pub fn set_instance_favorite(state: State<'_, LauncherState>, id: String, favorite: bool) -> Result<AppSnapshot> {
    state.store()?.set_favorite(&id, favorite)
}

#[tauri::command]
pub fn remove_instance(state: State<'_, LauncherState>, jobs: State<'_, game::GameJobs>, id: String) -> Result<AppSnapshot> {
    let _guard = jobs.lock_instance_edit(&id)?;
    state.store()?.remove_instance(&id)
}

#[tauri::command]
pub fn duplicate_instance(state: State<'_, LauncherState>, id: String) -> Result<AppSnapshot> {
    state.store()?.duplicate_instance(&id)
}

#[tauri::command]
pub fn list_installed_mods(
    state: State<'_, LauncherState>,
    id: String,
) -> Result<Vec<InstalledMod>> {
    state.store()?.installed_mods(&id)
}

#[tauri::command]
pub async fn get_versions(
    state: State<'_, LauncherState>,
    refresh: bool,
) -> Result<VersionCatalog> {
    let _fetch_guard = state.catalog_lock.lock().await;
    let (path, timeout) = {
        let store = state.store()?;
        if Accounts::read(&store.paths.data)?.offline {
            return versions::cached_catalog(&store.paths.cache.join("versions.json"));
        }
        (
            store.paths.cache.join("versions.json"),
            store.config.settings.request_timeout_seconds,
        )
    };
    versions::load_catalog(&path, refresh, timeout).await
}

#[tauri::command]
pub async fn get_fabric_loaders(
    state: State<'_, LauncherState>,
    game_version: String,
) -> Result<Vec<FabricLoaderVersion>> {
    let timeout = {
        let store = state.store()?;
        if Accounts::read(&store.paths.data)?.offline {
            return Err(accounts::offline_error());
        }
        store.config.settings.request_timeout_seconds
    };
    fabric::loader_versions(&game_version, timeout).await
}

#[tauri::command]
pub async fn get_forge_loaders(
    state: State<'_, LauncherState>,
    game_version: String,
) -> Result<Vec<FabricLoaderVersion>> {
    let timeout = {
        let store = state.store()?;
        if Accounts::read(&store.paths.data)?.offline {
            return Err(accounts::offline_error());
        }
        store.config.settings.request_timeout_seconds
    };
    crate::minecraft::forge::loader_versions(&game_version, timeout).await
}

#[tauri::command]
pub async fn get_fabric_game_versions(state: State<'_, LauncherState>) -> Result<Vec<String>> {
    let timeout = {
        let store = state.store()?;
        if Accounts::read(&store.paths.data)?.offline {
            return Err(accounts::offline_error());
        }
        store.config.settings.request_timeout_seconds
    };
    fabric::game_versions(timeout).await
}

#[tauri::command]
pub fn read_logs(state: State<'_, LauncherState>) -> Result<String> {
    let directory = state.store()?.paths.logs.clone();
    let mut paths = std::fs::read_dir(directory)
        .map_err(|e| AppError::io("read the log directory", e))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("launcher.log")
        })
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    paths.sort();
    let Some(path) = paths.last() else {
        return Ok("No log entries yet.".into());
    };
    read_log_tail(path)
}

fn read_log_tail(path: &std::path::Path) -> Result<String> {
    let mut file =
        std::fs::File::open(path).map_err(|e| AppError::io("open the current log", e))?;
    let length = file
        .metadata()
        .map_err(|e| AppError::io("inspect the current log", e))?
        .len();
    file.seek(SeekFrom::Start(length.saturating_sub(64 * 1024)))
        .map_err(|e| AppError::io("seek the current log", e))?;
    let mut bytes = Vec::new();
    file.take(64 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| AppError::io("read the current log", e))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
pub mod game;
pub mod updates;
