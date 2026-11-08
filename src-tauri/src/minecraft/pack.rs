use super::{
    download::{self, FileSpec},
    metadata::{segment, Artifact},
};
use crate::{
    errors::{AppError, Result},
    models::{Instance, InstanceInput, Loader},
};
use futures_util::{stream, StreamExt};
use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

#[derive(Deserialize)]
pub struct Pack {
    pub minecraft: String,
    pub fabric: String,
    files: Vec<PackFile>,
}

#[derive(Deserialize)]
struct PackFile {
    path: String,
    url: String,
    sha1: String,
    size: u64,
}

impl Pack {
    pub fn recommended() -> Result<Self> {
        serde_json::from_str(include_str!("rocket-pack.json"))
            .map_err(|e| AppError::new("PACK", format!("Could not read Rocket Pack: {e}")))
    }

    pub fn instance_input(self, ram_mb: u32) -> InstanceInput {
        InstanceInput {
            name: "Rocket Pack".into(),
            version: self.minecraft,
            loader: Loader::Fabric,
            loader_version: Some(self.fabric),
            ram_mb,
            java_path: None,
            parent_directory: None,
        }
    }

    pub async fn download(
        &self,
        directory: &Path,
        cache: &Path,
        timeout: u64,
        progress: &(impl Fn(usize, usize) + Sync),
    ) -> Result<()> {
        let client = download::client(timeout)?;
        let specs = self
            .files
            .iter()
            .map(|file| {
                let (folder, filename) = file
                    .path
                    .split_once('/')
                    .filter(|(folder, name)| match *folder {
                        "mods" => name.ends_with(".jar"),
                        "resourcepacks" | "shaderpacks" => name.ends_with(".zip"),
                        _ => false,
                    })
                    .ok_or_else(|| AppError::new("PACK", "Invalid Rocket Pack file path."))?;
                segment(filename)?;
                FileSpec::new(
                    PathBuf::from(folder).join(filename),
                    Artifact {
                        path: None,
                        url: file.url.clone(),
                        sha1: file.sha1.clone(),
                        size: file.size,
                    },
                )
            })
            .collect::<Result<Vec<_>>>()?;
        let total = specs.len();
        let completed = AtomicUsize::new(0);
        progress(0, total);
        let results = stream::iter(specs)
            .map(|spec| {
                let client = &client;
                let completed = &completed;
                async move {
                    let mut cached = spec.clone();
                    cached.relative = PathBuf::from(spec.artifact.sha1.to_ascii_lowercase());
                    download::ensure(client, cache, &cached, &|_| {}).await?;
                    let source = cache.join(&cached.relative);
                    let target = directory.join(&spec.relative);
                    tokio::task::spawn_blocking(move || {
                        super::install::copy_atomic(&source, &target)
                    })
                    .await
                    .map_err(|e| {
                        AppError::new("PACK", format!("Could not copy a cached pack file: {e}"))
                    })??;
                    progress(completed.fetch_add(1, Ordering::Relaxed) + 1, total);
                    Ok(())
                }
            })
            .buffer_unordered(6)
            .collect::<Vec<Result<()>>>()
            .await;
        results.into_iter().collect()
    }
}

pub async fn install_for_version(
    game: &str,
    directory: &Path,
    cache: &Path,
    ram: u32,
    timeout: u64,
    progress: impl Fn(usize, usize) + Sync,
) -> Result<InstanceInput> {
    let pinned = Pack::recommended()?;
    if game == pinned.minecraft {
        pinned
            .download(directory, cache, timeout, &progress)
            .await?;
        super::rocket_client::add_to_pack(directory, game)?;
        return Ok(pinned.instance_input(ram));
    }
    let loaders = super::fabric::loader_versions(game, timeout).await?;
    let loader = loaders
        .iter()
        .find(|loader| loader.stable)
        .or_else(|| loaders.first())
        .ok_or_else(|| AppError::new("PACK", "Fabric has no loader for this Minecraft version."))?;
    let instance = Instance {
        id: "rocket-pack".into(),
        name: format!("Rocket Pack {game}"),
        version: game.into(),
        version_type: "release".into(),
        loader: Loader::Fabric,
        loader_version: Some(loader.version.clone()),
        game_directory: directory.into(),
        ram_mb: ram,
        java_path: None,
        created_at: 0,
        updated_at: 0,
        last_selected_at: 0,
    };
    // Sodium, Lithium, and Fabric API are selected independently for each Minecraft version.
    let mut installed = 0;
    let mut fabric_api = false;
    for project in ["AANobbMI", "gvQqBUqZ", "P7dR8mSH"] {
        if let Some(version) =
            super::modrinth::preferred_version(&instance, project, timeout).await?
        {
            super::modrinth::stage(&instance, project, &version, timeout)
                .await?
                .publish(directory)?;
            installed += 1;
            fabric_api |= project == "P7dR8mSH";
        }
    }
    if installed == 0 {
        return Err(AppError::new(
            "PACK",
            "No Rocket Pack mods are available for this Minecraft version.",
        ));
    }
    if fabric_api {
        super::rocket_client::add_to_pack(directory, game)?;
    }
    Ok(InstanceInput {
        name: instance.name,
        version: instance.version,
        loader: instance.loader,
        loader_version: instance.loader_version,
        ram_mb: ram,
        java_path: None,
        parent_directory: None,
    })
}

#[cfg(all(test, windows, debug_assertions))]
#[tokio::test]
#[ignore = "downloads and starts Minecraft using the recommended pack and local Java"]
async fn live_rocket_pack_renders() {
    use super::{fabric, install, java, launch, metadata::Platform, versions};
    use crate::storage::Store;

    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(root.path().join("launcher")).unwrap();
    let folder = tempfile::tempdir_in(root.path()).unwrap();
    let pack = Pack::recommended().unwrap();
    pack.download(
        folder.path(),
        &root.path().join("pack-cache"),
        30,
        &|_, _| {},
    )
    .await
    .unwrap();
    let snapshot = store
        .create_pack_instance(pack.instance_input(4096), folder)
        .unwrap();
    let instance = &snapshot.instances[0].instance;
    let game_version = &instance.version;
    let loader = instance.loader_version.as_deref().unwrap();
    let cache = root.path().join("game-cache");
    let catalogue = root.path().join("versions.json");
    versions::load_catalog(&catalogue, true, 30).await.unwrap();
    let manifest = versions::read_cache(&catalogue).unwrap().manifest;
    install::install(&cache, &manifest, game_version, "test", 30, |_| {})
        .await
        .unwrap();
    fabric::install(&cache, game_version, loader, "test", 30, |_| {})
        .await
        .unwrap();
    let platform = Platform::windows().unwrap();
    let vanilla = install::verify_installed(&cache, game_version, &platform)
        .unwrap()
        .unwrap();
    let layered = fabric::verify(&cache, game_version, loader, &platform, &vanilla, true)
        .unwrap()
        .unwrap();
    let java = java::resolve(
        None,
        root.path(),
        layered.version.java_major(),
        platform.arch,
    )
    .await
    .unwrap();
    launch::smoke_render(&layered, instance, &platform, java).await;
}
