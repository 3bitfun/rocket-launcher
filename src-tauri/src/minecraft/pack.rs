use super::{
    download::{self, FileSpec},
    metadata::{segment, Artifact},
};
use crate::{
    errors::{AppError, Result},
    models::{Instance, InstanceInput, Loader},
};
use futures_util::{stream, StreamExt, TryStreamExt};
use serde::Deserialize;
use std::path::{Path, PathBuf};

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

    pub async fn download(&self, directory: &Path, timeout: u64) -> Result<()> {
        let client = download::client(timeout)?;
        let specs = self
            .files
            .iter()
            .map(|file| {
                let filename = file
                    .path
                    .strip_prefix("mods/")
                    .filter(|name| name.ends_with(".jar"))
                    .ok_or_else(|| AppError::new("PACK", "Invalid Rocket Pack file path."))?;
                segment(filename)?;
                FileSpec::new(
                    PathBuf::from(&file.path),
                    Artifact {
                        path: None,
                        url: file.url.clone(),
                        sha1: file.sha1.clone(),
                        size: file.size,
                    },
                )
            })
            .collect::<Result<Vec<_>>>()?;
        stream::iter(specs)
            .map(|spec| {
                let client = &client;
                async move { download::ensure(client, directory, &spec, &|_| {}).await }
            })
            .buffer_unordered(6)
            .try_collect::<Vec<_>>()
            .await?;
        Ok(())
    }
}

pub async fn install_for_version(
    game: &str,
    directory: &Path,
    ram: u32,
    timeout: u64,
) -> Result<InstanceInput> {
    let pinned = Pack::recommended()?;
    if game == pinned.minecraft {
        pinned.download(directory, timeout).await?;
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
    for project in ["AANobbMI", "gvQqBUqZ", "P7dR8mSH"] {
        if let Some(version) =
            super::modrinth::preferred_version(&instance, project, timeout).await?
        {
            super::modrinth::stage(&instance, project, &version, timeout)
                .await?
                .publish(directory)?;
            installed += 1;
        }
    }
    if installed == 0 {
        return Err(AppError::new(
            "PACK",
            "No Rocket Pack mods are available for this Minecraft version.",
        ));
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
    pack.download(folder.path(), 30).await.unwrap();
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
