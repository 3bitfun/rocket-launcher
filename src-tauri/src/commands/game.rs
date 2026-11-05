use super::LauncherState;
use crate::{
    errors::{AppError, Result},
    minecraft::{
        install::{self, Installed, Progress, Stage},
        java,
        launch::{self, GameStatus, ProcessManager, Readiness},
        metadata::Platform,
        versions,
    },
    models::Instance,
};
use std::{collections::HashMap, path::PathBuf, sync::Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Default)]
pub struct GameJobs {
    operation: tokio::sync::Mutex<()>,
    progress: Mutex<Option<Progress>>,
    statuses: Mutex<HashMap<String, GameStatus>>,
    processes: ProcessManager,
}
struct Input {
    instance: Instance,
    root: PathBuf,
    data: PathBuf,
    logs: PathBuf,
    catalogue: PathBuf,
    java: Option<PathBuf>,
    timeout: u64,
}

fn input(state: &LauncherState, id: &str) -> Result<Input> {
    let store = state.store()?;
    let instance = store.find_instance(id)?.clone();
    Ok(Input {
        java: instance
            .java_path
            .clone()
            .or_else(|| store.config.settings.java_path.clone()),
        instance,
        root: store.paths.cache.join("minecraft"),
        data: store.paths.data.clone(),
        logs: store.paths.logs.clone(),
        catalogue: store.paths.cache.join("versions.json"),
        timeout: store.config.settings.request_timeout_seconds,
    })
}
async fn verify(input: &Input) -> Result<Option<Installed>> {
    let root = input.root.clone();
    let version = input.instance.version.clone();
    tokio::task::spawn_blocking(move || {
        install::verify_installed(&root, &version, &Platform::windows()?)
    })
    .await
    .map_err(|e| AppError::new("INSTALL", format!("Could not verify the installation: {e}")))?
}
fn progress_status(progress: &Progress) -> Option<GameStatus> {
    let readiness = match progress.stage {
        Stage::Downloading => Readiness::Downloading,
        Stage::Installing => Readiness::Installing,
        _ => return None,
    };
    Some(GameStatus::new(
        &progress.instance_id,
        readiness,
        &progress.current,
    ))
}
fn missing_status(
    id: &str,
    checked: Result<Option<Installed>>,
) -> std::result::Result<Installed, GameStatus> {
    match checked {
        Ok(Some(installed)) => Ok(installed),
        Ok(None) => Err(GameStatus::new(
            id,
            Readiness::RequiresDownload,
            "Install the Minecraft files to continue.",
        )),
        Err(error) => Err(GameStatus::new(id, Readiness::MissingFiles, error.message)),
    }
}

#[tauri::command]
pub async fn check_game(
    state: State<'_, LauncherState>,
    jobs: State<'_, GameJobs>,
    id: String,
) -> Result<GameStatus> {
    let input = input(&state, &id)?;
    if jobs.processes.contains(&id) {
        return Ok(jobs
            .statuses
            .lock()
            .ok()
            .and_then(|statuses| statuses.get(&id).cloned())
            .unwrap_or_else(|| {
                GameStatus::new(&id, Readiness::Launching, "Preparing Minecraft…")
            }));
    }
    if let Some(status) = jobs.progress.lock().ok().and_then(|p| {
        p.as_ref()
            .filter(|p| p.instance_id == id)
            .and_then(progress_status)
    }) {
        return Ok(status);
    }
    if !input.instance.game_directory.is_dir() {
        return Ok(GameStatus::new(
            &id,
            Readiness::MissingFiles,
            "The instance game directory is missing. Install to recreate it.",
        ));
    }
    let installed = match missing_status(&id, verify(&input).await) {
        Ok(installed) => installed,
        Err(status) => return Ok(status),
    };
    let arch = Platform::windows()?.arch;
    match java::resolve(
        input.java,
        &input.data,
        installed.version.java_major(),
        arch,
    )
    .await
    {
        Err(error) => Ok(GameStatus::new(&id, Readiness::JavaRequired, error.message)),
        Ok(java) if launch::usable_session().is_none() => Ok(GameStatus::new(
            &id,
            Readiness::AuthenticationRequired,
            format!(
                "Files verified · Java {} ({}). {}",
                java.major,
                java.arch.name(),
                launch::authentication_required().message
            ),
        )),
        Ok(_) => Ok(GameStatus::new(
            &id,
            Readiness::Ready,
            "Minecraft is ready to launch.",
        )),
    }
}

fn publish_progress(app: &AppHandle, progress: Progress) {
    let jobs = app.state::<GameJobs>();
    if let Ok(mut latest) = jobs.progress.lock() {
        *latest = Some(progress.clone());
    }
    let _ = app.emit("game-download", progress);
}

#[tauri::command]
pub fn download_status(jobs: State<'_, GameJobs>) -> Option<Progress> {
    jobs.progress
        .lock()
        .ok()
        .and_then(|progress| progress.clone())
}

#[tauri::command]
pub async fn install_game(
    app: AppHandle,
    state: State<'_, LauncherState>,
    jobs: State<'_, GameJobs>,
    id: String,
) -> Result<()> {
    let _guard = jobs
        .operation
        .try_lock()
        .map_err(|_| AppError::new("BUSY", "Another Minecraft operation is in progress."))?;
    if jobs.processes.busy() {
        return Err(AppError::new(
            "BUSY",
            "Close Minecraft before installing or repairing game files.",
        ));
    }
    let input = input(&state, &id)?;
    let initial = Progress {
        instance_id: id.clone(),
        version: input.instance.version.clone(),
        stage: Stage::Downloading,
        current: "Preparing Minecraft installation".into(),
        transferred_bytes: 0,
        verified_bytes: 0,
        total_bytes: None,
        bytes_per_second: 0,
        error: None,
    };
    publish_progress(&app, initial.clone());
    let result = async {
        let _catalog = state.catalog_lock.lock().await;
        let mut manifest = versions::read_cache(&input.catalogue)
            .ok()
            .map(|cache| cache.manifest);
        if !manifest
            .as_ref()
            .is_some_and(|m| m.versions.iter().any(|v| v.id == input.instance.version))
        {
            versions::load_catalog(&input.catalogue, true, input.timeout).await?;
            manifest = Some(versions::read_cache(&input.catalogue)?.manifest);
        }
        drop(_catalog);
        crate::storage::validate_path(&input.instance.game_directory)?;
        tokio::fs::create_dir_all(&input.instance.game_directory)
            .await
            .map_err(|e| AppError::io("create game directory", e))?;
        install::install(
            &input.root,
            &manifest.expect("catalogue loaded"),
            &input.instance.version,
            &id,
            input.timeout,
            |progress| publish_progress(&app, progress),
        )
        .await
    }
    .await;
    if let Err(error) = &result {
        let mut progress = jobs
            .progress
            .lock()
            .ok()
            .and_then(|latest| latest.clone())
            .unwrap_or(initial);
        progress.stage = Stage::Error;
        progress.current = "Installation failed".into();
        progress.error = Some(error.message.clone());
        progress.bytes_per_second = 0;
        publish_progress(&app, progress);
    }
    result
}

fn publish_game(app: &AppHandle, status: GameStatus) {
    let jobs = app.state::<GameJobs>();
    if let Ok(mut statuses) = jobs.statuses.lock() {
        statuses.insert(status.instance_id.clone(), status.clone());
    }
    let _ = app.emit("game-status", status);
}

#[tauri::command]
pub async fn ignite_game(
    app: AppHandle,
    state: State<'_, LauncherState>,
    jobs: State<'_, GameJobs>,
    id: String,
) -> Result<()> {
    let input = input(&state, &id)?;
    let session = launch::usable_session().ok_or_else(launch::authentication_required)?;
    let _guard = jobs
        .operation
        .try_lock()
        .map_err(|_| AppError::new("BUSY", "Another Minecraft operation is in progress."))?;
    let reservation = jobs.processes.reserve(&id)?;
    publish_game(
        &app,
        GameStatus::new(
            &id,
            Readiness::Launching,
            "Verifying game files before launch…",
        ),
    );
    let result = async {
        let installed = verify(&input)
            .await?
            .ok_or_else(|| AppError::new("MISSING_FILES", "Install Minecraft before launching."))?;
        let platform = Platform::windows()?;
        let java = java::resolve(
            input.java,
            &input.data,
            installed.version.java_major(),
            platform.arch,
        )
        .await?;
        let event_app = app.clone();
        launch::start(
            reservation,
            launch::LaunchRequest {
                installed,
                instance: input.instance,
                java,
                platform,
                session,
                log_root: input.logs,
            },
            move |status| publish_game(&event_app, status),
        )
        .await
    }
    .await;
    if let Err(error) = &result {
        publish_game(&app, GameStatus::new(&id, Readiness::Error, &error.message));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readiness_distinguishes_missing_receipt_and_corruption() {
        assert!(
            matches!(missing_status("a", Ok(None)), Err(status) if status.readiness == Readiness::RequiresDownload)
        );
        assert!(
            matches!(missing_status("a", Err(AppError::new("MISSING_FILES", "Bad hash"))),
            Err(status) if status.readiness == Readiness::MissingFiles && status.message == "Bad hash")
        );
    }
}
