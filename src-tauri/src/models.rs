use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Dark,
    Light,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReducedMotion {
    System,
    Always,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub accent_color: String,
    pub theme: Theme,
    pub animations: bool,
    pub reduced_motion: ReducedMotion,
    pub default_ram_mb: u32,
    pub java_path: Option<PathBuf>,
    pub minecraft_directory: PathBuf,
    pub show_snapshots: bool,
    pub request_timeout_seconds: u64,
    #[serde(default = "enabled")]
    pub sync_servers: bool,
    #[serde(default = "enabled")]
    pub sync_keybinds: bool,
}

fn enabled() -> bool { true }

impl Settings {
    pub fn defaults(data_dir: &std::path::Path) -> Self {
        Self {
            accent_color: "#FF704D".into(),
            theme: Theme::Dark,
            animations: true,
            reduced_motion: ReducedMotion::System,
            default_ram_mb: 4096,
            java_path: None,
            minecraft_directory: data_dir.join("instances"),
            show_snapshots: false,
            request_timeout_seconds: 20,
            sync_servers: true,
            sync_keybinds: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    #[default]
    Vanilla,
    Fabric,
    Forge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub version: String,
    pub version_type: String,
    pub loader: Loader,
    #[serde(default)]
    pub loader_version: Option<String>,
    pub game_directory: PathBuf,
    pub ram_mb: u32,
    pub java_path: Option<PathBuf>,
    pub created_at: u64,
    pub updated_at: u64,
    pub last_selected_at: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceView {
    #[serde(flatten)]
    pub instance: Instance,
    pub readiness: crate::minecraft::launch::Readiness,
}

impl From<&Instance> for InstanceView {
    fn from(instance: &Instance) -> Self {
        Self {
            instance: instance.clone(),
            readiness: crate::minecraft::launch::Readiness::Checking,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstanceInput {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub loader: Loader,
    #[serde(default)]
    pub loader_version: Option<String>,
    pub ram_mb: u32,
    pub java_path: Option<PathBuf>,
    pub parent_directory: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Configuration {
    pub schema_version: u32,
    pub settings: Settings,
    pub instances: Vec<Instance>,
    pub active_instance_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPaths {
    pub data: PathBuf,
    pub cache: PathBuf,
    pub logs: PathBuf,
    pub config: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub app_version: &'static str,
    pub settings: Settings,
    pub instances: Vec<InstanceView>,
    pub active_instance_id: Option<String>,
    pub paths: AppPaths,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledMod {
    pub file_name: String,
    pub size_bytes: u64,
    pub enabled: bool,
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
