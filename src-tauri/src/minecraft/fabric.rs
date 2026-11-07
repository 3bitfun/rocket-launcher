use super::{
    download::{self, FileSpec},
    install::{Installed, Progress, Stage},
    metadata::*,
};
use crate::{
    errors::{AppError, Result},
    storage::atomic_json,
};
use futures_util::{stream, TryStreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering::Relaxed},
};

const META_HOST: &str = "https://meta.fabricmc.net/v2";
const MAX_PROFILE_BYTES: usize = 4 * 1024 * 1024;
const MAX_LIBRARY_BYTES: u64 = 512 * 1024 * 1024;

/// Libraries whose size is unknown are represented by this sentinel.
const UNKNOWN_SIZE: u64 = u64::MAX;

/// Does a library contribute to the download progress meter? Unknown sizes cannot
/// (each is still capped by `MAX_LIBRARY_BYTES` per file), so they count as zero.
fn meter_bytes(size: u64) -> u64 {
    if size == UNKNOWN_SIZE {
        0
    } else {
        size
    }
}

/// A compatible Fabric loader entry from `meta.fabricmc.net`; newest first.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricLoaderVersion {
    pub version: String,
    pub stable: bool,
}

#[derive(Debug, Deserialize)]
struct LoaderListEntry {
    loader: FabricLoaderVersion,
}

/// Loader versions are composed into URLs and profile names. Keep them plain identifiers.
pub fn validate_component(value: &str) -> Result<()> {
    if value.is_empty()
        || matches!(value, "." | "..")
        || value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'+' | b'-' | b'_'))
    {
        return Err(invalid(format!(
            "Invalid Fabric version identifier: {value}"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LibraryRecord {
    name: String,
    maven: String,
    url: String,
    path: PathBuf,
    sha1: String,
    size: u64,
}

/// The on-disk location of a Fabric library, rebuilt from its validated maven
/// coordinates. Receipts store the path with the host platform's separator
/// (backslashes on Windows), so it must not be re-validated as a slash path.
fn library_path(root: &Path, record: &LibraryRecord) -> Result<PathBuf> {
    Ok(root.join("libraries").join(relative(&record.maven)?))
}

#[derive(Serialize, Deserialize)]
struct Receipt {
    game_version: String,
    loader_version: String,
    profile_id: String,
    profile_sha1: String,
    main_class: String,
    libraries: Vec<LibraryRecord>,
}

struct Meter<'a, F> {
    instance_id: &'a str,
    version: &'a str,
    send: &'a F,
    transferred: AtomicU64,
    verified: AtomicU64,
    total: Option<u64>,
}
impl<F: Fn(Progress) + Sync> Meter<'_, F> {
    fn emit(&self, stage: Stage, current: impl Into<String>) {
        (self.send)(Progress {
            instance_id: self.instance_id.into(),
            version: self.version.into(),
            stage,
            current: current.into(),
            transferred_bytes: self.transferred.load(Relaxed),
            verified_bytes: self.verified.load(Relaxed),
            total_bytes: self.total,
            bytes_per_second: 0,
            error: None,
        });
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileLibrary {
    name: String,
    url: Option<String>,
    sha1: Option<String>,
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Profile {
    id: String,
    inherits_from: Option<String>,
    main_class: Option<String>,
    arguments: Option<Arguments>,
    minecraft_arguments: Option<String>,
    #[serde(default)]
    libraries: Vec<ProfileLibrary>,
    #[serde(rename = "type")]
    kind: Option<String>,
}

fn profile_id(game_version: &str, loader_version: &str) -> Result<String> {
    validate_component(game_version)?;
    validate_component(loader_version)?;
    Ok(format!("fabric-loader-{loader_version}-{game_version}"))
}

fn maven_path(name: &str) -> Result<String> {
    let bad = || invalid(format!("Invalid Fabric library name: {name}"));
    let mut parts = name.split(':');
    let group = parts.next().filter(|g| !g.is_empty()).ok_or_else(bad)?;
    let artifact = parts.next().filter(|a| !a.is_empty()).ok_or_else(bad)?;
    let version = parts.next().filter(|v| !v.is_empty()).ok_or_else(bad)?;
    let (classifier, extension) = match parts.next() {
        None => (None, "jar"),
        Some(rest) => {
            let (classifier, extension) = rest.split_once('@').unwrap_or((rest, "jar"));
            if extension.len() > 16
                || !extension
                    .bytes()
                    .all(|byte| byte.is_ascii_alphabetic() || byte == b'_')
            {
                return Err(bad());
            }
            (Some(classifier), extension)
        }
    };
    if parts.next().is_some() {
        return Err(bad());
    }
    let component = |value: &str| {
        !value.is_empty()
            && value.len() <= 128
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'+' | b'-' | b'_')
            })
    };
    for part in [group, artifact, version].into_iter().chain(classifier) {
        if !component(part) {
            return Err(bad());
        }
    }
    let file = match classifier {
        Some(classifier) => format!("{artifact}-{version}-{classifier}.{extension}"),
        None => format!("{artifact}-{version}.{extension}"),
    };
    Ok(format!(
        "{}/{}/{}/{}",
        group.replace('.', "/"),
        artifact,
        version,
        file
    ))
}

fn library_url(base: &str, maven: &str) -> Result<String> {
    if base.is_empty() {
        return Err(invalid("Fabric library has no download URL."));
    }
    Ok(format!("{}/{maven}", base.trim_end_matches('/')))
}

pub async fn loader_versions(game_version: &str, timeout: u64) -> Result<Vec<FabricLoaderVersion>> {
    validate_component(game_version)?;
    let client = download::client(timeout)?;
    let bytes = fetch_bytes(
        &client,
        &format!("{META_HOST}/versions/loader/{game_version}"),
        "Fabric's loader service",
    )
    .await?;
    let mut loaders: Vec<FabricLoaderVersion> =
        serde_json::from_slice::<Vec<LoaderListEntry>>(&bytes)
            .map_err(|e| {
                AppError::new(
                    "GAME_METADATA",
                    format!("Could not parse the Fabric loader list: {e}"),
                )
            })?
            .into_iter()
            .map(|entry| entry.loader)
            .collect();
    if loaders.len() > 1000 {
        return Err(invalid("Fabric returned an oversized loader list."));
    }
    for loader in &loaders {
        validate_component(&loader.version)?;
    }
    loaders.dedup_by(|a, b| a.version == b.version);
    Ok(loaders)
}

pub async fn game_versions(timeout: u64) -> Result<Vec<String>> {
    #[derive(Deserialize)]
    struct Game {
        version: String,
        stable: bool,
    }
    let client = download::client(timeout)?;
    let bytes = fetch_bytes(&client, &format!("{META_HOST}/versions/game"), "Fabric's game catalogue").await?;
    let games: Vec<Game> = serde_json::from_slice(&bytes).map_err(|e| invalid(format!("Invalid Fabric game catalogue: {e}")))?;
    Ok(games.into_iter().filter(|game| game.stable).map(|game| game.version).collect())
}

async fn fetch_bytes(client: &reqwest::Client, url: &str, label: &str) -> Result<Vec<u8>> {
    let url: reqwest::Url = url.parse().map_err(|_| invalid("Invalid Fabric URL."))?;
    let fabric_meta = url.host_str() == Some("meta.fabricmc.net");
    let network = |error: reqwest::Error| {
        AppError::new(
            "NETWORK",
            format!(
                "Could not reach {label}. Check your connection and try again. Details: {error}"
            ),
        )
    };
    let mut response = client.get(url).send().await.map_err(network)?;
    let status = response.status();
    if !status.is_success() {
        let hint = if fabric_meta {
            " No compatible Fabric loader may exist for this Minecraft version."
        } else {
            ""
        };
        return Err(AppError::new(
            "VERSION_UNKNOWN",
            format!("Fabric returned HTTP {status} from {label}.{hint}"),
        ));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_PROFILE_BYTES as u64)
    {
        return Err(invalid("Fabric returned an oversized response."));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(network)? {
        if bytes.len() + chunk.len() > MAX_PROFILE_BYTES {
            return Err(invalid("Fabric returned an oversized response."));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let file = fs::File::open(path).map_err(|e| AppError::io("open Fabric metadata", e))?;
    serde_json::from_reader(file.take(MAX_PROFILE_BYTES as u64)).map_err(|e| {
        invalid(format!(
            "Could not read Fabric metadata {}: {e}",
            path.display()
        ))
    })
}

fn missing(path: &Path) -> AppError {
    AppError::new(
        "MISSING_FILES",
        format!(
            "Missing or corrupt: {}. Install again to repair it.",
            path.display()
        ),
    )
}

/// Install the Fabric loader for `game_version`, assuming the vanilla game version is
/// already fully installed (shared cache, shared assets, shared libraries).
pub async fn install(
    root: &Path,
    game_version: &str,
    loader_version: &str,
    instance_id: &str,
    timeout: u64,
    send: impl Fn(Progress) + Sync,
) -> Result<()> {
    let id = profile_id(game_version, loader_version)?;
    let client = download::client(timeout)?;
    let mut meter = Meter {
        instance_id,
        version: game_version,
        send: &send,
        transferred: AtomicU64::new(0),
        verified: AtomicU64::new(0),
        total: None,
    };
    meter.emit(Stage::Downloading, "Resolving the Fabric loader profile");
    let bytes = fetch_bytes(
        &client,
        &format!("{META_HOST}/versions/loader/{game_version}/{loader_version}/profile/json"),
        "Fabric's loader service",
    )
    .await?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| invalid(format!("Could not parse the Fabric loader profile: {e}")))?;
    let profile: Profile = serde_json::from_value(value.clone())
        .map_err(|e| invalid(format!("Could not parse the Fabric loader profile: {e}")))?;
    if profile.id != id {
        return Err(invalid(
            "The Fabric profile does not match the requested loader version.",
        ));
    }
    if profile
        .inherits_from
        .as_deref()
        .is_some_and(|parent| parent != game_version)
    {
        return Err(invalid(
            "The Fabric profile inherits from an unexpected Minecraft version.",
        ));
    }
    let main_class = profile
        .main_class
        .as_deref()
        .filter(|main_class| !main_class.is_empty())
        .ok_or_else(|| invalid("The Fabric profile is missing its main class."))?
        .to_owned();

    meter.emit(Stage::Downloading, "Resolving Fabric loader libraries");
    let mut records = Vec::with_capacity(profile.libraries.len());
    for library in &profile.libraries {
        let maven = maven_path(&library.name)?;
        let relative = PathBuf::from("libraries").join(relative(&maven)?);
        let url = library_url(
            library
                .url
                .as_deref()
                .unwrap_or("https://libraries.minecraft.net/"),
            &maven,
        )?;
        let sha1 = match library.sha1 {
            Some(ref hash) if download::check_hash(hash).is_ok() => hash.to_ascii_lowercase(),
            _ => fetch_library_sha1(&client, &url).await?,
        };
        let size = library.size.unwrap_or(UNKNOWN_SIZE);
        records.push(LibraryRecord {
            name: library.name.clone(),
            maven,
            url,
            path: relative,
            sha1,
            size,
        });
    }
    // Build the download specs from the resolved records; unknown-size artifacts get a
    // generous cap and are verified by SHA-1 through the same engine as vanilla Minecraft.
    let specs = records
        .iter()
        .map(|record| -> Result<(FileSpec, u64)> {
            let mut spec = FileSpec::new(
                record.path.clone(),
                Artifact {
                    url: record.url.clone(),
                    sha1: record.sha1.clone(),
                    size: record.size,
                    path: Some(record.maven.clone()),
                },
            )?;
            if spec.artifact.size == UNKNOWN_SIZE {
                spec.max_size = MAX_LIBRARY_BYTES;
            }
            let size = spec.artifact.size;
            Ok((spec, size))
        })
        .collect::<Result<Vec<_>>>()?;

    // Libraries without a declared size carry the `u64::MAX` sentinel; they cannot be
    // counted against the meter's byte total (each one is capped by MAX_LIBRARY_BYTES
    // independently), so unknown sizes simply do not advance the progress bar.
    meter.total = Some(
        specs
            .iter()
            .try_fold(0u64, |sum, (_, size)| sum.checked_add(meter_bytes(*size)))
            .ok_or_else(|| invalid("The Fabric download total is too large."))?,
    );
    meter.emit(Stage::Downloading, "Downloading Fabric loader libraries");
    {
        let meter = &meter;
        let client = &client;
        stream::iter((0..records.len()).map(Ok::<_, AppError>))
            .try_for_each_concurrent(8, |index| {
                let record = &records[index];
                let spec = &specs[index].0;
                async move {
                    download::ensure(client, root, spec, &|bytes| {
                        meter.transferred.fetch_add(bytes, Relaxed);
                    })
                    .await?;
                    if record.size != UNKNOWN_SIZE {
                        meter.verified.fetch_add(record.size, Relaxed);
                    }
                    Ok(())
                }
            })
            .await?;
    }

    let directory = root.join("versions").join(segment(&id)?);
    let profile_path = directory.join(format!("{id}.json"));
    fs::create_dir_all(&directory).map_err(|e| AppError::io("create Fabric version folder", e))?;
    atomic_json(&profile_path, &value)?;
    let profile_sha1 = {
        let path = profile_path.clone();
        tokio::task::spawn_blocking(move || {
            download::file_hash(&path).map_err(|e| AppError::io("hash the Fabric profile", e))
        })
        .await
        .map_err(|e| AppError::new("INSTALL", e.to_string()))??
    };
    atomic_json(
        &directory.join(format!("fabric-{}.json", Platform::windows()?.arch.name())),
        &Receipt {
            game_version: game_version.into(),
            loader_version: loader_version.into(),
            profile_id: id,
            profile_sha1,
            main_class,
            libraries: records,
        },
    )?;
    meter.emit(
        Stage::Complete,
        format!("Fabric loader {loader_version} installed"),
    );
    Ok(())
}

async fn fetch_library_sha1(client: &reqwest::Client, url: &str) -> Result<String> {
    let bytes = fetch_bytes(client, &format!("{url}.sha1"), "Fabric's maven repository").await?;
    let hash = String::from_utf8_lossy(&bytes).trim().to_owned();
    download::check_hash(&hash)?;
    Ok(hash)
}

fn profile_version(receipt: &Receipt, profile: &Profile) -> Result<Version> {
    Ok(Version {
        id: receipt.profile_id.clone(),
        inherits_from: Some(receipt.game_version.clone()),
        main_class: receipt.main_class.clone().into(),
        java_version: None,
        asset_index: None,
        arguments: profile.arguments.clone(),
        minecraft_arguments: profile.minecraft_arguments.clone(),
        downloads: BTreeMap::default(),
        libraries: receipt
            .libraries
            .iter()
            .map(|record| Library {
                name: record.name.clone(),
                downloads: LibraryDownloads {
                    artifact: Some(Artifact {
                        url: record.url.clone(),
                        sha1: record.sha1.clone(),
                        size: record.size,
                        path: Some(record.maven.clone()),
                    }),
                    classifiers: BTreeMap::default(),
                },
                rules: None,
                natives: BTreeMap::default(),
                extract: Extract::default(),
            })
            .collect(),
        logging: BTreeMap::default(),
        kind: profile.kind.clone(),
    })
}

/// Verify the Fabric layer of an instance whose vanilla Minecraft version is already
/// verified, and return a launch-ready view that merges both. `Ok(None)` means the
/// Fabric loader still has to be installed.
pub fn verify(
    root: &Path,
    game_version: &str,
    loader_version: &str,
    platform: &Platform,
    vanilla: &Installed,
    full: bool,
) -> Result<Option<Installed>> {
    let id = profile_id(game_version, loader_version)?;
    let directory = root.join("versions").join(segment(&id)?);
    let receipt_path = directory.join(format!("fabric-{}.json", platform.arch.name()));
    if !receipt_path.exists() {
        return Ok(None);
    }
    let receipt: Receipt = read_json(&receipt_path)?;
    if receipt.game_version != game_version
        || receipt.loader_version != loader_version
        || receipt.profile_id != id
    {
        return Ok(None);
    }
    let profile_path = directory.join(format!("{id}.json"));
    let profile_meta = fs::metadata(&profile_path).map_err(|_| missing(&profile_path))?;
    if !profile_meta.is_file()
        || profile_meta.len() == 0
        || profile_meta.len() > MAX_PROFILE_BYTES as u64
        || (full
            && !download::verify(
                &profile_path,
                &receipt.profile_sha1,
                None,
                MAX_PROFILE_BYTES as u64,
            ))
    {
        return Err(missing(&profile_path));
    }
    let profile: Profile = read_json(&profile_path)?;
    if profile.id != receipt.profile_id
        || profile
            .inherits_from
            .as_deref()
            .is_some_and(|parent| parent != game_version)
        || profile.main_class.as_deref() != Some(receipt.main_class.as_str())
    {
        return Err(invalid(
            "The Fabric profile no longer matches its installation record.",
        ));
    }
    let mut classpath = Vec::with_capacity(receipt.libraries.len() + vanilla.classpath.len());
    for record in &receipt.libraries {
        let path = library_path(root, record)?;
        let meta = fs::metadata(&path).map_err(|_| missing(&path))?;
        if !meta.is_file()
            || meta.len() == 0
            || meta.len() > MAX_LIBRARY_BYTES
            || (record.size != UNKNOWN_SIZE && meta.len() != record.size)
            || (full
                && !download::verify(
                    &path,
                    &record.sha1,
                    (record.size != UNKNOWN_SIZE).then_some(record.size),
                    MAX_LIBRARY_BYTES,
                ))
        {
            return Err(missing(&path));
        }
        classpath.push(path);
    }
    for path in &vanilla.classpath {
        if !classpath.contains(path) {
            classpath.push(path.clone());
        }
    }
    let version = vanilla
        .version
        .clone()
        .inherit(profile_version(&receipt, &profile)?);
    Ok(Some(Installed {
        version,
        classpath,
        natives: vanilla.natives.clone(),
        assets: vanilla.assets.clone(),
        game_assets: vanilla.game_assets.clone(),
        libraries: vanilla.libraries.clone(),
        log_path: vanilla.log_path.clone(),
        resource_map: vanilla.resource_map.clone(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_coordinates_become_paths() {
        assert_eq!(
            maven_path("net.fabricmc:fabric-loader:0.20.1").unwrap(),
            "net/fabricmc/fabric-loader/0.20.1/fabric-loader-0.20.1.jar"
        );
        assert_eq!(
            maven_path("net.fabricmc:sponge-mixin:0.17.5+mixin.0.8.7").unwrap(),
            "net/fabricmc/sponge-mixin/0.17.5+mixin.0.8.7/sponge-mixin-0.17.5+mixin.0.8.7.jar"
        );
        assert_eq!(
            maven_path("org.ow2.asm:asm:9.10.1").unwrap(),
            "org/ow2/asm/asm/9.10.1/asm-9.10.1.jar"
        );
        assert_eq!(
            maven_path("a.b:c:1.0:tests@zip").unwrap(),
            "a/b/c/1.0/c-1.0-tests.zip"
        );
        assert!(maven_path("org.ow2.asm:asm").is_err());
        assert!(maven_path("org.ow2.asm::asm:1.0").is_err());
        assert!(maven_path("a:b:c:d:e").is_err());
        assert!(maven_path("a:b:c@jar:x").is_err());
    }

    #[test]
    fn components_reject_url_and_path_tricks() {
        assert!(validate_component("0.20.1").is_ok());
        assert!(validate_component("1.21.1").is_ok());
        assert!(validate_component("").is_err());
        assert!(validate_component("../evil").is_err());
        assert!(validate_component("a/b").is_err());
        assert!(validate_component("a\\b").is_err());
        assert!(validate_component("with space").is_err());
        assert_eq!(
            profile_id("1.21.1", "0.20.1").unwrap(),
            "fabric-loader-0.20.1-1.21.1"
        );
    }

    #[test]
    fn profile_merges_into_the_inherited_version() {
        let profile_json = serde_json::json!({
            "id": "fabric-loader-0.20.1-1.21.1",
            "inheritsFrom": "1.21.1",
            "time": "2026-11-05T01:35:34+0000",
            "releaseTime": "2026-11-05T01:35:34+0000",
            "type": "release",
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "arguments": { "game": [], "jvm": ["-DFabricMcEmu= net.minecraft.client.main.Main "] },
            "libraries": [
                { "name": "net.fabricmc:fabric-loader:0.20.1", "url": "https://maven.fabricmc.net/" },
                { "name": "org.ow2.asm:asm:9.10.1", "url": "https://maven.fabricmc.net/", "sha1": "ada2141c0cc52ee8f5c48cd5fa4ce0e794f22236", "size": 126151 }
            ]
        });
        let profile: Profile = serde_json::from_value(profile_json).unwrap();
        let vanilla: Version = serde_json::from_str(
            r#"{"id":"1.21.1","mainClass":"net.minecraft.client.main.Main","javaVersion":{"majorVersion":21},"arguments":{"game":["--old"],"jvm":["-Dvanilla=true"]},"assetIndex":{"id":"17","url":"https://piston-meta.mojang.com/a.json","sha1":"0000000000000000000000000000000000000000","size":10},"downloads":{"client":{"url":"https://piston-data.mojang.com/c.jar","sha1":"1111111111111111111111111111111111111111","size":10}},"libraries":[{"name":"org:vanilla:1.0","downloads":{"artifact":{"url":"https://libraries.minecraft.net/o/1/1.0/1.0.jar","sha1":"2222222222222222222222222222222222222222","size":10,"path":"o/1/1.0/1.0.jar"}}}]}"#,
        )
        .unwrap();
        let records = vec![LibraryRecord {
            name: "net.fabricmc:fabric-loader:0.20.1".into(),
            maven: "net/fabricmc/fabric-loader/0.20.1/fabric-loader-0.20.1.jar".into(),
            url: "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.20.1/fabric-loader-0.20.1.jar".into(),
            path: PathBuf::from("libraries/net/fabricmc/fabric-loader/0.20.1/fabric-loader-0.20.1.jar"),
            sha1: "3333333333333333333333333333333333333333".into(),
            size: 5,
        }];
        let receipt = Receipt {
            game_version: "1.21.1".into(),
            loader_version: "0.20.1".into(),
            profile_id: "fabric-loader-0.20.1-1.21.1".into(),
            profile_sha1: "4444444444444444444444444444444444444444".into(),
            main_class: profile.main_class.clone().unwrap(),
            libraries: records,
        };
        let merged = vanilla.inherit(profile_version(&receipt, &profile).unwrap());
        assert_eq!(
            merged.main_class.as_deref(),
            Some("net.fabricmc.loader.impl.launch.knot.KnotClient")
        );
        assert_eq!(merged.id, "fabric-loader-0.20.1-1.21.1");
        assert_eq!(merged.java_major(), 21);
        assert_eq!(merged.asset_index.as_ref().unwrap().id, "17");
        assert!(merged
            .arguments
            .as_ref()
            .unwrap()
            .jvm
            .iter()
            .any(|argument| matches!(argument, Argument::Text(text) if text == "-Dvanilla=true")));
        assert!(merged.arguments.as_ref().unwrap().jvm.iter().any(
            |argument| matches!(argument, Argument::Text(text) if text.contains("FabricMcEmu"))
        ));
        assert!(merged
            .arguments
            .as_ref()
            .unwrap()
            .game
            .iter()
            .any(|argument| matches!(argument, Argument::Text(text) if text == "--old")));
        assert!(merged
            .libraries
            .iter()
            .any(|library| library.name == "net.fabricmc:fabric-loader:0.20.1"));
        assert!(merged
            .libraries
            .iter()
            .any(|library| library.name == "org:vanilla:1.0"));
    }

    #[test]
    fn unknown_library_sizes_do_not_inflate_the_download_total() {
        // Fabric profiles omit `size` on some libraries; those must count as zero in the
        // progress meter instead of overflowing the total with `u64::MAX`.
        assert_eq!(meter_bytes(126_151), 126_151);
        assert_eq!(meter_bytes(0), 0);
        assert_eq!(meter_bytes(UNKNOWN_SIZE), 0);
        let total = [10u64, UNKNOWN_SIZE, 32]
            .into_iter()
            .try_fold(0u64, |sum, size| sum.checked_add(meter_bytes(size)))
            .unwrap();
        assert_eq!(total, 42);
    }

    #[test]
    fn library_paths_are_rebuilt_from_maven_coordinates() {
        // Receipts serialize the path with the platform separator (backslashes on
        // Windows); verification must rebuild it from the slash-separated coordinates
        // instead of re-validating the stored path.
        let record = LibraryRecord {
            name: "org.ow2.asm:asm:9.10.1".into(),
            maven: "org/ow2/asm/asm/9.10.1/asm-9.10.1.jar".into(),
            url: "https://maven.fabricmc.net/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar".into(),
            path: PathBuf::from("libraries\\org/ow2/asm/asm/9.10.1/asm-9.10.1.jar"),
            sha1: "ada2141c0cc52ee8f5c48cd5fa4ce0e794f22236".into(),
            size: 126_151,
        };
        let root = Path::new("root");
        assert_eq!(
            library_path(root, &record).unwrap(),
            root.join("libraries")
                .join("org/ow2/asm/asm/9.10.1/asm-9.10.1.jar")
        );
        // A tampered coordinate is rejected rather than escaping the cache.
        let mut escape = record.clone();
        escape.maven = "../../escape.jar".into();
        assert!(library_path(root, &escape).is_err());
    }

    #[test]
    #[ignore = "Live Fabric Meta endpoint; run explicitly when network access is available"]
    fn live_loader_list_and_profile_url() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let loaders = runtime.block_on(loader_versions("1.21.1", 20)).unwrap();
        assert!(loaders.first().is_some_and(|loader| loader.stable));
        runtime.block_on(loader_versions("1.12.2", 20)).unwrap_err();
    }

    #[tokio::test]
    #[ignore = "downloads real Minecraft and Fabric files; uses a temporary cache and network"]
    async fn live_fabric_install_over_vanilla_and_layered_verify() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let game_version = std::env::var("FABRIC_TEST_GAME").unwrap_or_else(|_| "1.21.1".into());
        let loader_version = match std::env::var("FABRIC_TEST_LOADER") {
            Ok(value) => value,
            Err(_) => {
                loader_versions(&game_version, 30)
                    .await
                    .unwrap()
                    .into_iter()
                    .find(|loader| loader.stable)
                    .unwrap()
                    .version
            }
        };
        let catalogue = dir.path().join("versions.json");
        super::super::versions::load_catalog(&catalogue, true, 30)
            .await
            .unwrap();
        let manifest = super::super::versions::read_cache(&catalogue)
            .unwrap()
            .manifest;
        super::super::install::install(root, &manifest, &game_version, "test", 30, |_| {})
            .await
            .unwrap();
        let platform = Platform::windows().unwrap();
        let vanilla = super::super::install::verify_installed(root, &game_version, &platform)
            .unwrap()
            .unwrap();
        install(root, &game_version, &loader_version, "test", 30, |_| {})
            .await
            .unwrap();
        let layered = verify(
            root,
            &game_version,
            &loader_version,
            &platform,
            &vanilla,
            true,
        )
        .unwrap()
        .expect("the Fabric layer must verify after installation");
        assert_eq!(
            layered.version.main_class.as_deref(),
            Some("net.fabricmc.loader.impl.launch.knot.KnotClient")
        );
        assert_eq!(layered.version.java_major(), vanilla.version.java_major());
        assert!(layered.classpath.len() > vanilla.classpath.len());
        assert_eq!(layered.assets, vanilla.assets);
        // Verification must be idempotent.
        assert!(verify(
            root,
            &game_version,
            &loader_version,
            &platform,
            &vanilla,
            true
        )
        .unwrap()
        .is_some());
        let jar = layered.classpath.first().unwrap();
        let mut bytes = fs::read(jar).unwrap();
        bytes[0] ^= 1;
        fs::write(jar, bytes).unwrap();
        assert!(verify(
            root,
            &game_version,
            &loader_version,
            &platform,
            &vanilla,
            false
        )
        .unwrap()
        .is_some());
        assert_eq!(
            verify(
                root,
                &game_version,
                &loader_version,
                &platform,
                &vanilla,
                true
            )
            .err()
            .unwrap()
            .code,
            "MISSING_FILES"
        );
    }
}
