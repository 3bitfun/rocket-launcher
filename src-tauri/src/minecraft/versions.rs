use crate::{
    errors::{AppError, Result},
    models::now,
    storage::atomic_json,
};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};

pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const MAX_MANIFEST_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub release_time: String,
    pub sha1: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedManifest {
    pub fetched_at: u64,
    pub manifest: Manifest,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionCatalog {
    pub latest: LatestVersions,
    pub versions: Vec<VersionEntry>,
    pub fetched_at: u64,
    pub source: &'static str,
    pub warning: Option<String>,
}

fn validate_manifest(manifest: Manifest) -> Result<Manifest> {
    if manifest.versions.is_empty() || manifest.versions.len() > 20000 {
        return Err(AppError::new(
            "MANIFEST_INVALID",
            "Mojang returned an empty or oversized version catalogue.",
        ));
    }
    let mut ids = std::collections::HashSet::new();
    for entry in &manifest.versions {
        let url = reqwest::Url::parse(&entry.url)
            .map_err(|_| AppError::new("MANIFEST_INVALID", "A version metadata URL is invalid."))?;
        if url.scheme() != "https"
            || !matches!(
                url.host_str(),
                Some("piston-meta.mojang.com" | "launchermeta.mojang.com")
            )
            || !url.username().is_empty()
            || url.password().is_some()
            || entry.id.is_empty()
            || entry.id.len() > 128
            || entry.id.contains(['/', '\\'])
            || entry.id.chars().any(char::is_control)
            || entry.sha1.len() != 40
            || !entry.sha1.bytes().all(|b| b.is_ascii_hexdigit())
            || !ids.insert(entry.id.clone())
        {
            return Err(AppError::new(
                "MANIFEST_INVALID",
                "Mojang version metadata failed validation. The previous cache has been preserved.",
            ));
        }
    }
    if !ids.contains(&manifest.latest.release) || !ids.contains(&manifest.latest.snapshot) {
        return Err(AppError::new(
            "MANIFEST_INVALID",
            "The catalogue's latest version entries are missing.",
        ));
    }
    Ok(manifest)
}

pub fn parse_manifest(bytes: &[u8]) -> Result<Manifest> {
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(AppError::new(
            "MANIFEST_INVALID",
            "The version catalogue exceeds the size limit.",
        ));
    }
    let manifest = serde_json::from_slice(bytes).map_err(|e| {
        AppError::new(
            "MANIFEST_INVALID",
            format!("Could not parse Mojang's version catalogue: {e}"),
        )
    })?;
    validate_manifest(manifest)
}

pub fn read_cache(path: &Path) -> Result<CachedManifest> {
    let file = std::fs::File::open(path)
        .map_err(|e| AppError::io("open the cached version catalogue", e))?;
    if file
        .metadata()
        .map_err(|e| AppError::io("inspect the catalogue cache", e))?
        .len()
        > (MAX_MANIFEST_BYTES * 2) as u64
    {
        return Err(AppError::new(
            "CACHE_INVALID",
            "The cached version catalogue is too large.",
        ));
    }
    let cached: CachedManifest = serde_json::from_reader(file).map_err(|e| {
        AppError::new(
            "CACHE_INVALID",
            format!("The cached version catalogue is invalid: {e}"),
        )
    })?;
    validate_manifest(cached.manifest.clone())?;
    Ok(cached)
}

pub fn version_type(cache: &Path, id: &str) -> Result<String> {
    let cached = read_cache(cache).map_err(|_| AppError::new("VERSIONS_REQUIRED", "Refresh the Mojang version catalogue before creating or changing an instance's version."))?;
    cached
        .manifest
        .versions
        .iter()
        .find(|v| v.id == id && matches!(v.kind.as_str(), "release" | "snapshot"))
        .map(|v| v.kind.clone())
        .ok_or_else(|| {
            AppError::new(
                "VERSION_UNKNOWN",
                "Choose a release or snapshot from the official Mojang catalogue.",
            )
        })
}

async fn fetch(timeout: u64) -> Result<Manifest> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("RocketLauncher/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(timeout))
        .connect_timeout(Duration::from_secs(timeout.min(10)))
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| {
            AppError::new(
                "NETWORK",
                format!("Could not initialize the HTTP client: {e}"),
            )
        })?;
    let mut response = client.get(MANIFEST_URL).send().await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| AppError::new("NETWORK", format!("Could not reach Mojang's version service. Check your connection and try again. Details: {e}")))?;
    if response
        .content_length()
        .is_some_and(|len| len > MAX_MANIFEST_BYTES as u64)
    {
        return Err(AppError::new(
            "MANIFEST_INVALID",
            "The version catalogue exceeds the size limit.",
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| {
        AppError::new(
            "NETWORK",
            format!("Version catalogue download was interrupted: {e}"),
        )
    })? {
        if bytes.len() + chunk.len() > MAX_MANIFEST_BYTES {
            return Err(AppError::new(
                "MANIFEST_INVALID",
                "The version catalogue exceeds the size limit.",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    parse_manifest(&bytes)
}

fn catalogue(
    cached: CachedManifest,
    source: &'static str,
    warning: Option<String>,
) -> VersionCatalog {
    VersionCatalog {
        latest: cached.manifest.latest,
        versions: cached
            .manifest
            .versions
            .into_iter()
            .filter(|v| matches!(v.kind.as_str(), "release" | "snapshot"))
            .collect(),
        fetched_at: cached.fetched_at,
        source,
        warning,
    }
}

pub fn cached_catalog(path: &Path) -> Result<VersionCatalog> {
    let cache = read_cache(path).map_err(|_| {
        AppError::new(
            "OFFLINE_MODE",
            "No version catalogue is cached. Turn off Offline Mode to download one.",
        )
    })?;
    Ok(catalogue(cache, "cache", None))
}

pub async fn load_catalog(path: &Path, refresh: bool, timeout: u64) -> Result<VersionCatalog> {
    let cached = read_cache(path).ok();
    if !refresh {
        if let Some(cache) = &cached {
            if now().saturating_sub(cache.fetched_at) < 3600 {
                return Ok(catalogue(cache.clone(), "cache", None));
            }
        }
    }
    let mut result = fetch(timeout).await;
    // Retry a transient connection failure once; never retry malformed responses.
    if matches!(&result, Err(error) if error.code == "NETWORK") {
        tokio::time::sleep(Duration::from_millis(500)).await;
        result = fetch(timeout).await;
    }
    match result {
        Ok(manifest) => {
            let cached = CachedManifest {
                fetched_at: now(),
                manifest,
            };
            // The UI must not offer newly fetched versions if they cannot be used by native commands.
            atomic_json(path, &cached)?;
            tracing::info!(
                event = "manifest_fetched",
                count = cached.manifest.versions.len()
            );
            Ok(catalogue(cached, "network", None))
        }
        Err(error) => {
            tracing::warn!(event = "manifest_fetch_failed", error = %error);
            match cached {
                Some(cache) => Ok(catalogue(
                    cache,
                    "cache",
                    Some(format!("Using saved versions. {}", error.message)),
                )),
                None => Err(error),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "latest": {"release":"1.21.1","snapshot":"24w33a"},
            "versions": [
                {"id":"1.21.1","type":"release","url":"https://piston-meta.mojang.com/v1/packages/test/1.21.1.json","releaseTime":"2024-08-08T12:00:00Z","sha1":"0000000000000000000000000000000000000000"},
                {"id":"24w33a","type":"snapshot","url":"https://piston-meta.mojang.com/v1/packages/test/24w33a.json","releaseTime":"2024-08-15T12:00:00Z","sha1":"1111111111111111111111111111111111111111"}
            ]
        })).unwrap()
    }

    #[test]
    fn manifest_parsing_cache_and_version_lookup() {
        let manifest = parse_manifest(&fixture()).unwrap();
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("versions.json");
        atomic_json(
            &path,
            &CachedManifest {
                fetched_at: now(),
                manifest,
            },
        )
        .unwrap();
        assert_eq!(version_type(&path, "1.21.1").unwrap(), "release");
        assert_eq!(version_type(&path, "24w33a").unwrap(), "snapshot");
        assert!(version_type(&path, "../../bad").is_err());
        assert_eq!(read_cache(&path).unwrap().manifest.versions.len(), 2);
    }

    #[test]
    fn malformed_or_untrusted_metadata_is_rejected() {
        assert!(parse_manifest(b"not json").is_err());
        let malicious = String::from_utf8(fixture())
            .unwrap()
            .replace("https://piston-meta.mojang.com", "http://example.com");
        assert!(parse_manifest(malicious.as_bytes()).is_err());
        assert!(parse_manifest(&vec![0; MAX_MANIFEST_BYTES + 1]).is_err());
    }

    #[test]
    #[ignore = "Live Mojang endpoint; run explicitly when network access is available"]
    fn live_manifest_fetch() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("versions.json");
        let result = runtime.block_on(load_catalog(&path, true, 20)).unwrap();
        assert_eq!(result.source, "network");
        assert!(result.versions.len() > 100);
        assert!(path.exists());
        let cached = runtime.block_on(load_catalog(&path, false, 20)).unwrap();
        assert_eq!(cached.source, "cache");
    }
}
