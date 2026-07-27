use crate::{config::Config, utils::dirs};
use anyhow::{Error, bail};
use arc_swap::ArcSwap;
use backon::{ConstantBuilder, Retryable as _};
use clash_verge_logging::{Type, logging};
use once_cell::sync::OnceCell;
use reqwest_dav::list_cmd::{ListEntity, ListFile, ListMultiStatus};
use smartstring::alias::String;
use std::{
    collections::HashMap,
    env::consts::OS,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{fs, io::AsyncWriteExt as _, time::timeout};

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

const TIMEOUT_UPLOAD: u64 = 300;
const TIMEOUT_DOWNLOAD: u64 = 300;
const TIMEOUT_LIST: u64 = 30;
const TIMEOUT_DELETE: u64 = 30;
const MAX_WEBDAV_BACKUP_SIZE: u64 = 544 * 1024 * 1024;
const MAX_WEBDAV_LIST_SIZE: u64 = 8 * 1024 * 1024;

#[derive(Clone)]
struct WebDavConfig {
    url: String,
    username: String,
    password: String,
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
enum Operation {
    Upload,
    Download,
    List,
    Delete,
}

impl Operation {
    const fn timeout(&self) -> u64 {
        match self {
            Self::Upload => TIMEOUT_UPLOAD,
            Self::Download => TIMEOUT_DOWNLOAD,
            Self::List => TIMEOUT_LIST,
            Self::Delete => TIMEOUT_DELETE,
        }
    }
}

pub struct WebDavClient {
    generation: AtomicU64,
    clients: ArcSwap<ClientCache>,
}

struct ClientCache {
    generation: u64,
    by_operation: HashMap<Operation, reqwest_dav::Client>,
}

struct PartialDownload {
    path: PathBuf,
    created: AtomicBool,
    committed: AtomicBool,
}

impl PartialDownload {
    const fn new(path: PathBuf) -> Self {
        Self {
            path,
            created: AtomicBool::new(false),
            committed: AtomicBool::new(false),
        }
    }

    fn mark_created(&self) {
        self.created.store(true, Ordering::Release);
    }

    fn disarm(&self) {
        self.committed.store(true, Ordering::Release);
    }
}

impl Drop for PartialDownload {
    fn drop(&mut self) {
        if self.created.load(Ordering::Acquire)
            && !self.committed.load(Ordering::Acquire)
            && let Err(err) = std::fs::remove_file(&self.path)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            logging!(
                warn,
                Type::Backup,
                "Failed to remove partial WebDAV download {}: {err}",
                self.path.display()
            );
        }
    }
}

fn create_partial_download_file(path: &Path, guard: &PartialDownload) -> std::io::Result<fs::File> {
    // Keep creation and ownership marking cancellation-free so a completed
    // create_new cannot leave an unowned partial file behind.
    let file = std::fs::OpenOptions::new().write(true).create_new(true).open(path)?;
    guard.mark_created();
    Ok(fs::File::from_std(file))
}

impl ClientCache {
    fn empty(generation: u64) -> Self {
        Self {
            generation,
            by_operation: HashMap::new(),
        }
    }
}

impl WebDavClient {
    fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            clients: ArcSwap::new(Arc::new(ClientCache::empty(0))),
        }
    }

    pub fn global() -> &'static Self {
        static WEBDAV_CLIENT: OnceCell<WebDavClient> = OnceCell::new();
        WEBDAV_CLIENT.get_or_init(Self::new)
    }

    fn is_generation_current(&self, generation: u64) -> bool {
        self.generation.load(Ordering::Acquire) == generation
    }

    fn cached_client(&self, generation: u64, op: Operation) -> Option<reqwest_dav::Client> {
        let clients = self.clients.load();
        if clients.generation != generation {
            return None;
        }
        let client = clients.by_operation.get(&op)?.clone();
        self.is_generation_current(generation).then_some(client)
    }

    fn cache_client_if_current(&self, generation: u64, op: Operation, client: reqwest_dav::Client) -> bool {
        if !self.is_generation_current(generation) {
            return false;
        }

        self.clients.rcu(|clients| {
            if !self.is_generation_current(generation) {
                return Arc::clone(clients);
            }

            let mut by_operation = if clients.generation == generation {
                clients.by_operation.clone()
            } else {
                HashMap::new()
            };
            by_operation.insert(op, client.clone());
            Arc::new(ClientCache {
                generation,
                by_operation,
            })
        });

        self.is_generation_current(generation) && self.clients.load().generation == generation
    }

    fn reset_if_current(&self, generation: u64) -> bool {
        let next_generation = generation.wrapping_add(1);
        if self
            .generation
            .compare_exchange(generation, next_generation, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        self.clients.store(Arc::new(ClientCache::empty(next_generation)));
        true
    }

    async fn get_client(&self, op: Operation) -> Result<reqwest_dav::Client, Error> {
        let generation = self.generation.load(Ordering::Acquire);
        if let Some(client) = self.cached_client(generation, op) {
            return Ok(client);
        }

        let verge = Config::verge().await.data_arc();
        if verge.webdav_url.is_none() || verge.webdav_username.is_none() || verge.webdav_password.is_none() {
            let msg: String = "Unable to create web dav client, please make sure the webdav config is correct".into();
            return Err(anyhow::Error::msg(msg));
        }
        let config = WebDavConfig {
            url: verge
                .webdav_url
                .clone()
                .unwrap_or_default()
                .trim_end_matches('/')
                .into(),
            username: verge.webdav_username.clone().unwrap_or_default(),
            password: verge.webdav_password.clone().unwrap_or_default(),
        };
        if !self.is_generation_current(generation) {
            bail!("WebDAV configuration changed while the client was being initialized");
        }

        let client = reqwest_dav::ClientBuilder::new()
            .set_agent(
                reqwest::Client::builder()
                    .use_rustls_tls()
                    .danger_accept_invalid_certs(true)
                    .timeout(Duration::from_secs(op.timeout()))
                    .user_agent(format!("clash-verge/{APP_VERSION} ({OS} WebDAV-Client)"))
                    .redirect(reqwest::redirect::Policy::custom(|attempt| {
                        // 允许所有请求类型的重定向，包括PUT
                        if attempt.previous().len() >= 5 {
                            attempt.error("重定向次数过多")
                        } else {
                            attempt.follow()
                        }
                    }))
                    .build()?,
            )
            .set_host(config.url.into())
            .set_auth(reqwest_dav::Auth::Basic(config.username.into(), config.password.into()))
            .build()?;

        if let Err(e) = client.mkcol(dirs::BACKUP_DIR).await {
            if !self.is_generation_current(generation) {
                bail!("WebDAV configuration changed while the client was being initialized");
            }
            let (status_code, message) = match &e {
                reqwest_dav::Error::Decode(reqwest_dav::DecodeError::Server(server_err)) => {
                    (Some(server_err.response_code), Some(server_err.message.as_str()))
                }
                reqwest_dav::Error::Decode(reqwest_dav::DecodeError::StatusMismatched(status_err)) => {
                    (Some(status_err.response_code), None)
                }
                reqwest_dav::Error::Reqwest(http_err) => (http_err.status().map(|s| s.as_u16()), None),
                _ => (None, None),
            };

            if status_code == Some(409) {
                logging!(
                    warn,
                    Type::Backup,
                    "Backup directory cannot be created because its parent folder does not exist"
                );
                self.reset_if_current(generation);
                return Err(anyhow::Error::msg(
                    "Failed to create backup directory: parent directory does not exist",
                ));
            }

            let already_exists = status_code == Some(405)
                || message.is_some_and(|m| {
                    let m = m.to_ascii_lowercase();
                    m.contains("already exist") || m.contains("already taken")
                });

            if already_exists {
                logging!(info, Type::Backup, "Backup directory already exists");
            } else {
                logging!(warn, Type::Backup, "Failed to create backup directory: {}", e);
                self.reset_if_current(generation);
                return Err(anyhow::Error::msg(format!("Failed to create backup directory: {}", e)));
            }
        } else {
            logging!(info, Type::Backup, "Successfully created backup directory");
        }

        if !self.cache_client_if_current(generation, op, client.clone()) {
            bail!("WebDAV configuration changed while the client was being initialized");
        }

        Ok(client)
    }

    pub fn reset(&self) {
        let next_generation = self.generation.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
        self.clients.store(Arc::new(ClientCache::empty(next_generation)));
    }

    pub async fn upload(&self, file_path: PathBuf, file_name: String) -> Result<(), Error> {
        let client = self.get_client(Operation::Upload).await?;
        let webdav_path: String = format!("{}/{}", dirs::BACKUP_DIR, file_name).into();

        let file_content = fs::read(&file_path).await?;

        let backoff = ConstantBuilder::default()
            .with_delay(Duration::from_millis(500))
            .with_max_times(1);

        (|| async {
            timeout(
                Duration::from_secs(TIMEOUT_UPLOAD),
                client.put(&webdav_path, file_content.clone()),
            )
            .await??;
            Ok::<(), Error>(())
        })
        .retry(backoff)
        .notify(|err, dur| {
            logging!(warn, Type::Backup, "Upload failed: {err}, retrying in {dur:?}");
        })
        .await
    }

    pub async fn download(&self, filename: String, storage_path: PathBuf) -> Result<(), Error> {
        let client = self.get_client(Operation::Download).await?;
        let path = format!("{}/{}", dirs::BACKUP_DIR, filename);
        let partial_download = Arc::new(PartialDownload::new(storage_path.clone()));
        let download_guard = Arc::clone(&partial_download);

        let fut = async {
            let mut response = client.get(path.as_str()).await?;
            if !response.status().is_success() {
                bail!("WebDAV download failed with status {}", response.status());
            }
            if response
                .content_length()
                .is_some_and(|length| length > MAX_WEBDAV_BACKUP_SIZE)
            {
                bail!("WebDAV backup exceeds the download size limit");
            }

            let mut output = create_partial_download_file(&storage_path, &download_guard)?;

            let mut downloaded = 0_u64;
            while let Some(chunk) = response.chunk().await? {
                downloaded = checked_body_size(downloaded, chunk.len(), MAX_WEBDAV_BACKUP_SIZE)?;
                output.write_all(&chunk).await?;
            }
            output.flush().await?;
            output.sync_all().await?;
            Ok::<(), Error>(())
        };

        let result = match timeout(Duration::from_secs(TIMEOUT_DOWNLOAD), fut).await {
            Ok(result) => result,
            Err(err) => Err(err.into()),
        };
        if result.is_ok() {
            partial_download.disarm();
        }
        result
    }

    pub async fn list(&self) -> Result<Vec<ListFile>, Error> {
        let client = self.get_client(Operation::List).await?;
        let path = format!("{}/", dirs::BACKUP_DIR);

        let fut = async {
            let response = client.list_raw(path.as_str(), reqwest_dav::Depth::Number(1)).await?;
            let status = response.status();
            if !status.is_success() {
                bail!("WebDAV list failed with status {status}");
            }

            let body = read_response_limited(response, MAX_WEBDAV_LIST_SIZE).await?;
            let xml = std::str::from_utf8(&body)?;
            let files = parse_webdav_list(xml)?;
            let mut final_files = Vec::new();
            for file in files {
                if let ListEntity::File(file) = file {
                    final_files.push(file);
                }
            }
            Ok::<Vec<ListFile>, Error>(final_files)
        };

        timeout(Duration::from_secs(TIMEOUT_LIST), fut).await?
    }

    pub async fn delete(&self, file_name: String) -> Result<(), Error> {
        let client = self.get_client(Operation::Delete).await?;
        let path = format!("{}/{}", dirs::BACKUP_DIR, file_name);

        let fut = client.delete(&path);
        timeout(Duration::from_secs(TIMEOUT_DELETE), fut).await??;
        Ok(())
    }
}

fn checked_body_size(current: u64, chunk_size: usize, limit: u64) -> Result<u64, Error> {
    let next = current
        .checked_add(chunk_size as u64)
        .ok_or_else(|| anyhow::anyhow!("WebDAV response size overflow"))?;
    if next > limit {
        bail!("WebDAV response exceeds the size limit");
    }
    Ok(next)
}

async fn read_response_limited(mut response: reqwest::Response, limit: u64) -> Result<Vec<u8>, Error> {
    if response.content_length().is_some_and(|length| length > limit) {
        bail!("WebDAV response exceeds the size limit");
    }

    let mut body = Vec::new();
    let mut received = 0_u64;
    while let Some(chunk) = response.chunk().await? {
        received = checked_body_size(received, chunk.len(), limit)?;
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn parse_webdav_list(xml: &str) -> Result<Vec<ListEntity>, reqwest_dav::Error> {
    let normalized_xml = xml.replace(" +0000</", " GMT</");
    let multi_status: ListMultiStatus = serde_xml_rs::from_str(&normalized_xml)?;
    multi_status.responses.into_iter().map(ListEntity::try_from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_client() -> Result<reqwest_dav::Client, reqwest_dav::Error> {
        reqwest_dav::ClientBuilder::new()
            .set_host("http://127.0.0.1".to_owned())
            .build()
    }

    #[test]
    fn reset_rejects_a_stale_client_cache_write() -> anyhow::Result<()> {
        let webdav = WebDavClient::new();
        let stale_generation = webdav.generation.load(Ordering::Acquire);
        webdav.reset();
        let current_generation = webdav.generation.load(Ordering::Acquire);

        assert!(webdav.cache_client_if_current(current_generation, Operation::List, test_client()?));
        assert!(!webdav.cache_client_if_current(stale_generation, Operation::List, test_client()?));
        assert!(!webdav.reset_if_current(stale_generation));
        assert!(webdav.cached_client(current_generation, Operation::List).is_some());
        Ok(())
    }

    #[test]
    fn response_size_limit_rejects_oversized_and_overflowing_chunks() -> anyhow::Result<()> {
        assert_eq!(checked_body_size(4, 4, 8)?, 8);
        assert!(checked_body_size(8, 1, 8).is_err());
        assert!(checked_body_size(u64::MAX, 1, u64::MAX).is_err());
        Ok(())
    }

    #[test]
    fn partial_download_guard_preserves_unowned_files_and_removes_owned_files() -> anyhow::Result<()> {
        let path = std::env::temp_dir().join(format!(
            "clash-verge-webdav-partial-{}",
            crate::utils::help::get_uid("t")
        ));
        std::fs::write(&path, b"existing")?;

        let unowned = PartialDownload::new(path.clone());
        assert!(create_partial_download_file(&path, &unowned).is_err());
        drop(unowned);
        assert_eq!(std::fs::read(&path)?, b"existing");
        std::fs::remove_file(&path)?;

        let owned = PartialDownload::new(path.clone());
        let file = create_partial_download_file(&path, &owned)?;
        drop(file);
        drop(owned);
        assert!(!path.exists());
        Ok(())
    }

    #[test]
    fn parses_webdav_numeric_utc_offset() -> anyhow::Result<()> {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
            <D:multistatus xmlns:D="DAV:">
                <D:response>
                    <D:href>/clash-verge-rev-backup/backup.zip</D:href>
                    <D:propstat>
                        <D:status>HTTP/1.1 200 OK</D:status>
                        <D:prop>
                            <D:getlastmodified>Sun, 12 Jul 2026 17:09:37 +0000</D:getlastmodified>
                            <D:resourcetype/>
                            <D:getcontentlength>42</D:getcontentlength>
                            <D:getcontenttype>application/zip</D:getcontenttype>
                        </D:prop>
                    </D:propstat>
                </D:response>
            </D:multistatus>"#;

        let files = parse_webdav_list(xml)?;
        let Some(ListEntity::File(file)) = files.first() else {
            anyhow::bail!("expected a file");
        };
        assert_eq!(file.href, "/clash-verge-rev-backup/backup.zip");
        assert_eq!(file.last_modified.timestamp(), 1_783_876_177);
        Ok(())
    }
}
