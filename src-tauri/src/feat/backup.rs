use crate::{
    config::{Config, ConfigType, IClashTemp, IProfiles, IVerge, profiles, runtime::IRuntime},
    constants::files::DNS_CONFIG,
    core::{CoreManager, backup, manager::ConfigUpdatePermit, timer::Timer},
    process::AsyncHandler,
    utils::{
        dirs::{PathBufExec as _, app_home_dir, local_backup_dir, verge_path},
        help,
    },
};
use anyhow::{Context as _, Result, anyhow, bail};
use chrono::Utc;
use clash_verge_logging::{Type, logging};
use reqwest_dav::list_cmd::ListFile;
use serde::Serialize;
use smartstring::alias::String;
use std::{
    collections::HashSet,
    fs as std_fs,
    io::{Read, Seek, Write as _},
    path::{Component, Path, PathBuf},
    string::String as StdString,
};
use tokio::{fs, io::AsyncWriteExt as _};
use zip::write::SimpleFileOptions;

const RESTORE_CONFIG_FILES: [&str; 4] = [
    crate::utils::dirs::CLASH_CONFIG,
    crate::utils::dirs::VERGE_CONFIG,
    crate::utils::dirs::PROFILE_YAML,
    DNS_CONFIG,
];

const MAX_ARCHIVE_ENTRIES: usize = 2_048;
const MAX_ARCHIVE_UNCOMPRESSED_SIZE: u64 = 512 * 1024 * 1024;
const MAX_ARCHIVE_ENTRY_SIZE: u64 = 128 * 1024 * 1024;
const MAX_CRITICAL_CONFIG_SIZE: u64 = 32 * 1024 * 1024;
const MAX_COMPRESSION_RATIO: u64 = 200;
const BACKUP_NAME_ATTEMPTS: usize = 16;

#[derive(Debug)]
struct BackupSnapshot {
    clash: Vec<u8>,
    verge: Vec<u8>,
    profiles: Vec<u8>,
    dns: Option<Vec<u8>>,
    profile_files: Vec<(StdString, Vec<u8>)>,
}

impl BackupSnapshot {
    fn capture_sync(app_home: &Path) -> Result<Self> {
        let clash = read_regular_file(&app_home.join(crate::utils::dirs::CLASH_CONFIG), true)?
            .ok_or_else(|| anyhow!("Clash configuration is missing"))?;
        let verge = read_regular_file(&app_home.join(crate::utils::dirs::VERGE_CONFIG), true)?
            .ok_or_else(|| anyhow!("Verge configuration is missing"))?;
        let profiles = read_regular_file(&app_home.join(crate::utils::dirs::PROFILE_YAML), true)?
            .ok_or_else(|| anyhow!("profiles configuration is missing"))?;
        let dns = read_regular_file(&app_home.join(DNS_CONFIG), false)?;
        if [clash.len(), verge.len(), profiles.len()]
            .into_iter()
            .chain(dns.as_ref().map(Vec::len))
            .any(|size| size as u64 > MAX_CRITICAL_CONFIG_SIZE)
        {
            bail!("backup configuration exceeds the size limit");
        }

        let profiles_dir = app_home.join("profiles");
        let metadata = std_fs::symlink_metadata(&profiles_dir)
            .with_context(|| format!("failed to inspect {}", profiles_dir.display()))?;
        if !metadata.file_type().is_dir() {
            bail!("profiles path is not a regular directory: {}", profiles_dir.display());
        }

        let mut profile_files = Vec::new();
        for entry in
            std_fs::read_dir(&profiles_dir).with_context(|| format!("failed to read {}", profiles_dir.display()))?
        {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if !file_type.is_file() {
                bail!("unsupported profile entry type: {}", entry.path().display());
            }
            let file_name = entry
                .file_name()
                .into_string()
                .map_err(|_| anyhow!("profile filename is not valid UTF-8"))?;
            profiles::validate_profile_file_name(&file_name)?;
            let contents = read_regular_file(&entry.path(), true)?
                .ok_or_else(|| anyhow!("profile file disappeared: {}", entry.path().display()))?;
            profile_files.push((file_name, contents));
        }
        profile_files.sort_by(|left, right| left.0.cmp(&right.0));

        let mut total_size = clash
            .len()
            .saturating_add(verge.len())
            .saturating_add(profiles.len())
            .saturating_add(dns.as_ref().map_or(0, Vec::len));
        for (_, contents) in &profile_files {
            if contents.len() as u64 > MAX_ARCHIVE_ENTRY_SIZE {
                bail!("profile file exceeds the backup size limit");
            }
            total_size = total_size.saturating_add(contents.len());
        }
        if total_size as u64 > MAX_ARCHIVE_UNCOMPRESSED_SIZE {
            bail!("backup contents exceed the total size limit");
        }

        Ok(Self {
            clash,
            verge,
            profiles,
            dns,
            profile_files,
        })
    }
}

#[derive(Debug)]
struct StagingDirectory(PathBuf);

impl StagingDirectory {
    fn create(parent: &Path) -> Result<Self> {
        for _ in 0..BACKUP_NAME_ATTEMPTS {
            let path = parent.join(format!(".restore-staging-{}", help::get_uid("r")));
            match std_fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(err) => return Err(err).with_context(|| format!("failed to create {}", path.display())),
            }
        }
        bail!("failed to allocate a unique restore staging directory")
    }
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if let Err(err) = std_fs::remove_dir_all(&self.0)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            logging!(warn, Type::Backup, "Failed to remove restore staging directory: {err}");
        }
    }
}

#[derive(Debug)]
struct FileSnapshot {
    path: PathBuf,
    contents: Option<Vec<u8>>,
}

#[derive(Debug)]
enum DirectoryEntrySnapshot {
    Directory(PathBuf),
    File(PathBuf, Vec<u8>),
}

#[derive(Debug)]
struct RestoreDiskSnapshot {
    files: Vec<FileSnapshot>,
    profiles_dir: PathBuf,
    profiles_dir_existed: bool,
    profile_entries: Vec<DirectoryEntrySnapshot>,
}

impl RestoreDiskSnapshot {
    async fn capture(app_home: PathBuf) -> Result<Self> {
        AsyncHandler::spawn_blocking(move || Self::capture_sync(&app_home)).await?
    }

    fn capture_sync(app_home: &Path) -> Result<Self> {
        let files = RESTORE_CONFIG_FILES
            .iter()
            .map(|name| {
                let path = app_home.join(name);
                let contents = match std_fs::read(&path) {
                    Ok(contents) => Some(contents),
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
                    Err(err) => return Err(err).with_context(|| format!("failed to snapshot {}", path.display())),
                };
                Ok(FileSnapshot { path, contents })
            })
            .collect::<Result<Vec<_>>>()?;

        let profiles_dir = app_home.join("profiles");
        let profiles_dir_existed = match std_fs::symlink_metadata(&profiles_dir) {
            Ok(metadata) if metadata.file_type().is_dir() => true,
            Ok(_) => bail!("profiles path is not a regular directory: {}", profiles_dir.display()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
            Err(err) => return Err(err).with_context(|| format!("failed to inspect {}", profiles_dir.display())),
        };
        let mut profile_entries = Vec::new();
        if profiles_dir_existed {
            capture_directory(&profiles_dir, &profiles_dir, &mut profile_entries)?;
        }

        Ok(Self {
            files,
            profiles_dir,
            profiles_dir_existed,
            profile_entries,
        })
    }

    async fn restore(self) -> Vec<StdString> {
        match AsyncHandler::spawn_blocking(move || self.restore_sync()).await {
            Ok(errors) => errors,
            Err(err) => vec![format!("failed to join disk rollback task: {err}")],
        }
    }

    fn restore_sync(self) -> Vec<StdString> {
        let mut errors = Vec::new();

        for snapshot in self.files {
            if let Err(err) = restore_file(snapshot) {
                errors.push(err.to_string());
            }
        }

        if let Err(err) = remove_path_if_exists(&self.profiles_dir) {
            errors.push(format!(
                "failed to clear restored profiles directory {}: {err}",
                self.profiles_dir.display()
            ));
            return errors;
        }

        if self.profiles_dir_existed {
            if let Err(err) = std_fs::create_dir_all(&self.profiles_dir) {
                errors.push(format!(
                    "failed to recreate profiles directory {}: {err}",
                    self.profiles_dir.display()
                ));
                return errors;
            }

            for entry in self.profile_entries {
                let result = match entry {
                    DirectoryEntrySnapshot::Directory(relative) => {
                        std_fs::create_dir_all(self.profiles_dir.join(relative))
                    }
                    DirectoryEntrySnapshot::File(relative, contents) => {
                        let path = self.profiles_dir.join(relative);
                        if let Some(parent) = path.parent()
                            && let Err(err) = std_fs::create_dir_all(parent)
                        {
                            errors.push(format!("failed to recreate {}: {err}", parent.display()));
                            continue;
                        }
                        std_fs::write(&path, contents)
                    }
                };
                if let Err(err) = result {
                    errors.push(format!("failed to restore profile entry: {err}"));
                }
            }
        }

        errors
    }
}

fn read_regular_file(path: &Path, required: bool) -> Result<Option<Vec<u8>>> {
    let metadata = match std_fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound && !required => return Ok(None),
        Err(err) => return Err(err).with_context(|| format!("failed to inspect {}", path.display())),
    };
    if !metadata.file_type().is_file() {
        bail!("backup source is not a regular file: {}", path.display());
    }
    if metadata.len() > MAX_ARCHIVE_ENTRY_SIZE {
        bail!("backup source exceeds the per-file size limit: {}", path.display());
    }
    std_fs::read(path)
        .map(Some)
        .with_context(|| format!("failed to read {}", path.display()))
}

#[derive(Clone)]
struct RestoreConfigSnapshot {
    clash: IClashTemp,
    verge: IVerge,
    profiles: IProfiles,
    runtime: IRuntime,
}

impl RestoreConfigSnapshot {
    async fn capture() -> Self {
        Self {
            clash: (*Config::clash().await.data_arc()).clone(),
            verge: (*Config::verge().await.data_arc()).clone(),
            profiles: (*Config::profiles().await.data_arc()).clone(),
            runtime: (*Config::runtime().await.data_arc()).clone(),
        }
    }

    async fn restore(&self) {
        let clash = Config::clash().await;
        clash.edit_draft(|draft| *draft = self.clash.clone());
        clash.apply();

        let verge = Config::verge().await;
        verge.edit_draft(|draft| *draft = self.verge.clone());
        verge.apply();

        let profiles = Config::profiles().await;
        profiles.edit_draft(|draft| *draft = self.profiles.clone());
        profiles.apply();

        let runtime = Config::runtime().await;
        runtime.edit_draft(|draft| *draft = self.runtime.clone());
        runtime.apply();
    }
}

fn capture_directory(root: &Path, current: &Path, entries: &mut Vec<DirectoryEntrySnapshot>) -> Result<()> {
    for entry in std_fs::read_dir(current).with_context(|| format!("failed to read {}", current.display()))? {
        let entry = entry?;
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .with_context(|| format!("failed to relativize {}", path.display()))?
            .to_path_buf();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            entries.push(DirectoryEntrySnapshot::Directory(relative));
            capture_directory(root, &path, entries)?;
        } else if file_type.is_file() {
            entries.push(DirectoryEntrySnapshot::File(relative, std_fs::read(&path)?));
        } else {
            bail!("unsupported profile entry type: {}", path.display());
        }
    }
    Ok(())
}

fn remove_path_if_exists(path: &Path) -> std::io::Result<()> {
    match std_fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => std_fs::remove_dir_all(path),
        Ok(_) => std_fs::remove_file(path),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn restore_file(snapshot: FileSnapshot) -> Result<()> {
    remove_path_if_exists(&snapshot.path).with_context(|| format!("failed to clear {}", snapshot.path.display()))?;
    if let Some(contents) = snapshot.contents {
        if let Some(parent) = snapshot.path.parent() {
            std_fs::create_dir_all(parent)?;
        }
        std_fs::write(&snapshot.path, contents)
            .with_context(|| format!("failed to restore {}", snapshot.path.display()))?;
    }
    Ok(())
}

fn validate_restore_archive<R: Read + Seek>(zip: &mut zip::ZipArchive<R>) -> Result<()> {
    if zip.len() > MAX_ARCHIVE_ENTRIES {
        bail!("backup contains too many entries");
    }

    let mut has_clash = false;
    let mut has_verge = false;
    let mut has_profiles = false;
    let mut has_profiles_root = false;
    let mut seen_paths = HashSet::with_capacity(zip.len());
    let mut total_uncompressed_size = 0_u64;

    for index in 0..zip.len() {
        let entry = zip.by_index(index)?;
        let path = entry
            .enclosed_name()
            .ok_or_else(|| anyhow!("backup contains an unsafe path: {}", entry.name()))?
            .to_path_buf();
        if !seen_paths.insert(path.clone()) {
            bail!("backup contains a duplicate entry: {}", path.display());
        }
        if entry.is_symlink() {
            bail!("backup contains a symbolic link: {}", path.display());
        }
        if let Some(mode) = entry.unix_mode() {
            let file_kind = mode & 0o170_000;
            let mode_matches_entry = file_kind == 0
                || (file_kind == 0o100_000 && entry.is_file())
                || (file_kind == 0o040_000 && entry.is_dir());
            if !mode_matches_entry {
                bail!("backup contains an unsupported entry type: {}", path.display());
            }
        }
        if !entry.is_file() && !entry.is_dir() {
            bail!("backup contains an unsupported entry type: {}", path.display());
        }
        if path == Path::new("profiles") && !entry.is_dir() {
            bail!("backup profiles root must be a directory");
        }
        has_profiles_root |= path == Path::new("profiles") && entry.is_dir();

        let is_profiles_entry = path == Path::new("profiles") || path.starts_with("profiles");
        let is_known_config = RESTORE_CONFIG_FILES.iter().any(|name| path == Path::new(name));
        if !is_profiles_entry && !is_known_config {
            bail!("backup contains an unsupported entry: {}", path.display());
        }
        if is_known_config && entry.is_dir() {
            bail!("backup config entry is a directory: {}", path.display());
        }

        if path.starts_with("profiles") && path != Path::new("profiles") {
            if entry.is_dir() {
                bail!("backup contains a nested profile directory: {}", path.display());
            }
            let mut components = path.components();
            let valid_profile_path = matches!(components.next(), Some(Component::Normal(root)) if root == "profiles")
                && matches!(components.next(), Some(Component::Normal(_)))
                && components.next().is_none();
            if !valid_profile_path {
                bail!("backup contains an unsafe profile path: {}", path.display());
            }
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| anyhow!("profile filename is not valid UTF-8"))?;
            profiles::validate_profile_file_name(file_name)?;
        }

        if entry.is_file() {
            let size = entry.size();
            if size > MAX_ARCHIVE_ENTRY_SIZE {
                bail!("backup entry exceeds the per-file size limit: {}", path.display());
            }
            if is_known_config && size > MAX_CRITICAL_CONFIG_SIZE {
                bail!("backup configuration exceeds the size limit: {}", path.display());
            }
            let compressed_size = entry.compressed_size();
            if size > 0 && (compressed_size == 0 || size > compressed_size.saturating_mul(MAX_COMPRESSION_RATIO)) {
                bail!("backup entry has an unsafe compression ratio: {}", path.display());
            }
            total_uncompressed_size = total_uncompressed_size
                .checked_add(size)
                .ok_or_else(|| anyhow!("backup uncompressed size overflow"))?;
            if total_uncompressed_size > MAX_ARCHIVE_UNCOMPRESSED_SIZE {
                bail!("backup exceeds the total uncompressed size limit");
            }

            has_clash |= path == Path::new(crate::utils::dirs::CLASH_CONFIG);
            has_verge |= path == Path::new(crate::utils::dirs::VERGE_CONFIG);
            has_profiles |= path == Path::new(crate::utils::dirs::PROFILE_YAML);
        }
    }

    if !has_clash || !has_verge || !has_profiles || !has_profiles_root {
        bail!("backup is missing one or more required configuration files");
    }
    Ok(())
}

fn extract_restore_archive<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, staging: &Path) -> Result<()> {
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        let path = entry
            .enclosed_name()
            .ok_or_else(|| anyhow!("backup contains an unsafe path: {}", entry.name()))?;
        let destination = staging.join(&path);

        if entry.is_dir() {
            std_fs::create_dir_all(&destination)
                .with_context(|| format!("failed to create {}", destination.display()))?;
            continue;
        }

        if let Some(parent) = destination.parent() {
            std_fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let mut output = std_fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .with_context(|| format!("failed to create {}", destination.display()))?;
        let expected_size = entry.size();
        let mut limited = entry.by_ref().take(expected_size.saturating_add(1));
        let written = std::io::copy(&mut limited, &mut output)
            .with_context(|| format!("failed to extract {}", destination.display()))?;
        if written != expected_size || written > MAX_ARCHIVE_ENTRY_SIZE {
            bail!("backup entry size changed while extracting: {}", path.display());
        }
        output.flush()?;
    }
    Ok(())
}

fn validate_staged_restore(staging: &Path) -> Result<()> {
    let clash = std_fs::read(staging.join(crate::utils::dirs::CLASH_CONFIG))?;
    serde_yaml_ng::from_slice::<serde_yaml_ng::Mapping>(&clash)
        .context("failed to parse staged Clash configuration")?;

    let verge = std_fs::read(staging.join(crate::utils::dirs::VERGE_CONFIG))?;
    serde_yaml_ng::from_slice::<IVerge>(&verge).context("failed to parse staged Verge configuration")?;

    let profile_metadata = std_fs::read(staging.join(crate::utils::dirs::PROFILE_YAML))?;
    let profiles_config = serde_yaml_ng::from_slice::<IProfiles>(&profile_metadata)
        .context("failed to parse staged profiles configuration")?;
    if let Some(items) = profiles_config.items.as_ref() {
        for item in items {
            let Some(file_name) = item.file.as_deref() else {
                continue;
            };
            profiles::validate_profile_file_name(file_name)?;
            let profile_path = staging.join("profiles").join(file_name);
            let metadata = std_fs::symlink_metadata(&profile_path)
                .with_context(|| format!("profile metadata references a missing file: {}", profile_path.display()))?;
            if !metadata.file_type().is_file() {
                bail!(
                    "profile metadata references a non-regular file: {}",
                    profile_path.display()
                );
            }
        }
    }

    if let Some(dns) = read_regular_file(&staging.join(DNS_CONFIG), false)? {
        serde_yaml_ng::from_slice::<serde_yaml_ng::Value>(&dns).context("failed to parse staged DNS configuration")?;
    }
    Ok(())
}

fn prepare_restore_archive(backup_path: &Path, app_home: &Path) -> Result<StagingDirectory> {
    let file =
        std_fs::File::open(backup_path).with_context(|| format!("failed to open backup {}", backup_path.display()))?;
    if !file.metadata()?.file_type().is_file() {
        bail!("backup is not a regular file: {}", backup_path.display());
    }
    let mut zip = zip::ZipArchive::new(file)?;
    validate_restore_archive(&mut zip)?;

    let staging = StagingDirectory::create(app_home)?;
    extract_restore_archive(&mut zip, &staging.0)?;
    validate_staged_restore(&staging.0)?;
    Ok(staging)
}

fn replace_staged_file(staging: &Path, app_home: &Path, name: &str, required: bool) -> Result<()> {
    let source = staging.join(name);
    let target = app_home.join(name);
    match std_fs::symlink_metadata(&source) {
        Ok(metadata) if metadata.file_type().is_file() => {
            remove_path_if_exists(&target).with_context(|| format!("failed to clear {}", target.display()))?;
            std_fs::rename(&source, &target)
                .with_context(|| format!("failed to install restored file {}", target.display()))?;
        }
        Ok(_) => bail!("staged restore entry is not a regular file: {}", source.display()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound && !required => {
            remove_path_if_exists(&target).with_context(|| format!("failed to remove stale {}", target.display()))?;
        }
        Err(err) => return Err(err).with_context(|| format!("failed to inspect {}", source.display())),
    }
    Ok(())
}

fn commit_staged_restore(staging: &Path, app_home: &Path) -> Result<()> {
    let staged_profiles = staging.join("profiles");
    let profile_metadata = std_fs::symlink_metadata(&staged_profiles)
        .with_context(|| format!("failed to inspect {}", staged_profiles.display()))?;
    if !profile_metadata.file_type().is_dir() {
        bail!("staged profiles root is not a regular directory");
    }

    let target_profiles = app_home.join("profiles");
    remove_path_if_exists(&target_profiles)
        .with_context(|| format!("failed to clear profiles directory {}", target_profiles.display()))?;
    std_fs::rename(&staged_profiles, &target_profiles)
        .with_context(|| format!("failed to install profiles directory {}", target_profiles.display()))?;

    for name in [
        crate::utils::dirs::CLASH_CONFIG,
        crate::utils::dirs::VERGE_CONFIG,
        crate::utils::dirs::PROFILE_YAML,
    ] {
        replace_staged_file(staging, app_home, name, true)?;
    }
    replace_staged_file(staging, app_home, DNS_CONFIG, false)?;
    Ok(())
}

fn redact_webdav_credentials(verge: &[u8]) -> Result<Vec<u8>> {
    let mut config = serde_yaml_ng::from_slice::<serde_yaml_ng::Value>(verge)?;
    let mapping = config
        .as_mapping_mut()
        .ok_or_else(|| anyhow!("Verge configuration root must be a mapping"))?;
    for key in ["webdav_username", "webdav_password", "webdav_url"] {
        mapping.remove(serde_yaml_ng::Value::String(key.to_owned()));
    }
    Ok(serde_yaml_ng::to_string(&config)?.into_bytes())
}

fn write_backup_archive(snapshot: BackupSnapshot) -> Result<(String, PathBuf)> {
    if snapshot.profile_files.len().saturating_add(5) > MAX_ARCHIVE_ENTRIES {
        bail!("too many profile files to create a backup");
    }

    let verge = redact_webdav_credentials(&snapshot.verge)?;
    if verge.len() as u64 > MAX_CRITICAL_CONFIG_SIZE {
        bail!("redacted Verge configuration exceeds the size limit");
    }
    let mut selected = None;
    for _ in 0..BACKUP_NAME_ATTEMPTS {
        let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S-%6f");
        let file_name: String = format!(
            "{}-backup-{}-{}.zip",
            std::env::consts::OS,
            timestamp,
            help::get_uid("b")
        )
        .into();
        let path = std::env::temp_dir().join(file_name.as_str());
        match std_fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => {
                selected = Some((file_name, path, file));
                break;
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err).with_context(|| format!("failed to create {}", path.display())),
        }
    }
    let (file_name, path, file) = selected.ok_or_else(|| anyhow!("failed to allocate a unique backup filename"))?;

    let write_result = (|| -> Result<()> {
        let mut zip = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.add_directory("profiles/", options)?;
        for (profile_name, contents) in snapshot.profile_files {
            zip.start_file(format!("profiles/{profile_name}"), options)?;
            zip.write_all(&contents)?;
        }
        for (name, contents) in [
            (crate::utils::dirs::CLASH_CONFIG, snapshot.clash),
            (crate::utils::dirs::VERGE_CONFIG, verge),
            (crate::utils::dirs::PROFILE_YAML, snapshot.profiles),
        ] {
            zip.start_file(name, options)?;
            zip.write_all(&contents)?;
        }
        if let Some(dns) = snapshot.dns {
            zip.start_file(DNS_CONFIG, options)?;
            zip.write_all(&dns)?;
        }
        zip.finish()?;
        Ok(())
    })();

    if let Err(err) = write_result {
        let _ = std_fs::remove_file(&path);
        return Err(err);
    }
    Ok((file_name, path))
}

async fn create_backup_archive() -> Result<(String, PathBuf)> {
    let snapshot = {
        let _profile_transaction = profiles::lock_profile_transaction().await;
        let _config_permit = CoreManager::global()
            .try_acquire_config_update()
            .ok_or_else(|| anyhow!("configuration update is already running"))?;
        IProfiles::try_new()
            .await
            .context("failed to validate profiles before backup")?;
        let app_home = app_home_dir()?;
        AsyncHandler::spawn_blocking(move || BackupSnapshot::capture_sync(&app_home)).await??
    };

    AsyncHandler::spawn_blocking(move || write_backup_archive(snapshot)).await?
}

#[derive(Debug, Serialize)]
pub struct LocalBackupFile {
    pub filename: String,
    pub path: String,
    pub last_modified: String,
    pub content_length: u64,
}

/// Load restored verge.yaml from disk, merge back WebDAV creds, save, and sync memory.
/// Also reload other restored configs so restarts won't overwrite them.
async fn finalize_restored_verge_config(
    webdav_url: Option<String>,
    webdav_username: Option<String>,
    webdav_password: Option<String>,
    config_permit: &ConfigUpdatePermit<'_>,
) -> Result<()> {
    // Do NOT silently fallback to defaults; a broken/missing verge.yaml means restore failed.
    // Propagate the error so the UI/user can react accordingly.
    let mut restored = help::read_yaml::<IVerge>(&verge_path()?).await?;
    restored.webdav_url = webdav_url;
    restored.webdav_username = webdav_username;
    restored.webdav_password = webdav_password;
    restored.save_file().await?;

    help::read_mapping(&crate::utils::dirs::clash_path()?)
        .await
        .context("failed to parse restored Clash config")?;
    let restored_clash = IClashTemp::new().await;
    let clash_draft = Config::clash().await;
    clash_draft.edit_draft(|d| {
        *d = restored_clash.clone();
    });

    let restored_profiles = IProfiles::try_new()
        .await
        .context("failed to strictly load restored profiles config")?;
    let profiles_draft = Config::profiles().await;
    profiles_draft.edit_draft(|d| {
        *d = restored_profiles.clone();
    });

    let verge_draft = Config::verge().await;
    verge_draft.edit_draft(|d| {
        *d = restored.clone();
    });

    // Ensure side-effects (flags, tray, sysproxy, hotkeys, auto-backup refresh, etc.) run.
    // Use not_save_file = true to avoid extra I/O (we already persisted the restored file).
    super::patch_verge_with_permit(&restored, true, config_permit)
        .await
        .map_err(|err| anyhow!("Failed to apply restored config: {err:#}"))?;
    clash_draft.apply();
    profiles_draft.apply();
    Ok(())
}

async fn restore_zip(backup_path: PathBuf) -> Result<()> {
    // Keep the commit/rollback sequence alive if the invoking IPC future is
    // dropped while extraction or finalization is in progress.
    tauri::async_runtime::spawn(async move { restore_zip_transaction(backup_path).await })
        .await
        .context("backup restore task failed")?
}

async fn restore_zip_transaction(backup_path: PathBuf) -> Result<()> {
    let app_home = app_home_dir()?;
    let staging_home = app_home.clone();
    let staging = AsyncHandler::spawn_blocking(move || prepare_restore_archive(&backup_path, &staging_home)).await??;

    let _profile_transaction = profiles::lock_profile_transaction().await;
    let config_permit = CoreManager::global()
        .try_acquire_config_update()
        .ok_or_else(|| anyhow!("configuration update is already running"))?;
    let disk_snapshot = RestoreDiskSnapshot::capture(app_home.clone()).await?;
    let config_snapshot = RestoreConfigSnapshot::capture().await;
    let webdav_url = config_snapshot.verge.webdav_url.clone();
    let webdav_username = config_snapshot.verge.webdav_username.clone();
    let webdav_password = config_snapshot.verge.webdav_password.clone();

    let restore_result = async {
        let staging_path = staging.0.clone();
        AsyncHandler::spawn_blocking(move || -> Result<()> { commit_staged_restore(&staging_path, &app_home) })
            .await??;
        finalize_restored_verge_config(webdav_url, webdav_username, webdav_password, &config_permit).await
    }
    .await;

    let Err(restore_error) = restore_result else {
        if let Err(err) = Timer::global().refresh().await {
            logging!(
                warn,
                Type::Backup,
                "Backup restore committed, but profile timer refresh failed: {err:#}"
            );
        }
        return Ok(());
    };

    let mut rollback_errors = disk_snapshot.restore().await;
    config_snapshot.restore().await;

    if let Err(err) = super::patch_verge_with_permit(&config_snapshot.verge, true, &config_permit).await {
        rollback_errors.push(format!("failed to restore previous Verge effects: {err:#}"));
    }

    // patch_verge may regenerate runtime state. Restore the exact committed
    // snapshots before writing and restarting the previous runtime.
    config_snapshot.restore().await;
    match Config::generate_file(ConfigType::Run).await {
        Ok(_) => {
            if let Err(err) = CoreManager::global().restart_core_with_permit(&config_permit).await {
                rollback_errors.push(format!("failed to restart previous core: {err:#}"));
            }
        }
        Err(err) => rollback_errors.push(format!("failed to restore previous runtime file: {err:#}")),
    }

    if let Err(err) = Timer::global().refresh().await {
        rollback_errors.push(format!("failed to restore profile timer state: {err:#}"));
    }

    if rollback_errors.is_empty() {
        Err(anyhow!("backup restore failed and was rolled back: {restore_error:#}"))
    } else {
        Err(anyhow!(
            "backup restore failed: {restore_error:#}; rollback errors: {}",
            rollback_errors.join("; ")
        ))
    }
}

/// Create a backup and upload to WebDAV
pub async fn create_backup_and_upload_webdav() -> Result<()> {
    let (file_name, temp_file_path) = create_backup_archive().await.map_err(|err| {
        logging!(error, Type::Backup, "Failed to create backup: {err:#?}");
        err
    })?;

    let upload_result = backup::WebDavClient::global()
        .upload(temp_file_path.clone(), file_name)
        .await;
    if let Err(err) = &upload_result {
        logging!(error, Type::Backup, "Failed to upload to WebDAV: {err:#?}");
        backup::WebDavClient::global().reset();
    }

    if let Err(err) = temp_file_path.remove_if_exists().await {
        logging!(warn, Type::Backup, "Failed to remove temp file: {err:#?}");
    }
    upload_result
}

/// List WebDAV backups
pub async fn list_wevdav_backup() -> Result<Vec<ListFile>> {
    backup::WebDavClient::global().list().await.map_err(|err| {
        logging!(error, Type::Backup, "Failed to list WebDAV backup files: {err:#?}");
        err
    })
}

/// Delete WebDAV backup
pub async fn delete_webdav_backup(filename: String) -> Result<()> {
    validate_backup_filename(filename.as_str())?;
    backup::WebDavClient::global().delete(filename).await.map_err(|err| {
        logging!(error, Type::Backup, "Failed to delete WebDAV backup file: {err:#?}");
        err
    })
}

/// Restore WebDAV backup
pub async fn restore_webdav_backup(filename: String) -> Result<()> {
    validate_backup_filename(filename.as_str())?;
    let backup_storage_path = std::env::temp_dir().join(format!("clash-verge-restore-{}.zip", help::get_uid("b")));
    backup::WebDavClient::global()
        .download(filename, backup_storage_path.clone())
        .await
        .map_err(|err| {
            logging!(error, Type::Backup, "Failed to download WebDAV backup file: {err:#?}");
            err
        })?;

    let res = restore_zip(backup_storage_path.clone()).await;
    // Finally remove the temp file (attempt cleanup even if finalize fails)
    let _ = backup_storage_path.remove_if_exists().await;
    res
}

/// Create a backup and save to local storage
pub async fn create_local_backup() -> Result<()> {
    create_local_backup_with_namer(|name| name.to_string().into())
        .await
        .map(|_| ())
}

pub async fn create_local_backup_with_namer<F>(namer: F) -> Result<String>
where
    F: FnOnce(&str) -> String,
{
    let (file_name, temp_file_path) = create_backup_archive().await.map_err(|err| {
        logging!(error, Type::Backup, "Failed to create local backup: {err:#?}");
        err
    })?;

    let final_name = namer(file_name.as_str());
    let target_path = safe_local_backup_path(final_name.as_str())?;

    if let Err(err) = move_file(temp_file_path.clone(), target_path.clone()).await {
        logging!(error, Type::Backup, "Failed to move local backup file: {err:#?}");
        // 清理临时文件
        if let Err(clean_err) = temp_file_path.remove_if_exists().await {
            logging!(
                warn,
                Type::Backup,
                "Failed to remove temp backup file after move error: {clean_err:#?}"
            );
        }
        return Err(err);
    }

    Ok(final_name)
}

/// Import an existing backup file into the local backup directory
pub async fn import_local_backup(source: String) -> Result<String> {
    let source_path = PathBuf::from(source.as_str());
    match fs::symlink_metadata(&source_path).await {
        Ok(metadata) if metadata.file_type().is_file() => {}
        Ok(_) => return Err(anyhow!("Backup path is not a regular file: {source}")),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(anyhow!("Backup file not found: {source}"));
        }
        Err(err) => return Err(err.into()),
    }

    let ext = source_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_default();
    if ext != "zip" {
        return Err(anyhow!("Only .zip backup files are supported"));
    }

    let file_name = source_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("Invalid backup file name"))?;

    let target_path = safe_local_backup_path(file_name)?;

    if target_path == source_path {
        // Already located in the backup directory
        return Ok(file_name.to_string().into());
    }

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    copy_regular_file_exclusive(&source_path, &target_path)
        .await
        .map_err(|err| anyhow!("Failed to import backup file: {err:#}"))?;

    Ok(file_name.to_string().into())
}

async fn move_file(from: PathBuf, to: PathBuf) -> Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).await?;
    }

    copy_regular_file_exclusive(&from, &to).await?;
    if let Err(err) = fs::remove_file(&from).await {
        logging!(
            warn,
            Type::Backup,
            "Backup was committed, but its temporary file could not be removed: {err:#}"
        );
    }
    Ok(())
}

async fn copy_regular_file_exclusive(from: &Path, to: &Path) -> Result<()> {
    let source_metadata = fs::symlink_metadata(from)
        .await
        .with_context(|| format!("failed to inspect {}", from.display()))?;
    if !source_metadata.file_type().is_file() {
        bail!("backup source is not a regular file: {}", from.display());
    }

    let mut source = fs::File::open(from)
        .await
        .with_context(|| format!("failed to open {}", from.display()))?;
    if !source.metadata().await?.file_type().is_file() {
        bail!("backup source is not a regular file: {}", from.display());
    }

    let mut destination = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)
        .await
        .with_context(|| format!("failed to exclusively create {}", to.display()))?;
    let copy_result = async {
        tokio::io::copy(&mut source, &mut destination).await?;
        destination.flush().await?;
        Ok::<(), std::io::Error>(())
    }
    .await;
    drop(destination);

    if let Err(err) = copy_result {
        let _ = fs::remove_file(to).await;
        return Err(err).with_context(|| format!("failed to copy backup to {}", to.display()));
    }
    Ok(())
}

struct StagedExportFile {
    file: Option<fs::File>,
    path: PathBuf,
    committed: bool,
}

impl StagedExportFile {
    async fn create(destination: &Path) -> Result<Self> {
        let file_name = destination
            .file_name()
            .ok_or_else(|| anyhow!("Invalid export destination: {}", destination.display()))?;
        let parent = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).await?;

        for _ in 0..BACKUP_NAME_ATTEMPTS {
            let path = parent.join(format!(
                ".{}.export-{}-{}",
                file_name.to_string_lossy(),
                std::process::id(),
                help::get_uid("e")
            ));
            match fs::OpenOptions::new().write(true).create_new(true).open(&path).await {
                Ok(file) => {
                    return Ok(Self {
                        file: Some(file),
                        path,
                        committed: false,
                    });
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(err) => return Err(err).context("failed to create export staging file"),
            }
        }

        bail!("failed to allocate a unique export staging file")
    }

    fn file_mut(&mut self) -> Result<&mut fs::File> {
        self.file.as_mut().context("export staging file is closed")
    }

    fn close(&mut self) {
        drop(self.file.take());
    }

    const fn disarm(&mut self) {
        self.committed = true;
    }
}

impl Drop for StagedExportFile {
    fn drop(&mut self) {
        self.close();
        if !self.committed {
            let _ = std_fs::remove_file(&self.path);
        }
    }
}

async fn destination_resolves_to_source(source: &Path, destination: &Path) -> Result<bool> {
    let source_path = fs::canonicalize(source)
        .await
        .with_context(|| format!("failed to inspect {}", source.display()))?;
    let destination_path = match fs::canonicalize(destination).await {
        Ok(path) => path,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err).with_context(|| format!("failed to inspect {}", destination.display())),
    };
    Ok(source_path == destination_path)
}

async fn export_regular_file_atomically(source: &Path, destination: &Path) -> Result<()> {
    if destination_resolves_to_source(source, destination).await? {
        bail!("export destination refers to the source backup file");
    }

    let source_metadata = fs::symlink_metadata(source)
        .await
        .with_context(|| format!("failed to inspect {}", source.display()))?;
    if !source_metadata.file_type().is_file() {
        bail!("backup source is not a regular file: {}", source.display());
    }
    let mut source_file = fs::File::open(source)
        .await
        .with_context(|| format!("failed to open {}", source.display()))?;
    if !source_file.metadata().await?.file_type().is_file() {
        bail!("backup source is not a regular file: {}", source.display());
    }

    let mut staged = StagedExportFile::create(destination).await?;
    tokio::io::copy(&mut source_file, staged.file_mut()?)
        .await
        .with_context(|| format!("failed to stage export to {}", destination.display()))?;
    staged.file_mut()?.flush().await?;
    staged.file_mut()?.sync_all().await?;
    staged.close();

    help::replace_file_atomically(&staged.path, destination)
        .await
        .with_context(|| format!("failed to replace export destination {}", destination.display()))?;
    staged.disarm();
    Ok(())
}

/// List local backups
pub async fn list_local_backup() -> Result<Vec<LocalBackupFile>> {
    let backup_dir = local_backup_dir()?;
    if !backup_dir.exists() {
        return Ok(vec![]);
    }

    let mut backups = Vec::new();
    let mut dir = fs::read_dir(&backup_dir).await?;
    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();
        if !entry.file_type().await?.is_file() {
            continue;
        }

        let file_name = match path.file_name().and_then(|name| name.to_str()) {
            Some(name) => name,
            None => continue,
        };
        if validate_backup_filename(file_name).is_err() {
            continue;
        }
        let metadata = entry.metadata().await?;
        let last_modified = metadata
            .modified()
            .map(|time| chrono::DateTime::<Utc>::from(time).to_rfc3339())
            .unwrap_or_default();
        backups.push(LocalBackupFile {
            filename: file_name.into(),
            path: path.to_string_lossy().into(),
            last_modified: last_modified.into(),
            content_length: metadata.len(),
        });
    }

    backups.sort_by(|a, b| b.filename.cmp(&a.filename));
    Ok(backups)
}

fn validate_backup_filename(filename: &str) -> Result<&std::ffi::OsStr> {
    if filename
        .chars()
        .any(|character| character == '/' || character == '\\' || character == '\0')
    {
        bail!("backup filename must be a single normal path component");
    }
    let path = Path::new(filename);
    let mut components = path.components();
    let name = match (components.next(), components.next()) {
        (Some(Component::Normal(name)), None) if !name.is_empty() => name,
        _ => bail!("backup filename must be a single normal path component"),
    };
    let is_zip = Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"));
    if !is_zip {
        bail!("backup filename must use the .zip extension");
    }
    Ok(name)
}

fn safe_local_backup_path(filename: &str) -> Result<PathBuf> {
    let name = validate_backup_filename(filename)?;
    Ok(local_backup_dir()?.join(name))
}

async fn require_regular_local_backup(path: &Path, filename: &str) -> Result<()> {
    match fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.file_type().is_file() => Ok(()),
        Ok(_) => bail!("Local backup is not a regular file: {filename}"),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => bail!("Backup file not found: {filename}"),
        Err(err) => Err(err.into()),
    }
}

/// Delete local backup
pub async fn delete_local_backup(filename: String) -> Result<()> {
    let target_path = safe_local_backup_path(filename.as_str())?;
    match fs::symlink_metadata(&target_path).await {
        Ok(metadata) if metadata.file_type().is_file() => fs::remove_file(target_path).await?,
        Ok(_) => bail!("Local backup is not a regular file: {filename}"),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            logging!(warn, Type::Backup, "Local backup file not found: {}", filename);
        }
        Err(err) => return Err(err.into()),
    }
    Ok(())
}

/// Restore local backup
pub async fn restore_local_backup(filename: String) -> Result<()> {
    let target_path = safe_local_backup_path(filename.as_str())?;
    require_regular_local_backup(&target_path, filename.as_str()).await?;

    restore_zip(target_path).await
}

/// Export local backup file to user selected destination
pub async fn export_local_backup(filename: String, destination: String) -> Result<()> {
    let source_path = safe_local_backup_path(filename.as_str())?;
    require_regular_local_backup(&source_path, filename.as_str()).await?;

    let dest_path = PathBuf::from(destination.as_str());
    export_regular_file_atomically(&source_path, &dest_path)
        .await
        .map_err(|err| anyhow!("Failed to export backup file: {err:#}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use zip::write::SimpleFileOptions;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Result<Self> {
            let path = std::env::temp_dir().join(format!("clash-verge-backup-test-{}", help::get_uid("t")));
            std_fs::create_dir_all(&path)?;
            Ok(Self(path))
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std_fs::remove_dir_all(&self.0);
        }
    }

    fn required_archive_writer() -> Result<zip::ZipWriter<Cursor<Vec<u8>>>> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        writer.add_directory("profiles/", options)?;
        for name in [
            crate::utils::dirs::CLASH_CONFIG,
            crate::utils::dirs::VERGE_CONFIG,
            crate::utils::dirs::PROFILE_YAML,
        ] {
            writer.start_file(name, options)?;
            writer.write_all(b"{}")?;
        }
        Ok(writer)
    }

    #[test]
    fn restore_archive_rejects_entries_outside_whitelist() -> Result<()> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        for name in [
            crate::utils::dirs::CLASH_CONFIG,
            crate::utils::dirs::VERGE_CONFIG,
            crate::utils::dirs::PROFILE_YAML,
        ] {
            writer.start_file(name, options)?;
            writer.write_all(b"{}")?;
        }
        writer.start_file("unexpected.txt", options)?;
        writer.write_all(b"not allowed")?;

        let mut cursor = writer.finish()?;
        cursor.set_position(0);
        let mut archive = zip::ZipArchive::new(cursor)?;
        let Err(error) = validate_restore_archive(&mut archive) else {
            bail!("unexpected file must be rejected");
        };
        assert!(error.to_string().contains("unsupported entry"));
        Ok(())
    }

    #[test]
    fn restore_archive_rejects_symbolic_links() -> Result<()> {
        let mut writer = required_archive_writer()?;
        let options = SimpleFileOptions::default();
        writer.add_symlink("profiles/Rlink.yaml", "../../outside", options)?;

        let mut cursor = writer.finish()?;
        cursor.set_position(0);
        let mut archive = zip::ZipArchive::new(cursor)?;
        let Err(error) = validate_restore_archive(&mut archive) else {
            bail!("symbolic link must be rejected");
        };
        assert!(error.to_string().contains("symbolic link"));
        Ok(())
    }

    #[test]
    fn restore_archive_rejects_unsafe_compression_ratio() -> Result<()> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let stored = SimpleFileOptions::default();
        writer.add_directory("profiles/", stored)?;
        writer.start_file(
            crate::utils::dirs::CLASH_CONFIG,
            stored.compression_method(zip::CompressionMethod::Deflated),
        )?;
        writer.write_all(&vec![b'a'; 1024 * 1024])?;
        for name in [crate::utils::dirs::VERGE_CONFIG, crate::utils::dirs::PROFILE_YAML] {
            writer.start_file(name, stored)?;
            writer.write_all(b"{}")?;
        }

        let mut cursor = writer.finish()?;
        cursor.set_position(0);
        let mut archive = zip::ZipArchive::new(cursor)?;
        let Err(error) = validate_restore_archive(&mut archive) else {
            bail!("compression bomb must be rejected");
        };
        assert!(error.to_string().contains("compression ratio"));
        Ok(())
    }

    #[test]
    fn restore_archive_rejects_excessive_entry_count() -> Result<()> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        for index in 0..=MAX_ARCHIVE_ENTRIES {
            writer.add_directory(format!("entry-{index}/"), options)?;
        }

        let mut cursor = writer.finish()?;
        cursor.set_position(0);
        let mut archive = zip::ZipArchive::new(cursor)?;
        let Err(error) = validate_restore_archive(&mut archive) else {
            bail!("oversized entry table must be rejected");
        };
        assert!(error.to_string().contains("too many entries"));
        Ok(())
    }

    #[test]
    fn staged_restore_rejects_metadata_path_traversal() -> Result<()> {
        let temp = TestDir::new()?;
        let staging = &temp.0;
        std_fs::create_dir_all(staging.join("profiles"))?;
        std_fs::write(staging.join(crate::utils::dirs::CLASH_CONFIG), b"{}")?;
        std_fs::write(staging.join(crate::utils::dirs::VERGE_CONFIG), b"{}")?;
        std_fs::write(
            staging.join(crate::utils::dirs::PROFILE_YAML),
            b"items:\n  - file: ../outside.yaml\n",
        )?;

        let Err(error) = validate_staged_restore(staging) else {
            bail!("metadata traversal must be rejected");
        };
        assert!(error.to_string().contains("profile file"));
        Ok(())
    }

    #[test]
    fn committing_restore_without_dns_removes_previous_dns() -> Result<()> {
        let temp = TestDir::new()?;
        let staging = temp.0.join("staging");
        let home = temp.0.join("home");
        std_fs::create_dir_all(staging.join("profiles"))?;
        std_fs::create_dir_all(home.join("profiles"))?;
        for name in [
            crate::utils::dirs::CLASH_CONFIG,
            crate::utils::dirs::VERGE_CONFIG,
            crate::utils::dirs::PROFILE_YAML,
        ] {
            std_fs::write(staging.join(name), b"new")?;
            std_fs::write(home.join(name), b"old")?;
        }
        std_fs::write(staging.join("profiles/Rnew.yaml"), b"new profile")?;
        std_fs::write(home.join("profiles/Rold.yaml"), b"old profile")?;
        std_fs::write(home.join(DNS_CONFIG), b"old dns")?;

        commit_staged_restore(&staging, &home)?;

        assert!(!home.join(DNS_CONFIG).exists());
        assert!(home.join("profiles/Rnew.yaml").is_file());
        assert!(!home.join("profiles/Rold.yaml").exists());
        Ok(())
    }

    #[tokio::test]
    async fn exclusive_copy_does_not_overwrite_existing_backup() -> Result<()> {
        let temp = TestDir::new()?;
        let source = temp.0.join("source.zip");
        let destination = temp.0.join("destination.zip");
        std_fs::write(&source, b"new")?;
        std_fs::write(&destination, b"existing")?;

        assert!(copy_regular_file_exclusive(&source, &destination).await.is_err());
        assert_eq!(std_fs::read(destination)?, b"existing");
        Ok(())
    }

    #[tokio::test]
    async fn export_rejects_the_source_file_without_modifying_it() -> Result<()> {
        let temp = TestDir::new()?;
        let source = temp.0.join("source.zip");
        fs::write(&source, b"backup").await?;

        assert!(export_regular_file_atomically(&source, &source).await.is_err());
        assert_eq!(fs::read(&source).await?, b"backup");
        Ok(())
    }

    #[tokio::test]
    async fn export_atomically_replaces_an_existing_destination() -> Result<()> {
        let temp = TestDir::new()?;
        let source = temp.0.join("source.zip");
        let destination = temp.0.join("destination.zip");
        fs::write(&source, b"new backup").await?;
        fs::write(&destination, b"old backup").await?;

        export_regular_file_atomically(&source, &destination).await?;

        assert_eq!(fs::read(&source).await?, b"new backup");
        assert_eq!(fs::read(&destination).await?, b"new backup");
        Ok(())
    }

    #[test]
    fn local_backup_filename_rejects_traversal_and_non_zip_files() {
        for filename in [
            "",
            ".",
            "..",
            "../outside.zip",
            "nested/backup.zip",
            "nested\\backup.zip",
            "C:/outside.zip",
            "backup.txt",
        ] {
            assert!(
                validate_backup_filename(filename).is_err(),
                "accepted unsafe filename: {filename}"
            );
        }
        assert!(validate_backup_filename("backup.zip").is_ok());
        assert!(validate_backup_filename("backup.ZIP").is_ok());
    }

    #[test]
    fn disk_snapshot_restores_deleted_files_and_removes_new_entries() -> Result<()> {
        let temp = TestDir::new()?;
        let home = &temp.0;
        let profiles_dir = home.join("profiles");
        std_fs::create_dir_all(&profiles_dir)?;
        std_fs::write(home.join(crate::utils::dirs::CLASH_CONFIG), b"old clash")?;
        std_fs::write(home.join(crate::utils::dirs::VERGE_CONFIG), b"old verge")?;
        std_fs::write(home.join(crate::utils::dirs::PROFILE_YAML), b"old profiles")?;
        std_fs::write(profiles_dir.join("old.yaml"), b"old profile")?;

        let snapshot = RestoreDiskSnapshot::capture_sync(home)?;
        std_fs::remove_file(home.join(crate::utils::dirs::VERGE_CONFIG))?;
        std_fs::write(home.join(DNS_CONFIG), b"new dns")?;
        std_fs::remove_file(profiles_dir.join("old.yaml"))?;
        std_fs::write(profiles_dir.join("new.yaml"), b"new profile")?;

        let errors = snapshot.restore_sync();
        assert!(errors.is_empty(), "rollback errors: {errors:?}");
        assert_eq!(std_fs::read(home.join(crate::utils::dirs::VERGE_CONFIG))?, b"old verge");
        assert!(!home.join(DNS_CONFIG).exists());
        assert_eq!(std_fs::read(profiles_dir.join("old.yaml"))?, b"old profile");
        assert!(!profiles_dir.join("new.yaml").exists());
        Ok(())
    }
}
