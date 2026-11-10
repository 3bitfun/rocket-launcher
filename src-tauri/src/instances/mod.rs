use crate::{
    errors::{AppError, Result},
    minecraft::fabric,
    models::*,
    storage::{validate_java, validate_path, validate_ram, Store},
};
use std::{fs, path::{Path, PathBuf}};

#[derive(serde::Serialize, serde::Deserialize)]
struct RocketMove {
    id: String,
    from: PathBuf,
    to: PathBuf,
}

fn ordinary_directory(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    fs::symlink_metadata(path).is_ok_and(|info| info.is_dir() && info.file_attributes() & 0x400 == 0)
}

pub(crate) fn validate_input(input: &InstanceInput) -> Result<()> {
    if input.name.trim().is_empty()
        || input.name.trim().chars().count() > 64
        || input.name.chars().any(char::is_control)
    {
        return Err(AppError::new(
            "VALIDATION",
            "Give the instance a name of 1–64 characters without control characters.",
        ));
    }
    validate_ram(input.ram_mb)?;
    validate_java(&input.java_path)?;
    match (&input.loader, input.loader_version.as_deref()) {
        (Loader::Fabric, Some(loader_version)) => fabric::validate_component(loader_version)?,
        (Loader::Forge, Some(loader_version)) => {
            crate::minecraft::forge::validate_instance(&input.version, loader_version)?
        }
        (Loader::Fabric | Loader::Forge, None) => {
            return Err(AppError::new(
                "VALIDATION",
                "Choose a loader version for this modded instance.",
            ))
        }
        (Loader::Vanilla, None) => {}
        (Loader::Vanilla, Some(_)) => {
            return Err(AppError::new(
                "VALIDATION",
                "Vanilla instances do not use a mod loader version.",
            ))
        }
    }
    Ok(())
}

impl Store {
    pub fn relocate_rocket_instances(&mut self) -> Result<()> {
        let journal = self.paths.data.join("rocket-pack-move.json");
        match fs::read(&journal) {
            Ok(bytes) => {
                let plan: RocketMove = serde_json::from_slice(&bytes)
                    .map_err(|e| AppError::io("read the Rocket folder move record", e))?;
                self.finish_rocket_move(&journal, plan)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(AppError::io("open the Rocket folder move record", e)),
        }

        let parent = self.config.settings.minecraft_directory.join("Rocket");
        let moves: Vec<_> = self.config.instances.iter()
            .filter(|instance| instance.is_rocket_pack() && instance.game_directory.parent() != Some(parent.as_path()))
            .filter(|instance| ordinary_directory(&instance.game_directory))
            .map(|instance| RocketMove {
                id: instance.id.clone(),
                from: instance.game_directory.clone(),
                to: parent.join(format!("rocket-pack-{}", instance.id)),
            })
            .collect();
        if moves.is_empty() { return Ok(()); }
        validate_path(&parent)?;
        fs::create_dir_all(&parent).map_err(|e| AppError::io("create the Rocket folder", e))?;
        for plan in moves {
            if !ordinary_directory(&parent) || !matches!(fs::symlink_metadata(&plan.to), Err(e) if e.kind() == std::io::ErrorKind::NotFound) {
                tracing::warn!(instance_id = %plan.id, "Rocket folder destination already exists or is unavailable");
                continue;
            }
            crate::storage::atomic_json(&journal, &plan)?;
            if let Err(error) = fs::rename(&plan.from, &plan.to) {
                fs::remove_file(&journal).map_err(|e| AppError::io("clear the Rocket folder move record", e))?;
                tracing::warn!(instance_id = %plan.id, %error, "Rocket folder move could not be completed");
                continue;
            }
            self.finish_rocket_move(&journal, plan)?;
        }
        Ok(())
    }

    fn finish_rocket_move(&mut self, journal: &Path, plan: RocketMove) -> Result<()> {
        validate_path(&plan.from)?;
        validate_path(&plan.to)?;
        let current = self.find_instance(&plan.id)?;
        let destination_name = format!("rocket-pack-{}", plan.id);
        if !current.is_rocket_pack()
            || plan.to.file_name().and_then(|name| name.to_str()) != Some(destination_name.as_str())
            || plan.to.parent().and_then(Path::file_name).and_then(|name| name.to_str()) != Some("Rocket")
            || (current.game_directory != plan.from && current.game_directory != plan.to)
        {
            return Err(AppError::new("PACK", "The Rocket folder move record does not match the saved instance."));
        }
        if current.game_directory == plan.from {
            if ordinary_directory(&plan.from) && matches!(fs::symlink_metadata(&plan.to), Err(e) if e.kind() == std::io::ErrorKind::NotFound) {
                // The move had not started. The next migration pass can retry it.
            } else if ordinary_directory(&plan.to) && matches!(fs::symlink_metadata(&plan.from), Err(e) if e.kind() == std::io::ErrorKind::NotFound) {
                let mut next = self.config.clone();
                let instance = next.instances.iter_mut().find(|instance| instance.id == plan.id).ok_or_else(not_found)?;
                instance.game_directory = plan.to.clone();
                if let Err(error) = self.commit(next.clone()) {
                    if fs::rename(&plan.to, &plan.from).is_err() {
                        // The journal lets the next startup finish saving the new path.
                        self.config = next;
                    }
                    return Err(error);
                }
            } else {
                return Err(AppError::new("PACK", "Could not safely finish moving this Rocket instance."));
            }
        }
        fs::remove_file(journal).map_err(|e| AppError::io("clear the Rocket folder move record", e))
    }

    pub fn create_instance(
        &mut self,
        input: InstanceInput,
        version_type: &str,
    ) -> Result<AppSnapshot> {
        validate_input(&input)?;
        let parent = input
            .parent_directory
            .as_ref()
            .unwrap_or(&self.config.settings.minecraft_directory);
        validate_path(parent)?;
        let id = uuid::Uuid::new_v4().to_string();
        let directory = parent.join(&id);
        fs::create_dir_all(parent)
            .map_err(|e| AppError::io("create the instance parent directory", e))?;
        fs::create_dir(&directory)
            .map_err(|e| AppError::io("create the instance game directory", e))?;
        if let Err(error) = fs::create_dir(directory.join("mods")) {
            let _ = fs::remove_dir(&directory);
            return Err(AppError::io("create the instance mods folder", error));
        }
        match self.register_instance(input, version_type, &id, directory.clone()) {
            Ok(snapshot) => Ok(snapshot),
            Err(error) => {
                let _ = fs::remove_dir(directory.join("mods"));
                let _ = fs::remove_dir(directory);
                Err(error)
            }
        }
    }

    pub fn create_pack_instance(
        &mut self,
        input: InstanceInput,
        directory: tempfile::TempDir,
    ) -> Result<AppSnapshot> {
        validate_input(&input)?;
        let id = uuid::Uuid::new_v4().to_string();
        let snapshot = self.register_instance(input, "release", &id, directory.path().into())?;
        let _ = directory.keep();
        Ok(snapshot)
    }

    fn register_instance(
        &mut self,
        input: InstanceInput,
        version_type: &str,
        id: &str,
        directory: PathBuf,
    ) -> Result<AppSnapshot> {
        let mut next = self.config.clone();
        next.instances.push(Instance {
            id: id.into(),
            name: input.name.trim().into(),
            version: input.version,
            version_type: version_type.into(),
            loader: input.loader,
            loader_version: input.loader_version.clone(),
            game_directory: directory,
            ram_mb: input.ram_mb,
            java_path: input.java_path,
            created_at: now(),
            updated_at: now(),
            last_selected_at: now(),
        });
        next.active_instance_id = Some(id.into());
        let snapshot = self.commit(next)?;
        tracing::info!(event = "instance_created", instance_id = %id);
        Ok(snapshot)
    }

    pub fn update_instance(
        &mut self,
        id: &str,
        input: InstanceInput,
        version_type: &str,
    ) -> Result<AppSnapshot> {
        validate_input(&input)?;
        if input.parent_directory.is_some() {
            return Err(AppError::new("VALIDATION", "Existing game directories cannot be relocated. Create a new instance in the desired directory."));
        }
        let mut next = self.config.clone();
        let instance = next
            .instances
            .iter_mut()
            .find(|i| i.id == id)
            .ok_or_else(not_found)?;
        if instance.is_rocket_pack()
            && (instance.version != input.version || instance.loader != input.loader
                || instance.loader_version != input.loader_version)
        {
            return Err(AppError::new("VALIDATION", "Choose another Rocket Pack version from the Launch Pad to keep each version's mods and saves separate."));
        }
        instance.name = input.name.trim().into();
        instance.version = input.version;
        instance.version_type = version_type.into();
        instance.loader = input.loader;
        instance.loader_version = input.loader_version.clone();
        instance.ram_mb = input.ram_mb;
        instance.java_path = input.java_path;
        instance.updated_at = now();
        let result = self.commit(next)?;
        tracing::info!(event = "instance_updated", instance_id = %id);
        Ok(result)
    }

    pub fn select_instance(&mut self, id: &str) -> Result<AppSnapshot> {
        let mut next = self.config.clone();
        let instance = next
            .instances
            .iter_mut()
            .find(|i| i.id == id)
            .ok_or_else(not_found)?;
        instance.last_selected_at = now();
        next.active_instance_id = Some(id.into());
        self.commit(next)
    }

    pub fn remove_instance(&mut self, id: &str) -> Result<AppSnapshot> {
        self.find_instance(id)?;
        let mut next = self.config.clone();
        next.instances.retain(|i| i.id != id);
        if next.active_instance_id.as_deref() == Some(id) {
            next.active_instance_id = next
                .instances
                .iter()
                .max_by_key(|i| i.last_selected_at)
                .map(|i| i.id.clone());
        }
        let result = self.commit(next)?;
        tracing::info!(event = "instance_removed", instance_id = %id, files_preserved = true);
        Ok(result)
    }

    pub fn duplicate_instance(&mut self, id: &str) -> Result<AppSnapshot> {
        let original = self.find_instance(id)?.clone();
        let name = format!(
            "{} (copy)",
            original.name.chars().take(57).collect::<String>()
        );
        self.create_instance(
            InstanceInput {
                name,
                version: original.version,
                loader: original.loader,
                loader_version: original.loader_version,
                ram_mb: original.ram_mb,
                java_path: original.java_path,
                parent_directory: None,
            },
            &original.version_type,
        )
    }

    pub fn find_instance(&self, id: &str) -> Result<&Instance> {
        self.config
            .instances
            .iter()
            .find(|i| i.id == id)
            .ok_or_else(not_found)
    }

    pub fn installed_mods(&self, id: &str) -> Result<Vec<InstalledMod>> {
        let game = &self.find_instance(id)?.game_directory;
        if !game.join("mods").exists() {
            return Ok(vec![]);
        }
        let directory = mods_directory(game)?;
        let mut mods = vec![];
        for entry in
            fs::read_dir(directory).map_err(|e| AppError::io("read the mods directory", e))?
        {
            let entry = entry.map_err(|e| AppError::io("read a mod entry", e))?;
            let kind = entry
                .file_type()
                .map_err(|e| AppError::io("inspect a mod file", e))?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let disabled = name.strip_suffix(".disabled");
            let filename = disabled.unwrap_or(name);
            if kind.is_file() && is_mod_jar(filename) {
                mods.push(InstalledMod {
                    file_name: filename.into(),
                    size_bytes: entry
                        .metadata()
                        .map_err(|e| AppError::io("inspect mod size", e))?
                        .len(),
                    enabled: disabled.is_none(),
                });
            }
        }
        mods.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        Ok(mods)
    }
}

fn is_mod_jar(name: &str) -> bool {
    std::path::Path::new(name).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("jar"))
}

pub(crate) fn mods_directory(game: &std::path::Path) -> Result<PathBuf> {
    let game = fs::canonicalize(game).map_err(|e| AppError::io("open the instance directory", e))?;
    let folder = game.join("mods");
    match fs::create_dir(&folder) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(AppError::io("create the mods folder", e)),
    }
    let resolved = fs::canonicalize(&folder).map_err(|e| AppError::io("open the mods folder", e))?;
    if resolved != folder || !resolved.is_dir() {
        return Err(AppError::new("MOD_FILES", "The mods folder must be inside the selected instance."));
    }
    Ok(resolved)
}

pub fn set_mod_enabled(game: &std::path::Path, filename: &str, enabled: bool) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::MoveFileW;

    crate::minecraft::metadata::segment(filename)?;
    if !is_mod_jar(filename) {
        return Err(AppError::new("MOD_FILES", "Choose a JAR file from this instance."));
    }
    let folder = mods_directory(game)?;
    let jar = folder.join(filename);
    let disabled = folder.join(format!("{filename}.disabled"));
    let (source, target) = if enabled { (disabled, jar) } else { (jar, disabled) };
    if !fs::symlink_metadata(&source).is_ok_and(|meta| meta.file_type().is_file()) {
        return Err(AppError::new("MOD_FILES", "The selected mod file is missing or has changed. Refresh the mod list."));
    }
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    // MoveFileW rejects an existing destination, even if it appeared after listing.
    if unsafe { MoveFileW(source.as_ptr(), target.as_ptr()) } == 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            return Err(AppError::new("MOD_FILES", "A mod file with that name already exists. Resolve the duplicate in the mods folder."));
        }
        return Err(AppError::io("change mod state", error));
    }
    Ok(())
}

fn not_found() -> AppError {
    AppError::new(
        "NOT_FOUND",
        "This instance no longer exists. Refresh the launcher and try again.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_existing_rocket_pack_without_losing_files() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = Store::open(temp.path().into()).unwrap();
        let original = tempfile::Builder::new().prefix("rocket-pack-").tempdir_in(temp.path()).unwrap();
        let source = original.path().to_owned();
        fs::write(source.join("world.txt"), "keep me").unwrap();
        let input = InstanceInput {
            name: "Rocket".into(),
            version: "1.21.11".into(),
            loader: Loader::Fabric,
            loader_version: Some("0.20.1".into()),
            ram_mb: 4096,
            java_path: None,
            parent_directory: None,
        };
        store.create_pack_instance(input, original).unwrap();
        let id = store.config.active_instance_id.clone().unwrap();
        store.relocate_rocket_instances().unwrap();
        let reopened = Store::open(temp.path().into()).unwrap();
        let instance = reopened.find_instance(&id).unwrap();
        assert_eq!(instance.game_directory.parent(), Some(reopened.config.settings.minecraft_directory.join("Rocket").as_path()));
        assert_eq!(fs::read_to_string(instance.game_directory.join("world.txt")).unwrap(), "keep me");
        assert!(!source.exists());
    }

    #[test]
    fn complete_instance_lifecycle_persists_and_preserves_game_files() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = Store::open(temp.path().into()).unwrap();
        let input = InstanceInput {
            name: "  Survival  ".into(),
            version: "1.21.1".into(),
            loader: Loader::Vanilla,
            loader_version: None,
            ram_mb: 4096,
            java_path: None,
            parent_directory: None,
        };
        store.create_instance(input.clone(), "release").unwrap();
        let id = store.config.active_instance_id.clone().unwrap();
        store.set_favorite(&id, true).unwrap();
        let path = store.find_instance(&id).unwrap().game_directory.clone();
        fs::write(path.join("world.txt"), "keep me").unwrap();
        let mut edited = input;
        edited.name = "New name".into();
        store.update_instance(&id, edited, "release").unwrap();
        store.duplicate_instance(&id).unwrap();
        assert_eq!(store.config.instances.len(), 2);
        assert_ne!(
            store.config.instances[0].game_directory,
            store.config.instances[1].game_directory
        );
        store.select_instance(&id).unwrap();
        let mut reopened = Store::open(temp.path().into()).unwrap();
        assert_eq!(reopened.find_instance(&id).unwrap().name, "New name");
        let favorites = reopened.snapshot().instances;
        assert_eq!(favorites.iter().filter(|instance| instance.favorite).count(), 1);
        assert!(favorites.iter().any(|instance| instance.favorite && instance.instance.id == id));
        assert_eq!(
            reopened.config.active_instance_id.as_deref(),
            Some(id.as_str())
        );
        reopened.remove_instance(&id).unwrap();
        assert!(path.join("world.txt").exists());
        let final_store = Store::open(temp.path().into()).unwrap();
        assert_eq!(final_store.config.instances.len(), 1);
        assert_ne!(
            final_store.config.active_instance_id.as_deref(),
            Some(id.as_str())
        );
        assert!(final_store.remove_unknown_for_test().is_err());
    }

    impl Store {
        fn remove_unknown_for_test(&self) -> Result<&Instance> {
            self.find_instance("missing")
        }
    }

    #[test]
    fn invalid_instance_does_not_change_configuration() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = Store::open(temp.path().into()).unwrap();
        assert!(store
            .create_instance(
                InstanceInput {
                    name: " ".into(),
                    version: "1.21".into(),
                    loader: Loader::Vanilla,
                    loader_version: None,
                    ram_mb: 4096,
                    java_path: None,
                    parent_directory: None
                },
                "release"
            )
            .is_err());
        assert!(store.config.instances.is_empty());
    }

    #[test]
    fn mods_are_read_from_the_selected_instance_only() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = Store::open(temp.path().into()).unwrap();
        store
            .create_instance(
                InstanceInput {
                    name: "Mods".into(),
                    version: "1.21".into(),
                    loader: Loader::Vanilla,
                    loader_version: None,
                    ram_mb: 4096,
                    java_path: None,
                    parent_directory: None,
                },
                "release",
            )
            .unwrap();
        let id = store.config.active_instance_id.clone().unwrap();
        let mods = store
            .find_instance(&id)
            .unwrap()
            .game_directory
            .join("mods");
        assert!(mods.is_dir());
        fs::write(mods.join("example.jar"), b"jar").unwrap();
        fs::write(mods.join("notes.txt"), b"not a mod").unwrap();
        assert_eq!(store.installed_mods(&id).unwrap().len(), 1);
        assert!(store.installed_mods("../outside").is_err());
    }

    #[test]
    fn fabric_instances_persist_their_loader_version_and_mods_folder() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = Store::open(temp.path().into()).unwrap();
        store
            .create_instance(
                InstanceInput {
                    name: "Modded".into(),
                    version: "1.21.1".into(),
                    loader: Loader::Fabric,
                    loader_version: Some("0.20.1".into()),
                    ram_mb: 4096,
                    java_path: None,
                    parent_directory: None,
                },
                "release",
            )
            .unwrap();
        let id = store.config.active_instance_id.clone().unwrap();
        let instance = store.find_instance(&id).unwrap();
        assert_eq!(instance.loader, Loader::Fabric);
        assert_eq!(instance.loader_version.as_deref(), Some("0.20.1"));
        assert!(instance.game_directory.join("mods").is_dir());
        // The loader version travels with a duplicate.
        store.duplicate_instance(&id).unwrap();
        let copy = store.find_instance(&store.config.instances[1].id).unwrap();
        assert_eq!(copy.loader, Loader::Fabric);
        assert_eq!(copy.loader_version.as_deref(), Some("0.20.1"));
        // Editing keeps the loader choice.
        store
            .update_instance(
                &id,
                InstanceInput {
                    name: "Renamed".into(),
                    version: "1.21.1".into(),
                    loader: Loader::Fabric,
                    loader_version: Some("0.20.1".into()),
                    ram_mb: 2048,
                    java_path: None,
                    parent_directory: None,
                },
                "release",
            )
            .unwrap();
        assert_eq!(store.find_instance(&id).unwrap().ram_mb, 2048);
        // A Fabric input without a loader version is rejected.
        assert!(store
            .create_instance(
                InstanceInput {
                    name: "Broken".into(),
                    version: "1.21.1".into(),
                    loader: Loader::Fabric,
                    loader_version: None,
                    ram_mb: 4096,
                    java_path: None,
                    parent_directory: None,
                },
                "release",
            )
            .is_err());
        assert_eq!(store.config.instances.len(), 2);
        // A loader version outside the allowed set is rejected.
        assert!(store
            .create_instance(
                InstanceInput {
                    name: "Broken".into(),
                    version: "1.21.1".into(),
                    loader: Loader::Fabric,
                    loader_version: Some("../evil".into()),
                    ram_mb: 4096,
                    java_path: None,
                    parent_directory: None,
                },
                "release",
            )
            .is_err());
    }
}
