use crate::errors::{AppError, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

pub fn invalid(message: impl Into<String>) -> AppError {
    AppError::new("GAME_METADATA", message)
}

pub fn relative(value: &str) -> Result<PathBuf> {
    if value.is_empty()
        || value.split('/').any(|p| {
            p.is_empty()
                || p == "."
                || p == ".."
                || p.ends_with(['.', ' '])
                || p.chars()
                    .any(|c| c.is_control() || "\\:*?\"<>|;".contains(c))
                || matches!(
                    p.split('.')
                        .next()
                        .unwrap_or("")
                        .to_ascii_uppercase()
                        .as_str(),
                    "CON"
                        | "PRN"
                        | "AUX"
                        | "NUL"
                        | "COM1"
                        | "COM2"
                        | "COM3"
                        | "COM4"
                        | "COM5"
                        | "COM6"
                        | "COM7"
                        | "COM8"
                        | "COM9"
                        | "LPT1"
                        | "LPT2"
                        | "LPT3"
                        | "LPT4"
                        | "LPT5"
                        | "LPT6"
                        | "LPT7"
                        | "LPT8"
                        | "LPT9"
                )
        })
    {
        return Err(invalid(format!("Unsafe Minecraft path: {value}")));
    }
    Ok(value.into())
}

pub fn segment(value: &str) -> Result<&str> {
    if value.contains('/') {
        return Err(invalid("Expected one Minecraft path component."));
    }
    relative(value)?;
    Ok(value)
}

#[derive(Clone, Debug, Deserialize)]
pub struct Artifact {
    pub url: String,
    pub sha1: String,
    pub size: u64,
    pub path: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct AssetIndex {
    pub id: String,
    #[serde(flatten)]
    pub artifact: Artifact,
}
#[derive(Debug, Deserialize)]
pub struct Assets {
    pub objects: BTreeMap<String, Asset>,
    #[serde(default, rename = "virtual")]
    pub virtual_: bool,
    #[serde(default)]
    pub map_to_resources: bool,
}
#[derive(Debug, Deserialize)]
pub struct Asset {
    pub hash: String,
    pub size: u64,
}
#[derive(Clone, Debug, Default, Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Option<Artifact>,
    #[serde(default)]
    pub classifiers: BTreeMap<String, Artifact>,
}
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Extract {
    #[serde(default)]
    pub exclude: Vec<String>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: LibraryDownloads,
    pub rules: Option<Vec<Rule>>,
    #[serde(default)]
    pub natives: BTreeMap<String, String>,
    #[serde(default)]
    pub extract: Extract,
}
impl Library {
    fn key(&self) -> (String, String, String) {
        let mut parts = self.name.split(':');
        let group = parts.next().unwrap_or("").into();
        let artifact = parts.next().unwrap_or("").into();
        (group, artifact, parts.nth(1).unwrap_or("").into())
    }
}
#[derive(Clone, Debug, Deserialize)]
pub struct Rule {
    pub action: Action,
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: BTreeMap<String, bool>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Allow,
    Disallow,
}
#[derive(Clone, Debug, Deserialize)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
    pub version: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Architecture {
    X86,
    X64,
    Arm64,
}
impl Architecture {
    pub fn host() -> Result<Self> {
        match std::env::consts::ARCH {
            "x86" => Ok(Self::X86),
            "x86_64" => Ok(Self::X64),
            "aarch64" => Ok(Self::Arm64),
            _ => Err(invalid("This CPU architecture is unsupported.")),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::X86 => "x86",
            Self::X64 => "x64",
            Self::Arm64 => "arm64",
        }
    }
    pub fn java_name(self) -> &'static str {
        match self {
            Self::X86 => "x86",
            Self::X64 => "amd64",
            Self::Arm64 => "aarch64",
        }
    }
    pub fn bits(self) -> &'static str {
        if self == Self::X86 {
            "32"
        } else {
            "64"
        }
    }
}
pub struct Platform {
    pub arch: Architecture,
    pub version: String,
}
impl Platform {
    pub fn windows() -> Result<Self> {
        Ok(Self {
            arch: Architecture::host()?,
            version: "10.0".into(),
        })
    }
}
pub fn allowed(rules: Option<&[Rule]>, platform: &Platform) -> Result<bool> {
    let Some(rules) = rules else { return Ok(true) };
    let mut allow = false;
    for rule in rules {
        if rule.features.values().any(|&enabled| enabled) {
            continue;
        }
        if let Some(os) = &rule.os {
            if os.name.as_deref().is_some_and(|s| s != "windows") {
                continue;
            }
            if let Some(arch) = &os.arch {
                let matches = match arch.as_str() {
                    "x86" | "i386" => platform.arch == Architecture::X86,
                    "x86_64" | "amd64" | "x64" => platform.arch == Architecture::X64,
                    "arm64" | "aarch64" => platform.arch == Architecture::Arm64,
                    pattern => Regex::new(pattern)
                        .map_err(invalid_regex)?
                        .is_match(platform.arch.java_name()),
                };
                if !matches {
                    continue;
                }
            }
            if let Some(pattern) = &os.version {
                if !Regex::new(pattern)
                    .map_err(invalid_regex)?
                    .is_match(&platform.version)
                {
                    continue;
                }
            }
        }
        allow = matches!(rule.action, Action::Allow);
    }
    Ok(allow)
}
fn invalid_regex(err: regex::Error) -> AppError {
    invalid(format!("Invalid Minecraft platform rule: {err}"))
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    Text(String),
    Conditional { rules: Vec<Rule>, value: Values },
}
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Values {
    One(String),
    Many(Vec<String>),
}
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub major_version: u32,
}
#[derive(Clone, Debug, Deserialize)]
pub struct LogConfig {
    pub argument: String,
    pub file: LogFile,
}
#[derive(Clone, Debug, Deserialize)]
pub struct LogFile {
    pub id: String,
    #[serde(flatten)]
    pub artifact: Artifact,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub id: String,
    pub inherits_from: Option<String>,
    pub main_class: Option<String>,
    pub java_version: Option<JavaVersion>,
    pub asset_index: Option<AssetIndex>,
    pub arguments: Option<Arguments>,
    pub minecraft_arguments: Option<String>,
    #[serde(default)]
    pub downloads: BTreeMap<String, Artifact>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub logging: BTreeMap<String, LogConfig>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
}
impl Version {
    pub fn inherit(mut self, child: Self) -> Self {
        self.id = child.id;
        self.inherits_from = child.inherits_from;
        self.main_class = child.main_class.or(self.main_class);
        self.java_version = child.java_version.or(self.java_version);
        self.asset_index = child.asset_index.or(self.asset_index);
        self.kind = child.kind.or(self.kind);
        if child.minecraft_arguments.is_some() && child.arguments.is_none() {
            self.arguments = None;
        }
        self.minecraft_arguments = child.minecraft_arguments.or(self.minecraft_arguments);
        if let Some(arguments) = child.arguments {
            let parent = self.arguments.get_or_insert_with(Arguments::default);
            parent.jvm.extend(arguments.jvm);
            parent.game.extend(arguments.game);
        }
        self.downloads.extend(child.downloads);
        self.logging.extend(child.logging);
        for library in child.libraries {
            let key = library.key();
            self.libraries.retain(|old| old.key() != key);
            self.libraries.push(library);
        }
        self
    }
    pub fn java_major(&self) -> u32 {
        self.java_version.as_ref().map_or(8, |v| v.major_version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_rules_use_last_match_and_architecture() {
        let rules: Vec<Rule> = serde_json::from_str(r#"[{"action":"allow","os":{"name":"windows","version":"^10\\."}},{"action":"disallow","os":{"arch":"arm64"}}]"#).unwrap();
        assert!(allowed(
            Some(&rules),
            &Platform {
                arch: Architecture::X64,
                version: "10.0".into()
            }
        )
        .unwrap());
        assert!(!allowed(
            Some(&rules),
            &Platform {
                arch: Architecture::Arm64,
                version: "10.0".into()
            }
        )
        .unwrap());
        assert!(!allowed(
            Some(&[]),
            &Platform {
                arch: Architecture::X64,
                version: "10.0".into()
            }
        )
        .unwrap());
    }

    #[test]
    fn inheritance_keeps_parent_java_and_overrides_library_version() {
        let parent: Version = serde_json::from_str(r#"{"id":"parent","mainClass":"Game","javaVersion":{"majorVersion":21},"libraries":[{"name":"org:lib:1"},{"name":"org:lib:1:natives-windows"}]}"#).unwrap();
        let child: Version = serde_json::from_str(
            r#"{"id":"child","inheritsFrom":"parent","libraries":[{"name":"org:lib:2"}]}"#,
        )
        .unwrap();
        let resolved = parent.inherit(child);
        assert_eq!(resolved.java_major(), 21);
        assert_eq!(resolved.main_class.as_deref(), Some("Game"));
        assert_eq!(
            resolved
                .libraries
                .iter()
                .map(|lib| lib.name.as_str())
                .collect::<Vec<_>>(),
            ["org:lib:1:natives-windows", "org:lib:2"]
        );
    }
}
