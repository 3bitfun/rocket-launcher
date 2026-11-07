use super::install::copy_atomic;
use crate::{
    errors::{AppError, Result},
    storage::atomic_json,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize)]
pub struct PendingSync {
    game_directory: PathBuf,
    servers: bool,
    keybinds: bool,
}

fn keybinds(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.starts_with("key_")
                .then(|| (key.to_owned(), value.to_owned()))
        })
        .collect()
}

fn read_text(path: &Path) -> Result<Option<String>> {
    match fs::read(path) {
        Ok(bytes) if bytes.len() <= 4 * 1024 * 1024 => {
            String::from_utf8(bytes).map(Some).map_err(|_| {
                AppError::new(
                    "SYNC",
                    format!("Keybind file is not UTF-8: {}", path.display()),
                )
            })
        }
        Ok(_) => Err(AppError::new(
            "SYNC",
            "The Minecraft options file is too large to sync.",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(AppError::io("read Minecraft keybinds", error)),
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::new("SYNC", "Invalid sync destination."))?;
    fs::create_dir_all(parent).map_err(|e| AppError::io("create sync directory", e))?;
    let mut file = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| AppError::io("stage synced file", e))?;
    file.write_all(bytes)
        .and_then(|_| file.as_file().sync_all())
        .map_err(|e| AppError::io("write synced file", e))?;
    file.persist(path)
        .map_err(|e| AppError::io("save synced file", e.error))?;
    Ok(())
}

fn backup(path: &Path, backup: &Path) -> Result<()> {
    if !path.is_file() || backup.exists() {
        return Ok(());
    }
    let parent = backup
        .parent()
        .ok_or_else(|| AppError::new("SYNC", "Invalid backup path."))?;
    let mut file = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| AppError::io("stage sync backup", e))?;
    let mut source = fs::File::open(path).map_err(|e| AppError::io("read sync backup", e))?;
    std::io::copy(&mut source, &mut file).map_err(|e| AppError::io("write sync backup", e))?;
    match file.persist_noclobber(backup) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(AppError::io("save sync backup", error.error)),
    }
}

fn apply_keys(local: &Path, shared: &Path) -> Result<()> {
    let Some(source) = read_text(shared)? else {
        return Ok(());
    };
    let bindings = keybinds(&source);
    let existing = read_text(local)?.unwrap_or_default();
    let mut written = BTreeMap::new();
    let mut lines = Vec::new();
    for line in existing.lines() {
        if let Some((key, _)) = line.split_once(':') {
            if let Some(value) = bindings.get(key) {
                if written.insert(key.to_owned(), ()).is_none() {
                    lines.push(format!("{key}:{value}"));
                }
                continue;
            }
        }
        lines.push(line.to_owned());
    }
    for (key, value) in bindings {
        if !written.contains_key(&key) {
            lines.push(format!("{key}:{value}"));
        }
    }
    if !lines.is_empty() {
        write_atomic(local, format!("{}\n", lines.join("\n")).as_bytes())?;
    }
    Ok(())
}

pub fn finish(root: &Path, pending: &PendingSync) -> Result<()> {
    let options = pending.game_directory.join("options.txt");
    if pending.keybinds {
        if let Some(text) = read_text(&options)? {
            let mut bindings = read_text(&root.join("keybinds.txt"))?
                .map(|text| keybinds(&text))
                .unwrap_or_default();
            bindings.extend(keybinds(&text));
            if !bindings.is_empty() {
                let text = bindings
                    .into_iter()
                    .map(|(key, value)| format!("{key}:{value}\n"))
                    .collect::<String>();
                write_atomic(&root.join("keybinds.txt"), text.as_bytes())?;
            }
        }
    }
    let servers = pending.game_directory.join("servers.dat");
    if pending.servers && servers.is_file() {
        copy_atomic(&servers, &root.join("servers.dat"))?;
    }
    match fs::remove_file(root.join("pending.json")) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AppError::io("finish instance sync", error)),
    }
}

pub fn prepare(
    root: &Path,
    game: &Path,
    servers: bool,
    keybinds_enabled: bool,
) -> Result<PendingSync> {
    fs::create_dir_all(root).map_err(|e| AppError::io("create sync directory", e))?;
    let journal = root.join("pending.json");
    match fs::read(&journal) {
        Ok(bytes) => {
            let previous: PendingSync = serde_json::from_slice(&bytes).map_err(|_| {
                AppError::new("SYNC", "The previous instance sync could not be recovered.")
            })?;
            finish(root, &previous)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(AppError::io("read instance sync state", error)),
    }
    if servers {
        let local = game.join("servers.dat");
        let shared = root.join("servers.dat");
        if shared.is_file() {
            backup(&local, &game.join("servers.rocket-before-sync.dat"))?;
            copy_atomic(&shared, &local)?;
        } else if local.is_file() {
            copy_atomic(&local, &shared)?;
        }
    }
    if keybinds_enabled {
        let local = game.join("options.txt");
        let shared = root.join("keybinds.txt");
        if shared.is_file() {
            backup(&local, &game.join("options.rocket-before-sync.txt"))?;
            apply_keys(&local, &shared)?;
        } else if let Some(text) = read_text(&local)? {
            let bindings = keybinds(&text);
            if !bindings.is_empty() {
                let text = bindings
                    .into_iter()
                    .map(|(key, value)| format!("{key}:{value}\n"))
                    .collect::<String>();
                write_atomic(&shared, text.as_bytes())?;
            }
        }
    }
    let pending = PendingSync {
        game_directory: game.into(),
        servers,
        keybinds: keybinds_enabled,
    };
    atomic_json(&journal, &pending)?;
    Ok(pending)
}
