use super::LauncherState;
use crate::{
    errors::{AppError, Result},
    minecraft::{
        fabric, forge,
        install::{self, Installed, Progress, Stage},
        java,
        launch::{self, GameStatus, ProcessManager, Readiness},
        metadata::Platform,
        modrinth, multiplayer, pack, rocket_client, versions,
    },
    models::{AppSnapshot, Instance, Loader},
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

impl GameJobs {
    pub(super) fn lock_for_update(&self) -> Result<tokio::sync::MutexGuard<'_, ()>> {
        let guard = self.operation.try_lock().map_err(|_| {
            AppError::new("BUSY", "Wait for the current Minecraft operation to finish before updating.")
        })?;
        if self.processes.busy() {
            return Err(AppError::new("BUSY", "Close Minecraft before updating Rocket Launcher."));
        }
        Ok(guard)
    }
}
struct Input {
    instance: Instance,
    root: PathBuf,
    data: PathBuf,
    logs: PathBuf,
    catalogue: PathBuf,
    java: Option<PathBuf>,
    timeout: u64,
    sync_servers: bool,
    sync_keybinds: bool,
    sync_rocket_client: bool,
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
        sync_servers: store.config.settings.sync_servers,
        sync_keybinds: store.config.settings.sync_keybinds,
        sync_rocket_client: store.config.settings.sync_rocket_client,
    })
}

#[tauri::command]
pub fn set_instance_mod_enabled(
    state: State<'_, LauncherState>,
    jobs: State<'_, GameJobs>,
    instance_id: String,
    file_name: String,
    enabled: bool,
) -> Result<()> {
    let _guard = jobs.operation.try_lock().map_err(|_| {
        AppError::new("BUSY", "Wait for the current Minecraft operation to finish.")
    })?;
    if jobs.processes.contains(&instance_id) {
        return Err(AppError::new("BUSY", "Close this instance before changing its mods."));
    }
    let store = state.store()?;
    let instance = store.find_instance(&instance_id)?;
    if enabled && instance.loader == Loader::Fabric {
        multiplayer::ensure_addon_can_enable(instance, &file_name)?;
    }
    crate::instances::set_mod_enabled(&instance.game_directory, &file_name, enabled)
}

#[tauri::command]
pub fn saved_servers(state: State<'_, LauncherState>) -> Result<Vec<multiplayer::SavedServer>> {
    let store = state.store()?;
    multiplayer::saved_servers(&store.paths.data)
}

#[tauri::command]
pub fn save_server(state: State<'_, LauncherState>, server: multiplayer::SavedServer) -> Result<Vec<multiplayer::SavedServer>> {
    let store = state.store()?;
    if let Some(id) = server.instance_id.as_deref() { store.find_instance(id)?; }
    multiplayer::save_server(&store.paths.data, server)
}

#[tauri::command]
pub fn remove_saved_server(state: State<'_, LauncherState>, id: String) -> Result<Vec<multiplayer::SavedServer>> {
    let store = state.store()?;
    multiplayer::remove_server(&store.paths.data, &id)
}

#[tauri::command]
pub async fn ping_server(state: State<'_, LauncherState>, address: String) -> Result<multiplayer::ServerStatus> {
    modpack_timeout(&state)?;
    multiplayer::ping(multiplayer::Address::parse(&address)?).await
}

#[tauri::command]
pub async fn multiplayer_addons(state: State<'_, LauncherState>, instance_id: String) -> Result<Vec<multiplayer::InstalledAddon>> {
    let instance = state.store()?.find_instance(&instance_id)?.clone();
    tokio::task::spawn_blocking(move || multiplayer::addons(&instance)).await
        .map_err(|reason| AppError::new("MULTIPLAYER", format!("Could not inspect installed add-ons: {reason}")))?
}

#[tauri::command]
pub async fn install_multiplayer_addon(
    state: State<'_, LauncherState>, jobs: State<'_, GameJobs>, instance_id: String, feature: String,
) -> Result<String> {
    let _guard = jobs.operation.try_lock().map_err(|_| AppError::new("BUSY", "Wait for the current Minecraft operation."))?;
    if jobs.processes.contains(&instance_id) { return Err(AppError::new("BUSY", "Close this instance before adding multiplayer mods.")); }
    let input = input(&state, &instance_id)?;
    require_online(&input)?;
    let instance = &input.instance;
    if instance.loader != Loader::Fabric { return Err(AppError::new("MULTIPLAYER", "Choose a Fabric instance.")); }
    let projects: &[&str] = match feature.as_str() {
        "via" => &[multiplayer::VIA_PLUS, multiplayer::VIA_FABRIC],
        "hosting" => &[multiplayer::E4MC],
        _ => return Err(AppError::new("MULTIPLAYER", "Unknown multiplayer add-on.")),
    };
    let installed = multiplayer::addons(instance)?;
    let matching: Vec<_> = installed.iter().filter(|item| if feature == "via" {
        multiplayer::is_via(&item.id)
    } else {
        multiplayer::is_hosting(&item.id)
    }).collect();
    let active: Vec<_> = matching.iter().filter(|item| item.enabled).collect();
    if active.len() > 1 { return Err(AppError::new("MULTIPLAYER", "Disable duplicate add-ons in Installed mods.")); }
    if let Some(item) = active.first() { return Ok(item.filename.clone()); }
    if !matching.is_empty() {
        return Err(AppError::new("MULTIPLAYER", "An installed copy is disabled. Enable it in Installed mods or remove it before installing a new copy."));
    }
    let mut candidate = None;
    for project in projects {
        if let Some(version) = modrinth::preferred_version(instance, project, input.timeout).await? {
            candidate = Some((*project, version));
            break;
        }
    }
    let (project, version) = candidate.ok_or_else(|| AppError::new("MULTIPLAYER", "Modrinth has no compatible build for this Minecraft version."))?;
    let api = rocket_client::installed_fabric_api(instance)?;
    let addon = modrinth::stage_with_api(instance, project, &version, input.timeout, true).await?;
    let staged_api = if api.is_none() {
        let version = modrinth::preferred_version(instance, multiplayer::FABRIC_API, input.timeout).await?
            .ok_or_else(|| AppError::new("MULTIPLAYER", "Fabric API has no compatible build for this instance."))?;
        Some(modrinth::stage(instance, multiplayer::FABRIC_API, &version, input.timeout).await?)
    } else { None };
    require_online(&input)?;
    let store = state.store()?;
    let current = store.find_instance(&instance_id)?;
    if current.version != instance.version || current.loader != instance.loader
        || current.loader_version != instance.loader_version || current.game_directory != instance.game_directory {
        return Err(AppError::new("MULTIPLAYER", "The instance changed during installation. Try again."));
    }
    if let Some(api) = staged_api { api.publish(&current.game_directory)?; }
    if let Some((filename, false)) = api { crate::instances::set_mod_enabled(&current.game_directory, &filename, true)?; }
    addon.publish(&current.game_directory)
}

#[tauri::command]
pub fn open_instance_mods_folder(state: State<'_, LauncherState>, instance_id: String) -> Result<()> {
    let game = state.store()?.find_instance(&instance_id)?.game_directory.clone();
    crate::instances::mods_directory(&game)?;
    open_folder(&game.join("mods"))
}

#[tauri::command]
pub fn open_instance_folder(state: State<'_, LauncherState>, instance_id: String) -> Result<()> {
    let game = state.store()?.find_instance(&instance_id)?.game_directory.clone();
    open_folder(&game)
}

fn open_folder(path: &std::path::Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};

    let folder: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    let result = unsafe {
        ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), folder.as_ptr(), std::ptr::null(),
            std::ptr::null(), SW_SHOWNORMAL)
    };
    if result as isize <= 32 {
        return Err(AppError::new("INSTANCE_FOLDER", "Windows could not open the instance folder."));
    }
    Ok(())
}

#[tauri::command]
pub async fn rocket_client_status(
    state: State<'_, LauncherState>,
    instance_id: String,
) -> Result<rocket_client::Status> {
    let instance = state.store()?.find_instance(&instance_id)?.clone();
    tokio::task::spawn_blocking(move || rocket_client::status(&instance))
        .await.map_err(|e| AppError::new("ROCKET_CLIENT", e.to_string()))?
}

#[tauri::command]
pub async fn install_rocket_client(
    state: State<'_, LauncherState>,
    jobs: State<'_, GameJobs>,
    instance_id: String,
) -> Result<()> {
    let _guard = jobs.operation.try_lock()
        .map_err(|_| AppError::new("BUSY", "Another Minecraft operation is in progress."))?;
    if jobs.processes.contains(&instance_id) {
        return Err(AppError::new("BUSY", "Close this instance before changing its mods."));
    }
    let original = input(&state, &instance_id)?;
    let offline = crate::auth::accounts::Accounts::read(&original.data)?.offline;
    let api = rocket_client::prepare_api(&original.instance, offline, original.timeout).await?;
    let store = state.store()?;
    let current = store.find_instance(&instance_id)?;
    if current.version != original.instance.version
        || current.loader != original.instance.loader
        || current.loader_version != original.instance.loader_version
        || current.game_directory != original.instance.game_directory
    {
        return Err(AppError::new("ROCKET_CLIENT", "The instance changed during installation. Try again."));
    }
    rocket_client::install(current, api)
}

#[tauri::command]
pub async fn install_rocket_pack(
    state: State<'_, LauncherState>,
    jobs: State<'_, GameJobs>,
    version: String,
    input: Option<crate::models::InstanceInput>,
    progress: tauri::ipc::Channel<[usize; 2]>,
) -> Result<AppSnapshot> {
    use crate::{auth::accounts, storage::validate_path};
    let _guard = jobs
        .operation
        .try_lock()
        .map_err(|_| AppError::new("BUSY", "Another Minecraft operation is in progress."))?;
    let (input, parent, cache, timeout) = {
        let mut store = state.store()?;
        if input.is_none() {
            let existing = store.config.instances.iter()
                .filter(|instance| instance.version == version && instance.is_rocket_pack())
                .max_by_key(|instance| instance.last_selected_at)
                .map(|instance| instance.id.clone());
            if let Some(id) = existing {
                return store.select_instance(&id);
            }
        }
        if accounts::Accounts::read(&store.paths.data)?.offline {
            return Err(accounts::offline_error());
        }
        let settings = &store.config.settings;
        if version != "1.21.11"
            && versions::version_type(&store.paths.cache.join("versions.json"), &version)? != "release"
        {
            return Err(AppError::new("PACK", "Choose a Minecraft release for Rocket Pack."));
        }
        let input = input.unwrap_or_else(|| crate::models::InstanceInput {
            name: format!("Rocket Pack {version}"),
            version: version.clone(),
            loader: crate::models::Loader::Fabric,
            loader_version: None,
            ram_mb: settings.default_ram_mb,
            java_path: None,
            parent_directory: None,
        });
        if input.version != version || input.loader != crate::models::Loader::Fabric {
            return Err(AppError::new("PACK", "Rocket Pack requires a matching Fabric instance."));
        }
        let parent = input.parent_directory.clone()
            .unwrap_or_else(|| settings.minecraft_directory.join("Rocket"));
        (
            input,
            parent,
            store.paths.cache.join("rocket-pack"),
            settings.request_timeout_seconds,
        )
    };
    validate_path(&parent)?;
    std::fs::create_dir_all(&parent).map_err(|e| AppError::io("create the instance parent directory", e))?;
    let directory = tempfile::Builder::new()
        .prefix("rocket-pack-")
        .tempdir_in(parent)
        .map_err(|e| AppError::io("create the Rocket Pack directory", e))?;
    let input = pack::install_for_version(input, directory.path(), &cache, timeout, |completed, total| {
        let _ = progress.send([completed, total]);
    }).await?;
    let mut store = state.store()?;
    if accounts::Accounts::read(&store.paths.data)?.offline {
        return Err(accounts::offline_error());
    }
    store.create_pack_instance(input, directory)
}
async fn verify(input: &Input, full: bool, allow_missing: bool) -> Result<Option<Installed>> {
    let platform = Platform::windows()?;
    let root = input.root.clone();
    let version = input.instance.version.clone();
    // The vanilla game version is always verified first; Fabric layers on top of it.
    let vanilla = tokio::task::spawn_blocking(move || {
        if allow_missing {
            install::inspect_for_launch(&root, &version, &platform)
        } else if full {
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
        Loader::Forge => {
            let loader = input.instance.loader_version.clone().ok_or_else(|| AppError::new("VALIDATION", "Choose a Forge loader version in the instance editor."))?;
            let root = input.root.clone();
            let game = input.instance.version.clone();
            let platform = Platform::windows()?;
            tokio::task::spawn_blocking(move || {
                if allow_missing { forge::verify_for_launch(&root, &game, &loader, &platform, vanilla) }
                else { forge::verify(&root, &game, &loader, &platform, vanilla, full) }
            })
                .await.map_err(|e| AppError::new("FORGE", format!("Could not verify Forge: {e}")))?
        }
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
                if allow_missing {
                    fabric::verify_for_launch(&root, &game_version, &loader_version, &platform, &vanilla)
                } else {
                fabric::verify(
                    &root,
                    &game_version,
                    &loader_version,
                    &platform,
                    &vanilla,
                    full,
                )
                }
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
    sort: Option<String>,
) -> Result<modrinth::SearchPage> {
    let input = input(&state, &instance_id)?;
    require_online(&input)?;
    modrinth::search(&input.instance, &query, offset, input.timeout, sort.as_deref()).await
}

#[tauri::command]
pub async fn search_modpacks(
    state: State<'_, LauncherState>, query: String, offset: u32, sort: String,
    filters: modrinth::PackFilters,
) -> Result<modrinth::SearchPage> {
    let timeout = modpack_timeout(&state)?;
    modrinth::search_packs(&query, offset, &sort, filters, timeout).await
}

#[tauri::command]
pub async fn modpack_details(state: State<'_, LauncherState>, project_id: String) -> Result<modrinth::PackDetails> {
    let timeout = modpack_timeout(&state)?;
    modrinth::pack_details(&project_id, timeout).await
}

fn modpack_timeout(state: &LauncherState) -> Result<u64> {
    let store = state.store()?;
    if crate::auth::accounts::Accounts::read(&store.paths.data)?.offline {
        return Err(crate::auth::accounts::offline_error());
    }
    Ok(store.config.settings.request_timeout_seconds)
}

#[tauri::command]
pub async fn install_modpack(
    state: State<'_, LauncherState>, jobs: State<'_, GameJobs>, project_id: String,
    version_id: String, game: String, loader: String, progress: tauri::ipc::Channel<modrinth::PackProgress>,
) -> Result<AppSnapshot> {
    let _guard = jobs.operation.try_lock()
        .map_err(|_| AppError::new("BUSY", "Another Minecraft operation is in progress."))?;
    let timeout = modpack_timeout(&state)?;
    let (parent, cache, ram) = {
        let store = state.store()?;
        if modrinth::has_pack(&store.config.instances, &project_id, &version_id) {
            return Err(AppError::new("MODRINTH", "This modpack version is already installed."));
        }
        let settings = &store.config.settings;
        (settings.minecraft_directory.clone(), store.paths.cache.join("modrinth-packs"), settings.default_ram_mb)
    };
    fabric::validate_component(&game)?;
    crate::storage::validate_path(&parent)?;
    std::fs::create_dir_all(&parent).map_err(|e| AppError::io("create the instance parent directory", e))?;
    let directory = tempfile::Builder::new().prefix("modrinth-pack-").tempdir_in(parent)
        .map_err(|e| AppError::io("stage the modpack instance", e))?;
    let input = modrinth::install_pack(&project_id, &version_id, &game, &loader, directory.path(),
        &cache, ram, timeout, &|status| { let _ = progress.send(status); }).await?;
    modpack_timeout(&state)?;
    state.store()?.create_pack_instance(input, directory)
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
    version_id: Option<String>,
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
    if matches!(project_id.as_str(), multiplayer::VIA_PLUS | multiplayer::VIA_FABRIC) {
        let opposite = if project_id == multiplayer::VIA_PLUS { "viafabric" } else { "viafabricplus" };
        if multiplayer::addons(&original.instance)?.iter().any(|item| item.enabled && item.id == opposite) {
            return Err(AppError::new("MULTIPLAYER", "Disable the active ViaFabric variant before installing another one."));
        }
    }
    let version_id = match version_id {
        Some(id) => id,
        None => {
            modrinth::preferred_version(&original.instance, &project_id, original.timeout).await?
                .ok_or_else(|| AppError::new("MODRINTH", "No downloadable version matches this instance."))?
        }
    };
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
        || current.loader_version != original.instance.loader_version
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
    let installed = match missing_status(&id, verify(&input, false, false).await) {
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
            Ok(crate::auth::accounts::AccountMode::Offline) => Ok(GameStatus::new(
                &id, Readiness::Ready, "Offline profile ready for local play.",
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
    let modded = input.instance.loader != Loader::Vanilla;
    require_online(&input)?;
    let initial = Progress {
        instance_id: id.clone(),
        version: input.instance.version.clone(),
        stage: Stage::Downloading,
        current: if modded {
            "Preparing modded installation".into()
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
                if !modded || !matches!(progress.stage, Stage::Complete) {
                    publish_progress(&app, progress);
                }
            },
        )
        .await?;
        if !modded {
            return Ok(());
        }
        let loader_version = input.instance.loader_version.as_deref().ok_or_else(||
            AppError::new("VALIDATION", "Choose a loader version in the instance editor."))?;
        if input.instance.loader == Loader::Forge {
            let platform = Platform::windows()?;
            let vanilla = install::inspect_installed(&input.root, &input.instance.version, &platform, false)?
                .ok_or_else(|| AppError::new("MISSING_FILES", "Minecraft installation is missing."))?;
            let java = java::resolve(input.java, &input.data, vanilla.version.java_major(), platform.arch).await?;
            let mut progress = initial.clone();
            progress.stage = Stage::Installing;
            progress.current = "Running the Forge client installer".into();
            publish_progress(&app, progress.clone());
            forge::install(&input.root, &input.instance.version, loader_version, &platform, &java, input.timeout).await?;
            progress.stage = Stage::Complete;
            progress.current = "Forge installation complete".into();
            publish_progress(&app, progress);
            return Ok(());
        }
        fabric::install(
            &input.root,
            &input.instance.version,
            loader_version,
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
    server: Option<String>,
) -> Result<()> {
    let input = input(&state, &id)?;
    let server = if let Some(address) = server {
        require_online(&input)?;
        Some(multiplayer::Address::parse(&address)?.minecraft())
    } else { None };
    let _guard = jobs
        .operation
        .try_lock()
        .map_err(|_| AppError::new("BUSY", "Another Minecraft operation is in progress."))?;
    if (input.sync_servers || input.sync_keybinds) && jobs.processes.busy() {
        return Err(AppError::new("BUSY", "Close other Minecraft instances before launching with shared servers or keybinds."));
    }
    let reservation = jobs.processes.reserve(&id)?;
    publish_game(
        &app,
        GameStatus::new(
            &id,
            Readiness::Launching,
            "Preparing Minecraft…",
        ),
    );
    let result = async {
        let installed = verify(&input, false, true)
            .await?
            .ok_or_else(|| AppError::new("MISSING_FILES", "Install Minecraft before launching."))?;
        reservation.check_cancelled()?;
        let platform = Platform::windows()?;
        let java = java::resolve(
            input.java,
            &input.data,
            installed.version.java_major(),
            platform.arch,
        )
        .await?;
        reservation.check_cancelled()?;
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
            crate::auth::accounts::AccountMode::Offline => {
                let (player, uuid) = crate::auth::accounts::offline_identity(&input.data)?;
                launch::LaunchAuth::Offline { player, uuid }
            }
        };
        reservation.check_cancelled()?;
        let sync = if input.sync_servers || input.sync_keybinds {
            let root = input.data.join("instance-sync");
            let game = input.instance.game_directory.clone();
            let servers = input.sync_servers;
            let keybinds = input.sync_keybinds;
            let result = tokio::task::spawn_blocking({
                let root = root.clone();
                move || crate::minecraft::sync::prepare(&root, &game, servers, keybinds)
            }).await.map_err(|e| AppError::new("SYNC", format!("Could not prepare instance sync: {e}")))??;
            Some((root, result))
        } else {
            None
        };
        let launch_message =
            if matches!(account_mode, crate::auth::accounts::AccountMode::LocalTest) {
                "Local Offline Test — Unauthenticated. Starting Minecraft…"
            } else {
                "Preparing Minecraft…"
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
                sync,
                rocket_client_config: input.sync_rocket_client
                    .then(|| input.data.join("instance-sync").join("rocket-client.properties")),
                server,
            },
            move |status| publish_game(&event_app, status),
        )
        .await
    }
    .await;
    if let Err(error) = &result {
        let readiness = match error.code {
            "MISSING_FILES" => Readiness::MissingFiles,
            "ABORTED" => Readiness::Ready,
            _ => Readiness::Error,
        };
        publish_game(&app, GameStatus::new(&id, readiness, &error.message));
    }
    result
}

#[tauri::command]
pub fn abort_game(jobs: State<'_, GameJobs>, instance_id: String) -> Result<()> {
    jobs.processes.cancel(&instance_id)
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
