use super::metadata::Architecture;
use crate::errors::{AppError, Result};
use std::{
    collections::HashSet,
    ffi::OsStr,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::process::Command;

#[derive(Debug)]
pub struct Java {
    pub executable: PathBuf,
    pub major: u32,
    pub arch: Architecture,
}

pub fn configure(command: &mut Command) {
    command
        .kill_on_drop(true)
        .env_remove("JAVA_TOOL_OPTIONS")
        .env_remove("_JAVA_OPTIONS")
        .env_remove("JDK_JAVA_OPTIONS")
        .env_remove("CLASSPATH");
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW
}

fn parse_probe(output: &str, executable: PathBuf) -> Result<Java> {
    let property = |name: &str| {
        output.lines().find_map(|line| {
            let (key, value) = line.trim().split_once('=')?;
            (key.trim() == name).then_some(value.trim())
        })
    };
    let invalid = || {
        AppError::new(
            "JAVA",
            format!(
                "Could not determine the Java version and architecture for {}.",
                executable.display()
            ),
        )
    };
    let version = property("java.specification.version").ok_or_else(invalid)?;
    let major = version
        .strip_prefix("1.")
        .unwrap_or(version)
        .split('.')
        .next()
        .and_then(|value| value.parse().ok())
        .ok_or_else(invalid)?;
    let arch = match property("os.arch").unwrap_or("") {
        "amd64" | "x86_64" => Architecture::X64,
        "x86" | "i386" | "i586" | "i686" => Architecture::X86,
        "aarch64" | "arm64" => Architecture::Arm64,
        _ => return Err(invalid()),
    };
    Ok(Java {
        executable,
        major,
        arch,
    })
}

async fn probe(executable: PathBuf) -> Result<Java> {
    let mut command = Command::new(&executable);
    configure(&mut command);
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        command
            .args(["-XshowSettings:properties", "-version"])
            .output(),
    )
    .await
    .map_err(|_| AppError::new("JAVA", "Java did not respond within five seconds."))?
    .map_err(|e| {
        AppError::new(
            "JAVA",
            format!("Could not run {}: {e}", executable.display()),
        )
    })?;
    if !result.status.success() {
        return Err(AppError::new(
            "JAVA",
            format!(
                "{} returned {} during the version check. Select a working Java executable.",
                executable.display(),
                result.status
            ),
        ));
    }
    let output = [result.stderr, result.stdout].concat();
    parse_probe(&String::from_utf8_lossy(&output), executable)
}

fn scan(root: &Path, depth: u8, found: &mut Vec<PathBuf>) {
    if found.len() >= 96 {
        return;
    }
    let bin = root.join("bin/java.exe");
    if bin.is_file() {
        found.push(bin);
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten().take(96) {
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            scan(&entry.path(), depth - 1, found);
        }
    }
}

fn candidates(data: &Path) -> Vec<PathBuf> {
    let mut found = vec![];
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        found.push(PathBuf::from(home).join("bin/java.exe"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        found.extend(
            std::env::split_paths(&path)
                .filter(|p| p.is_absolute())
                .map(|p| p.join("java.exe")),
        );
    }
    for variable in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Some(programs) = std::env::var_os(variable) {
            for folder in [
                "Java",
                "Eclipse Adoptium",
                "Microsoft",
                "Amazon Corretto",
                "Zulu",
                "BellSoft",
                "Semeru",
                "Minecraft Launcher/runtime",
            ] {
                scan(&PathBuf::from(&programs).join(folder), 3, &mut found);
            }
        }
    }
    scan(&data.join("runtime"), 4, &mut found);
    scan(&data.join("runtimes"), 4, &mut found);
    if let Some(appdata) = std::env::var_os("APPDATA") {
        scan(
            &PathBuf::from(appdata).join(".minecraft/runtime"),
            4,
            &mut found,
        );
    }
    found
}

fn compatible(java: &Java, major: u32, arch: Architecture) -> bool {
    // Older LWJGL and JVM options are not guaranteed to work on later Java majors.
    java.major == major && java.arch == arch
}

pub async fn resolve(
    configured: Option<PathBuf>,
    data: &Path,
    major: u32,
    arch: Architecture,
) -> Result<Java> {
    let requirement = format!(
        "Install Java {major} ({}) or choose its java.exe in the instance editor.",
        arch.name()
    );
    if let Some(path) = configured {
        let path = absolute_executable(&path).ok_or_else(|| {
            AppError::new(
                "JAVA",
                format!("Configured Java executable is missing or invalid. {requirement}"),
            )
        })?;
        let java = probe(path)
            .await
            .map_err(|e| AppError::new("JAVA", format!("{} {requirement}", e.message)))?;
        if compatible(&java, major, arch) {
            return Ok(java);
        }
        return Err(AppError::new(
            "JAVA",
            format!(
                "Configured Java is {} ({}). {requirement}",
                java.major,
                java.arch.name()
            ),
        ));
    }
    let data = data.to_owned();
    let candidates = tokio::task::spawn_blocking(move || candidates(&data))
        .await
        .map_err(|e| AppError::new("JAVA", e.to_string()))?;
    let mut seen = HashSet::new();
    for path in candidates
        .iter()
        .filter_map(|path| absolute_executable(path))
        .filter(|path| seen.insert(path.clone()))
        .take(24)
    {
        if let Ok(java) = probe(path).await {
            if compatible(&java, major, arch) {
                return Ok(java);
            }
        }
    }
    Err(AppError::new(
        "JAVA",
        format!("No compatible Java runtime found. {requirement}"),
    ))
}

fn absolute_executable(path: &Path) -> Option<PathBuf> {
    (path.is_absolute()
        && path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case(OsStr::new("exe")))
        && path.is_file())
    .then(|| path.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_legacy_and_modern_java_and_rejects_wrong_runtime() {
        let old = parse_probe(
            "java.specification.version = 1.8\nos.arch = x86",
            "java.exe".into(),
        )
        .unwrap();
        let modern = parse_probe(
            "java.specification.version = 21\nos.arch = amd64",
            "java.exe".into(),
        )
        .unwrap();
        assert!(compatible(&old, 8, Architecture::X86));
        assert!(!compatible(&old, 8, Architecture::X64));
        assert!(compatible(&modern, 21, Architecture::X64));
        assert!(!compatible(&modern, 17, Architecture::X64));
    }
    #[tokio::test]
    #[ignore = "requires x64 Java 21 installed on the host"]
    async fn live_java_detection() {
        let java = resolve(None, Path::new("unused"), 21, Architecture::X64)
            .await
            .unwrap();
        assert_eq!(java.major, 21);
        assert!(java.executable.is_file());
    }
}
