use super::{
    download::{self, FileSpec},
    metadata::{segment, Artifact},
};
use crate::{
    errors::{AppError, Result},
    models::{InstanceInput, Loader},
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
