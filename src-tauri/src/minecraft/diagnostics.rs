use crate::errors::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::SystemTime,
};

const LOG_LIMIT: u64 = 64 * 1024;
pub const SESSION_RESULT: &str = "result.json";

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Exited,
    Stopped,
    Failed,
    Unknown,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResult {
    pub outcome: Outcome,
    pub exit_code: Option<i32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Log {
    kind: &'static str,
    name: String,
    text: String,
    truncated: bool,
}

#[derive(Serialize)]
struct Finding {
    title: &'static str,
    suggestion: &'static str,
    source: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    sessions: Vec<SessionOption>,
    selected_session: Option<String>,
    session: Option<SessionResult>,
    logs: Vec<Log>,
    findings: Vec<Finding>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionOption {
    id: String,
    started_at: u64,
    outcome: Option<Outcome>,
}

fn read_result(directory: &Path) -> Option<SessionResult> {
    regular(&directory.join(SESSION_RESULT))
        .and_then(|path| File::open(path).ok())
        .and_then(|file| serde_json::from_reader(file.take(4096)).ok())
}

// Paths are derived from registered instances and fixed log names. Refuse links
// or junctions that resolve outside the canonical instance or launcher root.
fn regular(path: &Path) -> Option<PathBuf> {
    let canonical = fs::canonicalize(path).ok()?;
    (canonical == path && canonical.is_file()).then_some(canonical)
}

fn dated_entries(
    directory: &Path,
    matches: impl Fn(&str) -> bool,
    sessions: bool,
) -> Result<Vec<(SystemTime, PathBuf)>> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(AppError::io("list Minecraft logs", e)),
    };
    Ok(entries
        .flatten()
        .filter_map(|entry| {
            if !matches(entry.file_name().to_str()?) {
                return None;
            }
            let path = entry.path();
            let metadata = entry.metadata().ok()?;
            if sessions {
                if !metadata.is_dir() || fs::canonicalize(&path).ok()? != path {
                    return None;
                }
                Some((metadata.created().ok()?, path))
            } else {
                regular(&path)?;
                Some((metadata.modified().ok()?, path))
            }
        })
        .collect())
}

fn captured_sessions(root: &Path, id: &str) -> Result<Vec<(SystemTime, PathBuf)>> {
    let root = fs::canonicalize(root).map_err(|e| AppError::io("open launcher logs", e))?;
    let mut sessions = dated_entries(
        &root.join("game").join(id),
        |name| uuid::Uuid::parse_str(name).is_ok(),
        true,
    )?;
    sessions.sort_unstable_by(|left, right| right.cmp(left));
    sessions.truncate(12);
    Ok(sessions)
}

fn open_log(path: &Path, kind: &'static str) -> Result<Option<Log>> {
    let Some(path) = regular(path) else {
        return Ok(None);
    };
    let mut file = File::open(&path).map_err(|e| AppError::io("open Minecraft log", e))?;
    let size = file
        .metadata()
        .map_err(|e| AppError::io("inspect Minecraft log", e))?
        .len();
    let truncated = size > LOG_LIMIT;
    let mut data = Vec::new();
    if truncated {
        // Keep the initial exception and the newest lines, even on long sessions.
        let head = LOG_LIMIT / 4;
        (&mut file)
            .take(head)
            .read_to_end(&mut data)
            .map_err(|e| AppError::io("read Minecraft log", e))?;
        data.extend_from_slice(b"\n\n[Middle of log omitted]\n\n");
        file.seek(SeekFrom::Start(size - (LOG_LIMIT - head)))
            .map_err(|e| AppError::io("seek Minecraft log", e))?;
        file.take(LOG_LIMIT - head)
            .read_to_end(&mut data)
            .map_err(|e| AppError::io("read Minecraft log", e))?;
    } else {
        file.take(LOG_LIMIT)
            .read_to_end(&mut data)
            .map_err(|e| AppError::io("read Minecraft log", e))?;
    }
    Ok(Some(Log {
        kind,
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        text: String::from_utf8_lossy(&data).into_owned(),
        truncated,
    }))
}

pub fn read_captured(root: &Path, id: &str, errors: bool) -> Result<Option<String>> {
    let Some((_, directory)) = captured_sessions(root, id)?.into_iter().next() else {
        return Ok(None);
    };
    Ok(open_log(
        &directory.join(if errors { "stderr.log" } else { "stdout.log" }),
        "output",
    )?
    .map(|log| log.text))
}

pub fn inspect(root: &Path, game: &Path, id: &str, requested: Option<&str>) -> Result<Diagnostics> {
    let captured = captured_sessions(root, id)?;
    let sessions: Vec<_> = captured
        .iter()
        .map(|(stamp, directory)| SessionOption {
            id: directory
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            started_at: stamp
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            outcome: read_result(directory).map(|result| result.outcome),
        })
        .collect();
    let index = match requested {
        Some(id) => sessions
            .iter()
            .position(|session| session.id == id)
            .ok_or_else(|| {
                AppError::new(
                    "LOG",
                    "This Minecraft session is no longer in the recent history.",
                )
            })?,
        None => 0,
    };
    let selected_session = sessions.get(index).map(|session| session.id.clone());
    let started = captured.get(index).map(|(stamp, _)| *stamp);
    let next_start = index
        .checked_sub(1)
        .and_then(|newer| captured.get(newer))
        .map(|(stamp, _)| *stamp);
    let mut logs = Vec::new();
    let session = if let Some((_, directory)) = captured.get(index) {
        logs.extend(open_log(&directory.join("stdout.log"), "output")?);
        logs.extend(open_log(&directory.join("stderr.log"), "errors")?);
        read_result(directory)
    } else {
        None
    };

    if let Ok(game) = fs::canonicalize(game) {
        let latest = game.join("logs").join("latest.log");
        let belongs_to_session = |modified: SystemTime| {
            started.map_or(true, |started| modified >= started)
                && next_start.map_or(true, |next| modified < next)
        };
        if regular(&latest)
            .and_then(|path| fs::metadata(path).ok())
            .and_then(|info| info.modified().ok())
            .is_some_and(belongs_to_session)
        {
            logs.extend(open_log(&latest, "game")?);
        }
        for (directory, kind, prefix, suffix) in [
            (game.join("crash-reports"), "crash", "crash-", ".txt"),
            (game, "jvm", "hs_err_pid", ".log"),
        ] {
            let report = dated_entries(
                &directory,
                |name| name.starts_with(prefix) && name.ends_with(suffix),
                false,
            )?
            .into_iter()
            .filter(|(modified, _)| belongs_to_session(*modified))
            .max();
            if let Some((_, path)) = report {
                logs.extend(open_log(&path, kind)?);
            }
        }
    }
    let findings = if session.as_ref().is_some_and(|result: &SessionResult| {
        matches!(result.outcome, Outcome::Exited | Outcome::Stopped)
    }) {
        Vec::new()
    } else {
        diagnose(&logs)
    };
    Ok(Diagnostics {
        sessions,
        selected_session,
        session,
        logs,
        findings,
    })
}

fn diagnose(logs: &[Log]) -> Vec<Finding> {
    let sources: Vec<_> = logs
        .iter()
        .map(|log| (log, log.text.to_ascii_lowercase()))
        .collect();
    let rules: &[(&[&str], &str, &str)] = &[
        (&["unsupportedclassversionerror", "only recognizes class file versions up to", "requires java version"], "Incompatible Java version", "Check the Java version required by Minecraft and the mods. In Edit settings, select a compatible 64-bit Java executable or clear an outdated override."),
        (&["outofmemoryerror: java heap space", "gc overhead limit exceeded"], "Minecraft ran out of heap memory", "Increase the instance’s RAM allocation if your computer has spare memory, or reduce the pack’s memory use."),
        (&["could not reserve enough space", "invalid maximum heap size", "insufficient memory for the java runtime"], "Java could not allocate memory", "Lower the instance’s RAM allocation, close memory-heavy apps, and check that the selected Java runtime is 64-bit."),
        (&["incompatible mods found", "missing or unsupported mandatory dependencies", "mod resolution encountered an incompatible mod set", "requires any version of"], "Mod dependency or version mismatch", "Read the named mods and versions in the log. Install the required dependency or choose releases that match this instance’s Minecraft version and loader."),
        (&["duplicate mods", "duplicate mod id", "duplicate versions for mod"], "Duplicate mods", "In Installed mods, disable older or duplicate copies of the mod named in the log."),
        (&["mixinapplyerror", "invalidmixinexception", "mixin transformer error", "mixintransformererror", "injectionerror"], "A mod failed to apply a mixin", "Check the first mod or mixin named in the error. Update it for this Minecraft version, or disable it and retry."),
        (&["glfw error 65542", "glfw error 65543", "failed to create window", "failed to create an opengl context"], "Graphics initialization failed", "Check the GPU driver and Java architecture. If this began after a shader or graphics mod change, disable that change and retry."),
        (&["exception_access_violation", "a fatal error has been detected by the java runtime environment"], "Native JVM crash", "Read the JVM report’s Problematic frame. Check the matching Java runtime and any graphics driver or native library named there."),
        (&["zip end header not found", "invalid or corrupt jarfile", "error in opening zip file"], "Unreadable JAR or archive", "Check which file the error names. Reinstall that mod or use Repair files for a shared Minecraft or loader file."),
        (&["there is not enough space on the disk", "no space left on device"], "The drive ran out of space", "Free space on the instance and launcher cache drives before trying again."),
    ];
    rules
        .iter()
        .filter_map(|(patterns, title, suggestion)| {
            sources
                .iter()
                .find(|(_, text)| patterns.iter().any(|pattern| text.contains(pattern)))
                .map(|(log, _)| Finding {
                    title,
                    suggestion,
                    source: log.name.clone(),
                })
        })
        .take(4)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_a_failed_session_without_reusing_an_old_crash_report() {
        let temp = tempfile::tempdir().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let directory = temp
            .path()
            .join("game")
            .join(&id)
            .join(uuid::Uuid::new_v4().to_string());
        let game = temp.path().join("instance");
        fs::create_dir_all(&directory).unwrap();
        fs::create_dir_all(game.join("crash-reports")).unwrap();
        let old = File::create(game.join("crash-reports").join("crash-old.txt")).unwrap();
        old.set_modified(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000))
            .unwrap();
        fs::write(
            directory.join("stderr.log"),
            "java.lang.OutOfMemoryError: Java heap space",
        )
        .unwrap();
        let result = directory.join(SESSION_RESULT);
        crate::storage::atomic_json(
            &result,
            &SessionResult {
                outcome: Outcome::Failed,
                exit_code: Some(1),
            },
        )
        .unwrap();

        let report = inspect(temp.path(), &game, &id, None).unwrap();
        assert_eq!(report.session.unwrap().exit_code, Some(1));
        assert_eq!(report.logs.len(), 1);
        assert_eq!(report.findings[0].title, "Minecraft ran out of heap memory");

        crate::storage::atomic_json(
            &result,
            &SessionResult {
                outcome: Outcome::Stopped,
                exit_code: Some(1),
            },
        )
        .unwrap();
        assert!(inspect(temp.path(), &game, &id, None)
            .unwrap()
            .findings
            .is_empty());
    }

    #[test]
    fn a_long_log_keeps_both_exception_headers_and_recent_output() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let path = root.join("latest.log");
        let mut bytes = b"first line\n".to_vec();
        bytes.resize(100_000, b'-');
        bytes.extend_from_slice(b"\nlast line");
        fs::write(&path, bytes).unwrap();
        let log = open_log(&path, "game").unwrap().unwrap();
        assert!(log.truncated);
        assert!(log.text.starts_with("first line"));
        assert!(log.text.ends_with("last line"));
        assert!(log.text.len() < 66_000);
    }

    #[test]
    fn reads_a_selected_session_and_rejects_unknown_paths() {
        let temp = tempfile::tempdir().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let session = uuid::Uuid::new_v4().to_string();
        let directory = temp.path().join("game").join(&id).join(&session);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("stderr.log"), "GLFW error 65542").unwrap();

        let report = inspect(temp.path(), temp.path(), &id, Some(&session)).unwrap();
        assert_eq!(report.selected_session.as_deref(), Some(session.as_str()));
        assert_eq!(report.findings[0].title, "Graphics initialization failed");
        assert!(inspect(temp.path(), temp.path(), &id, Some("..")).is_err());
    }
}
