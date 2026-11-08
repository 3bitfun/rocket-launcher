use super::{
    download::{self, FileSpec},
    metadata::{segment, Artifact},
};
use crate::{
    errors::{AppError, Result},
    instances::mods_directory,
    models::{Instance, Loader},
    storage::atomic_json,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    path::Path,
    time::Duration,
};

const BASE: &str = "https://api.modrinth.com/v2";
pub const PAGE_SIZE: u32 = 20;
const MAX_JAR_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct Project {
    project_id: String,
    title: String,
    description: String,
    author: String,
    downloads: u64,
    icon_url: Option<String>,
    featured_gallery: Option<String>,
    #[serde(default)]
    display_categories: Vec<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct SearchPage {
    hits: Vec<Project>,
    total_hits: u32,
}

#[derive(Deserialize)]
struct Version {
    id: String,
    project_id: String,
    version_number: String,
    version_type: String,
    date_published: String,
    game_versions: Vec<String>,
    loaders: Vec<String>,
    files: Vec<VersionFile>,
    #[serde(default)]
    dependencies: Vec<Dependency>,
}

#[derive(Deserialize)]
struct VersionFile {
    filename: String,
    url: String,
    hashes: Hashes,
    size: u64,
    primary: bool,
}

#[derive(Deserialize)]
struct Hashes {
    sha1: String,
}

#[derive(Deserialize)]
struct Dependency {
    dependency_type: String,
    project_id: Option<String>,
    version_id: Option<String>,
    file_name: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModVersion {
    pub id: String,
    version_number: String,
    pub version_type: String,
    file_name: String,
    required_dependencies: Vec<String>,
}

fn error(message: impl Into<String>) -> AppError {
    AppError::new("MODRINTH", message)
}

fn incompatible(message: impl Into<String>) -> AppError {
    AppError::new("MODRINTH_INCOMPATIBLE", message)
}

fn loader(instance: &Instance) -> Result<&'static str> {
    match instance.loader {
        Loader::Fabric => Ok("fabric"),
        Loader::Forge => Ok("forge"),
        Loader::Vanilla => Err(error(
            "Choose a Fabric or Forge instance to browse and install mods.",
        )),
    }
}

fn id(value: &str) -> Result<&str> {
    if value.len() == 8 && value.bytes().all(|b| b.is_ascii_alphanumeric()) {
        Ok(value)
    } else {
        Err(error("Invalid Modrinth identifier."))
    }
}

pub(super) fn client(timeout: u64) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(timeout.clamp(5, 120)))
        .user_agent(concat!(
            "RocketLauncher/",
            env!("CARGO_PKG_VERSION"),
            " (Modrinth browser)"
        ))
        .build()
        .map_err(|_| error("Could not initialize Modrinth."))
}

async fn get<T: serde::de::DeserializeOwned>(request: reqwest::RequestBuilder) -> Result<T> {
    let mut response = request
        .send()
        .await
        .map_err(|_| error("Could not reach Modrinth. Check your connection."))?;
    if !response.status().is_success() {
        return Err(error(format!(
            "Modrinth returned HTTP {}.",
            response.status().as_u16()
        )));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| error("Modrinth response was interrupted."))?
    {
        if body.len() + chunk.len() > 8 * 1024 * 1024 {
            return Err(error("Modrinth response was too large."));
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| error("Modrinth returned an invalid response."))
}

pub async fn search(
    instance: &Instance,
    query: &str,
    offset: u32,
    timeout: u64,
    sort: Option<&str>,
) -> Result<SearchPage> {
    let loader = loader(instance)?;
    if query.len() > 300 || offset > 980 || offset % PAGE_SIZE != 0 {
        return Err(error(
            "Invalid Modrinth search. Try a shorter query or fewer pages.",
        ));
    }
    let facets = serde_json::json!([
        ["project_type:mod"],
        [format!("categories:{loader}")],
        [format!("versions:{}", instance.version)],
        ["client_side:required", "client_side:optional"]
    ])
    .to_string();
    let sort = match sort {
        Some("updated") => "updated",
        Some("downloads") => "downloads",
        None if query.trim().is_empty() => "downloads",
        None => "relevance",
        _ => return Err(error("Unknown Modrinth sort order.")),
    };
    get(client(timeout)?.get(format!("{BASE}/search")).query(&[
        ("query", query.trim()),
        ("facets", &facets),
        ("limit", &PAGE_SIZE.to_string()),
        ("offset", &offset.to_string()),
        ("index", sort),
    ]))
    .await
}

fn matches(version: &Version, loader: &str, game_version: &str) -> bool {
    version.loaders.iter().any(|value| value == loader)
        && version
            .game_versions
            .iter()
            .any(|value| value == game_version)
}

fn jar(version: &Version) -> Option<&VersionFile> {
    let mut files = version
        .files
        .iter()
        .filter(|file| file.filename.to_ascii_lowercase().ends_with(".jar"));
    files
        .clone()
        .find(|file| file.primary)
        .or_else(|| files.next())
}

fn jar_spec(file: &VersionFile) -> Result<FileSpec> {
    let name = segment(&file.filename)?;
    let url = reqwest::Url::parse(&file.url).map_err(|_| error("Invalid Modrinth file URL."))?;
    if url.host_str() != Some("cdn.modrinth.com") || file.size == 0 || file.size > MAX_JAR_BYTES {
        return Err(error(
            "This file is outside the supported Modrinth download limits.",
        ));
    }
    FileSpec::new(
        name.into(),
        Artifact {
            url: file.url.clone(),
            sha1: file.hashes.sha1.clone(),
            size: file.size,
            path: None,
        },
    )
}

async fn matching_versions(
    client: &reqwest::Client,
    instance: &Instance,
    project: &str,
) -> Result<Vec<Version>> {
    let loader = loader(instance)?;
    let project = id(project)?;
    let mut versions: Vec<Version> = get(client
        .get(format!("{BASE}/project/{project}/version"))
        .query(&[
            ("loaders", serde_json::json!([loader]).to_string()),
            (
                "game_versions",
                serde_json::json!([instance.version]).to_string(),
            ),
            ("include_changelog", "false".into()),
        ]))
    .await?;
    versions.retain(|version| {
        version.project_id == project && matches(version, loader, &instance.version)
    });
    versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
    Ok(versions)
}

pub async fn versions(instance: &Instance, project: &str, timeout: u64) -> Result<Vec<ModVersion>> {
    let versions = matching_versions(&client(timeout)?, instance, project).await?;
    Ok(versions
        .into_iter()
        .filter_map(|version| {
            let file = jar(&version)?;
            jar_spec(file).ok()?;
            Some(ModVersion {
                id: version.id.clone(),
                version_number: version.version_number.clone(),
                version_type: version.version_type.clone(),
                file_name: file.filename.clone(),
                required_dependencies: version
                    .dependencies
                    .iter()
                    .filter(|dep| dep.dependency_type == "required")
                    .map(|dep| {
                        dep.file_name
                            .clone()
                            .or_else(|| dep.project_id.clone())
                            .or_else(|| dep.version_id.clone())
                            .unwrap_or_else(|| "Unknown dependency".into())
                    })
                    .collect(),
            })
        })
        .collect())
}

pub async fn preferred_version(instance: &Instance, project: &str, timeout: u64) -> Result<Option<String>> {
    let versions = versions(instance, project, timeout).await?;
    Ok(versions.iter().find(|version| version.version_type == "release")
        .or_else(|| versions.first()).map(|version| version.id.clone()))
}

pub struct StagedMod {
    staging: tempfile::TempDir,
    primary: String,
    files: BTreeMap<String, (String, FileSpec)>,
}

#[derive(Serialize, Deserialize)]
struct InstalledVersion {
    id: String,
    filename: String,
}

fn installed_index(folder: &Path) -> Result<BTreeMap<String, InstalledVersion>> {
    let path = folder.join(".rocket-modrinth.json");
    match fs::read(&path) {
        Ok(bytes) if bytes.len() <= 1024 * 1024 => serde_json::from_slice(&bytes)
            .map_err(|_| error("The Modrinth installation index is invalid.")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        _ => Err(error("Could not read the Modrinth installation index.")),
    }
}

async fn version_by_id(client: &reqwest::Client, version_id: &str) -> Result<Version> {
    let version_id = id(version_id)?;
    let version: Version = get(client.get(format!("{BASE}/version/{version_id}"))).await?;
    if version.id != version_id {
        return Err(error("Modrinth returned a different version."));
    }
    id(&version.project_id)?;
    Ok(version)
}

pub async fn stage(
    instance: &Instance,
    project: &str,
    version_id: &str,
    timeout: u64,
) -> Result<StagedMod> {
    let loader = loader(instance)?;
    let project = id(project)?;
    let client = client(timeout)?;
    let version = version_by_id(&client, version_id).await?;
    if version.project_id != project || !matches(&version, loader, &instance.version) {
        return Err(error(
            "This mod version does not match the selected Minecraft version and loader.",
        ));
    }
    stage_version(instance, &client, version, None).await
}

pub(super) async fn install_pack_mod(
    instance: &Instance,
    client: &reqwest::Client,
    project: &str,
    cache: &Path,
) -> Result<bool> {
    let folder = mods_directory(&instance.game_directory)?;
    if installed_index(&folder)?.contains_key(project) {
        return Ok(true);
    }
    let mut versions = matching_versions(client, instance, project).await?;
    // Prefer stable releases. Older releases can still have working dependencies
    // when the newest one requires a different version of another pack mod.
    versions.sort_by_key(|version| version.version_type != "release");
    for version in versions.into_iter()
        .filter(|version| jar(version).is_some_and(|file| jar_spec(file).is_ok()))
        .take(3)
    {
        match stage_version(instance, client, version, Some(cache)).await
            .and_then(|staged| staged.publish(&instance.game_directory))
        {
            Ok(_) => return Ok(true),
            Err(error) if error.code == "MODRINTH_INCOMPATIBLE" => {
                tracing::debug!(project, %error, "skipping incompatible Rocket Pack candidate");
            }
            Err(error) => return Err(error),
        }
    }
    Ok(false)
}

async fn stage_version(
    instance: &Instance,
    client: &reqwest::Client,
    version: Version,
    cache: Option<&Path>,
) -> Result<StagedMod> {
    let loader = loader(instance)?;
    let folder = mods_directory(&instance.game_directory)?;
    let installed = installed_index(&folder)?;
    let primary = jar(&version)
        .ok_or_else(|| error("This mod version has no JAR file."))?
        .filename
        .clone();
    let mut queue = VecDeque::from([version]);
    let mut files: BTreeMap<String, (String, FileSpec)> = BTreeMap::new();
    while let Some(version) = queue.pop_front() {
        if cache.is_some() && version.project_id == "bEpr0Arc" {
            return Err(incompatible("Litematica is excluded from Rocket Pack."));
        }
        if let Some((chosen, _)) = files.get(&version.project_id) {
            if chosen != &version.id {
                return Err(incompatible(
                    "The selected mods require conflicting versions of a dependency.",
                ));
            }
            continue;
        }
        if files.len() >= 48 || !matches(&version, loader, &instance.version) {
            return Err(incompatible("A required mod dependency does not match this instance, or there are too many dependencies."));
        }
        let file = jar(&version).ok_or_else(|| incompatible("A required mod version has no JAR file."))?;
        let spec = jar_spec(file)?;
        ensure_enabled(&folder, &file.filename)?;
        if let Some(old) = installed.get(&version.project_id) {
            ensure_enabled(&folder, segment(&old.filename)?)?;
            if cache.is_some() && old.id != version.id && folder.join(&old.filename).is_file() {
                return Err(incompatible("A different version of this dependency is already in the pack."));
            }
        }
        for (_, previous) in files.values() {
            if previous
                .relative
                .to_string_lossy()
                .eq_ignore_ascii_case(&file.filename)
            {
                return Err(incompatible("Two required mods use the same filename."));
            }
        }
        files.insert(version.project_id.clone(), (version.id, spec));
        for dep in version
            .dependencies
            .into_iter()
            .filter(|dep| dep.dependency_type == "required")
        {
            if queue.len() >= 48 {
                return Err(error("There are too many required mod dependencies."));
            }
            if let Some(project_id) = dep.project_id.as_deref() {
                id(project_id)?;
                if let Some((chosen, _)) = files.get(project_id) {
                    if dep
                        .version_id
                        .as_ref()
                        .is_some_and(|pinned| pinned != chosen)
                    {
                        return Err(incompatible(
                            "The selected mods require conflicting versions of a dependency.",
                        ));
                    }
                    continue;
                }
            }
            let next = if let Some(version_id) = dep.version_id.as_deref() {
                version_by_id(client, version_id).await?
            } else if let Some(project_id) = dep.project_id.as_deref() {
                let candidates = matching_versions(client, instance, project_id).await?;
                let installed_id = installed.get(project_id).and_then(|entry| {
                    segment(&entry.filename)
                        .ok()
                        .filter(|name| folder.join(name).is_file())
                        .map(|_| &entry.id)
                });
                let chosen = candidates
                    .iter()
                    .position(|candidate| Some(&candidate.id) == installed_id)
                    .or_else(|| {
                        candidates.iter().position(|candidate| {
                            candidate.version_type == "release" && jar(candidate).is_some()
                        })
                    })
                    .or_else(|| {
                        candidates
                            .iter()
                            .position(|candidate| jar(candidate).is_some())
                    })
                    .ok_or_else(|| {
                        incompatible(format!(
                            "Required dependency {project_id} has no compatible JAR."
                        ))
                    })?;
                candidates
                    .into_iter()
                    .nth(chosen)
                    .expect("selected candidate")
            } else {
                let filename = dep
                    .file_name
                    .as_deref()
                    .ok_or_else(|| incompatible("Modrinth did not identify a required dependency."))?;
                if folder.join(segment(filename)?).is_file() {
                    continue;
                }
                return Err(incompatible(format!(
                    "Install the required external file {filename} first."
                )));
            };
            if dep
                .project_id
                .as_ref()
                .is_some_and(|id| id != &next.project_id)
            {
                return Err(error("Modrinth returned the wrong dependency project."));
            }
            queue.push_back(next);
        }
    }
    let staging = tempfile::Builder::new()
        .prefix(".rocket-modrinth-")
        .tempdir_in(folder)
        .map_err(|e| AppError::io("stage the mod download", e))?;
    for (_, spec) in files.values() {
        if let Some(cache) = cache {
            let mut cached = spec.clone();
            cached.relative = spec.artifact.sha1.to_ascii_lowercase().into();
            download::ensure(client, cache, &cached, &|_| {}).await?;
            super::install::copy_atomic(&cache.join(&cached.relative), &staging.path().join(&spec.relative))?;
        } else {
            download::ensure(client, staging.path(), spec, &|_| {}).await?;
        }
    }
    Ok(StagedMod {
        staging,
        primary,
        files,
    })
}

fn ensure_enabled(folder: &Path, filename: &str) -> Result<()> {
    if folder.join(format!("{filename}.disabled")).is_file() {
        return Err(error(format!("{filename} is disabled. Enable it in Installed mods first.")));
    }
    Ok(())
}

impl StagedMod {
    pub fn publish(self, game: &Path) -> Result<String> {
        let folder = mods_directory(game)?;
        let mut installed = installed_index(&folder)?;
        let mut added = Vec::new();
        let result = (|| {
            for (project, (version, spec)) in &self.files {
                ensure_enabled(&folder, &spec.relative.to_string_lossy())?;
                if let Some(old) = installed.get(project) {
                    let filename = segment(&old.filename)?;
                    if &old.id == version {
                        ensure_enabled(&folder, filename)?;
                    } else if folder.join(filename).is_file() {
                        return Err(error(format!("{} is enabled. Disable that JAR in Installed mods before choosing another version.", old.filename)));
                    }
                }
                let target = folder.join(&spec.relative);
                let staged =
                    tempfile::TempPath::try_from_path(self.staging.path().join(&spec.relative))
                        .map_err(|e| AppError::io("open the staged mod", e))?;
                match staged.persist_noclobber(&target) {
                    Ok(()) => added.push(target),
                    Err(e)
                        if e.error.kind() == std::io::ErrorKind::AlreadyExists
                            && spec.verify(&folder) => {}
                    Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
                        return Err(error(format!(
                            "{} already exists with different contents.",
                            spec.relative.display()
                        )));
                    }
                    Err(e) => return Err(AppError::io("install the verified mod", e.error)),
                }
                installed.insert(
                    project.clone(),
                    InstalledVersion {
                        id: version.clone(),
                        filename: spec.relative.to_string_lossy().into_owned(),
                    },
                );
            }
            atomic_json(&folder.join(".rocket-modrinth.json"), &installed)
        })();
        if result.is_err() {
            for file in added {
                let _ = fs::remove_file(file);
            }
        }
        result?;
        Ok(self.primary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "uses live Modrinth search, version and CDN endpoints"]
    async fn live_modrinth_matching_and_installation() {
        let game = tempfile::tempdir().unwrap();
        let mut instance = Instance {
            id: "test".into(),
            name: "Modrinth test".into(),
            version: "1.21.1".into(),
            version_type: "release".into(),
            loader: Loader::Fabric,
            loader_version: None,
            game_directory: game.path().to_path_buf(),
            ram_mb: 4096,
            java_path: None,
            created_at: 0,
            updated_at: 0,
            last_selected_at: 0,
        };
        let page = search(&instance, "Sodium", 0, 20, None).await.unwrap();
        let project = page.hits.iter().find(|hit| hit.title == "Sodium").unwrap();
        let version = versions(&instance, &project.project_id, 20)
            .await
            .unwrap()
            .into_iter()
            .find(|version| version.version_type == "release")
            .unwrap();
        let staged = stage(&instance, &project.project_id, &version.id, 20)
            .await
            .unwrap();
        let target = game.path().join("mods").join(&version.file_name);
        fs::write(&target, b"existing file").unwrap();
        assert!(staged.publish(game.path()).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"existing file");
        fs::remove_file(&target).unwrap();
        let filename = stage(&instance, &project.project_id, &version.id, 20)
            .await
            .unwrap()
            .publish(game.path())
            .unwrap();
        assert_eq!(filename, version.file_name);
        assert!(target.metadata().unwrap().len() > 0);
        let iris_game = tempfile::tempdir().unwrap();
        let mut iris_instance = instance.clone();
        iris_instance.game_directory = iris_game.path().to_path_buf();
        let iris_page = search(&iris_instance, "Iris Shaders", 0, 20, None).await.unwrap();
        let iris = iris_page
            .hits
            .iter()
            .find(|hit| hit.title == "Iris Shaders")
            .unwrap();
        let iris_version = versions(&iris_instance, &iris.project_id, 20)
            .await
            .unwrap()
            .into_iter()
            .find(|version| {
                version.version_type == "release" && !version.required_dependencies.is_empty()
            })
            .unwrap();
        stage(&iris_instance, &iris.project_id, &iris_version.id, 20)
            .await
            .unwrap()
            .publish(iris_game.path())
            .unwrap();
        let installed = installed_index(&iris_game.path().join("mods")).unwrap();
        assert!(installed.contains_key(&iris.project_id));
        assert!(installed.contains_key(&project.project_id));
        #[cfg(all(windows, debug_assertions))]
        if std::env::var_os("ROCKET_SMOKE_GAME").is_some() {
            use super::super::{fabric, install, java, launch, metadata::Platform, versions};

            let cache = tempfile::tempdir().unwrap();
            let root = cache.path();
            let catalogue = root.join("versions.json");
            versions::load_catalog(&catalogue, true, 30).await.unwrap();
            let manifest = versions::read_cache(&catalogue).unwrap().manifest;
            let game_version = &iris_instance.version;
            let loader = fabric::loader_versions(game_version, 30)
                .await
                .unwrap()
                .into_iter()
                .find(|loader| loader.stable)
                .unwrap();
            install::install(root, &manifest, game_version, "test", 30, |_| {})
                .await
                .unwrap();
            fabric::install(root, game_version, &loader.version, "test", 30, |_| {})
                .await
                .unwrap();
            let platform = Platform::windows().unwrap();
            let vanilla = install::verify_installed(root, game_version, &platform)
                .unwrap()
                .unwrap();
            let layered = fabric::verify(
                root,
                game_version,
                &loader.version,
                &platform,
                &vanilla,
                true,
            )
            .unwrap()
            .unwrap();
            let java = java::resolve(None, root, layered.version.java_major(), platform.arch)
                .await
                .unwrap();
            launch::smoke_render(&layered, &iris_instance, &platform, java).await;
        }
        instance.version = "1.12.2".into();
        assert!(stage(&instance, &project.project_id, &version.id, 20)
            .await
            .is_err());
    }
}
