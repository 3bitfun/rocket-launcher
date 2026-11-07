use super::{game::GameJobs, LauncherState};
use crate::{
    auth::accounts::{self, AccountJobs},
    errors::{AppError, Result},
    minecraft::download,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tauri::{ipc::Channel, AppHandle, State};
use tokio::io::{AsyncWriteExt, BufWriter};

const API: &str = "https://api.github.com/repos/3bitfun/rocket-launcher/releases";
const RELEASES: &str = "https://github.com/3bitfun/rocket-launcher/releases";
const MAX_INSTALLER_BYTES: u64 = 128 * 1024 * 1024;
const STAGED_PREFIX: &str = "rocket-update-";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    state: String,
    size: u64,
    digest: Option<String>,
}

impl Asset {
    fn checksum(&self) -> Option<&str> {
        let hash = self.digest.as_deref()?.strip_prefix("sha256:")?;
        (hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(hash)
    }

    fn installable(&self) -> bool {
        self.state == "uploaded"
            && self.name.ends_with("_x64-setup.exe")
            && self
                .name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
            && (1..=MAX_INSTALLER_BYTES).contains(&self.size)
            && self.checksum().is_some()
    }
}

fn update_error(message: impl Into<String>) -> AppError {
    AppError::new("UPDATE", message)
}

fn version(value: &str) -> Option<[u32; 3]> {
    let mut parts = value.split('.');
    let version = [
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ];
    (parts.next().is_none() && value == format!("{}.{}.{}", version[0], version[1], version[2]))
        .then_some(version)
}

fn current_version() -> [u32; 3] {
    version(env!("CARGO_PKG_VERSION")).expect("numeric package version")
}

fn update_settings(state: &LauncherState) -> Result<(PathBuf, u64)> {
    let store = state.store()?;
    if accounts::Accounts::read(&store.paths.data)?.offline {
        return Err(accounts::offline_error());
    }
    Ok((
        store.paths.cache.join("launcher-updates"),
        store.config.settings.request_timeout_seconds,
    ))
}

async fn github_metadata<T: serde::de::DeserializeOwned>(url: &str, timeout: u64) -> Result<T> {
    let error = || update_error("Could not read launcher releases from GitHub. Try again.");
    download::client(timeout)?
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| error())?
        .error_for_status()
        .map_err(|_| error())?
        .json()
        .await
        .map_err(|_| error())
}

#[tauri::command]
pub async fn check_launcher_update(state: State<'_, LauncherState>) -> Result<Option<String>> {
    let (_, timeout) = update_settings(&state)?;
    let releases: Vec<Release> = github_metadata(&format!("{API}?per_page=30"), timeout).await?;
    Ok(releases
        .into_iter()
        .filter_map(|release| {
            let tag = release.tag_name.strip_prefix('v')?;
            let parsed = version(tag)?;
            (!release.draft
                && parsed > current_version()
                && release.assets.iter().any(Asset::installable))
            .then_some((parsed, tag.to_owned()))
        })
        .max()
        .map(|(_, tag)| tag))
}

fn download_client(timeout: u64) -> Result<reqwest::Client> {
    let timeout = Duration::from_secs(timeout.clamp(5, 120));
    reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(timeout)
        .read_timeout(timeout)
        .user_agent(concat!("RocketLauncher/", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let url = attempt.url();
            if attempt.previous().len() <= 3
                && url.scheme() == "https"
                && url.username().is_empty()
                && url.password().is_none()
                && url.port().is_none()
                && matches!(
                    url.host_str(),
                    Some("release-assets.githubusercontent.com" | "objects.githubusercontent.com")
                )
            {
                attempt.follow()
            } else {
                attempt.error("Unexpected GitHub release download redirect")
            }
        }))
        .build()
        .map_err(|_| update_error("Could not initialize the update download."))
}

async fn stage_installer(
    directory: &Path,
    tag: &str,
    asset: &Asset,
    timeout: u64,
    progress: &Channel<u8>,
) -> Result<tempfile::TempPath> {
    let network = || update_error("The installer download was interrupted. Try again.");
    let invalid =
        || update_error("The installer did not match the size or checksum published by GitHub.");
    let mut response = download_client(timeout)?
        .get(format!("{RELEASES}/download/{tag}/{}", asset.name))
        .send()
        .await
        .map_err(|_| network())?
        .error_for_status()
        .map_err(|_| network())?;
    if response
        .content_length()
        .is_some_and(|size| size != asset.size)
    {
        return Err(invalid());
    }
    tokio::fs::create_dir_all(directory)
        .await
        .map_err(|e| AppError::io("create the update cache", e))?;
    let file = tempfile::Builder::new()
        .prefix(STAGED_PREFIX)
        .suffix(".exe")
        .tempfile_in(directory)
        .map_err(|e| AppError::io("stage the launcher installer", e))?;
    let handle = file
        .reopen()
        .map_err(|e| AppError::io("open the staged installer", e))?;
    let mut output = BufWriter::with_capacity(128 * 1024, tokio::fs::File::from_std(handle));
    let mut checksum = Sha256::new();
    let mut received = 0;
    let mut last_percent = 0;
    while let Some(chunk) = response.chunk().await.map_err(|_| network())? {
        received += chunk.len() as u64;
        if received > asset.size {
            return Err(invalid());
        }
        checksum.update(&chunk);
        output
            .write_all(&chunk)
            .await
            .map_err(|e| AppError::io("write the launcher installer", e))?;
        let percent = (received * 100 / asset.size).min(99) as u8;
        if percent != last_percent {
            let _ = progress.send(percent);
            last_percent = percent;
        }
    }
    let actual = format!("{:x}", checksum.finalize());
    if received != asset.size || !actual.eq_ignore_ascii_case(asset.checksum().ok_or_else(invalid)?)
    {
        return Err(invalid());
    }
    output
        .flush()
        .await
        .map_err(|e| AppError::io("flush the launcher installer", e))?;
    output
        .get_ref()
        .sync_all()
        .await
        .map_err(|e| AppError::io("save the launcher installer", e))?;
    drop(output);
    Ok(file.into_temp_path())
}

#[tauri::command]
pub async fn install_launcher_update(
    app: AppHandle,
    state: State<'_, LauncherState>,
    jobs: State<'_, GameJobs>,
    account_jobs: State<'_, AccountJobs>,
    release_version: String,
    progress: Channel<u8>,
) -> Result<()> {
    if !version(&release_version).is_some_and(|parsed| parsed > current_version()) {
        return Err(update_error(
            "Choose a launcher release newer than the running version.",
        ));
    }
    let (directory, timeout) = update_settings(&state)?;
    let _game_guard = jobs.lock_for_update()?;
    let tag = format!("v{release_version}");
    let release: Release = github_metadata(&format!("{API}/tags/{tag}"), timeout).await?;
    if release.draft || release.tag_name != tag {
        return Err(update_error(
            "The selected launcher release is unavailable.",
        ));
    }
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.installable())
        .ok_or_else(|| update_error("This release has no verifiable Windows installer."))?;
    let installer = stage_installer(&directory, &tag, asset, timeout, &progress).await?;
    let _account_guard = account_jobs.0.try_lock().map_err(|_| accounts::busy())?;
    update_settings(&state)?;
    let path = installer
        .keep()
        .map_err(|e| AppError::io("prepare the verified installer", e.error))?;
    if let Err(error) = launch_installer(&path) {
        let _ = std::fs::remove_file(&path);
        return Err(error);
    }
    let _ = progress.send(100);
    app.exit(0);
    Ok(())
}

fn launch_installer(path: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};

    let file: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    let args: Vec<u16> = "/P /UPDATE /R".encode_utf16().chain(Some(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            args.as_ptr(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize <= 32 {
        return Err(update_error(
            "Windows could not start the installer. Try again.",
        ));
    }
    Ok(())
}

pub fn clean_staged_installers(cache: &Path) {
    if let Ok(entries) = std::fs::read_dir(cache.join("launcher-updates")) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(STAGED_PREFIX)
                && name.ends_with(".exe")
                && entry
                    .metadata()
                    .and_then(|meta| meta.modified())
                    .ok()
                    .and_then(|modified| modified.elapsed().ok())
                    .is_some_and(|age| age >= Duration::from_secs(24 * 60 * 60))
            {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}
