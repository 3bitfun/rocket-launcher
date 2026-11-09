use super::{
    install::{copy_atomic, Installed},
    java::{self, Java},
    metadata::*,
};
use crate::{
    errors::{AppError, Result},
    models::{now, Instance},
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader},
    process::{Child, Command},
};

/// Only native account code may issue this after ownership, profile and expiry checks.
pub struct ValidatedSession {
    pub(crate) player: String,
    pub(crate) uuid: String,
    pub(crate) access_token: String,
    pub(crate) xuid: String,
    pub(crate) client_id: String,
    pub(crate) expires_at: u64,
    pub(crate) offline: bool,
}

pub enum LaunchAuth {
    Microsoft(ValidatedSession),
    Offline { player: String, uuid: String },
    #[cfg(debug_assertions)]
    LocalTest { player: String, uuid: String },
}

struct LaunchIdentity {
    player: String,
    uuid: String,
    access_token: String,
    xuid: String,
    client_id: String,
    auth_session: String,
    user_type: String,
}

impl LaunchIdentity {
    fn authenticated(session: &ValidatedSession) -> Self {
        Self {
            player: session.player.clone(),
            uuid: session.uuid.clone(),
            access_token: session.access_token.clone(),
            xuid: session.xuid.clone(),
            client_id: session.client_id.clone(),
            auth_session: format!("token:{}:{}", session.access_token, session.uuid),
            user_type: "msa".into(),
        }
    }

    fn local_test(player: String, uuid: String) -> Self {
        Self {
            player,
            uuid,
            access_token: String::new(),
            xuid: String::new(),
            client_id: String::new(),
            auth_session: String::new(),
            user_type: "legacy".into(),
        }
    }
}

pub fn authentication_required() -> AppError {
    AppError::new("AUTH_REQUIRED", "Create an offline profile or sign in with a Microsoft account in Settings before launching.")
}

fn expand(template: &str, values: &BTreeMap<&str, String>) -> Result<String> {
    let mut remaining = template;
    let mut result = String::new();
    while let Some((before, placeholder)) = remaining.split_once("${") {
        let (name, after) = placeholder
            .split_once('}')
            .ok_or_else(|| invalid("Invalid launch argument placeholder."))?;
        let value = values
            .get(name)
            .ok_or_else(|| invalid(format!("Unsupported launch argument variable: {name}.")))?;
        result.push_str(before);
        result.push_str(value);
        remaining = after;
    }
    result.push_str(remaining);
    Ok(result)
}
fn arguments(
    args: &[Argument],
    platform: &Platform,
    values: &BTreeMap<&str, String>,
) -> Result<Vec<String>> {
    let mut out = vec![];
    for arg in args {
        match arg {
            Argument::Text(text) => out.push(expand(text, values)?),
            Argument::Conditional { rules, value } if allowed(Some(rules), platform)? => {
                match value {
                    Values::One(text) => out.push(expand(text, values)?),
                    Values::Many(texts) => {
                        for text in texts {
                            out.push(expand(text, values)?);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(out)
}
fn classpath(paths: &[PathBuf]) -> Result<String> {
    if paths.is_empty()
        || paths
            .iter()
            .any(|path| path.to_str().map_or(true, |s| s.contains(';')))
    {
        return Err(invalid(
            "Minecraft classpath is empty or contains an unsupported path.",
        ));
    }
    Ok(paths
        .iter()
        .map(|p| p.to_string_lossy())
        .collect::<Vec<_>>()
        .join(";"))
}

fn resolve_args(
    installed: &Installed,
    instance: &Instance,
    platform: &Platform,
    auth: &LaunchIdentity,
) -> Result<Vec<String>> {
    let path = |p: &Path| p.to_string_lossy().into_owned();
    let version = &installed.version;
    let index = version
        .asset_index
        .as_ref()
        .ok_or_else(|| invalid("Missing Minecraft asset index."))?;
    let values = BTreeMap::from([
        ("auth_player_name", auth.player.clone()),
        ("auth_uuid", auth.uuid.clone()),
        ("auth_access_token", auth.access_token.clone()),
        ("auth_session", auth.auth_session.clone()),
        ("auth_xuid", auth.xuid.clone()),
        ("clientid", auth.client_id.clone()),
        ("user_type", auth.user_type.clone()),
        ("user_properties", "{}".into()),
        ("profile_properties", "{}".into()),
        ("version_name", version.id.clone()),
        (
            "version_type",
            version
                .kind
                .clone()
                .unwrap_or_else(|| instance.version_type.clone()),
        ),
        ("game_directory", path(&instance.game_directory)),
        ("assets_root", path(&installed.assets)),
        ("game_assets", path(&installed.game_assets)),
        ("assets_index_name", index.id.clone()),
        ("natives_directory", path(&installed.natives)),
        ("library_directory", path(&installed.libraries)),
        ("launcher_name", "RocketLauncher".into()),
        ("launcher_version", env!("CARGO_PKG_VERSION").into()),
        ("classpath", classpath(&installed.classpath)?),
        ("classpath_separator", ";".into()),
        (
            "path",
            installed.log_path.as_deref().map(path).unwrap_or_default(),
        ),
    ]);
    let mut args = vec![
        format!("-Xmx{}M", instance.ram_mb),
        "-Dlog4j2.formatMsgNoLookups=true".into(),
    ];
    let game = if let Some(modern) = &version.arguments {
        args.extend(arguments(&modern.jvm, platform, &values)?);
        arguments(&modern.game, platform, &values)?
    } else {
        args.extend([
            format!("-Djava.library.path={}", values["natives_directory"]),
            "-Dminecraft.launcher.brand=RocketLauncher".into(),
            "-cp".into(),
            values["classpath"].clone(),
        ]);
        let legacy = version
            .minecraft_arguments
            .as_deref()
            .ok_or_else(|| invalid("Missing Minecraft launch arguments."))?;
        legacy_arguments(legacy, &values)?
    };
    if let Some(logging) = version.logging.get("client") {
        args.push(expand(&logging.argument, &values)?);
    }
    args.push(
        version
            .main_class
            .clone()
            .ok_or_else(|| invalid("Missing Minecraft main class."))?,
    );
    args.extend(game);
    Ok(args)
}

fn legacy_arguments(legacy: &str, values: &BTreeMap<&str, String>) -> Result<Vec<String>> {
    let mut quote = None;
    let mut current = String::new();
    let mut out = vec![];
    let mut started = false;
    for c in legacy.chars() {
        match quote {
            Some(q) if q == c => quote = None,
            Some(_) => current.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                started = true;
            }
            None if c.is_whitespace() => {
                if started {
                    out.push(expand(&current, values)?);
                    current.clear();
                    started = false;
                }
            }
            None => {
                current.push(c);
                started = true;
            }
        }
    }
    if quote.is_some() {
        return Err(invalid("Unclosed quote in legacy Minecraft arguments."));
    }
    if started {
        out.push(expand(&current, values)?);
    }
    Ok(out)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Readiness {
    Checking,
    RequiresDownload,
    MissingFiles,
    Downloading,
    Installing,
    JavaRequired,
    AuthenticationRequired,
    Ready,
    Launching,
    Running,
    Error,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStatus {
    pub instance_id: String,
    pub readiness: Readiness,
    pub message: String,
    pub exit_code: Option<i32>,
}
impl GameStatus {
    pub fn new(id: &str, readiness: Readiness, message: impl Into<String>) -> Self {
        Self {
            instance_id: id.into(),
            readiness,
            message: message.into(),
            exit_code: None,
        }
    }
}

#[derive(Clone, Default)]
pub struct ProcessManager(Arc<Mutex<HashSet<String>>>);
pub struct Reservation {
    manager: ProcessManager,
    id: String,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        if let Ok(mut running) = self.manager.0.lock() {
            running.remove(&self.id);
        }
    }
}
impl ProcessManager {
    pub fn reserve(&self, id: &str) -> Result<Reservation> {
        let mut running = self
            .0
            .lock()
            .map_err(|_| AppError::new("PROCESS", "Process manager is unavailable."))?;
        if !running.insert(id.into()) {
            return Err(AppError::new(
                "PROCESS",
                "This instance is already launching or running.",
            ));
        }
        Ok(Reservation {
            manager: self.clone(),
            id: id.into(),
        })
    }
    pub fn busy(&self) -> bool {
        self.0.lock().map_or(true, |set| !set.is_empty())
    }
    pub fn contains(&self, id: &str) -> bool {
        self.0.lock().is_ok_and(|set| set.contains(id))
    }
}

pub struct LaunchRequest {
    pub installed: Installed,
    pub instance: Instance,
    pub java: Java,
    pub platform: Platform,
    pub auth: LaunchAuth,
    pub log_root: PathBuf,
    pub sync: Option<(PathBuf, super::sync::PendingSync)>,
    pub rocket_client_config: Option<PathBuf>,
    pub server: Option<String>,
}

pub async fn start(
    reservation: Reservation,
    request: LaunchRequest,
    report: impl Fn(GameStatus) + Send + Sync + 'static,
) -> Result<()> {
    let LaunchRequest {
        installed,
        instance,
        java,
        platform,
        auth,
        log_root,
        sync,
        rocket_client_config,
        server,
    } = request;
    let (identity, access_token, expires_at, local_test) = match auth {
        LaunchAuth::Offline { player, uuid } => (
            LaunchIdentity::local_test(player, uuid),
            String::new(),
            None,
            true,
        ),
        LaunchAuth::Microsoft(session) => {
            let expires_at = (!session.offline).then_some(session.expires_at);
            if expires_at.is_some_and(|expires_at| expires_at <= now()) {
                return Err(authentication_required());
            }
            (
                LaunchIdentity::authenticated(&session),
                session.access_token,
                expires_at,
                false,
            )
        }
        #[cfg(debug_assertions)]
        LaunchAuth::LocalTest { player, uuid } => (
            LaunchIdentity::local_test(player, uuid),
            String::new(),
            None,
            true,
        ),
    };
    let mut args = resolve_args(&installed, &instance, &platform, &identity)?;
    if let Some(server) = server {
        let supported = installed.version.arguments.as_ref().is_some_and(|arguments|
            arguments.game.iter().any(|argument| matches!(argument,
                Argument::Conditional { value: Values::Many(values), .. }
                    if values.first().is_some_and(|value| value == "--quickPlayMultiplayer"))));
        if !supported {
            return Err(AppError::new("JOIN_UNSUPPORTED", "This Minecraft version does not support Quick Play. Copy the address and use Multiplayer → Direct Connection."));
        }
        args.extend(["--quickPlayMultiplayer".into(), server]);
    }
    tokio::task::spawn_blocking({
        let game = instance.game_directory.clone();
        move || -> Result<()> {
            for (source, name) in installed.resource_map {
                let target = game.join("resources").join(name);
                // Existing user resources remain intact.
                if !target.exists() {
                    copy_atomic(&source, &target)?;
                }
            }
            Ok(())
        }
    })
    .await
    .map_err(|e| AppError::new("PROCESS", e.to_string()))??;
    let logs = log_root.join("game").join(uuid::Uuid::new_v4().to_string());
    tokio::fs::create_dir_all(&logs)
        .await
        .map_err(|e| AppError::io("create game log directory", e))?;
    let stdout = tokio::fs::File::create(logs.join("stdout.log"))
        .await
        .map_err(|e| AppError::io("create game stdout log", e))?;
    let stderr = tokio::fs::File::create(logs.join("stderr.log"))
        .await
        .map_err(|e| AppError::io("create game stderr log", e))?;
    let mut command = Command::new(java.executable);
    java::configure(&mut command);
    if let Some(path) = rocket_client_config {
        let mut argument = std::ffi::OsString::from("-Drocketlauncher.clientConfig=");
        argument.push(path);
        command.arg(argument);
    }
    command
        .current_dir(&instance.game_directory)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if expires_at.is_some_and(|expires_at| expires_at <= now()) {
        return Err(authentication_required());
    }
    let mut child = spawn(&mut command)?;
    #[cfg(windows)]
    let job = match windows_job(&child) {
        Ok(job) => job,
        Err(error) => {
            let _ = child.kill().await;
            return Err(error);
        }
    };
    let out = child.stdout.take().expect("piped stdout");
    let err = child.stderr.take().expect("piped stderr");
    let running_message = if local_test {
        format!(
            "Offline profile. Minecraft is running (PID {}).",
            child.id().unwrap_or_default()
        )
    } else {
        format!(
            "Minecraft is running (PID {}).",
            child.id().unwrap_or_default()
        )
    };
    report(GameStatus::new(&instance.id, Readiness::Running, running_message));
    tauri::async_runtime::spawn(async move {
        let token = access_token;
        let mut out = tokio::spawn(capture(out, stdout, token.clone()));
        let mut err = tokio::spawn(capture(err, stderr, token));
        let exit = child.wait().await;
        #[cfg(windows)]
        drop(job);
        if tokio::time::timeout(std::time::Duration::from_secs(2), async {
            let _ = tokio::join!(&mut out, &mut err);
        })
        .await
        .is_err()
        {
            out.abort();
            err.abort();
        }
        let sync_error = if let Some((root, pending)) = sync {
            match tokio::task::spawn_blocking(move || super::sync::finish(&root, &pending)).await {
                Ok(Ok(())) => None,
                Ok(Err(error)) => Some(error.message),
                Err(error) => Some(format!("Could not save shared instance settings: {error}")),
            }
        } else {
            None
        };
        drop(reservation);
        let (readiness, message, exit_code) = match exit {
            Ok(status) if sync_error.is_some() => (
                Readiness::Error,
                format!("Minecraft exited. Instance sync will retry on the next launch: {}", sync_error.unwrap_or_default()),
                status.code(),
            ),
            Ok(status) if status.success() => (
                Readiness::Ready,
                "Minecraft exited normally.".into(),
                status.code(),
            ),
            Ok(status) => (
                Readiness::Error,
                format!("Minecraft exited with {status}. Logs: {}", logs.display()),
                status.code(),
            ),
            Err(error) => (
                Readiness::Error,
                format!(
                    "Could not wait for Minecraft: {error}. Logs: {}",
                    logs.display()
                ),
                None,
            ),
        };
        tracing::info!(event = "minecraft_exit", instance_id = %instance.id, ?exit_code);
        report(GameStatus {
            instance_id: instance.id,
            readiness,
            message,
            exit_code,
        });
    });
    Ok(())
}

fn spawn(command: &mut Command) -> Result<Child> {
    command.spawn().map_err(|e| {
        AppError::new(
            "PROCESS",
            format!("Could not start Minecraft using the selected Java executable: {e}"),
        )
    })
}

async fn capture(input: impl AsyncRead + Unpin, mut output: tokio::fs::File, secret: String) {
    // Cap each log; keep draining after that so Minecraft cannot block on a full pipe.
    let mut reader = BufReader::new(input);
    let mut line = Vec::new();
    let mut discarded = false;
    let mut written = 0usize;
    while let Ok(chunk) = reader.fill_buf().await {
        if chunk.is_empty() {
            break;
        }
        let newline = chunk.iter().position(|&byte| byte == b'\n');
        let consumed = newline.map_or(chunk.len(), |n| n + 1);
        if !discarded && line.len() + consumed <= 128 * 1024 {
            line.extend_from_slice(&chunk[..consumed]);
        } else {
            discarded = true;
        }
        reader.consume(consumed);
        if newline.is_some() {
            if !discarded && written < 10 * 1024 * 1024 {
                let text = String::from_utf8_lossy(&line);
                let text = if secret.is_empty() {
                    text.into_owned()
                } else {
                    text.replace(&secret, "[redacted]")
                };
                if output.write_all(text.as_bytes()).await.is_err() {
                    written = usize::MAX;
                } else {
                    written += text.len();
                }
            }
            line.clear();
            discarded = false;
        }
    }
    if !discarded && !line.is_empty() && written < 10 * 1024 * 1024 {
        let text = String::from_utf8_lossy(&line);
        let text = if secret.is_empty() {
            text.into_owned()
        } else {
            text.replace(&secret, "[redacted]")
        };
        let _ = output.write_all(text.as_bytes()).await;
    }
}

#[cfg(windows)]
fn windows_job(child: &Child) -> Result<std::os::windows::io::OwnedHandle> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::System::JobObjects::*;
    let error = || {
        AppError::new(
            "PROCESS",
            format!(
                "Could not supervise Minecraft on Windows: {}",
                std::io::Error::last_os_error()
            ),
        )
    };
    let process = child.raw_handle().ok_or_else(|| {
        AppError::new(
            "PROCESS",
            "Minecraft exited before its process could be supervised.",
        )
    })?;
    // The unnamed job is private to this launcher. Its last handle closes on app exit.
    unsafe {
        let raw = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if raw.is_null() {
            return Err(error());
        }
        let job = OwnedHandle::from_raw_handle(raw);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            job.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&limits) as u32,
        ) == 0
            || AssignProcessToJobObject(job.as_raw_handle(), process) == 0
        {
            return Err(error());
        }
        Ok(job)
    }
}

#[cfg(all(test, windows, debug_assertions))]
pub(super) async fn smoke_render(
    installed: &Installed,
    instance: &Instance,
    platform: &Platform,
    java: Java,
) {
    let identity = LaunchIdentity::local_test(
        "RocketSmoke".into(),
        uuid::Uuid::new_v4().simple().to_string(),
    );
    let args = resolve_args(installed, instance, platform, &identity).unwrap();
    let error_log = tempfile::tempfile().unwrap();
    let mut command = Command::new(java.executable);
    java::configure(&mut command);
    command
        .current_dir(&instance.game_directory)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(error_log.try_clone().unwrap()));
    let mut child = spawn(&mut command).unwrap();
    let job = windows_job(&child).unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut tail = std::collections::VecDeque::with_capacity(16);
    let rendered = tokio::time::timeout(std::time::Duration::from_secs(90), async {
        while let Some(line) = lines.next_line().await.unwrap() {
            if line.contains("Created: ") && line.contains("blocks.png-atlas") {
                println!("{line}");
                return true;
            }
            if tail.len() == 16 {
                tail.pop_front();
            }
            tail.push_back(line);
        }
        false
    })
    .await
    .unwrap_or(false);
    let _ = child.kill().await;
    let _ = child.wait().await;
    drop(job);
    if !rendered {
        use std::io::{Read, Seek};
        let mut error_log = error_log;
        error_log.rewind().unwrap();
        let mut stderr = Vec::new();
        error_log.take(8192).read_to_end(&mut stderr).unwrap();
        panic!(
            "Minecraft did not initialize the block atlas: {tail:#?}\n{}",
            String::from_utf8_lossy(&stderr)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(debug_assertions)]
    #[test]
    fn local_test_identity_contains_no_authentication_material() {
        let identity = LaunchIdentity::local_test(
            "RocketTester".into(),
            "9c203c2e-8e51-4eed-8915-93a4b3131881".into(),
        );

        assert_eq!(identity.player, "RocketTester");
        assert_eq!(identity.access_token, "");
        assert_eq!(identity.auth_session, "");
        assert_eq!(identity.xuid, "");
        assert_eq!(identity.client_id, "");
        assert_eq!(identity.user_type, "legacy");
    }

    #[tokio::test]
    async fn captured_output_keeps_last_line_and_redacts_session_material() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("game.log");
        let file = tokio::fs::File::create(&path).await.unwrap();
        capture(&b"line one\nsecret-value"[..], file, "secret-value".into()).await;
        assert_eq!(
            tokio::fs::read_to_string(path).await.unwrap(),
            "line one\n[redacted]"
        );
    }
    #[test]
    fn resolves_arguments_without_splitting_paths_or_enabling_demo() {
        let platform = Platform {
            arch: Architecture::X64,
            version: "10.0".into(),
        };
        let paths = [
            PathBuf::from(r"C:\Game Data\library.jar"),
            PathBuf::from(r"C:\Game Data\client.jar"),
        ];
        let cp = classpath(&paths).unwrap();
        let values = BTreeMap::from([
            ("classpath", cp.clone()),
            ("game_directory", r"C:\Game Data".into()),
        ]);
        let modern: Vec<Argument> = serde_json::from_str(r#"["-cp","${classpath}",{"rules":[{"action":"allow","features":{"is_demo_user":true}}],"value":"--demo"}]"#).unwrap();
        assert_eq!(
            arguments(&modern, &platform, &values).unwrap(),
            ["-cp", &cp]
        );
        assert_eq!(
            legacy_arguments(
                r#"--gameDir "${game_directory}" --label "two words""#,
                &values
            )
            .unwrap(),
            ["--gameDir", r"C:\Game Data", "--label", "two words"]
        );
        assert!(expand("${unknown}", &values).is_err());
    }

    #[tokio::test]
    async fn spawn_failure_and_duplicate_reservation_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            spawn(&mut Command::new(dir.path().join("missing-java.exe")))
                .err()
                .unwrap()
                .code,
            "PROCESS"
        );
        let manager = ProcessManager::default();
        let reservation = manager.reserve("instance").unwrap();
        assert!(manager.reserve("instance").is_err());
        drop(reservation);
        assert!(manager.reserve("instance").is_ok());
    }
}
