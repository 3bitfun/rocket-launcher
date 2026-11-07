use super::{
    download::{self, FileSpec},
    fabric::FabricLoaderVersion,
    install::Installed,
    java::{self, Java},
    metadata::{self, Artifact, Platform, Version},
};
use crate::errors::{AppError, Result};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

const MAVEN: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge";
const MAX_JAR: u64 = 256 * 1024 * 1024;

fn error(message: impl Into<String>) -> AppError {
    AppError::new("FORGE", message)
}

fn supported(game: &str) -> bool {
    let parts = game
        .split('.')
        .map(str::parse::<u32>)
        .collect::<std::result::Result<Vec<_>, _>>();
    match parts.as_deref() {
        Ok([1, 20, patch]) => *patch >= 1,
        Ok([1, minor] | [1, minor, _]) => *minor >= 21,
        Ok([year, _] | [year, _, _]) => *year >= 26,
        _ => false,
    }
}

fn validate_loader(version: &str) -> Result<()> {
    let parts: Vec<_> = version.split('.').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|part| part.parse::<u32>().is_err() || !part.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(error("Invalid Forge loader version."));
    }
    Ok(())
}

#[derive(Deserialize)]
struct Promotions {
    promos: HashMap<String, String>,
}

pub async fn loader_versions(game: &str, timeout: u64) -> Result<Vec<FabricLoaderVersion>> {
    if !supported(game) {
        return Ok(Vec::new());
    }
    let data: Promotions = download::client(timeout)?
        .get("https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json")
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|e| error(format!("Could not fetch Forge versions: {e}")))?
        .json()
        .await
        .map_err(|e| error(format!("Invalid Forge version list: {e}")))?;
    let mut versions = Vec::new();
    for channel in ["recommended", "latest"] {
        if let Some(version) = data.promos.get(&format!("{game}-{channel}")) {
            validate_loader(version)?;
            if !versions
                .iter()
                .any(|item: &FabricLoaderVersion| &item.version == version)
            {
                versions.push(FabricLoaderVersion {
                    version: version.clone(),
                    stable: channel == "recommended",
                });
            }
        }
    }
    Ok(versions)
}

pub fn validate_instance(game: &str, loader: &str) -> Result<()> {
    if !supported(game) {
        return Err(error(
            "Forge is supported for Minecraft 1.20.1 and newer releases.",
        ));
    }
    validate_loader(loader)
}

fn profile_id(game: &str, loader: &str) -> Result<String> {
    validate_instance(game, loader)?;
    Ok(format!("{game}-forge-{loader}"))
}

fn profile(directory: &Path, id: &str, game: &str) -> Result<Version> {
    let path = directory
        .join("versions")
        .join(id)
        .join(format!("{id}.json"));
    let size = fs::metadata(&path)
        .map_err(|e| AppError::io("inspect Forge profile", e))?
        .len();
    if size > 2 * 1024 * 1024 {
        return Err(error("The Forge launch profile is too large."));
    }
    let bytes = fs::read(&path).map_err(|e| AppError::io("read Forge profile", e))?;
    let version: Version =
        serde_json::from_slice(&bytes).map_err(|e| error(format!("Invalid Forge profile: {e}")))?;
    if version.id != id
        || version.inherits_from.as_deref() != Some(game)
        || version.main_class.is_none()
    {
        return Err(error(
            "The installed Forge profile does not match this instance.",
        ));
    }
    Ok(version)
}

fn library_paths(
    directory: &Path,
    child: &Version,
    platform: &Platform,
    full: bool,
) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for library in &child.libraries {
        if !metadata::allowed(library.rules.as_deref(), platform)? {
            continue;
        }
        let artifact = library
            .downloads
            .artifact
            .as_ref()
            .ok_or_else(|| error("Forge library metadata is incomplete."))?;
        let relative = artifact
            .path
            .as_deref()
            .ok_or_else(|| error("A Forge library has no path."))?;
        let path = directory
            .join("libraries")
            .join(metadata::relative(relative)?);
        let size_ok = path
            .metadata()
            .is_ok_and(|meta| meta.is_file() && meta.len() == artifact.size);
        if !size_ok
            || (full && !download::verify(&path, &artifact.sha1, Some(artifact.size), MAX_JAR))
        {
            return Err(AppError::new(
                "MISSING_FILES",
                format!("Repair Forge to restore {relative}."),
            ));
        }
        paths.push(path);
    }
    Ok(paths)
}

pub fn verify(
    root: &Path,
    game: &str,
    loader: &str,
    platform: &Platform,
    vanilla: Installed,
    full: bool,
) -> Result<Option<Installed>> {
    let id = profile_id(game, loader)?;
    let directory = root.join("forge").join(&id);
    if !directory.exists() {
        return Ok(None);
    }
    let child = profile(&directory, &id, game)?;
    if full {
        let hash = fs::read_to_string(directory.join("profile.sha1"))
            .map_err(|e| AppError::io("read the Forge profile checksum", e))?;
        let path = directory
            .join("versions")
            .join(&id)
            .join(format!("{id}.json"));
        if !download::verify(&path, hash.trim(), None, 2 * 1024 * 1024) {
            return Err(AppError::new(
                "MISSING_FILES",
                "Repair Forge to restore its launch profile.",
            ));
        }
    }
    let mut classpath = library_paths(&directory, &child, platform, full)?;
    let version = vanilla.version.clone().inherit(child);
    for library in &version.libraries {
        if !metadata::allowed(library.rules.as_deref(), platform)? {
            continue;
        }
        if let Some(relative) = library
            .downloads
            .artifact
            .as_ref()
            .and_then(|artifact| artifact.path.as_deref())
        {
            let relative = metadata::relative(relative)?;
            if !classpath.contains(&directory.join("libraries").join(&relative)) {
                let path = vanilla.libraries.join(relative);
                if vanilla.classpath.contains(&path) {
                    classpath.push(path);
                }
            }
        }
    }
    classpath.extend(vanilla.classpath.last().cloned());
    Ok(Some(Installed {
        version,
        classpath,
        libraries: directory.join("libraries"),
        ..vanilla
    }))
}

pub async fn install(
    root: &Path,
    game: &str,
    loader: &str,
    platform: &Platform,
    java: &Java,
    timeout: u64,
) -> Result<()> {
    let id = profile_id(game, loader)?;
    let coordinate = format!("{game}-{loader}");
    let filename = format!("forge-{coordinate}-installer.jar");
    let url = format!("{MAVEN}/{coordinate}/{filename}");
    let client = download::client(timeout)?;
    let hash = client
        .get(format!("{url}.sha1"))
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|e| error(format!("Could not fetch the Forge installer checksum: {e}")))?
        .text()
        .await
        .map_err(|e| error(format!("Could not read the Forge installer checksum: {e}")))?;
    let parent = root.join("forge");
    fs::create_dir_all(&parent).map_err(|e| AppError::io("create Forge cache", e))?;
    let staging =
        tempfile::tempdir_in(&parent).map_err(|e| AppError::io("stage Forge installation", e))?;
    let stage = staging.path();
    let mut installer = FileSpec::new(
        PathBuf::from(&filename),
        Artifact {
            url,
            sha1: hash.trim().into(),
            size: u64::MAX,
            path: None,
        },
    )?;
    installer.max_size = MAX_JAR;
    download::ensure(&client, stage, &installer, &|_| {}).await?;
    let base = PathBuf::from("versions")
        .join(game)
        .join(format!("{game}.jar"));
    super::install::copy_atomic(&root.join(&base), &stage.join(&base))?;
    fs::write(stage.join("launcher_profiles.json"), r#"{"profiles":{}}"#)
        .map_err(|e| AppError::io("create the Forge staging profile", e))?;
    let log_path = stage.join("installer-output.log");
    let log =
        fs::File::create(&log_path).map_err(|e| AppError::io("open the Forge installer log", e))?;
    let stderr = log
        .try_clone()
        .map_err(|e| AppError::io("open the Forge installer error log", e))?;
    let mut command = tokio::process::Command::new(&java.executable);
    java::configure(&mut command);
    command
        .current_dir(stage)
        .arg("-jar")
        .arg(&filename)
        .arg("--installClient")
        .arg(".")
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr));
    let status = tokio::time::timeout(Duration::from_secs(600), command.status())
        .await
        .map_err(|_| error("The Forge installer timed out."))?
        .map_err(|e| AppError::io("run the Forge installer", e))?;
    if !status.success() {
        return Err(error(format!("The Forge installer exited with {status}.")));
    }
    let child = profile(stage, &id, game)?;
    library_paths(stage, &child, platform, true)?;
    let profile_path = stage.join("versions").join(&id).join(format!("{id}.json"));
    let hash = download::file_hash(&profile_path)
        .map_err(|e| AppError::io("hash the Forge profile", e))?;
    fs::write(stage.join("profile.sha1"), hash)
        .map_err(|e| AppError::io("record the Forge profile checksum", e))?;
    for file in [stage.join(&filename), stage.join(base)] {
        let _ = fs::remove_file(file);
    }
    let destination = parent.join(id);
    let backup = tempfile::tempdir_in(&parent)
        .map_err(|e| AppError::io("stage Forge cache replacement", e))?;
    let previous = backup.path().join("previous");
    if destination.exists() {
        fs::rename(&destination, &previous)
            .map_err(|e| AppError::io("back up Forge installation", e))?;
    }
    if let Err(e) = fs::rename(stage, &destination) {
        if previous.exists() {
            if let Err(restore) = fs::rename(&previous, &destination) {
                let saved = backup.keep();
                return Err(error(format!(
                    "Forge repair could not restore its backup at {}: {restore}",
                    saved.display()
                )));
            }
        }
        return Err(AppError::io("publish Forge installation", e));
    }
    Ok(())
}
