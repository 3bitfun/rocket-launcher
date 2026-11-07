use super::LauncherState;
use crate::{
    auth::{accounts, browser},
    errors::{AppError, Result},
    minecraft::download,
};
use serde::Deserialize;
use tauri::State;

const RELEASES: &str = "https://github.com/3bitfun/rocket-launcher/releases";

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
}

fn version(value: &str) -> Option<[u32; 3]> {
    let mut parts = value.split('.');
    let version = [
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ];
    parts.next().is_none().then_some(version)
}

#[tauri::command]
pub async fn check_launcher_update(state: State<'_, LauncherState>) -> Result<Option<String>> {
    let timeout = {
        let store = state.store()?;
        if accounts::Accounts::read(&store.paths.data)?.offline {
            return Err(accounts::offline_error());
        }
        store.config.settings.request_timeout_seconds
    };
    let current = version(env!("CARGO_PKG_VERSION")).expect("numeric package version");
    let error = || {
        AppError::new(
            "UPDATE",
            "Could not check GitHub for launcher updates. Try again.",
        )
    };
    let releases = download::client(timeout)?
        .get("https://api.github.com/repos/3bitfun/rocket-launcher/releases?per_page=30")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| error())?
        .error_for_status()
        .map_err(|_| error())?
        .json::<Vec<Release>>()
        .await
        .map_err(|_| error())?;
    Ok(releases
        .into_iter()
        .filter_map(|release| {
            let tag = release.tag_name.strip_prefix('v')?;
            let parsed = version(tag)?;
            (!release.draft
                && parsed > current
                && release.assets.iter().any(|asset| {
                    asset.state == "uploaded" && asset.name.ends_with("_x64-setup.exe")
                }))
            .then_some((parsed, tag.to_owned()))
        })
        .max()
        .map(|(_, tag)| tag))
}

#[tauri::command]
pub fn open_launcher_release(release_version: String) -> Result<()> {
    let [major, minor, patch] = version(&release_version)
        .ok_or_else(|| AppError::new("UPDATE", "Invalid launcher version."))?;
    let url = reqwest::Url::parse(&format!("{RELEASES}/tag/v{major}.{minor}.{patch}"))
        .expect("constructed GitHub release URL");
    browser::open(url)
        .map_err(|_| AppError::new("UPDATE", "Could not open GitHub in your browser."))
}
