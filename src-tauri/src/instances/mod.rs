use crate::{
    errors::{AppError, Result},
    minecraft::fabric,
    models::*,
    storage::{validate_java, validate_path, validate_ram, Store},
};
use std::{fs, path::PathBuf};

fn validate_input(input: &InstanceInput) -> Result<()> {
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
        let directory: PathBuf = self.find_instance(id)?.game_directory.join("mods");
        if !directory.exists() {
            return Ok(vec![]);
        }
        let mut mods = vec![];
        for entry in
            fs::read_dir(directory).map_err(|e| AppError::io("read the mods directory", e))?
        {
            let entry = entry.map_err(|e| AppError::io("read a mod entry", e))?;
            let kind = entry
                .file_type()
                .map_err(|e| AppError::io("inspect a mod file", e))?;
            if kind.is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("jar"))
            {
                mods.push(InstalledMod {
                    file_name: entry.file_name().to_string_lossy().into_owned(),
                    size_bytes: entry
                        .metadata()
                        .map_err(|e| AppError::io("inspect mod size", e))?
                        .len(),
                });
            }
        }
        mods.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        Ok(mods)
    }
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
