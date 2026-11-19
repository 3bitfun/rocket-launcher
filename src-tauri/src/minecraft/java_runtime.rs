use super::{java::{self, Java}, metadata::Architecture};
use crate::errors::{AppError, Result};
use futures_util::StreamExt;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{fs, io::{Read, Write}, path::{Path, PathBuf}, time::Duration};
use tokio::io::AsyncWriteExt;

const BUNDLED: &[u8] = include_bytes!("../../resources/java/temurin21-windows-x64.zip");
const BUNDLED_METADATA: &str = include_str!("../../resources/java/temurin21.json");
const MAX_ARCHIVE: u64 = 256 * 1024 * 1024;
static PROVISION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Deserialize)]
struct Asset { binary: Binary }
#[derive(Deserialize)]
struct Binary { package: Package }
#[derive(Deserialize)]
struct Package { link: String, checksum: String, size: u64 }

fn error(message: impl Into<String>) -> AppError { AppError::new("JAVA", message) }

pub fn supported(major: u32, arch: Architecture) -> bool {
    matches!(major, 8 | 16 | 17 | 21 | 25) && matches!(arch, Architecture::X64)
}

pub fn target(data: &Path, major: u32) -> PathBuf {
    data.join("runtimes").join(format!("temurin-{major}-windows-x64"))
}

fn client(timeout: u64) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(timeout.clamp(5, 120)))
        .read_timeout(Duration::from_secs(timeout.clamp(5, 120)))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let host = attempt.url().host_str().unwrap_or("");
            if attempt.previous().len() < 5 && attempt.url().scheme() == "https"
                && matches!(host, "github.com" | "release-assets.githubusercontent.com" | "objects.githubusercontent.com")
            { attempt.follow() } else { attempt.error("Unexpected Java download redirect") }
        }))
        .user_agent(concat!("RocketLauncher/", env!("CARGO_PKG_VERSION")))
        .build().map_err(|e| error(format!("Could not prepare a Java download: {e}")))
}

fn validate(package: &Package) -> Result<()> {
    let url = reqwest::Url::parse(&package.link).map_err(|_| error("Invalid Java runtime download URL."))?;
    if url.scheme() != "https" || url.host_str() != Some("github.com")
        || !url.path().starts_with("/adoptium/temurin")
        || !url.path().ends_with(".zip") || url.port().is_some()
        || !url.username().is_empty() || url.password().is_some()
        || !(1..=MAX_ARCHIVE).contains(&package.size)
        || package.checksum.len() != 64 || !package.checksum.bytes().all(|b| b.is_ascii_hexdigit())
    { return Err(error("The Java provider returned an unsupported runtime package.")); }
    Ok(())
}

fn extract(archive: &Path, destination: &Path, checksum: &str) -> Result<()> {
    let mut file = fs::File::open(archive).map_err(|e| AppError::io("open the Java archive", e))?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|e| AppError::io("verify the Java archive", e))?;
        if read == 0 { break; }
        digest.update(&buffer[..read]);
    }
    if !format!("{:x}", digest.finalize()).eq_ignore_ascii_case(checksum) {
        return Err(error("Java download failed its SHA-256 check. Try launching again."));
    }
    let file = fs::File::open(archive).map_err(|e| AppError::io("reopen the Java archive", e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| error(format!("Invalid Java archive: {e}")))?;
    if zip.len() > 15_000 { return Err(error("The Java archive has too many files.")); }
    let mut total = 0u64;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|e| error(e.to_string()))?;
        let relative = entry.enclosed_name().ok_or_else(|| error("Unsafe path in the Java archive."))?;
        // Temurin archives contain a single runtime root folder.
        let name = relative.strip_prefix(relative.components().next().ok_or_else(|| error("Empty Java archive path."))?.as_os_str())
            .map_err(|_| error("Invalid Java archive root."))?;
        if name.as_os_str().is_empty() { continue; }
        super::metadata::relative(&name.to_string_lossy().replace('\\', "/"))?;
        if entry.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000) {
            return Err(error("Links are unsupported in the Java archive."));
        }
        total = total.checked_add(entry.size()).ok_or_else(|| error("Java archive is too large."))?;
        if total > 1024 * 1024 * 1024 { return Err(error("Java archive is too large.")); }
        let path = destination.join(name);
        if entry.is_dir() {
            fs::create_dir_all(path).map_err(|e| AppError::io("create a Java runtime folder", e))?;
        } else {
            fs::create_dir_all(path.parent().ok_or_else(|| error("Invalid Java file path."))?)
                .map_err(|e| AppError::io("create a Java runtime folder", e))?;
            let mut output = fs::OpenOptions::new().write(true).create_new(true).open(path)
                .map_err(|e| AppError::io("extract a Java runtime file", e))?;
            std::io::copy(&mut entry, &mut output).map_err(|e| AppError::io("extract Java", e))?;
        }
    }
    Ok(())
}

/// Only explicit install/launch operations may provision a runtime. Readiness checks stay local.
pub async fn ensure(
    configured: Option<PathBuf>, data: &Path, major: u32, arch: Architecture,
    offline: bool, timeout: u64, report: impl Fn(&str) -> Result<()>,
) -> Result<Java> {
    let found = java::resolve(configured.clone(), data, major, arch).await;
    if configured.is_some() || found.is_ok() { return found; }
    if !supported(major, arch) { return found; }
    let _guard = PROVISION.lock().await;
    if let Ok(java) = java::resolve(None, data, major, arch).await { return Ok(java); }
    let bundled = major == 21;
    if offline && !bundled {
        return Err(error(format!("Go online once to download Java {major}. It will then work offline.")));
    }
    report(&format!("Preparing Java {major}…"))?;
    let parent = data.join("runtimes");
    fs::create_dir_all(&parent).map_err(|e| AppError::io("create the managed Java folder", e))?;
    let archive = tempfile::NamedTempFile::new_in(&parent).map_err(|e| AppError::io("stage Java", e))?;
    let package = if bundled {
        let asset: Asset = serde_json::from_str(BUNDLED_METADATA).map_err(|_| error("Bundled Java metadata is invalid."))?;
        let mut output = archive.reopen().map_err(|e| AppError::io("open the bundled Java archive", e))?;
        output.write_all(BUNDLED).map_err(|e| AppError::io("unpack bundled Java", e))?;
        asset.binary.package
    } else {
        let client = client(timeout)?;
        let response = client.get(format!("https://api.adoptium.net/v3/assets/latest/{major}/hotspot"))
            .query(&[("architecture", "x64"), ("image_type", if major == 16 { "jdk" } else { "jre" }), ("os", "windows"), ("vendor", "eclipse")])
            .send().await.map_err(|e| error(format!("Could not find Java {major}: {e}")))?
            .error_for_status().map_err(|e| error(format!("Java provider could not supply Java {major}: {e}")))?;
        if response.content_length().is_some_and(|size| size > 1024 * 1024) {
            return Err(error("Java provider returned oversized metadata."));
        }
        let mut metadata = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| error(format!("Could not read Java metadata: {e}")))?;
            if metadata.len() + chunk.len() > 1024 * 1024 { return Err(error("Java metadata is too large.")); }
            metadata.extend_from_slice(&chunk);
        }
        let assets: Vec<Asset> = serde_json::from_slice(&metadata).map_err(|_| error("Java provider returned invalid metadata."))?;
        let package = assets.into_iter().next().ok_or_else(|| error(format!("Java {major} is unavailable for this platform.")))?.binary.package;
        validate(&package)?;
        let response = client.get(&package.link).send().await
            .map_err(|e| error(format!("Could not download Java {major}: {e}")))?
            .error_for_status().map_err(|e| error(format!("Java download failed: {e}")))?;
        let mut stream = response.bytes_stream();
        let mut output = tokio::fs::File::from_std(archive.reopen().map_err(|e| AppError::io("open the Java download", e))?);
        let mut downloaded = 0u64;
        let mut last_percent = 101;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| error(format!("Java download interrupted: {e}. Try launching again.")))?;
            downloaded += chunk.len() as u64;
            if downloaded > package.size { return Err(error("Java download exceeded its expected size.")); }
            output.write_all(&chunk).await.map_err(|e| AppError::io("save the Java download", e))?;
            let percent = downloaded * 100 / package.size;
            if percent != last_percent {
                report(&format!("Downloading Java {major} · {percent}%"))?;
                last_percent = percent;
            }
        }
        output.flush().await.map_err(|e| AppError::io("flush the Java download", e))?;
        if downloaded != package.size { return Err(error("Java download was incomplete. Try launching again.")); }
        package
    };
    validate(&package)?;
    report(&format!("Verifying and installing Java {major}…"))?;
    let staging = tempfile::Builder::new().prefix(".java-").tempdir_in(&parent)
        .map_err(|e| AppError::io("stage the Java runtime", e))?;
    let path = staging.path().to_owned();
    let archive_path = archive.path().to_owned();
    tokio::task::spawn_blocking(move || extract(&archive_path, &path, &package.checksum))
        .await.map_err(|e| error(format!("Java extraction failed: {e}")))??;
    let java = java::resolve(Some(staging.path().join("bin/java.exe")), data, major, arch).await?;
    report(&format!("Java {major} is ready."))?;
    let destination = target(data, major);
    // Move a damaged installation aside only after its replacement has passed the JVM probe.
    let backup = parent.join(format!(".old-java-{}", uuid::Uuid::new_v4()));
    let had_old = destination.exists();
    if had_old {
        fs::rename(&destination, &backup).map_err(|e| AppError::io("replace the previous managed Java runtime", e))?;
    }
    if let Err(e) = fs::rename(staging.path(), &destination) {
        if had_old { let _ = fs::rename(&backup, &destination); }
        return Err(AppError::io("publish the managed Java runtime", e));
    }
    if had_old {
        // This generated path is a sibling of the managed target under the validated runtime parent.
        if backup.parent() == Some(parent.as_path()) { let _ = fs::remove_dir_all(backup); }
    }
    Ok(Java { executable: destination.join("bin/java.exe"), ..java })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive(name: &str) -> (tempfile::NamedTempFile, String) {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut zip = zip::ZipWriter::new(file.reopen().unwrap());
        zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(b"runtime fixture").unwrap();
        zip.finish().unwrap();
        let checksum = format!("{:x}", Sha256::digest(fs::read(file.path()).unwrap()));
        (file, checksum)
    }

    #[test]
    fn extracts_only_verified_archive_entries_inside_the_runtime() {
        let root = tempfile::tempdir().unwrap();
        let (file, checksum) = archive("jre/bin/java.exe");
        extract(file.path(), root.path(), &checksum).unwrap();
        assert_eq!(fs::read(root.path().join("bin/java.exe")).unwrap(), b"runtime fixture");
        let empty = tempfile::tempdir().unwrap();
        assert!(extract(file.path(), empty.path(), &"0".repeat(64)).is_err());
        assert_eq!(fs::read_dir(empty.path()).unwrap().count(), 0);
        let (unsafe_file, checksum) = archive("jre/../../escape.exe");
        assert!(extract(unsafe_file.path(), empty.path(), &checksum).is_err());
    }

    #[tokio::test]
    #[ignore = "extracts and executes the bundled Windows x64 runtime"]
    async fn bundled_runtime_passes_real_version_and_architecture_probe() {
        let root = tempfile::tempdir().unwrap();
        let archive = root.path().join("runtime.zip");
        fs::write(&archive, BUNDLED).unwrap();
        let asset: Asset = serde_json::from_str(BUNDLED_METADATA).unwrap();
        let destination = root.path().join("extracted");
        fs::create_dir(&destination).unwrap();
        extract(&archive, &destination, &asset.binary.package.checksum).unwrap();
        assert!(destination.join("legal").is_dir());
        let runtime = java::resolve(Some(destination.join("bin/java.exe")), root.path(), 21, Architecture::X64).await.unwrap();
        assert_eq!(runtime.major, 21);
    }
}
