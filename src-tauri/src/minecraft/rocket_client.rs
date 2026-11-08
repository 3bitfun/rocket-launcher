use super::modrinth;
use crate::{
    errors::{AppError, Result},
    instances::{mods_directory, set_mod_enabled},
    models::{Instance, Loader},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{ErrorKind, Read, Write},
    path::Path,
};

const RELEASE: &str = "2.0.0";
const API_PROJECT: &str = "P7dR8mSH";
const CLIENT_ID: &str = "rocket_companion";
const BUILDS: &[(&str, &[u8])] = include!(concat!(env!("OUT_DIR"), "/rocket_client_builds.rs"));

pub fn build(game: &str) -> Option<&'static [u8]> {
    BUILDS
        .iter()
        .find(|(version, _)| *version == game)
        .map(|(_, jar)| *jar)
}

fn version(game: &str) -> String {
    format!("{RELEASE}+mc{game}")
}

fn filename(game: &str) -> String {
    format!("rocketlauncher-client-{}.jar", version(game))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    state: &'static str,
    message: &'static str,
    needs_download: bool,
}

fn supported(instance: &Instance) -> std::result::Result<&'static [u8], &'static str> {
    if instance.loader != Loader::Fabric {
        return Err("Rocket Client requires a Fabric instance.");
    }
    let bytes = build(&instance.version)
        .ok_or("This Minecraft version has no bundled Rocket Client build.")?;
    let (minimum, message) = match instance.version.as_str() {
        "26.3" => (
            [0, 19, 3],
            "Choose Fabric Loader 0.19.3 or newer in instance settings.",
        ),
        game if game.starts_with("26.") => (
            [0, 18, 4],
            "Choose Fabric Loader 0.18.4 or newer in instance settings.",
        ),
        _ => (
            [0, 18, 2],
            "Choose Fabric Loader 0.18.2 or newer in instance settings.",
        ),
    };
    let loader = instance.loader_version.as_deref().unwrap_or("");
    if !release_parts(loader)
        .is_some_and(|parts| parts > minimum || (parts == minimum && !loader.contains('-')))
    {
        return Err(message);
    }
    Ok(bytes)
}

#[derive(Deserialize)]
struct ModMetadata {
    id: String,
    version: String,
    #[serde(default)]
    depends: ModDependencies,
}

#[derive(Default, Deserialize)]
struct ModDependencies {
    minecraft: Option<VersionRequirements>,
    fabricloader: Option<VersionRequirements>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum VersionRequirements {
    One(String),
    Any(Vec<String>),
}

impl VersionRequirements {
    fn matches(&self, version: &str) -> bool {
        match self {
            Self::One(requirement) => matches_range(version, requirement),
            Self::Any(requirements) => requirements.iter().any(|r| matches_range(version, r)),
        }
    }
}

fn release_parts(version: &str) -> Option<[u32; 3]> {
    let mut parts = version.split(['-', '+']).next()?.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    parts.next().is_none().then_some([major, minor, patch])
}

// Fabric API uses exact releases, tilde ranges, and space-separated numeric
// bounds such as ">=1.21.11- <1.21.12-". A trailing dash sorts before a release.
fn matches_range(version: &str, requirement: &str) -> bool {
    let Some(release) = release_parts(version) else {
        return false;
    };
    requirement.split_whitespace().all(|term| {
        if term == "*" {
            return true;
        }
        let bound = term.trim_start_matches(['<', '>', '=', '~', '^']);
        let Some(limit) = release_parts(bound) else {
            return false;
        };
        let ordering = (release, !version.contains('-')).cmp(&(limit, !bound.contains('-')));
        match &term[..term.len() - bound.len()] {
            ">=" => !ordering.is_lt(),
            ">" => ordering.is_gt(),
            "<=" => !ordering.is_gt(),
            "<" => ordering.is_lt(),
            "~" => !ordering.is_lt() && release[..2] == limit[..2],
            "^" => !ordering.is_lt() && release[0] == limit[0],
            "=" | "" => ordering.is_eq(),
            _ => false,
        }
    })
}

struct Mod {
    metadata: ModMetadata,
    filename: String,
    enabled: bool,
}

fn read_metadata(path: &Path) -> Option<ModMetadata> {
    let mut archive = zip::ZipArchive::new(fs::File::open(path).ok()?).ok()?;
    let file = archive.by_name("fabric.mod.json").ok()?;
    serde_json::from_reader(file.take(64 * 1024)).ok()
}

fn related_mods(game: &Path) -> Result<Vec<Mod>> {
    if !game.join("mods").exists() {
        return Ok(vec![]);
    }
    let folder = mods_directory(game)?;
    let mut mods = vec![];
    for entry in fs::read_dir(folder).map_err(|e| AppError::io("read the mods folder", e))? {
        let entry = entry.map_err(|e| AppError::io("read a mod file", e))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let filename = name.strip_suffix(".disabled").unwrap_or(name);
        if !Path::new(filename)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("jar"))
            || !entry
                .file_type()
                .map_err(|e| AppError::io("inspect a mod file", e))?
                .is_file()
        {
            continue;
        }
        if let Some(metadata) = read_metadata(&entry.path()) {
            if matches!(metadata.id.as_str(), CLIENT_ID | "fabric-api" | "fabric") {
                mods.push(Mod {
                    metadata,
                    filename: filename.into(),
                    enabled: name == filename,
                });
            }
        }
    }
    mods.sort_by(|a, b| a.filename.cmp(&b.filename));
    Ok(mods)
}

fn api<'a>(
    mods: &'a [Mod],
    instance: &Instance,
) -> std::result::Result<Option<&'a Mod>, &'static str> {
    let mut selected = None;
    for item in mods
        .iter()
        .filter(|item| matches!(item.metadata.id.as_str(), "fabric-api" | "fabric"))
    {
        let depends = &item.metadata.depends;
        let game_matches = depends
            .minecraft
            .as_ref()
            .map_or(true, |req| req.matches(&instance.version));
        let loader_matches = depends.fabricloader.as_ref().map_or(true, |req| {
            instance
                .loader_version
                .as_deref()
                .is_some_and(|loader| req.matches(loader))
        });
        if !game_matches || !loader_matches {
            if item.enabled {
                return Err("Disable the incompatible Fabric API JAR in Installed mods, then install Rocket Client.");
            }
            continue;
        }
        if item.enabled && selected.is_some_and(|previous: &Mod| previous.enabled) {
            return Err("Keep one Fabric API JAR enabled in Installed mods.");
        }
        if selected.is_none() || item.enabled {
            selected = Some(item);
        }
    }
    Ok(selected)
}

pub fn status(instance: &Instance) -> Result<Status> {
    if let Err(message) = supported(instance) {
        return Ok(Status {
            state: "unsupported",
            message,
            needs_download: false,
        });
    }
    let mods = related_mods(&instance.game_directory)?;
    let current_version = version(&instance.version);
    let installed = mods
        .iter()
        .filter(|item| item.metadata.id == CLIENT_ID && item.enabled)
        .collect::<Vec<_>>();
    let matching =
        |item: &&Mod| item.metadata.id == CLIENT_ID && item.metadata.version == current_version;
    let dependency = match api(&mods, instance) {
        Ok(dependency) => dependency,
        Err(message) => {
            return Ok(Status {
                state: "blocked",
                message,
                needs_download: false,
            })
        }
    };
    let needs_download = dependency.is_none();
    let (state, message) = if installed.len() == 1
        && matching(&installed[0])
        && dependency.is_some_and(|item| item.enabled)
    {
        (
            "ready",
            "Open Rocket from the Minecraft title or pause menu.",
        )
    } else if !installed.is_empty() && (installed.len() > 1 || !matching(&installed[0])) {
        (
            "upgrade",
            "Install the matching build and disable the previous Rocket Client.",
        )
    } else if mods
        .iter()
        .any(|item| item.metadata.id == CLIENT_ID && item.metadata.version == current_version)
    {
        (
            "disabled",
            "Enable Rocket Client and its required Fabric API.",
        )
    } else {
        (
            "available",
            "Add a customizable HUD with FPS, coordinates, heading, world time, ping, and memory.",
        )
    };
    Ok(Status {
        state,
        message,
        needs_download,
    })
}

pub async fn prepare_api(
    instance: &Instance,
    offline: bool,
    timeout: u64,
) -> Result<Option<modrinth::StagedMod>> {
    supported(instance).map_err(error)?;
    let game = instance.game_directory.clone();
    let mods = tokio::task::spawn_blocking(move || related_mods(&game))
        .await
        .map_err(|e| error(e.to_string()))??;
    if api(&mods, instance).map_err(error)?.is_some() {
        return Ok(None);
    }
    if offline {
        return Err(error("Go online once to download the matching Fabric API."));
    }
    let version = modrinth::preferred_version(instance, API_PROJECT, timeout)
        .await?
        .ok_or_else(|| error("Fabric API is unavailable for this Minecraft version."))?;
    Ok(Some(
        modrinth::stage(instance, API_PROJECT, &version, timeout).await?,
    ))
}

pub fn install(instance: &Instance, staged_api: Option<modrinth::StagedMod>) -> Result<()> {
    let bytes = supported(instance).map_err(error)?;
    let game = &instance.game_directory;
    let mods = related_mods(game)?;
    match api(&mods, instance).map_err(error)? {
        Some(item) if !item.enabled => set_mod_enabled(game, &item.filename, true)?,
        Some(_) => {}
        None => {
            let staged = staged_api.ok_or_else(|| {
                error("Fabric API is missing. Try installing Rocket Client again.")
            })?;
            staged.publish(game)?;
        }
    }
    let matching_version = version(&instance.version);
    let matching = mods
        .iter()
        .filter(|item| item.metadata.id == CLIENT_ID && item.metadata.version == matching_version)
        .max_by_key(|item| item.enabled);
    let mut disabled: Vec<&str> = vec![];
    let result = (|| {
        for item in mods
            .iter()
            .filter(|item| item.metadata.id == CLIENT_ID && item.enabled)
        {
            if matching.is_some_and(|chosen| std::ptr::eq(item, chosen)) {
                continue;
            }
            set_mod_enabled(game, &item.filename, false)?;
            disabled.push(&item.filename);
        }
        match matching {
            Some(item) if !item.enabled => set_mod_enabled(game, &item.filename, true),
            Some(_) => Ok(()),
            None => install_bundled(game, &instance.version, bytes),
        }
    })();
    if result.is_err() {
        for filename in disabled.into_iter().rev() {
            if let Err(error) = set_mod_enabled(game, filename, true) {
                tracing::warn!(%error, filename, "could not restore Rocket Client mod state");
            }
        }
    }
    result
}

pub fn add_to_pack(game: &Path, version: &str) -> Result<()> {
    if let Some(bytes) = build(version) {
        install_bundled(game, version, bytes)?;
    }
    Ok(())
}

fn install_bundled(game: &Path, version: &str, bytes: &[u8]) -> Result<()> {
    let folder = mods_directory(game)?;
    let target = folder.join(filename(version));
    let mut staged = tempfile::NamedTempFile::new_in(&folder)
        .map_err(|e| AppError::io("stage Rocket Client", e))?;
    staged
        .write_all(bytes)
        .map_err(|e| AppError::io("write Rocket Client", e))?;
    match staged.persist_noclobber(target) {
        Ok(_) => Ok(()),
        Err(e) if e.error.kind() == ErrorKind::AlreadyExists => Err(error(
            "A Rocket Client JAR with that filename already exists. Check the mods folder.",
        )),
        Err(e) => Err(AppError::io("install Rocket Client", e.error)),
    }
}

fn error(message: impl Into<String>) -> AppError {
    AppError::new("ROCKET_CLIENT", message)
}
