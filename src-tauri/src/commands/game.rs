use super::LauncherState;
use crate::{
    errors::{AppError, Result},
    minecraft::{
        fabric,
        install::{self, Installed, Progress, Stage},
        java,
        launch::{self, GameStatus, ProcessManager, Readiness},
        metadata::Platform,
        modrinth, versions,
    },
    models::{Instance, Loader},
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

fn require_online(input: &Input) -> Result<()> {
    if crate::auth::accounts::Accounts::read(&input.data)?.offline {
        return Err(crate::auth::accounts::offline_error());
    }
    Ok(())
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
async fn verify(input: &Input, full: bool) -> Result<Option<Installed>> {
    let platform = Platform::windows()?;
    let root = input.root.clone();
    let version = input.instance.version.clone();
    // The vanilla game version is always verified first; Fabric layers on top of it.
    let vanilla = tokio::task::spawn_blocking(move || {
        if full {
            install::verify_installed(&root, &version, &platform)
        } else {
            install::inspect_installed(&root, &version, &platform, false)
        }
    })
    .await
    .map_err(|e| AppError::new("INSTALL", format!("Could not verify the installation: {e}")))??;
    let Some(vanilla) = vanilla else {
        return Ok(None);
    };
    match input.instance.loader {
        Loader::Vanilla => Ok(Some(vanilla)),
        Loader::Fabric => {
            let loader_version = input.instance.loader_version.clone().ok_or_else(|| {
                AppError::new(
                    "VALIDATION",
                    "This Fabric instance has no loader version. Edit the instance and choose one.",
                )
            })?;
            let root = input.root.clone();
            let game_version = input.instance.version.clone();
            let platform = Platform::windows()?;
            let fabric = tokio::task::spawn_blocking(move || {
                fabric::verify(
                    &root,
                    &game_version,
                    &loader_version,
                    &platform,
                    &vanilla,
                    full,
                )
            })
            .await
            .map_err(|e| {
                AppError::new("INSTALL", format!("Could not verify the installation: {e}"))
            })??;
            Ok(fabric)
        }
    }
}

#[tauri::command]
pub async fn search_modrinth(
    state: State<'_, LauncherState>,
    instance_id: String,
    query: String,
    offset: u32,
) -> Result<modrinth::SearchPage> {
    let input = input(&state, &instance_id)?;
    require_online(&input)?;
    modrinth::search(&input.instance, &query, offset, input.timeout).await
}

#[tauri::command]
pub async fn modrinth_versions(
    state: State<'_, LauncherState>,
    instance_id: String,
    project_id: String,
) -> Result<Vec<modrinth::ModVersion>> {
    let input = input(&state, &instance_id)?;
    require_online(&input)?;
    modrinth::versions(&input.instance, &project_id, input.timeout).await
}

#[tauri::command]
pub async fn install_modrinth(
    state: State<'_, LauncherState>,
    jobs: State<'_, GameJobs>,
    instance_id: String,
    project_id: String,
    version_id: String,
) -> Result<String> {
    let _guard = jobs
        .operation
        .try_lock()
        .map_err(|_| AppError::new("BUSY", "Another Minecraft operation is in progress."))?;
    if jobs.processes.contains(&instance_id) {
        return Err(AppError::new(
            "BUSY",
            "Close this instance before installing mods.",
        ));
    }
    let original = input(&state, &instance_id)?;
    require_online(&original)?;
    let staged = modrinth::stage(
        &original.instance,
        &project_id,
        &version_id,
        original.timeout,
    )
    .await?;
    let store = state.store()?;
    let current = store.find_instance(&instance_id)?;
    if current.version != original.instance.version
        || current.loader != original.instance.loader
        || current.game_directory != original.instance.game_directory
    {
        return Err(AppError::new(
            "MODRINTH",
            "The instance changed during the download. Search again before installing.",
        ));
    }
    staged.publish(&current.game_directory)
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
    let installed = match missing_status(&id, verify(&input, false).await) {
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
        Ok(java) => match crate::auth::accounts::readiness(&input.data) {
            Ok(crate::auth::accounts::AccountMode::Microsoft { offline }) => Ok(GameStatus::new(
                &id,
                Readiness::Ready,
                if offline {
                    "Minecraft is ready to launch offline."
                } else {
                    "Minecraft is ready to launch."
                },
            )),
            Ok(crate::auth::accounts::AccountMode::LocalTest) => Ok(GameStatus::new(
                &id,
                Readiness::Ready,
                "Local Offline Test — Unauthenticated. Ready to start Minecraft locally.",
            )),
            Err(error) => Ok(GameStatus::new(
                &id,
                Readiness::AuthenticationRequired,
                format!(
                    "Files verified · Java {} ({}). {}",
                    java.major,
                    java.arch.name(),
                    error.message
                ),
            )),
        },
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
    let fabric = input.instance.loader == Loader::Fabric;
    require_online(&input)?;
    let initial = Progress {
        instance_id: id.clone(),
        version: input.instance.version.clone(),
        stage: Stage::Downloading,
        current: if fabric {
            "Preparing Fabric installation".into()
        } else {
            "Preparing Minecraft installation".into()
        },
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
            |progress| {
                if !fabric || !matches!(progress.stage, Stage::Complete) {
                    publish_progress(&app, progress);
                }
            },
        )
        .await?;
        // Fabric rides on top of the installed vanilla version: profile, libraries, receipt.
        let loader_version = match input.instance.loader {
            Loader::Vanilla => return Ok(()),
            Loader::Fabric => input
                .instance
                .loader_version
                .as_deref()
                .ok_or_else(|| {
                    AppError::new(
                        "VALIDATION",
                        "This Fabric instance has no loader version. Edit the instance and choose one.",
                    )
                })?
                .to_owned(),
        };
        fabric::install(
            &input.root,
            &input.instance.version,
            &loader_version,
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
    account_jobs: State<'_, crate::auth::accounts::AccountJobs>,
    id: String,
) -> Result<()> {
    let input = input(&state, &id)?;
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
        let installed = verify(&input, true)
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
        let _account_guard = account_jobs
            .0
            .try_lock()
            .map_err(|_| crate::auth::accounts::busy())?;
        let account_mode = crate::auth::accounts::readiness(&input.data)?;
        let auth = match account_mode {
            crate::auth::accounts::AccountMode::Microsoft { .. } => launch::LaunchAuth::Microsoft(
                crate::auth::accounts::launch_session(&input.data).await?,
            ),
            crate::auth::accounts::AccountMode::LocalTest => {
                #[cfg(debug_assertions)]
                {
                    let (player, uuid) = crate::auth::accounts::local_test_identity(&input.data)?
                        .ok_or_else(launch::authentication_required)?;
                    launch::LaunchAuth::LocalTest { player, uuid }
                }
                #[cfg(not(debug_assertions))]
                {
                    return Err(launch::authentication_required());
                }
            }
        };
        let launch_message =
            if matches!(account_mode, crate::auth::accounts::AccountMode::LocalTest) {
                "Local Offline Test — Unauthenticated. Starting Minecraft…"
            } else {
                "Verifying game files before launch…"
            };
        publish_game(
            &app,
            GameStatus::new(&id, Readiness::Launching, launch_message),
        );
        let event_app = app.clone();
        launch::start(
            reservation,
            launch::LaunchRequest {
                installed,
                instance: input.instance,
                java,
                platform,
                auth,
                log_root: input.logs,
            },
            move |status| publish_game(&event_app, status),
        )
        .await
    }
    .await;
    if let Err(error) = &result {
        let readiness = if error.code == "MISSING_FILES" {
            Readiness::MissingFiles
        } else {
            Readiness::Error
        };
        publish_game(&app, GameStatus::new(&id, readiness, &error.message));
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
