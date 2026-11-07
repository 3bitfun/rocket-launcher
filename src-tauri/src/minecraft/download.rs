use super::metadata::{invalid, Artifact};
use crate::errors::{AppError, Result};
use sha1::{Digest, Sha1};
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::io::AsyncWriteExt;

#[derive(Clone)]
pub struct FileSpec {
    pub relative: PathBuf,
    pub artifact: Artifact,
    pub max_size: u64,
}
impl FileSpec {
    pub fn new(relative: PathBuf, artifact: Artifact) -> Result<Self> {
        check_hash(&artifact.sha1)?;
        official_url(&artifact.url)?;
        let max_size = artifact.size;
        Ok(Self {
            relative,
            artifact,
            max_size,
        })
    }
    fn expected(&self) -> Option<u64> {
        (self.artifact.size != u64::MAX).then_some(self.artifact.size)
    }
    pub fn verify(&self, root: &Path) -> bool {
        verify(
            &root.join(&self.relative),
            &self.artifact.sha1,
            self.expected(),
            self.max_size,
        )
    }
}
pub fn check_hash(hash: &str) -> Result<()> {
    if hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid("Invalid SHA-1 in Minecraft metadata."));
    }
    Ok(())
}

fn official_url(url: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(url).map_err(|_| invalid("Invalid Minecraft download URL."))?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || !matches!(
            url.host_str(),
            Some(
                "piston-meta.mojang.com"
                    | "launchermeta.mojang.com"
                    | "piston-data.mojang.com"
                    | "launcher.mojang.com"
                    | "libraries.minecraft.net"
                    | "resources.download.minecraft.net"
                    | "meta.fabricmc.net"
                    | "maven.fabricmc.net"
                    | "maven.minecraftforge.net"
                    | "cdn.modrinth.com"
            )
        )
    {
        return Err(invalid(
            "Download is not on an approved HTTPS host.",
        ));
    }
    Ok(url)
}
pub fn client(timeout: u64) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(timeout.clamp(5, 120)))
        .read_timeout(Duration::from_secs(timeout.clamp(5, 120)))
        .user_agent(concat!("RocketLauncher/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| AppError::new("DOWNLOAD", e.to_string()))
}
pub fn file_hash(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
pub fn verify(path: &Path, hash: &str, size: Option<u64>, max_size: u64) -> bool {
    path.metadata().is_ok_and(|meta| {
        meta.is_file() && meta.len() <= max_size && size.map_or(true, |size| size == meta.len())
    }) && file_hash(path).is_ok_and(|actual| actual.eq_ignore_ascii_case(hash))
}
pub async fn ensure(
    client: &reqwest::Client,
    root: &Path,
    spec: &FileSpec,
    transferred: &(impl Fn(u64) + Sync),
) -> Result<()> {
    let cached = {
        let root = root.to_owned();
        let spec = spec.clone();
        tokio::task::spawn_blocking(move || spec.verify(&root))
            .await
            .map_err(|e| AppError::new("DOWNLOAD", e.to_string()))?
    };
    if cached {
        return Ok(());
    }
    let url = official_url(&spec.artifact.url)?;
    let path = root.join(&spec.relative);
    let parent = path
        .parent()
        .ok_or_else(|| invalid("Download destination has no parent."))?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| AppError::io("create download folder", e))?;
    for attempt in 0..3 {
        match fetch(client, url.clone(), &path, spec, transferred).await {
            Ok(()) => return Ok(()),
            Err(e) if attempt == 2 || !matches!(e.code, "DOWNLOAD" | "INTEGRITY") => return Err(e),
            Err(_) => tokio::time::sleep(Duration::from_millis(300 * (attempt + 1))).await,
        }
    }
    unreachable!()
}

async fn fetch(
    client: &reqwest::Client,
    url: reqwest::Url,
    path: &Path,
    spec: &FileSpec,
    transferred: &(impl Fn(u64) + Sync),
) -> Result<()> {
    let network = |e: reqwest::Error| {
        AppError::new(
            "DOWNLOAD",
            format!("Could not download {}: {e}", spec.relative.display()),
        )
    };
    let integrity = || {
        AppError::new(
            "INTEGRITY",
            format!("Wrong size or SHA-1 for {}.", spec.relative.display()),
        )
    };
    let mut response = client.get(url).send().await.map_err(network)?;
    if !response.status().is_success() {
        return Err(AppError::new(
            "DOWNLOAD",
            format!(
                "The download server returned HTTP {} for {}.",
                response.status(),
                spec.relative.display()
            ),
        ));
    }
    if response.content_length().is_some_and(|len| {
        len > spec.max_size || spec.expected().is_some_and(|expected| len != expected)
    }) {
        return Err(integrity());
    }
    let temp = tempfile::NamedTempFile::new_in(path.parent().unwrap())
        .map_err(|e| AppError::io("create temporary download", e))?;
    let mut output = tokio::fs::File::from_std(
        temp.reopen()
            .map_err(|e| AppError::io("open temporary download", e))?,
    );
    let mut sha = Sha1::new();
    let mut length = 0;
    while let Some(chunk) = response.chunk().await.map_err(network)? {
        length += chunk.len() as u64;
        transferred(chunk.len() as u64);
        if length > spec.max_size {
            return Err(integrity());
        }
        sha.update(&chunk);
        output
            .write_all(&chunk)
            .await
            .map_err(|e| AppError::io("write download", e))?;
    }
    if spec.expected().is_some_and(|expected| length != expected)
        || !format!("{:x}", sha.finalize()).eq_ignore_ascii_case(&spec.artifact.sha1)
    {
        return Err(integrity());
    }
    output
        .sync_all()
        .await
        .map_err(|e| AppError::io("flush download", e))?;
    drop(output);
    temp.persist(path)
        .map_err(|e| AppError::io("publish verified download", e.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_size_and_rejects_same_size_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        let hash = "a9993e364706816aba3e25717850c26c9cd0d89d";
        std::fs::write(&path, b"abc").unwrap();
        assert!(verify(&path, hash, Some(3), 3));
        std::fs::write(&path, b"xyz").unwrap();
        assert!(!verify(&path, hash, Some(3), 3));
        assert!(official_url("https://libraries.minecraft.net.example.com/x").is_err());
    }

    #[tokio::test]
    async fn interrupted_response_never_publishes_partial_file() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            let _ = stream.read(&mut request).await.unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nab")
                .await
                .unwrap();
        });
        let dir = tempfile::tempdir().unwrap();
        let spec = FileSpec::new(
            "test.jar".into(),
            Artifact {
                url: "https://libraries.minecraft.net/test.jar".into(),
                sha1: "a9993e364706816aba3e25717850c26c9cd0d89d".into(),
                size: 3,
                path: None,
            },
        )
        .unwrap();
        let output = dir.path().join("test.jar");
        assert!(fetch(
            &reqwest::Client::new(),
            format!("http://{address}/").parse().unwrap(),
            &output,
            &spec,
            &|_| {}
        )
        .await
        .is_err());
        assert!(!output.exists());
        assert_eq!(dir.path().read_dir().unwrap().count(), 0);
    }
}
