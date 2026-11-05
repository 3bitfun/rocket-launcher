use crate::{
    errors::{AppError, Result},
    models::*,
};
use std::{
    fs,
    io::Write,
    path::{Component, Path},
};

pub struct Store {
    pub config: Configuration,
    pub paths: AppPaths,
}

pub fn atomic_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::new("PATH", "The file has no parent directory."))?;
    fs::create_dir_all(parent)
        .map_err(|e| AppError::io("create the configuration directory", e))?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| AppError::io("create a temporary file", e))?;
    serde_json::to_writer_pretty(&mut temp, value)
        .map_err(|e| AppError::io("serialize configuration", e))?;
    temp.write_all(b"\n")
        .map_err(|e| AppError::io("write configuration", e))?;
    temp.as_file()
        .sync_all()
        .map_err(|e| AppError::io("flush configuration", e))?;
    // Same-directory atomic replacement, including existing destinations on Windows.
    temp.persist(path)
        .map_err(|e| AppError::io("save configuration atomically", e.error))?;
    Ok(())
}

pub fn validate_path(path: &Path) -> Result<()> {
    if !path.is_absolute() || path.components().any(|p| matches!(p, Component::ParentDir)) {
        return Err(AppError::new(
            "VALIDATION",
            "Choose an absolute path without '..' components.",
        ));
    }
    #[cfg(windows)]
    if path.to_string_lossy().starts_with(r"\\") {
        return Err(AppError::new(
            "VALIDATION",
            "Choose a local drive path. Network and device paths are not supported.",
        ));
    }
    Ok(())
}

pub fn validate_ram(ram: u32) -> Result<()> {
    if !(1024..=65536).contains(&ram) || ram % 256 != 0 {
        return Err(AppError::new(
            "VALIDATION",
            "Memory must be 1–64 GB, in steps of 256 MB.",
        ));
    }
    Ok(())
}

pub fn validate_java(path: &Option<std::path::PathBuf>) -> Result<()> {
    if let Some(path) = path {
        validate_path(path)?;
        if !path.is_file() {
            return Err(AppError::new(
                "VALIDATION",
                "The selected Java executable does not exist.",
            ));
        }
        #[cfg(windows)]
        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
        {
            return Err(AppError::new(
                "VALIDATION",
                "Choose a Java .exe executable.",
            ));
        }
    }
    Ok(())
}

pub fn validate_settings(settings: &Settings) -> Result<()> {
    let accent = settings.accent_color.as_bytes();
    if accent.len() != 7 || accent[0] != b'#' || !accent[1..].iter().all(u8::is_ascii_hexdigit) {
        return Err(AppError::new(
            "VALIDATION",
            "Accent colour must be a six-digit hex colour, such as #FF704D.",
        ));
    }
    validate_ram(settings.default_ram_mb)?;
    validate_path(&settings.minecraft_directory)?;
    validate_java(&settings.java_path)?;
    if !(5..=120).contains(&settings.request_timeout_seconds) {
        return Err(AppError::new(
            "VALIDATION",
            "Network timeout must be between 5 and 120 seconds.",
        ));
    }
    Ok(())
}

impl Store {
    pub fn open(data_dir: std::path::PathBuf) -> Result<Self> {
        let paths = AppPaths {
            cache: data_dir.join("cache"),
            logs: data_dir.join("logs"),
            config: data_dir.join("launcher.json"),
            data: data_dir.clone(),
        };
        for directory in [&paths.data, &paths.cache, &paths.logs] {
            fs::create_dir_all(directory)
                .map_err(|e| AppError::io("create launcher directories", e))?;
        }
        let config = if paths.config.exists() {
            let bytes =
                fs::read(&paths.config).map_err(|e| AppError::io("read launcher.json", e))?;
            let config: Configuration = serde_json::from_slice(&bytes).map_err(|e| AppError::new(
                "CONFIG_INVALID", format!("launcher.json could not be read. Your file has been preserved at {}. Restore a valid copy and restart. Details: {e}", paths.config.display())))?;
            if config.schema_version != 1 {
                return Err(AppError::new("CONFIG_VERSION", "This configuration was created by an unsupported launcher version. The file has not been changed."));
            }
            // Missing external Java or game directories must not prevent recovery in Settings.
            validate_ram(config.settings.default_ram_mb)?;
            validate_path(&config.settings.minecraft_directory)?;
            let mut ids = std::collections::HashSet::new();
            for instance in &config.instances {
                uuid::Uuid::parse_str(&instance.id).map_err(|_| {
                    AppError::new("CONFIG_INVALID", "An instance has an invalid ID.")
                })?;
                if !ids.insert(instance.id.clone()) {
                    return Err(AppError::new(
                        "CONFIG_INVALID",
                        "Duplicate instance IDs were found. Configuration has been preserved.",
                    ));
                }
                validate_path(&instance.game_directory)?;
                validate_ram(instance.ram_mb)?;
            }
            if config
                .active_instance_id
                .as_ref()
                .is_some_and(|id| !ids.contains(id))
            {
                return Err(AppError::new(
                    "CONFIG_INVALID",
                    "The selected instance is missing from configuration.",
                ));
            }
            config
        } else {
            let config = Configuration {
                schema_version: 1,
                settings: Settings::defaults(&data_dir),
                instances: vec![],
                active_instance_id: None,
            };
            atomic_json(&paths.config, &config)?;
            config
        };
        Ok(Self { config, paths })
    }

    pub fn commit(&mut self, config: Configuration) -> Result<AppSnapshot> {
        atomic_json(&self.paths.config, &config)?;
        self.config = config;
        Ok(self.snapshot())
    }

    pub fn snapshot(&self) -> AppSnapshot {
        AppSnapshot {
            app_version: env!("CARGO_PKG_VERSION"),
            settings: self.config.settings.clone(),
            instances: self
                .config
                .instances
                .iter()
                .map(InstanceView::from)
                .collect(),
            active_instance_id: self.config.active_instance_id.clone(),
            paths: self.paths.clone(),
        }
    }

    pub fn save_settings(&mut self, settings: Settings) -> Result<AppSnapshot> {
        validate_settings(&settings)?;
        let mut next = self.config.clone();
        next.settings = settings;
        let result = self.commit(next)?;
        tracing::info!(event = "settings_saved");
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_survive_reopen_and_invalid_changes_do_not_write() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = Store::open(temp.path().into()).unwrap();
        let mut settings = store.config.settings.clone();
        settings.accent_color = "#6495ED".into();
        settings.show_snapshots = true;
        settings.animations = false;
        store.save_settings(settings.clone()).unwrap();
        settings.accent_color = "url(invalid)".into();
        assert!(store.save_settings(settings).is_err());
        let reopened = Store::open(temp.path().into()).unwrap();
        assert_eq!(reopened.config.settings.accent_color, "#6495ED");
        assert!(reopened.config.settings.show_snapshots);
        assert!(!reopened.config.settings.animations);
    }

    #[test]
    fn corrupt_configuration_is_preserved() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("launcher.json"), "broken JSON").unwrap();
        assert!(Store::open(temp.path().into()).is_err());
        assert_eq!(
            fs::read_to_string(temp.path().join("launcher.json")).unwrap(),
            "broken JSON"
        );
    }

    #[test]
    fn unsafe_paths_and_bad_memory_are_rejected() {
        assert!(validate_path(Path::new("../somewhere")).is_err());
        assert!(validate_ram(0).is_err());
        assert!(validate_ram(4097).is_err());
        assert!(validate_ram(4096).is_ok());
    }
}
