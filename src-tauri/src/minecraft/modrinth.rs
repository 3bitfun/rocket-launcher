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
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    versions: Vec<String>,
    date_modified: Option<String>,
    #[serde(default)]
    gallery: Vec<String>,
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

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PackFilters {
    game: String,
    loader: String,
    category: String,
    side: String,
    collection: String,
}

pub async fn search_packs(query: &str, offset: u32, sort: &str, filters: PackFilters, timeout: u64) -> Result<SearchPage> {
    if query.len() > 400 || offset > 980 || offset % PAGE_SIZE != 0 {
        return Err(error("Invalid Modrinth search range."));
    }
    if !matches!(sort, "relevance" | "downloads" | "follows" | "updated" | "newest") {
        return Err(error("Unknown Modrinth sort order."));
    }
    let mut facets = vec![vec!["project_type:modpack".to_owned()]];
    for (field, value) in [("versions", filters.game), ("categories", filters.loader), ("categories", filters.category)] {
        if value.is_empty() { continue; }
        if value.len() > 48 || !value.bytes().all(|byte| byte.is_ascii_alphanumeric() || b".-_".contains(&byte)) {
            return Err(error("Invalid Modrinth filter."));
        }
        facets.push(vec![format!("{field}:{value}")]);
    }
    match filters.side.as_str() {
        "client" => facets.push(vec!["client_side:required".into(), "client_side:optional".into()]),
        "server" => facets.push(vec!["server_side:required".into(), "server_side:optional".into()]),
        "" => {}
        _ => return Err(error("Unknown compatibility filter.")),
    }
    match filters.collection.as_str() {
        "popular" => facets.push(vec!["downloads>=10000".into()]),
        "featured" => facets.push(["1KVo5zza", "BYN9yKrV", "NHUidpfR"].map(|id| format!("project_id:{id}")).into()),
        "" => {}
        _ => return Err(error("Unknown modpack collection.")),
    }
    get(client(timeout)?.get(format!("{BASE}/search")).query(&[
        ("query", query.trim()),
        ("facets", &serde_json::to_string(&facets).map_err(|_| error("Invalid search filters."))?),
        ("index", sort),
        ("limit", &PAGE_SIZE.to_string()),
        ("offset", &offset.to_string()),
    ])).await
}

#[derive(Serialize, Deserialize)]
pub struct GalleryImage {
    url: String,
    title: Option<String>,
    description: Option<String>,
    featured: bool,
}

#[derive(Deserialize)]
struct PackProject {
    id: String,
    title: String,
    project_type: String,
    body: String,
    client_side: Option<String>,
    #[serde(default)]
    gallery: Vec<GalleryImage>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackDetails {
    body: String,
    gallery: Vec<GalleryImage>,
    versions: Vec<PackVersion>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackVersion {
    id: String,
    version_number: String,
    version_type: String,
    date_published: String,
    game_versions: Vec<String>,
    loaders: Vec<String>,
    required_dependencies: Vec<String>,
}

async fn pack_project(client: &reqwest::Client, project: &str) -> Result<PackProject> {
    let project = id(project)?;
    let details: PackProject = get(client.get(format!("{BASE}/project/{project}"))).await?;
    if details.id != project || details.project_type != "modpack" {
        return Err(error("This project is not a Modrinth modpack."));
    }
    Ok(details)
}

fn pack_file(version: &Version) -> Option<&VersionFile> {
    let files = version.files.iter().filter(|file| file.filename.to_ascii_lowercase().ends_with(".mrpack"));
    files.clone().find(|file| file.primary).or_else(|| files.clone().next())
}

pub async fn pack_details(project: &str, timeout: u64) -> Result<PackDetails> {
    let client = client(timeout)?;
    let details = pack_project(&client, project).await?;
    let versions: Vec<Version> = get(client.get(format!("{BASE}/project/{project}/version"))
        .query(&[("include_changelog", "false")])).await?;
    let mut versions: Vec<_> = versions.into_iter().filter(|version| version.project_id == project && pack_file(version).is_some())
        .map(|version| PackVersion {
            id: version.id,
            version_number: version.version_number,
            version_type: version.version_type,
            date_published: version.date_published,
            game_versions: version.game_versions,
            loaders: version.loaders,
            required_dependencies: version.dependencies.into_iter().filter(|item| item.dependency_type == "required")
                .map(|item| item.file_name.or(item.project_id).or(item.version_id).unwrap_or_else(|| "Unidentified dependency".into())).collect(),
        }).collect();
    versions.sort_by(|left, right| right.date_published.cmp(&left.date_published));
    Ok(PackDetails { body: details.body, gallery: details.gallery, versions })
}

#[derive(Serialize, Deserialize)]
struct PackReceipt { project: String, version: String }

const PACK_RECEIPT: &str = ".rocket-modpack.json";
const PACK_BYTE_LIMIT: u64 = 2 * 1024 * 1024 * 1024;

pub fn has_pack(instances: &[Instance], project: &str, version: &str) -> bool {
    use std::io::Read;
    instances.iter().any(|instance| {
        let Ok(file) = fs::File::open(instance.game_directory.join(PACK_RECEIPT)) else { return false; };
        serde_json::from_reader::<_, PackReceipt>(file.take(2048))
            .is_ok_and(|receipt| receipt.project == project && receipt.version == version)
    })
}

#[derive(Serialize, Clone)]
pub struct PackProgress {
    phase: &'static str,
    completed: usize,
    total: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackIndex {
    format_version: u32,
    game: String,
    dependencies: BTreeMap<String, String>,
    files: Vec<PackIndexFile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackIndexFile {
    path: String,
    hashes: Hashes,
    downloads: Vec<String>,
    file_size: u64,
    #[serde(default)]
    env: BTreeMap<String, String>,
}

fn archive_error(reason: impl std::fmt::Display) -> AppError {
    error(format!("Could not read the modpack archive: {reason}"))
}

fn pack_path(value: &str) -> Result<std::path::PathBuf> {
    if value.len() > 512 || value.split('/').any(|part| part.to_ascii_lowercase().starts_with(".rocket-")) {
        return Err(error("The modpack uses a reserved or excessively long path."));
    }
    super::metadata::relative(value)
}

fn unpack_pack(source: &Path, destination: &Path) -> Result<PackIndex> {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(fs::File::open(source).map_err(archive_error)?).map_err(archive_error)?;
    if zip.len() > 4096 { return Err(error("This modpack has too many archive entries.")); }
    let mut index_data = Vec::new();
    zip.by_name("modrinth.index.json").map_err(archive_error)?
        .take(8 * 1024 * 1024 + 1).read_to_end(&mut index_data).map_err(archive_error)?;
    if index_data.len() > 8 * 1024 * 1024 { return Err(error("The modpack index is too large.")); }
    let index: PackIndex = serde_json::from_slice(&index_data).map_err(archive_error)?;
    if index.format_version != 1 || index.game != "minecraft" || index.files.len() > 4096 {
        return Err(error("This modpack uses an unsupported archive format."));
    }
    let mut bytes = 0u64;
    for prefix in ["overrides/", "client-overrides/"] {
        for i in 0..zip.len() {
            let entry = zip.by_index(i).map_err(archive_error)?;
            if entry.is_dir() { continue; }
            let Some(path) = entry.name().strip_prefix(prefix) else { continue; };
            let target = destination.join(pack_path(path)?);
            bytes = bytes.saturating_add(entry.size());
            if bytes > PACK_BYTE_LIMIT || entry.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000) {
                return Err(error("The modpack has unsafe or oversized overrides."));
            }
            let parent = target.parent().ok_or_else(|| error("Invalid override path."))?;
            fs::create_dir_all(parent).map_err(archive_error)?;
            // Client overrides intentionally replace common overrides per the .mrpack format.
            let mut output = fs::OpenOptions::new().write(true).create(true).truncate(true)
                .open(&target).map_err(archive_error)?;
            let expected = entry.size();
            let copied = std::io::copy(&mut entry.take(expected + 1), &mut output).map_err(archive_error)?;
            if copied != expected { return Err(error("An override did not match its declared size.")); }
        }
    }
    Ok(index)
}

pub async fn install_pack(
    project: &str, version: &str, game: &str, selected_loader: &str,
    directory: &Path, cache: &Path, ram_mb: u32, timeout: u64,
    progress: &(impl Fn(PackProgress) + Sync),
) -> Result<crate::models::InstanceInput> {
    use futures_util::{stream, StreamExt, TryStreamExt};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let report = |phase, completed, total| progress(PackProgress { phase, completed, total });
    report("Downloading modpack archive", 0, 0);
    let api = client(timeout)?;
    let project_details = pack_project(&api, project).await?;
    if project_details.client_side.as_deref() == Some("unsupported") {
        return Err(incompatible("This modpack is for servers only."));
    }
    let version = version_by_id(&api, version).await?;
    if version.project_id != project || !matches(&version, selected_loader, game) {
        return Err(incompatible("This version does not match the selected Minecraft version and loader."));
    }
    let loader = match selected_loader {
        "fabric" => Loader::Fabric,
        "forge" => Loader::Forge,
        _ => return Err(incompatible("This launcher can install Fabric and supported Forge packs.")),
    };
    let file = pack_file(&version).ok_or_else(|| error("This release has no .mrpack download."))?;
    let mut archive = jar_spec(file)?;
    archive.relative = format!("{}.mrpack", archive.artifact.sha1.to_ascii_lowercase()).into();
    let downloads = download::client(timeout)?;
    download::ensure(&downloads, cache, &archive, &|_| {}).await?;
    report("Reading pack requirements and overrides", 0, 0);
    let source = cache.join(&archive.relative);
    let destination = directory.to_owned();
    let index = tokio::task::spawn_blocking(move || unpack_pack(&source, &destination))
        .await.map_err(archive_error)??;
    let loader_version = index.dependencies.get(if loader == Loader::Fabric { "fabric-loader" } else { "forge" })
        .ok_or_else(|| incompatible("The archive does not declare the selected mod loader."))?.clone();
    if index.dependencies.get("minecraft").map(String::as_str) != Some(game) ||
        index.dependencies.keys().any(|key| !matches!(key.as_str(), "minecraft" | "fabric-loader" | "forge")) ||
        index.dependencies.len() != 2
    {
        return Err(incompatible("The archive requires a different Minecraft version or loader."));
    }
    let input = crate::models::InstanceInput {
        name: project_details.title.chars().filter(|c| !c.is_control()).take(64).collect(),
        version: game.to_owned(), loader, loader_version: Some(loader_version), ram_mb,
        java_path: None, parent_directory: None,
    };
    crate::instances::validate_input(&input)?;
    let mut paths = std::collections::HashSet::new();
    let mut bundled_mods = BTreeMap::new();
    let mut size = 0u64;
    let specs = index.files.into_iter().filter(|file|
        !file.env.get("client").is_some_and(|value| value == "unsupported" || value == "optional")
    ).map(|file| {
        let path = pack_path(&file.path)?;
        if !paths.insert(file.path.to_lowercase()) || directory.join(&path).exists() {
            return Err(error(format!("The pack contains overlapping files at {}.", file.path)));
        }
        size = size.saturating_add(file.file_size);
        if file.file_size == 0 || file.file_size > MAX_JAR_BYTES || size > PACK_BYTE_LIMIT {
            return Err(error("The pack exceeds the download size limit."));
        }
        let url = file.downloads.into_iter().find(|url| url.starts_with("https://cdn.modrinth.com/"))
            .ok_or_else(|| error(format!("{} requires a download from an unsupported host.", file.path)))?;
        if let Some(filename) = file.path.strip_prefix("mods/").filter(|filename|
            !filename.contains('/') && filename.to_ascii_lowercase().ends_with(".jar"))
        {
            let parsed = reqwest::Url::parse(&url).map_err(|_| error("Invalid Modrinth download URL."))?;
            let parts: Vec<_> = parsed.path().trim_start_matches('/').split('/').collect();
            if let ["data", project_id, "versions", version_id, _] = parts.as_slice() {
                if id(project_id).is_ok() && id(version_id).is_ok() {
                    let bundled = InstalledVersion { id: (*version_id).into(), filename: filename.into() };
                    if bundled_mods.insert((*project_id).to_owned(), bundled).is_some() {
                        return Err(incompatible(format!("The modpack includes multiple JARs for project {project_id}.")));
                    }
                }
            }
        }
        FileSpec::new(path, Artifact { url, sha1: file.hashes.sha1, size: file.file_size, path: None })
    }).collect::<Result<Vec<_>>>()?;
    let total = specs.len();
    let completed = AtomicUsize::new(0);
    report("Downloading and verifying pack files", 0, total);
    let downloads = &downloads;
    let completed = &completed;
    let report = &report;
    stream::iter(specs).map(|spec| async move {
        download::ensure(downloads, directory, &spec, &|_| {}).await.map_err(|reason|
            error(format!("{}: {}", spec.relative.display(), reason.message)))?;
        report("Downloading and verifying pack files", completed.fetch_add(1, Ordering::Relaxed) + 1, total);
        Ok::<_, AppError>(())
    }).buffer_unordered(6).try_collect::<Vec<_>>().await?;
    if !bundled_mods.is_empty() {
        atomic_json(&directory.join("mods/.rocket-modrinth.json"), &bundled_mods)?;
    }
    let instance = Instance {
        id: String::new(), name: input.name.clone(), version: game.into(), version_type: "release".into(),
        loader, loader_version: input.loader_version.clone(), game_directory: directory.into(),
        ram_mb, java_path: None, created_at: 0, updated_at: 0, last_selected_at: 0,
    };
    let required: Vec<_> = version.dependencies.iter().filter(|dep| dep.dependency_type == "required").collect();
    report("Resolving required mod dependencies", 0, required.len());
    for (index, dependency) in required.iter().enumerate() {
        if let Some(bundled) = dependency.project_id.as_ref().and_then(|id| bundled_mods.get(id)) {
            if dependency.version_id.as_ref().is_some_and(|id| id != &bundled.id) {
                return Err(incompatible(format!("{} conflicts with a pinned pack dependency.", bundled.filename)));
            }
            report("Resolving required mod dependencies", index + 1, required.len());
            continue;
        }
        let target = if let Some(pinned) = &dependency.version_id {
            version_by_id(&api, pinned).await?
        } else if let Some(project) = &dependency.project_id {
            matching_versions(&api, &instance, project).await?.into_iter()
                .filter(|version| jar(version).is_some()).min_by_key(|version| version.version_type != "release")
                .ok_or_else(|| incompatible(format!("Required dependency {project} has no compatible mod file.")))?
        } else {
            let filename = dependency.file_name.as_deref().ok_or_else(|| incompatible("A required dependency has no identifier."))?;
            if !directory.join("mods").join(segment(filename)?).is_file() {
                return Err(incompatible(format!("Required external file {filename} is missing from the pack.")));
            }
            report("Resolving required mod dependencies", index + 1, required.len());
            continue;
        };
        if dependency.project_id.as_ref().is_some_and(|id| id != &target.project_id) {
            return Err(error("Modrinth returned the wrong dependency project."));
        }
        stage_version(&instance, &api, target, None).await?.publish(directory)?;
        report("Resolving required mod dependencies", index + 1, required.len());
    }
    report("Registering instance", 0, 0);
    atomic_json(&directory.join(PACK_RECEIPT), &PackReceipt { project: project.into(), version: version.id })?;
    Ok(input)
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
