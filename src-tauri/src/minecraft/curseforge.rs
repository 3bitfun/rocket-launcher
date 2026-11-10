use super::{
    download::{self, FileSpec},
    metadata::{segment, Artifact},
    modrinth::{ModVersion, Project, SearchPage, StagedMod, PAGE_SIZE},
};
use crate::{
    errors::{AppError, Result},
    instances::mods_directory,
    models::{Instance, Loader},
};
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{de::DeserializeOwned, Deserialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const API: &str = "https://api.curseforge.com/v1";
const MINECRAFT: u32 = 432;
const MODS: u32 = 6;
const MAX_FILE: u64 = 256 * 1024 * 1024;
const CREDENTIAL: &str = "curseforge:api-key";

fn error(message: impl Into<String>) -> AppError {
    AppError::new("CURSEFORGE", message)
}

fn download_host(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && matches!(
            url.host_str(),
            Some("edge.forgecdn.net" | "media.forgecdn.net" | "mediafilez.forgecdn.net")
        )
}

pub fn set_key(key: Option<String>) -> Result<()> {
    let Some(key) = key else {
        return crate::auth::credential_store::delete(CREDENTIAL);
    };
    let key = key.trim();
    if key.is_empty() || key.len() > 512 || HeaderValue::from_str(key).is_err() {
        return Err(error("Enter a valid CurseForge API key."));
    }
    crate::auth::credential_store::write(CREDENTIAL, key.as_bytes())
}

pub fn has_saved_key() -> Result<bool> {
    Ok(crate::auth::credential_store::read(CREDENTIAL)?.is_some())
}

fn client(timeout: u64) -> Result<reqwest::Client> {
    let saved = crate::auth::credential_store::read(CREDENTIAL)?;
    let env = std::env::var("ROCKET_CURSEFORGE_API_KEY").unwrap_or_default();
    let key = saved.as_deref().unwrap_or_else(|| env.trim().as_bytes());
    if key.is_empty() {
        return Err(error(
            "Add your CurseForge API key under API key setup to browse mods.",
        ));
    }
    let mut value =
        HeaderValue::from_bytes(key).map_err(|_| error("The CurseForge API key is invalid."))?;
    value.set_sensitive(true);
    let mut headers = HeaderMap::new();
    headers.insert("x-api-key", value);
    reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(timeout.clamp(5, 120)))
        .user_agent(concat!("RocketLauncher/", env!("CARGO_PKG_VERSION")))
        .default_headers(headers)
        .build()
        .map_err(|_| error("Could not initialize CurseForge."))
}

#[derive(Deserialize)]
struct Response<T> {
    data: T,
}

async fn get<T: DeserializeOwned>(request: reqwest::RequestBuilder) -> Result<T> {
    let mut response = request
        .send()
        .await
        .map_err(|_| error("Could not reach CurseForge."))?;
    if !response.status().is_success() {
        return Err(error(format!(
            "CurseForge returned HTTP {}. Check your API key and try again.",
            response.status().as_u16()
        )));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| error("CurseForge response was interrupted."))?
    {
        if body.len() + chunk.len() > 8 * 1024 * 1024 {
            return Err(error("CurseForge response exceeded the size limit."));
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| error("CurseForge returned an invalid response."))
}

fn loader(instance: &Instance) -> Result<&'static str> {
    match instance.loader {
        Loader::Fabric => Ok("4"),
        Loader::Forge => Ok("1"),
        Loader::Vanilla => Err(error(
            "Choose a Fabric or Forge instance to browse CurseForge mods.",
        )),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Image {
    thumbnail_url: String,
    url: String,
}

#[derive(Deserialize)]
struct Named {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Mod {
    id: u32,
    name: String,
    summary: String,
    download_count: u64,
    logo: Option<Image>,
    #[serde(default)]
    screenshots: Vec<Image>,
    #[serde(default)]
    authors: Vec<Named>,
    #[serde(default)]
    categories: Vec<Named>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pagination {
    total_count: u32,
}

#[derive(Deserialize)]
struct Search {
    data: Vec<Mod>,
    pagination: Pagination,
}

pub async fn search(
    instance: &Instance,
    query: &str,
    offset: u32,
    sort: Option<&str>,
    timeout: u64,
) -> Result<SearchPage> {
    if query.len() > 400 || offset > 980 || offset % PAGE_SIZE != 0 {
        return Err(error("Invalid CurseForge search range."));
    }
    let sort = match sort {
        None if query.trim().is_empty() => "2",
        None => "1",
        Some("downloads") => "6",
        Some("updated") => "3",
        _ => return Err(error("Unknown CurseForge sort order.")),
    };
    let results: Search = get(client(timeout)?.get(format!("{API}/mods/search")).query(&[
        ("gameId", MINECRAFT.to_string().as_str()),
        ("classId", MODS.to_string().as_str()),
        ("searchFilter", query.trim()),
        ("gameVersion", instance.version.as_str()),
        ("modLoaderType", loader(instance)?),
        ("index", &offset.to_string()),
        ("pageSize", &PAGE_SIZE.to_string()),
        ("sortField", sort),
        ("sortOrder", "desc"),
    ]))
    .await?;
    Ok(SearchPage {
        total_hits: results.pagination.total_count,
        hits: results
            .data
            .into_iter()
            .map(|item| Project {
                project_id: item.id.to_string(),
                title: item.name,
                description: item.summary,
                author: item
                    .authors
                    .into_iter()
                    .map(|author| author.name)
                    .collect::<Vec<_>>()
                    .join(", "),
                downloads: item.download_count,
                icon_url: item.logo.map(|image| image.thumbnail_url),
                featured_gallery: item.screenshots.first().map(|image| image.url.clone()),
                display_categories: item
                    .categories
                    .into_iter()
                    .map(|category| category.name)
                    .collect(),
                categories: vec![],
                versions: vec![],
                date_modified: None,
                gallery: item
                    .screenshots
                    .into_iter()
                    .take(8)
                    .map(|image| image.url)
                    .collect(),
            })
            .collect(),
    })
}

#[derive(Deserialize)]
struct Hash {
    algo: u8,
    value: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Dependency {
    mod_id: u32,
    relation_type: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct File {
    id: u32,
    mod_id: u32,
    game_id: u32,
    is_available: bool,
    file_name: String,
    display_name: String,
    release_type: u8,
    file_length: u64,
    download_url: Option<String>,
    game_versions: Vec<String>,
    hashes: Vec<Hash>,
    dependencies: Vec<Dependency>,
}

impl File {
    fn release(&self) -> &'static str {
        match self.release_type {
            1 => "release",
            2 => "beta",
            _ => "alpha",
        }
    }

    fn spec(&self) -> Result<FileSpec> {
        let url = self.download_url.clone().ok_or_else(|| {
            error("The author does not allow this file to be downloaded by third-party launchers.")
        })?;
        let host = reqwest::Url::parse(&url).map_err(|_| error("Invalid CurseForge file URL."))?;
        if !download_host(&host) || !(1..=MAX_FILE).contains(&self.file_length) {
            return Err(error(
                "The CurseForge download host or file size is unsupported.",
            ));
        }
        let sha1 = self
            .hashes
            .iter()
            .find(|hash| hash.algo == 1)
            .ok_or_else(|| error("CurseForge did not supply a SHA-1 checksum for this file."))?;
        FileSpec::new(
            segment(&self.file_name)?.into(),
            Artifact {
                path: None,
                url,
                sha1: sha1.value.clone(),
                size: self.file_length,
            },
        )
    }
}

fn required(file: &File) -> impl Iterator<Item = u32> + '_ {
    file.dependencies
        .iter()
        .filter(|dependency| dependency.relation_type == 3)
        .map(|dependency| dependency.mod_id)
}

async fn files(client: &reqwest::Client, instance: &Instance, project: u32) -> Result<Vec<File>> {
    let result: Response<Vec<File>> =
        get(client.get(format!("{API}/mods/{project}/files")).query(&[
            ("gameVersion", instance.version.as_str()),
            ("modLoaderType", loader(instance)?),
            ("pageSize", "50"),
        ]))
        .await?;
    Ok(result
        .data
        .into_iter()
        .filter(|file| {
            file.game_id == MINECRAFT
                && file.mod_id == project
                && file.is_available
                && file.game_versions.contains(&instance.version)
                && file.file_name.to_ascii_lowercase().ends_with(".jar")
        })
        .collect())
}

pub async fn versions(instance: &Instance, project: u32, timeout: u64) -> Result<Vec<ModVersion>> {
    let files = files(&client(timeout)?, instance, project).await?;
    Ok(files
        .into_iter()
        .map(|file| ModVersion {
            id: file.id.to_string(),
            version_type: file.release().into(),
            required_dependencies: required(&file).map(|id| id.to_string()).collect(),
            version_number: file.display_name,
            file_name: file.file_name,
        })
        .collect())
}

async fn resolve(
    client: &reqwest::Client,
    instance: &Instance,
    project: u32,
    requested: Option<u32>,
) -> Result<File> {
    let mut files = files(client, instance, project).await?;
    if let Some(id) = requested {
        return files.into_iter().find(|file| file.id == id).ok_or_else(|| {
            error("This CurseForge file does not match the selected version and loader.")
        });
    }
    files.sort_by_key(|file| (file.release_type, std::cmp::Reverse(file.id)));
    files.into_iter().find(|file| file.download_url.is_some())
        .ok_or_else(|| error(format!("CurseForge project {project} has no downloadable file for this version and loader.")))
}

pub async fn stage(
    instance: &Instance,
    project: u32,
    requested: Option<u32>,
    timeout: u64,
) -> Result<StagedMod> {
    let api = client(timeout)?;
    let selected = resolve(&api, instance, project, requested).await?;
    let primary = selected.file_name.clone();
    let mut queue = VecDeque::from([selected]);
    let mut seen = BTreeSet::from([project]);
    let mut planned = BTreeMap::new();
    let mut names = BTreeSet::new();
    while let Some(file) = queue.pop_front() {
        let spec = file.spec()?;
        if !names.insert(file.file_name.to_ascii_lowercase()) {
            return Err(error("Two CurseForge files use the same name."));
        }
        planned.insert(file.mod_id.to_string(), (file.id.to_string(), spec));
        for id in required(&file) {
            if seen.insert(id) {
                if seen.len() > 32 {
                    return Err(error("This mod has too many required dependencies."));
                }
                queue.push_back(resolve(&api, instance, id, None).await?);
            }
        }
    }
    let folder = mods_directory(&instance.game_directory)?;
    let staging = tempfile::Builder::new()
        .prefix(".rocket-curseforge-")
        .tempdir_in(folder)
        .map_err(|e| AppError::io("stage CurseForge files", e))?;
    // File downloads use a separate client so the API key stays on api.curseforge.com.
    let timeout = std::time::Duration::from_secs(timeout.clamp(5, 120));
    let downloads = reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(timeout)
        .read_timeout(timeout)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() < 4 && download_host(attempt.url()) {
                attempt.follow()
            } else {
                attempt.error("Unexpected CurseForge download redirect")
            }
        }))
        .user_agent(concat!("RocketLauncher/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| error("Could not initialize CurseForge downloads."))?;
    for (_, spec) in planned.values() {
        download::ensure(&downloads, staging.path(), spec, &|_| {}).await?;
    }
    Ok(StagedMod {
        staging,
        primary,
        files: planned,
        index_name: ".rocket-curseforge.json",
    })
}
