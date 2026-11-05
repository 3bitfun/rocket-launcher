use crate::{
    errors::{AppError, Result},
    minecraft::versions::{self, VersionCatalog},
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
    id: String,
    input: InstanceInput,
) -> Result<AppSnapshot> {
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
pub fn remove_instance(state: State<'_, LauncherState>, id: String) -> Result<AppSnapshot> {
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
        (
            store.paths.cache.join("versions.json"),
            store.config.settings.request_timeout_seconds,
        )
    };
    versions::load_catalog(&path, refresh, timeout).await
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
