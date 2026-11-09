use crate::{
    errors::{AppError, Result},
    instances::mods_directory,
    models::Instance,
    storage::atomic_json,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    net::IpAddr,
    path::Path,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

pub const VIA_PLUS: &str = "rIC2XJV4";
pub const VIA_FABRIC: &str = "YlKdE5VK";
pub const E4MC: &str = "qANg5Jrr";
pub const FABRIC_API: &str = "P7dR8mSH";

fn error(message: impl Into<String>) -> AppError {
    AppError::new("MULTIPLAYER", message)
}

pub struct Address {
    host: String,
    port: u16,
    explicit_port: bool,
}

impl Address {
    pub fn parse(value: &str) -> Result<Self> {
        let value = value.trim();
        let invalid = || error("Enter a Java server hostname or IP, optionally followed by :port.");
        let (host, port) = if let Some(value) = value.strip_prefix('[') {
            let (host, tail) = value.split_once(']').ok_or_else(invalid)?;
            if host.parse::<std::net::Ipv6Addr>().is_err() {
                return Err(invalid());
            }
            let port = if tail.is_empty() {
                None
            } else {
                Some(tail.strip_prefix(':').ok_or_else(invalid)?)
            };
            (host, port)
        } else if value.parse::<IpAddr>().is_ok() {
            (value, None)
        } else if let Some((host, port)) = value.split_once(':') {
            (host, Some(port))
        } else {
            (value, None)
        };
        let host = host.trim_end_matches('.');
        if host.parse::<IpAddr>().is_err()
            && (host.len() > 253
                || host.split('.').any(|label| {
                    label.is_empty()
                        || label.len() > 63
                        || label.starts_with('-')
                        || label.ends_with('-')
                        || !label
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                }))
        {
            return Err(invalid());
        }
        let explicit_port = port.is_some();
        let port = port
            .map_or(Ok(25565), str::parse::<u16>)
            .map_err(|_| invalid())?;
        if port == 0 {
            return Err(invalid());
        }
        Ok(Self {
            host: host.to_ascii_lowercase(),
            port,
            explicit_port,
        })
    }

    pub fn minecraft(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        if self.explicit_port {
            format!("{host}:{}", self.port)
        } else {
            host
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedServer {
    pub id: String,
    pub name: String,
    pub address: String,
    pub instance_id: Option<String>,
    pub favorite: bool,
    #[serde(default)]
    pub last_joined: Option<u64>,
}

pub fn saved_servers(data: &Path) -> Result<Vec<SavedServer>> {
    let path = data.join("multiplayer.json");
    match fs::File::open(path) {
        Ok(file) => serde_json::from_reader(file.take(128 * 1024))
            .map_err(|_| error("Could not read your saved servers.")),
        Err(reason) if reason.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(reason) => Err(AppError::io("open saved servers", reason)),
    }
}

pub fn save_server(data: &Path, mut server: SavedServer) -> Result<Vec<SavedServer>> {
    server.name = server.name.trim().to_owned();
    if server.name.is_empty()
        || server.name.chars().count() > 64
        || server.name.chars().any(char::is_control)
    {
        return Err(error("Give the server a name of 1–64 characters."));
    }
    server.address = Address::parse(&server.address)?.minecraft();
    let mut servers = saved_servers(data)?;
    let previous = servers.iter().position(|entry| entry.id == server.id);
    if servers
        .iter()
        .any(|entry| entry.id != server.id && entry.address == server.address)
    {
        return Err(error("That server address is already saved."));
    }
    if let Some(index) = previous {
        servers[index] = server;
    } else {
        if servers.len() >= 100 {
            return Err(error("Remove a saved server before adding another."));
        }
        server.id = uuid::Uuid::new_v4().to_string();
        servers.push(server);
    }
    atomic_json(&data.join("multiplayer.json"), &servers)?;
    Ok(servers)
}

pub fn remove_server(data: &Path, id: &str) -> Result<Vec<SavedServer>> {
    let mut servers = saved_servers(data)?;
    servers.retain(|server| server.id != id);
    atomic_json(&data.join("multiplayer.json"), &servers)?;
    Ok(servers)
}

#[derive(Serialize)]
pub struct InstalledAddon {
    pub id: String,
    pub filename: String,
    pub enabled: bool,
}

pub fn addons(instance: &Instance) -> Result<Vec<InstalledAddon>> {
    let folder = instance.game_directory.join("mods");
    if !folder.exists() {
        return Ok(vec![]);
    }
    let folder = mods_directory(&instance.game_directory)?;
    let mut result = Vec::new();
    for entry in fs::read_dir(folder).map_err(|e| AppError::io("read installed mods", e))? {
        let entry = entry.map_err(|e| AppError::io("read an installed mod", e))?;
        if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        let filename = entry.file_name().to_string_lossy().into_owned();
        let name = filename.strip_suffix(".disabled").unwrap_or(&filename);
        if !name.to_ascii_lowercase().ends_with(".jar") {
            continue;
        }
        if let Some(id) = addon_id(&entry.path()) {
            result.push(InstalledAddon {
                id,
                filename: name.into(),
                enabled: name == filename,
            });
        }
    }
    Ok(result)
}

fn addon_id(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let mut zip = zip::ZipArchive::new(file).ok()?;
    let file = zip.by_name("fabric.mod.json").ok()?;
    let meta: serde_json::Value = serde_json::from_reader(file.take(64 * 1024)).ok()?;
    let id = meta["id"].as_str()?;
    (is_via(id) || is_hosting(id)).then(|| id.into())
}

pub fn ensure_addon_can_enable(instance: &Instance, filename: &str) -> Result<()> {
    super::metadata::segment(filename)?;
    let folder = mods_directory(&instance.game_directory)?;
    let path = folder.join(format!("{filename}.disabled"));
    if !fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_file()) {
        return Ok(());
    }
    if addon_id(&path).is_some_and(|id| is_via(&id))
        && addons(instance)?.iter().any(|item| item.enabled && is_via(&item.id))
    {
        return Err(error("Disable the active ViaFabric variant before enabling another one."));
    }
    Ok(())
}

pub fn is_via(id: &str) -> bool {
    matches!(id, "viafabric" | "viafabricplus")
}

pub fn is_hosting(id: &str) -> bool {
    matches!(id, "e4mc" | "e4mc_minecraft")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    version: String,
    protocol: i32,
    players: u32,
    max_players: u32,
    latency_ms: u64,
}

#[derive(Deserialize)]
struct StatusResponse {
    version: ServerVersion,
    players: ServerPlayers,
}
#[derive(Deserialize)]
struct ServerVersion {
    name: String,
    protocol: i32,
}
#[derive(Deserialize)]
struct ServerPlayers {
    online: u32,
    max: u32,
}

fn varint(mut value: u32, out: &mut Vec<u8>) {
    while value >= 128 {
        out.push((value as u8 & 127) | 128);
        value >>= 7;
    }
    out.push(value as u8);
}

async fn read_varint(stream: &mut (impl tokio::io::AsyncRead + Unpin)) -> std::io::Result<u32> {
    let mut value = 0u32;
    for shift in [0, 7, 14, 21, 28] {
        let byte = stream.read_u8().await?;
        if shift == 28 && byte > 15 {
            break;
        }
        value |= u32::from(byte & 127) << shift;
        if byte & 128 == 0 {
            return Ok(value);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "invalid VarInt",
    ))
}

#[cfg(windows)]
fn resolve_srv(host: &str) -> Option<(String, u16)> {
    use windows_sys::Win32::NetworkManagement::Dns::{
        DnsFree, DnsFreeRecordList, DnsQuery_W, DNS_RECORDW, DNS_TYPE_SRV,
    };
    let query: Vec<_> = format!("_minecraft._tcp.{host}.")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut records = std::ptr::null_mut();
    let code = unsafe {
        DnsQuery_W(
            query.as_ptr(),
            DNS_TYPE_SRV,
            0,
            std::ptr::null_mut(),
            &mut records,
            std::ptr::null_mut(),
        )
    };
    let mut selected: Option<(String, u16, u16, u16)> = None;
    let mut next = records.cast::<DNS_RECORDW>();
    while code == 0 && !next.is_null() {
        let record = unsafe { &*next };
        next = record.pNext;
        if record.wType != DNS_TYPE_SRV {
            continue;
        }
        let srv = unsafe { record.Data.SRV };
        if srv.pNameTarget.is_null() || srv.wPort == 0 {
            continue;
        }
        let len = (0..=254).find(|index| unsafe { *srv.pNameTarget.add(*index) } == 0);
        let Some(len) = len else {
            continue;
        };
        let host =
            String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(srv.pNameTarget, len) });
        let host = host.trim_end_matches('.');
        if Address::parse(host).is_err() {
            continue;
        }
        if selected.as_ref().map_or(true, |(_, _, priority, weight)| {
            (srv.wPriority, std::cmp::Reverse(srv.wWeight))
                < (*priority, std::cmp::Reverse(*weight))
        }) {
            selected = Some((host.into(), srv.wPort, srv.wPriority, srv.wWeight));
        }
    }
    if !records.is_null() {
        unsafe { DnsFree(records.cast(), DnsFreeRecordList) };
    }
    selected.map(|(host, port, _, _)| (host, port))
}

pub async fn ping(address: Address) -> Result<ServerStatus> {
    tokio::time::timeout(Duration::from_secs(5), async {
        let host = address.host.clone();
        let mut target = (host.clone(), address.port);
        #[cfg(windows)]
        if !address.explicit_port && host.parse::<IpAddr>().is_err() {
            if let Ok(Some(record)) = tokio::task::spawn_blocking(move || resolve_srv(&host)).await
            {
                target = record;
            }
        }
        let mut stream = TcpStream::connect((target.0.as_str(), target.1)).await?;
        let start = Instant::now();
        let mut handshake = vec![0];
        varint(u32::MAX, &mut handshake);
        varint(address.host.len() as u32, &mut handshake);
        handshake.extend_from_slice(address.host.as_bytes());
        handshake.extend_from_slice(&address.port.to_be_bytes());
        handshake.push(1);
        let mut request = Vec::new();
        varint(handshake.len() as u32, &mut request);
        request.extend(handshake);
        request.extend([1, 0]);
        stream.write_all(&request).await?;
        let length = read_varint(&mut stream).await?;
        if length == 0 || length > 131_072 {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        let mut packet = vec![0u8; length as usize];
        stream.read_exact(&mut packet).await?;
        let mut response = packet.as_slice();
        if read_varint(&mut response).await? != 0 {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        let len = read_varint(&mut response).await? as usize;
        if len != response.len() {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        let status: StatusResponse = serde_json::from_slice(response)?;
        Ok::<_, std::io::Error>(ServerStatus {
            version: status
                .version
                .name
                .chars()
                .filter(|c| !c.is_control())
                .take(80)
                .collect(),
            protocol: status.version.protocol,
            players: status.players.online,
            max_players: status.players.max,
            latency_ms: start.elapsed().as_millis() as u64,
        })
    })
    .await
    .map_err(|_| error("The server did not respond in time."))?
    .map_err(|_| error("Could not read this Java server’s status. Check the address and port."))
}
