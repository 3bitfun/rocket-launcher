use super::{
    download::{self, FileSpec},
    metadata::*,
    versions::{Manifest, VersionEntry},
};
use crate::{
    errors::{AppError, Result},
    storage::atomic_json,
};
use futures_util::{stream, TryStreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering::Relaxed},
        Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Downloading,
    Installing,
    Complete,
    Error,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub instance_id: String,
    pub version: String,
    pub stage: Stage,
    pub current: String,
    pub transferred_bytes: u64,
    pub verified_bytes: u64,
    pub total_bytes: Option<u64>,
    pub bytes_per_second: u64,
    pub error: Option<String>,
}

struct Meter<'a, F> {
    instance_id: &'a str,
    version: &'a str,
    send: F,
    transferred: AtomicU64,
    verified: AtomicU64,
    total: Option<u64>,
    previous: Mutex<(Instant, u64)>,
}
impl<F: Fn(Progress)> Meter<'_, F> {
    fn update(&self, stage: Stage, current: impl Into<String>, force: bool) {
        let Ok(mut previous) = self.previous.lock() else {
            return;
        };
        let elapsed = previous.0.elapsed();
        if !force && elapsed < Duration::from_millis(200) {
            return;
        }
        let transferred = self.transferred.load(Relaxed);
        let speed = if elapsed.as_millis() > 0 && matches!(stage, Stage::Downloading) {
            (transferred.saturating_sub(previous.1) as f64 / elapsed.as_secs_f64()) as u64
        } else {
            0
        };
        *previous = (Instant::now(), transferred);
        (self.send)(Progress {
            instance_id: self.instance_id.into(),
            version: self.version.into(),
            stage,
            current: current.into(),
            transferred_bytes: transferred,
            verified_bytes: self.verified.load(Relaxed),
            total_bytes: self.total,
            bytes_per_second: speed,
            error: None,
        });
    }
}

#[derive(Serialize, Deserialize)]
struct Receipt {
    chain: Vec<VersionEntry>,
    natives: String,
    native_hashes: BTreeMap<String, String>,
}
pub struct Installed {
    pub version: Version,
    pub classpath: Vec<PathBuf>,
    pub natives: PathBuf,
    pub assets: PathBuf,
    pub game_assets: PathBuf,
    pub libraries: PathBuf,
    pub log_path: Option<PathBuf>,
    pub resource_map: Vec<(PathBuf, PathBuf)>,
}
struct Plan {
    files: BTreeMap<PathBuf, FileSpec>,
    classpath: Vec<PathBuf>,
    archives: Vec<(PathBuf, Extract)>,
    aliases: Vec<(PathBuf, PathBuf)>,
    resource_map: Vec<(PathBuf, PathBuf)>,
    index: FileSpec,
    log_path: Option<PathBuf>,
}
impl Plan {
    fn add(&mut self, path: PathBuf, artifact: Artifact) -> Result<()> {
        let spec = FileSpec::new(path.clone(), artifact)?;
        if let Some(old) = self.files.get(&path) {
            if old.artifact.sha1 != spec.artifact.sha1 || old.artifact.size != spec.artifact.size {
                return Err(invalid(format!(
                    "Conflicting artifacts for {}.",
                    path.display()
                )));
            }
        } else {
            self.files.insert(path, spec);
        }
        Ok(())
    }
    fn new(version: &Version, platform: &Platform) -> Result<Self> {
        if version.main_class.as_deref().unwrap_or("").is_empty() {
            return Err(invalid("Missing Minecraft main class."));
        }
        let index = version
            .asset_index
            .as_ref()
            .ok_or_else(|| invalid("Missing Minecraft asset index."))?;
        let index_file = FileSpec::new(
            PathBuf::from("assets/indexes").join(format!("{}.json", segment(&index.id)?)),
            index.artifact.clone(),
        )?;
        let mut plan = Self {
            files: BTreeMap::new(),
            classpath: vec![],
            archives: vec![],
            aliases: vec![],
            resource_map: vec![],
            index: index_file,
            log_path: None,
        };
        for lib in &version.libraries {
            if !allowed(lib.rules.as_deref(), platform)? {
                continue;
            }
            if let Some(artifact) = &lib.downloads.artifact {
                let path = library_path(artifact)?;
                plan.add(path.clone(), artifact.clone())?;
                if !plan.classpath.contains(&path) {
                    plan.classpath.push(path);
                }
            }
            if let Some(classifier) = lib.natives.get("windows") {
                let classifier = classifier.replace("${arch}", platform.arch.bits());
                let artifact = lib.downloads.classifiers.get(&classifier).ok_or_else(|| {
                    invalid(format!("Missing Windows native artifact for {}.", lib.name))
                })?;
                let path = library_path(artifact)?;
                plan.add(path.clone(), artifact.clone())?;
                plan.archives.push((path, lib.extract.clone()));
            } else if lib.downloads.artifact.is_none() && lib.natives.is_empty() {
                return Err(invalid(format!(
                    "Missing downloadable artifact for {}.",
                    lib.name
                )));
            }
        }
        let jar = PathBuf::from("versions")
            .join(segment(&version.id)?)
            .join(format!("{}.jar", version.id));
        plan.add(
            jar.clone(),
            version
                .downloads
                .get("client")
                .ok_or_else(|| invalid("Missing Minecraft client download."))?
                .clone(),
        )?;
        plan.classpath.push(jar);
        if let Some(config) = version.logging.get("client") {
            let path = PathBuf::from("assets/log_configs").join(segment(&config.file.id)?);
            plan.add(path.clone(), config.file.artifact.clone())?;
            plan.log_path = Some(path);
        }
        Ok(plan)
    }
    fn add_assets(&mut self, root: &Path, index_id: &str) -> Result<()> {
        let assets: Assets = read_json(&root.join(&self.index.relative))?;
        for (name, asset) in assets.objects {
            let name = relative(&name)?;
            download::check_hash(&asset.hash)?;
            let hash = asset.hash.to_ascii_lowercase();
            let hash_path = format!("{}/{}", &hash[..2], hash);
            let path = PathBuf::from("assets/objects").join(&hash_path);
            self.add(
                path.clone(),
                Artifact {
                    url: format!("https://resources.download.minecraft.net/{hash_path}"),
                    sha1: hash,
                    size: asset.size,
                    path: None,
                },
            )?;
            if assets.virtual_ {
                self.aliases.push((
                    path.clone(),
                    PathBuf::from("assets/virtual")
                        .join(segment(index_id)?)
                        .join(&name),
                ));
            }
            if assets.map_to_resources {
                self.resource_map.push((path, name));
            }
        }
        Ok(())
    }
}
fn library_path(artifact: &Artifact) -> Result<PathBuf> {
    Ok(PathBuf::from("libraries").join(relative(
        artifact
            .path
            .as_deref()
            .ok_or_else(|| invalid("Library artifact has no path."))?,
    )?))
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let file = fs::File::open(path).map_err(|e| AppError::io("open Minecraft metadata", e))?;
    serde_json::from_reader(io::BufReader::new(file.take(32 * 1024 * 1024)))
        .map_err(|e| invalid(format!("Could not read {}: {e}", path.display())))
}
fn version_spec(entry: &VersionEntry) -> Result<FileSpec> {
    let mut spec = FileSpec::new(
        PathBuf::from("versions")
            .join(segment(&entry.id)?)
            .join(format!("{}.json", segment(&entry.sha1)?)),
        Artifact {
            url: entry.url.clone(),
            sha1: entry.sha1.clone(),
            size: u64::MAX,
            path: None,
        },
    )?;
    spec.max_size = 8 * 1024 * 1024;
    Ok(spec)
}
fn read_version(root: &Path, entry: &VersionEntry) -> Result<Version> {
    let spec = version_spec(entry)?;
    if !spec.verify(root) {
        return Err(missing(&spec.relative));
    }
    let version: Version = read_json(&root.join(spec.relative))?;
    if version.id != entry.id {
        return Err(invalid(
            "Minecraft metadata version does not match its catalogue entry.",
        ));
    }
    Ok(version)
}
// Fast readiness check: metadata only, no SHA-1 reads. Full integrity checks
// remain in the installation/download pipeline.
fn quick_file_check(path: &Path, expected_size: Option<u64>, max_size: u64) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.len() > max_size {
        return false;
    }
    expected_size.map_or(true, |size| meta.len() == size)
}

fn read_version_quick(root: &Path, entry: &VersionEntry) -> Result<Version> {
    let spec = version_spec(entry)?;
    if !quick_file_check(&root.join(&spec.relative), None, spec.max_size) {
        return Err(missing(&spec.relative));
    }
    let version: Version = read_json(&root.join(&spec.relative))?;
    if version.id != entry.id {
        return Err(invalid(
            "Minecraft metadata version does not match its catalogue entry.",
        ));
    }
    Ok(version)
}

fn receipt_path(root: &Path, id: &str, platform: &Platform) -> Result<PathBuf> {
    Ok(root
        .join("versions")
        .join(segment(id)?)
        .join(format!("installed-{}.json", platform.arch.name())))
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
pub fn verify_installed(root: &Path, id: &str, platform: &Platform) -> Result<Option<Installed>> {
    inspect_installed(root, id, platform, true)
}

pub fn inspect_installed(
    root: &Path,
    id: &str,
    platform: &Platform,
    full: bool,
) -> Result<Option<Installed>> {
    inspect_with_options(root, id, platform, full, false)
}

pub fn inspect_for_launch(root: &Path, id: &str, platform: &Platform) -> Result<Option<Installed>> {
    inspect_with_options(root, id, platform, false, true)
}

fn inspect_with_options(
    root: &Path,
    id: &str,
    platform: &Platform,
    full: bool,
    allow_missing: bool,
) -> Result<Option<Installed>> {
    let path = receipt_path(root, id, platform)?;
    if !path.exists() {
        return Ok(None);
    }
    let receipt: Receipt = read_json(&path)?;
    if receipt.chain.is_empty() || receipt.chain.len() > 16 {
        return Err(invalid("Invalid installation metadata chain."));
    }
    let mut next = Some(id.to_owned());
    let mut chain = vec![];
    for entry in &receipt.chain {
        if next.as_ref() != Some(&entry.id) {
            return Err(invalid("Invalid installation inheritance order."));
        }
        let version = if full {
            read_version(root, entry)?
        } else {
            read_version_quick(root, entry)?
        };
        next.clone_from(&version.inherits_from);
        chain.push(version);
    }
    if next.is_some() {
        return Err(invalid("Installation metadata has an unresolved parent."));
    }
    let version = merge(chain)?;
    let mut plan = Plan::new(&version, platform)?;
    if !(if full {
        plan.index.verify(root)
    } else {
        quick_file_check(
            &root.join(&plan.index.relative),
            Some(plan.index.artifact.size),
            plan.index.max_size,
        )
    }) {
        return Err(missing(&plan.index.relative));
    }
    let index_id = &version.asset_index.as_ref().unwrap().id;
    plan.add_assets(root, index_id)?;
    // Asset objects can number in the thousands. They were checked during
    // installation; don't stat or hash them again for routine launch readiness.
    for spec in plan.files.values() {
        if allow_missing {
            break;
        }
        if !full && spec.relative.starts_with("assets/objects") {
            continue;
        }
        if !(if full {
            spec.verify(root)
        } else {
            quick_file_check(
                &root.join(&spec.relative),
                Some(spec.artifact.size),
                spec.max_size,
            )
        }) {
            return Err(missing(&spec.relative));
        }
    }
    if full {
        for (source, target) in &plan.aliases {
            let spec = &plan.files[source];
            if !download::verify(
                &root.join(target),
                &spec.artifact.sha1,
                Some(spec.artifact.size),
                spec.max_size,
            ) {
                return Err(missing(target));
            }
        }
    }
    let natives = path.parent().unwrap().join(segment(&receipt.natives)?);
    if !receipt.natives.starts_with("natives-") || (!allow_missing && !natives.is_dir()) {
        return Err(missing(&natives));
    }
    if !allow_missing {
        for item in fs::read_dir(&natives).map_err(|e| AppError::io("read extracted natives", e))? {
            let item = item.map_err(|e| AppError::io("inspect extracted native", e))?;
            let name = item.file_name().to_string_lossy().into_owned();
            if !receipt.native_hashes.contains_key(&name) {
                return Err(missing(&item.path()));
            }
        }
    }
    for (name, hash) in &receipt.native_hashes {
        let file = natives.join(segment(name)?);
        if allow_missing {
            continue;
        }
        let valid = if full {
            download::verify(&file, hash, None, 256 * 1024 * 1024)
        } else {
            quick_file_check(&file, None, 256 * 1024 * 1024)
        };
        if !valid {
            return Err(missing(&file));
        }
    }
    let assets = root.join("assets");
    let index: Assets = read_json(&root.join(&plan.index.relative))?;
    let game_assets = if index.virtual_ {
        assets.join("virtual").join(index_id)
    } else {
        assets.clone()
    };
    Ok(Some(Installed {
        version,
        classpath: plan.classpath.iter().map(|p| root.join(p)).collect(),
        natives,
        assets,
        game_assets,
        libraries: root.join("libraries"),
        log_path: plan.log_path.map(|p| root.join(p)),
        resource_map: plan
            .resource_map
            .into_iter()
            .map(|(p, name)| (root.join(p), name))
            .collect(),
    }))
}
fn merge(chain: Vec<Version>) -> Result<Version> {
    let mut chain = chain.into_iter().rev();
    let parent = chain
        .next()
        .ok_or_else(|| invalid("Minecraft metadata chain is empty."))?;
    Ok(chain.fold(parent, Version::inherit))
}

pub async fn install(
    root: &Path,
    manifest: &Manifest,
    id: &str,
    instance_id: &str,
    timeout: u64,
    send: impl Fn(Progress) + Sync,
) -> Result<()> {
    let platform = Platform::windows()?;
    let client = download::client(timeout)?;
    let mut meter = Meter {
        instance_id,
        version: id,
        send,
        total: None,
        transferred: AtomicU64::new(0),
        verified: AtomicU64::new(0),
        previous: Mutex::new((Instant::now(), 0)),
    };
    meter.update(
        Stage::Downloading,
        "Resolving Mojang version metadata",
        true,
    );
    let mut entries = vec![];
    let mut chain = vec![];
    let mut seen = HashSet::new();
    let mut next = Some(id.to_owned());
    while let Some(id) = next.take() {
        if !seen.insert(id.clone()) || entries.len() >= 16 {
            return Err(invalid(
                "Minecraft metadata contains an inheritance cycle or exceeds 16 versions.",
            ));
        }
        let entry = manifest
            .versions
            .iter()
            .find(|v| v.id == id)
            .ok_or_else(|| invalid(format!("Version {id} is not in the Mojang catalogue.")))?;
        let spec = version_spec(entry)?;
        download::ensure(&client, root, &spec, &|bytes| {
            meter.transferred.fetch_add(bytes, Relaxed);
            meter.update(
                Stage::Downloading,
                spec.relative.display().to_string(),
                false,
            );
        })
        .await?;
        let version = read_version(root, entry)?;
        next.clone_from(&version.inherits_from);
        entries.push(entry.clone());
        chain.push(version);
    }
    let version = merge(chain)?;
    let mut plan = Plan::new(&version, &platform)?;
    download::ensure(&client, root, &plan.index, &|bytes| {
        meter.transferred.fetch_add(bytes, Relaxed);
        meter.update(Stage::Downloading, "Downloading asset index", false);
    })
    .await?;
    plan.add_assets(root, &version.asset_index.as_ref().unwrap().id)?;
    meter.total = Some(
        plan.files
            .values()
            .try_fold(plan.index.artifact.size, |sum, f| {
                sum.checked_add(f.artifact.size)
                    .ok_or_else(|| invalid("Minecraft download total is too large."))
            })?,
    );
    meter.verified.store(plan.index.artifact.size, Relaxed);
    meter.update(
        Stage::Downloading,
        "Checking shared libraries and assets",
        true,
    );
    stream::iter(plan.files.values().map(Ok::<_, AppError>))
        .try_for_each_concurrent(16, |spec| {
            let meter = &meter;
            let client = &client;
            async move {
                download::ensure(client, root, spec, &|bytes| {
                    meter.transferred.fetch_add(bytes, Relaxed);
                    meter.update(
                        Stage::Downloading,
                        spec.relative.display().to_string(),
                        false,
                    );
                })
                .await?;
                meter.verified.fetch_add(spec.artifact.size, Relaxed);
                meter.update(
                    Stage::Downloading,
                    spec.relative.display().to_string(),
                    false,
                );
                Ok(())
            }
        })
        .await?;
    meter.update(
        Stage::Installing,
        "Extracting native libraries and preparing assets",
        true,
    );
    let receipt = receipt_path(root, id, &platform)?;
    let root = root.to_owned();
    tokio::task::spawn_blocking(move || finish(&root, &receipt, &plan, entries))
        .await
        .map_err(|e| AppError::new("INSTALL", e.to_string()))??;
    meter.update(Stage::Complete, "Minecraft files verified", true);
    Ok(())
}

fn finish(root: &Path, receipt: &Path, plan: &Plan, entries: Vec<VersionEntry>) -> Result<()> {
    for (source, target) in &plan.aliases {
        let spec = &plan.files[source];
        let destination = root.join(target);
        if !download::verify(
            &destination,
            &spec.artifact.sha1,
            Some(spec.artifact.size),
            spec.max_size,
        ) {
            copy_atomic(&root.join(source), &destination)?;
        }
    }
    let temp = tempfile::Builder::new()
        .prefix("natives-")
        .tempdir_in(receipt.parent().unwrap())
        .map_err(|e| AppError::io("create native library folder", e))?;
    let mut hashes = BTreeMap::new();
    let mut extracted_bytes = 0u64;
    for (path, rules) in &plan.archives {
        let file =
            fs::File::open(root.join(path)).map_err(|e| AppError::io("open native archive", e))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| invalid(format!("Invalid native archive: {e}")))?;
        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|e| invalid(format!("Invalid native archive entry: {e}")))?;
            if entry.is_dir()
                || !entry.name().to_ascii_lowercase().ends_with(".dll")
                || rules
                    .exclude
                    .iter()
                    .any(|prefix| entry.name().starts_with(prefix))
            {
                continue;
            }
            let name = entry.name().rsplit('/').next().unwrap_or("").to_owned();
            relative(entry.name())?;
            segment(&name)?;
            let size = entry.size();
            extracted_bytes = extracted_bytes.saturating_add(size);
            if extracted_bytes > 256 * 1024 * 1024
                || hashes
                    .keys()
                    .any(|key: &String| key.eq_ignore_ascii_case(&name))
            {
                return Err(invalid(
                    "Native archive is oversized or contains duplicate libraries.",
                ));
            }
            let path = temp.path().join(&name);
            let mut output =
                fs::File::create(&path).map_err(|e| AppError::io("create native library", e))?;
            if io::copy(&mut (&mut entry).take(size + 1), &mut output)
                .map_err(|e| AppError::io("extract native library", e))?
                != size
            {
                return Err(invalid(
                    "Extracted native library does not match archive size.",
                ));
            }
            drop(output);
            hashes.insert(
                name,
                download::file_hash(&path).map_err(|e| AppError::io("verify native library", e))?,
            );
        }
    }
    let natives = temp
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    atomic_json(
        receipt,
        &Receipt {
            chain: entries,
            natives,
            native_hashes: hashes,
        },
    )?;
    let _ = temp.keep();
    Ok(())
}

pub fn copy_atomic(source: &Path, target: &Path) -> Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| invalid("Asset destination has no parent."))?;
    fs::create_dir_all(parent).map_err(|e| AppError::io("create asset directory", e))?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| AppError::io("create temporary asset", e))?;
    let mut input = fs::File::open(source).map_err(|e| AppError::io("open verified asset", e))?;
    io::copy(&mut input, &mut temp).map_err(|e| AppError::io("copy asset", e))?;
    temp.as_file()
        .sync_all()
        .map_err(|e| AppError::io("flush asset", e))?;
    temp.persist(target)
        .map_err(|e| AppError::io("publish asset", e.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "downloads real Minecraft files; uses a temporary cache"]
    async fn live_install_and_offline_repair() {
        let dir = tempfile::tempdir().unwrap();
        let catalogue = dir.path().join("manifest.json");
        super::super::versions::load_catalog(&catalogue, true, 20)
            .await
            .unwrap();
        let manifest = super::super::versions::read_cache(&catalogue)
            .unwrap()
            .manifest;
        let id = std::env::var("ROCKET_TEST_VERSION").unwrap_or_else(|_| "1.12.2".into());
        let root = dir.path().join("minecraft");
        install(&root, &manifest, &id, "test", 20, |_| {})
            .await
            .unwrap();
        let platform = Platform::windows().unwrap();
        let installed = verify_installed(&root, &id, &platform).unwrap().unwrap();
        assert!(!installed.classpath.is_empty());
        let client = installed.classpath.last().unwrap();
        fs::remove_file(client).unwrap();
        assert_eq!(
            verify_installed(&root, &id, &platform).err().unwrap().code,
            "MISSING_FILES"
        );
        install(&root, &manifest, &id, "test", 20, |_| {})
            .await
            .unwrap();
        assert!(verify_installed(&root, &id, &platform).unwrap().is_some());
    }
}
