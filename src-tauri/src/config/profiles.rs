use super::{
    PrfOption,
    prfitem::{PrfItem, PrfSelected},
};
use crate::{
    core::{handle, tray::Tray},
    utils::{
        dirs::{self, PathBufExec as _},
        help,
    },
};
use anyhow::{Context as _, Result, bail};
use clash_verge_logging::{Type, logging};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Mapping;
use smartstring::alias::String;
use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
    sync::{
        LazyLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tauri_plugin_mihomo::models::{Proxies, ProxyType};
use tokio::{fs, task::JoinHandle};

#[allow(clippy::unwrap_used)]
static REGEX_PROFILE_FILE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^(?:[RLmrpg][a-zA-Z0-9]+\.yaml|s[a-zA-Z0-9]+\.js)$").unwrap());

static ACTIVATE_SELECTED_TASK: LazyLock<Mutex<Option<JoinHandle<()>>>> = LazyLock::new(|| Mutex::new(None));
static ACTIVATE_SELECTED_GENERATION: AtomicU64 = AtomicU64::new(0);
static PROFILE_TRANSACTION_LOCK: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));
const MIHOMO_OPERATION_TIMEOUT: Duration = Duration::from_secs(10);
const SELECTED_NODES_RECHECK_DELAY: Duration = Duration::from_secs(1);

pub fn validate_profile_file_name(file: &str) -> Result<()> {
    let path = Path::new(file);
    let mut components = path.components();
    if file.is_empty()
        || file.contains('/')
        || file.contains('\\')
        || path.is_absolute()
        || !matches!(
            (components.next(), components.next()),
            (Some(Component::Normal(_)), None)
        )
    {
        bail!("profile file must be a single filename");
    }

    Ok(())
}

pub fn profile_file_path(file: &str) -> Result<PathBuf> {
    validate_profile_file_name(file)?;
    Ok(dirs::app_profiles_dir()?.join(file))
}

pub async fn lock_profile_transaction() -> tokio::sync::MutexGuard<'static, ()> {
    PROFILE_TRANSACTION_LOCK.lock().await
}

pub fn try_lock_profile_transaction() -> Option<tokio::sync::MutexGuard<'static, ()>> {
    PROFILE_TRANSACTION_LOCK.try_lock().ok()
}

/// Define the `profiles.yaml` schema
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct IProfiles {
    /// same as PrfConfig.current
    pub current: Option<String>,

    /// profile list
    pub items: Option<Vec<PrfItem>>,
}

pub struct IProfilePreview<'a> {
    pub uid: &'a String,
    pub name: &'a String,
    pub is_current: bool,
}

/// 清理结果
#[derive(Debug, Clone)]
pub struct CleanupResult {
    pub total_files: usize,
    pub deleted_files: usize,
    pub failed_deletions: usize,
}

#[derive(Debug)]
pub struct ProfileDeleteOutcome {
    pub should_update_runtime: bool,
    delete_files: Vec<String>,
}

impl ProfileDeleteOutcome {
    pub async fn remove_files(self) {
        for file in self.delete_files {
            let path = match profile_file_path(file.as_str()) {
                Ok(path) => path,
                Err(err) => {
                    logging!(
                        warn,
                        Type::Config,
                        "refusing to remove invalid profile file {file:?}: {err:#}"
                    );
                    continue;
                }
            };
            if let Err(err) = path.remove_if_exists().await {
                logging!(
                    warn,
                    Type::Config,
                    "profile deletion is committed, but orphan file cleanup failed for {file}: {err:#}"
                );
            }
        }
    }
}

macro_rules! patch {
    ($lv: expr, $rv: expr, $key: tt) => {
        if ($rv.$key).is_some() {
            $lv.$key = $rv.$key.to_owned();
        }
    };
}

impl IProfiles {
    fn normalize_loaded(mut self) -> Result<Self> {
        let items = self.items.get_or_insert_with(Vec::new);
        for (index, item) in items.iter_mut().enumerate() {
            if item.uid.is_none() {
                item.uid = Some(help::get_uid("d").into());
            }
            if let Some(file) = item.file.as_deref() {
                validate_profile_file_name(file).with_context(|| format!("invalid profile file at items[{index}]"))?;
            }
        }
        Ok(self)
    }

    fn validate_profile_files(&self) -> Result<()> {
        for (index, item) in self.items.as_deref().unwrap_or_default().iter().enumerate() {
            if let Some(file) = item.file.as_deref() {
                validate_profile_file_name(file).with_context(|| format!("invalid profile file at items[{index}]"))?;
            }
        }
        Ok(())
    }

    pub async fn try_new() -> Result<Self> {
        let path = dirs::profiles_path()?;
        help::read_yaml::<Self>(&path)
            .await
            .with_context(|| format!("failed to load profiles from \"{}\"", path.display()))?
            .normalize_loaded()
    }

    pub async fn new() -> Self {
        match Self::try_new().await {
            Ok(profiles) => profiles,
            Err(err) => {
                logging!(error, Type::Config, "failed to load profiles: {err:#}");
                Self::default()
            }
        }
    }

    pub async fn save_file(&self) -> Result<()> {
        self.validate_profile_files()?;
        help::save_yaml(&dirs::profiles_path()?, self, Some("# Profiles Config for Clash Verge")).await
    }

    /// 只修改current，valid和chain
    pub fn patch_config(&mut self, patch: &Self) {
        if self.items.is_none() {
            self.items = Some(vec![]);
        }

        if let Some(current) = &patch.current
            && let Some(items) = self.items.as_ref()
        {
            let some_uid = Some(current);
            if items.iter().any(|e| e.uid.as_ref() == some_uid) {
                self.current = some_uid.cloned();
            }
        }
    }

    pub const fn get_current(&self) -> Option<&String> {
        self.current.as_ref()
    }

    /// get items ref
    pub const fn get_items(&self) -> Option<&Vec<PrfItem>> {
        self.items.as_ref()
    }

    /// find the item by the uid
    pub fn get_item(&self, uid: impl AsRef<str>) -> Result<&PrfItem> {
        let uid_str = uid.as_ref();

        if let Some(items) = self.items.as_ref() {
            for each in items.iter() {
                if let Some(uid_val) = &each.uid
                    && uid_val.as_str() == uid_str
                {
                    return Ok(each);
                }
            }
        }

        bail!("failed to get the profile item \"uid:{}\"", uid_str);
    }

    /// append new item
    /// if the file_data is some
    /// then should save the data to file
    pub async fn append_item(&mut self, item: &mut PrfItem) -> Result<()> {
        self.validate_profile_files()?;
        let uid = &item.uid;
        if uid.is_none() {
            bail!("the uid should not be null");
        }
        if let Some(file) = item.file.as_deref() {
            validate_profile_file_name(file)?;
        }

        // save the file data
        // move the field value after save
        if let Some(file_data) = item.file_data.take() {
            if item.file.is_none() {
                bail!("the file should not be null");
            }

            let file = item
                .file
                .clone()
                .ok_or_else(|| anyhow::anyhow!("file field is required when file_data is provided"))?;
            let path = profile_file_path(file.as_str())?;

            fs::write(&path, file_data.as_bytes())
                .await
                .with_context(|| format!("failed to write to file \"{file}\""))?;
        }

        if self.current.is_none() && (item.itype == Some("remote".into()) || item.itype == Some("local".into())) {
            self.current = uid.to_owned();
        }

        if self.items.is_none() {
            self.items = Some(vec![]);
        }

        if let Some(items) = self.items.as_mut() {
            items.push(item.to_owned());
        }

        Ok(())
    }

    /// reorder items
    pub async fn reorder(&mut self, active_id: &String, over_id: &String) -> Result<()> {
        {
            let items = self.items.as_mut().context("profile list is missing")?;
            let old_idx = items
                .iter()
                .position(|item| item.uid.as_ref() == Some(active_id))
                .with_context(|| format!("failed to find the active profile item \"uid:{active_id}\""))?;
            let new_idx = items
                .iter()
                .position(|item| item.uid.as_ref() == Some(over_id))
                .with_context(|| format!("failed to find the target profile item \"uid:{over_id}\""))?;
            if old_idx == new_idx {
                return Ok(());
            }
            let item = items.remove(old_idx);
            items.insert(new_idx, item);
        }
        self.save_file().await
    }

    /// update the item value
    pub async fn patch_item(&mut self, uid: &String, item: &PrfItem) -> Result<()> {
        if let Some(file) = &item.file {
            validate_profile_file_name(file)?;
        }

        let mut items = self.items.take().unwrap_or_default();

        for each in items.iter_mut() {
            if each.uid.as_ref() == Some(uid) {
                patch!(each, item, itype);
                patch!(each, item, name);
                patch!(each, item, desc);
                patch!(each, item, file);
                patch!(each, item, url);
                patch!(each, item, selected);
                patch!(each, item, extra);
                patch!(each, item, updated);
                patch!(each, item, option);

                self.items = Some(items);
                return self.save_file().await;
            }
        }

        self.items = Some(items);
        bail!("failed to find the profile item \"uid:{uid}\"")
    }

    /// be used to update the remote item
    /// only patch `updated` `extra` `file_data`
    pub async fn update_item(&mut self, uid: &String, item: &mut PrfItem) -> Result<()> {
        self.validate_profile_files()?;
        if let Some(file) = item.file.as_deref() {
            validate_profile_file_name(file)?;
        }
        if self.items.is_none() {
            self.items = Some(vec![]);
        }

        // find the item
        let _ = self.get_item(uid)?;

        if let Some(items) = self.items.as_mut() {
            let some_uid = Some(uid.clone());

            for each in items.iter_mut() {
                if each.uid == some_uid {
                    each.extra = item.extra;
                    each.updated = item.updated;
                    each.home = item.home.to_owned();
                    each.option = PrfOption::merge(each.option.as_ref(), item.option.as_ref());
                    // save the file data
                    // move the field value after save
                    if let Some(file_data) = item.file_data.take() {
                        let file = each.file.take();
                        let file =
                            file.unwrap_or_else(|| item.file.take().unwrap_or_else(|| format!("{}.yaml", &uid).into()));

                        // the file must exists
                        each.file = Some(file.clone());

                        let path = profile_file_path(file.as_str())?;

                        fs::write(&path, file_data.as_bytes())
                            .await
                            .with_context(|| format!("failed to write to file \"{file}\""))?;
                    }

                    break;
                }
            }
        }

        self.save_file().await
    }

    fn option_references(option: Option<&PrfOption>, uid: &str) -> bool {
        option.is_some_and(|option| {
            [
                option.merge.as_deref(),
                option.script.as_deref(),
                option.rules.as_deref(),
                option.proxies.as_deref(),
                option.groups.as_deref(),
            ]
            .into_iter()
            .flatten()
            .any(|reference| reference == uid)
        })
    }

    fn is_primary_profile(item: &PrfItem) -> bool {
        matches!(item.itype.as_deref(), Some("remote" | "local"))
    }

    fn is_dependency(item: &PrfItem) -> bool {
        matches!(
            item.itype.as_deref(),
            Some("merge" | "script" | "rules" | "proxies" | "groups")
        )
    }

    fn delete_item_from_state(&mut self, uid: &String) -> Result<ProfileDeleteOutcome> {
        self.validate_profile_files()?;
        if matches!(uid.as_str(), "Merge" | "Script") {
            bail!("the global profile item \"{uid}\" is protected");
        }

        let current = self.current.as_ref().unwrap_or(uid);
        let current = current.clone();
        let target_index = self
            .items
            .as_deref()
            .unwrap_or_default()
            .iter()
            .position(|item| item.uid.as_ref() == Some(uid))
            .with_context(|| format!("failed to get the profile item \"uid:{uid}\""))?;
        let mut items = self.items.take().unwrap_or_default();
        let removed_main = items.remove(target_index);
        let dependency_uids = removed_main.option.as_ref().map_or(Vec::new(), |option| {
            [
                option.merge.clone(),
                option.script.clone(),
                option.rules.clone(),
                option.proxies.clone(),
                option.groups.clone(),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
        });
        let mut removed_items = vec![removed_main];
        let mut visited_dependencies = HashSet::new();

        for dependency_uid in dependency_uids {
            if matches!(dependency_uid.as_str(), "Merge" | "Script")
                || !visited_dependencies.insert(dependency_uid.clone())
                || items
                    .iter()
                    .filter(|item| Self::is_primary_profile(item))
                    .any(|item| Self::option_references(item.option.as_ref(), dependency_uid.as_str()))
            {
                continue;
            }

            let Some(index) = items.iter().position(|item| item.uid.as_ref() == Some(&dependency_uid)) else {
                continue;
            };
            if Self::is_dependency(&items[index]) {
                removed_items.push(items.remove(index));
            }
        }

        let remaining_files: HashSet<&str> = items.iter().filter_map(|item| item.file.as_deref()).collect();
        let mut queued_files = HashSet::new();
        let delete_files = removed_items
            .into_iter()
            .filter_map(|item| item.file)
            .filter(|file| !remaining_files.contains(file.as_str()) && queued_files.insert(file.clone()))
            .collect();

        if current == *uid {
            self.current = items
                .iter()
                .find(|item| Self::is_primary_profile(item))
                .and_then(|item| item.uid.clone());
        }

        self.items = Some(items);
        Ok(ProfileDeleteOutcome {
            should_update_runtime: current == *uid,
            delete_files,
        })
    }

    /// Delete an item and any unshared per-profile dependencies.
    pub async fn delete_item(&mut self, uid: &String) -> Result<ProfileDeleteOutcome> {
        let outcome = self.delete_item_from_state(uid)?;
        self.save_file().await?;
        Ok(outcome)
    }

    /// 获取current指向的订阅内容
    pub async fn current_mapping(&self) -> Result<Mapping> {
        match (self.current.as_ref(), self.items.as_ref()) {
            (Some(current), Some(items)) => {
                if let Some(item) = items.iter().find(|e| e.uid.as_ref() == Some(current)) {
                    let file_path = match item.file.as_ref() {
                        Some(file) => profile_file_path(file.as_str())?,
                        None => bail!("failed to get the file field"),
                    };
                    return help::read_mapping(&file_path).await;
                }
                bail!("failed to find the current profile \"uid:{current}\"");
            }
            _ => Ok(Mapping::new()),
        }
    }

    /// 判断profile是否是current指向的
    pub fn is_current_profile_index(&self, index: &String) -> bool {
        self.current.as_ref() == Some(index)
    }

    /// 获取所有的profiles(uid，名称, 是否为 current)
    pub fn profiles_preview(&self) -> Option<Vec<IProfilePreview<'_>>> {
        self.items.as_ref().map(|items| {
            items
                .iter()
                .filter_map(|e| {
                    if let (Some(uid), Some(name)) = (e.uid.as_ref(), e.name.as_ref()) {
                        let is_current = self.is_current_profile_index(uid);
                        let preview = IProfilePreview { uid, name, is_current };
                        Some(preview)
                    } else {
                        None
                    }
                })
                .collect()
        })
    }

    /// 通过 uid 获取名称
    pub fn get_name_by_uid(&self, uid: &String) -> Option<&String> {
        if let Some(items) = &self.items {
            for item in items {
                if item.uid.as_ref() == Some(uid) {
                    return item.name.as_ref();
                }
            }
        }
        None
    }

    /// 以 app 中的 profile 列表为准，删除不再需要的文件
    pub async fn cleanup_orphaned_files(&self) -> Result<()> {
        let profiles_dir = dirs::app_profiles_dir()?;

        if !profiles_dir.exists() {
            return Ok(());
        }

        // 获取所有 active profile 的文件名集合
        let active_files = self.get_all_active_files();

        // 添加全局扩展配置文件到保护列表
        let protected_files = self.get_protected_global_files();

        // 扫描 profiles 目录下的所有文件
        let mut total_files = 0;
        let mut deleted_files = 0;
        let mut failed_deletions = 0;

        let mut dir_entries = tokio::fs::read_dir(&profiles_dir).await?;
        while let Some(entry) = dir_entries.next_entry().await? {
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            total_files += 1;

            if let Some(file_name) = path.file_name().and_then(|n| n.to_str())
                && Self::is_profile_file(file_name)
            {
                // 检查是否为全局扩展文件
                if protected_files.contains(file_name) {
                    logging!(debug, Type::Config, "保护全局扩展配置文件: {file_name}");
                    continue;
                }

                // 检查是否为活跃文件
                if !active_files.contains(file_name) {
                    match path.to_path_buf().remove_if_exists().await {
                        Ok(_) => {
                            deleted_files += 1;
                            logging!(debug, Type::Config, "已清理冗余文件: {file_name}");
                        }
                        Err(e) => {
                            failed_deletions += 1;
                            logging!(warn, Type::Config, "Warning: 清理文件失败: {file_name} - {e}");
                        }
                    }
                }
            }
        }

        let result = CleanupResult {
            total_files,
            deleted_files,
            failed_deletions,
        };

        logging!(
            info,
            Type::Config,
            "Profile 文件清理完成: 总文件数={}, 删除文件数={}, 失败数={}",
            result.total_files,
            result.deleted_files,
            result.failed_deletions
        );

        Ok(())
    }

    /// 不删除全局扩展配置
    fn get_protected_global_files(&self) -> HashSet<String> {
        let mut protected_files = HashSet::new();

        protected_files.insert("Merge.yaml".into());
        protected_files.insert("Script.js".into());

        protected_files
    }

    /// 获取所有 active profile 关联的文件名
    fn get_all_active_files(&self) -> HashSet<&str> {
        let mut active_files: HashSet<&str> = HashSet::new();

        if let Some(items) = &self.items {
            for item in items {
                // 收集所有类型 profile 的文件
                if let Some(file) = &item.file {
                    active_files.insert(file);
                }

                // 对于主 profile 类型（remote/local），还需要收集其关联的扩展文件
                if let Some(itype) = &item.itype
                    && (itype == "remote" || itype == "local")
                    && let Some(option) = &item.option
                {
                    // 收集关联的扩展文件
                    if let Some(merge_uid) = &option.merge
                        && let Ok(merge_item) = self.get_item(merge_uid)
                        && let Some(file) = &merge_item.file
                    {
                        active_files.insert(file);
                    }

                    if let Some(script_uid) = &option.script
                        && let Ok(script_item) = self.get_item(script_uid)
                        && let Some(file) = &script_item.file
                    {
                        active_files.insert(file);
                    }

                    if let Some(rules_uid) = &option.rules
                        && let Ok(rules_item) = self.get_item(rules_uid)
                        && let Some(file) = &rules_item.file
                    {
                        active_files.insert(file);
                    }

                    if let Some(proxies_uid) = &option.proxies
                        && let Ok(proxies_item) = self.get_item(proxies_uid)
                        && let Some(file) = &proxies_item.file
                    {
                        active_files.insert(file);
                    }

                    if let Some(groups_uid) = &option.groups
                        && let Ok(groups_item) = self.get_item(groups_uid)
                        && let Some(file) = &groups_item.file
                    {
                        active_files.insert(file);
                    }
                }
            }
        }

        active_files
    }

    /// 检查文件名是否符合 profile 文件的命名规则
    fn is_profile_file(filename: &str) -> bool {
        REGEX_PROFILE_FILE.is_match(filename)
    }
}

// 特殊的Send-safe helper函数，完全避免跨await持有guard
use crate::config::Config;

pub async fn profiles_append_item_with_filedata_safe(item: &PrfItem, file_data: Option<String>) -> Result<()> {
    let item = &mut PrfItem::from(item, file_data).await?;
    profiles_append_item_safe(item).await
}

pub async fn profiles_append_item_safe(item: &mut PrfItem) -> Result<()> {
    Config::profiles()
        .await
        .with_data_modify(|mut profiles| async move {
            profiles.append_item(item).await?;
            Ok((profiles, ()))
        })
        .await
}

pub async fn profiles_patch_item_safe(index: &String, item: &PrfItem) -> Result<()> {
    Config::profiles()
        .await
        .with_data_modify(|mut profiles| async move {
            profiles.patch_item(index, item).await?;
            Ok((profiles, ()))
        })
        .await
}

pub async fn profiles_delete_item_safe(index: &String) -> Result<ProfileDeleteOutcome> {
    Config::profiles()
        .await
        .with_data_modify(|mut profiles| async move {
            let outcome = profiles.delete_item(index).await?;
            Ok((profiles, outcome))
        })
        .await
}

pub async fn profiles_reorder_safe(active_id: &String, over_id: &String) -> Result<()> {
    Config::profiles()
        .await
        .with_data_modify(|mut profiles| async move {
            profiles.reorder(active_id, over_id).await?;
            Ok((profiles, ()))
        })
        .await
}

pub async fn profiles_save_file_safe() -> Result<()> {
    Config::profiles()
        .await
        .with_data_modify(|profiles| async move {
            profiles.save_file().await?;
            Ok((profiles, ()))
        })
        .await
}

pub async fn profiles_restore_snapshot_safe(snapshot: IProfiles) -> Result<()> {
    let profiles = Config::profiles().await;
    let new_files: HashSet<String> = profiles
        .data_arc()
        .get_all_active_files()
        .into_iter()
        .map(String::from)
        .collect();
    let previous_files: HashSet<String> = snapshot.get_all_active_files().into_iter().map(String::from).collect();

    profiles
        .with_data_modify(|_| async move {
            snapshot.save_file().await?;
            Ok((snapshot, ()))
        })
        .await?;

    for file in new_files.difference(&previous_files) {
        let path = profile_file_path(file.as_str())?;
        if let Err(err) = path.remove_if_exists().await {
            logging!(
                warn,
                Type::Config,
                "profile state is restored, but orphan file cleanup is deferred for {file}: {err:#}"
            );
        }
    }
    Ok(())
}

pub async fn profiles_draft_update_item_safe(index: &String, item: &mut PrfItem) -> Result<()> {
    Config::profiles()
        .await
        .with_data_modify(|mut profiles| async move {
            profiles.update_item(index, item).await?;
            Ok((profiles, ()))
        })
        .await
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct SelectedNodeActivation {
    group_name: String,
    node: String,
    unfix_after_select: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct SelectedNodesPlan {
    selected: Vec<PrfSelected>,
    activations: Vec<SelectedNodeActivation>,
    repaired_count: usize,
}

fn node_is_available(available_nodes: &[std::string::String], node: &str) -> bool {
    available_nodes.iter().any(|available| available == node)
}

fn selected_nodes_need_confirmation(selected: &[PrfSelected], proxies: &Proxies) -> bool {
    selected.iter().any(|selected_item| {
        let (Some(group_name), Some(node)) = (&selected_item.name, &selected_item.now) else {
            return false;
        };
        let Some(group) = proxies.proxies.get(group_name.as_str()) else {
            return true;
        };
        let Some(available_nodes) = group.all.as_deref().filter(|nodes| !nodes.is_empty()) else {
            return true;
        };
        !node_is_available(available_nodes, node)
    })
}

fn is_smart_group(proxy_type: &ProxyType) -> bool {
    matches!(proxy_type, ProxyType::Unknown(value) if value.eq_ignore_ascii_case("smart"))
}

fn is_selectable_group(proxy_type: &ProxyType) -> bool {
    matches!(
        proxy_type,
        ProxyType::Selector | ProxyType::URLTest | ProxyType::Fallback | ProxyType::LoadBalance
    ) || is_smart_group(proxy_type)
}

fn reconcile_selected_nodes(
    selected: &[PrfSelected],
    previous: Option<&Proxies>,
    proxies: &Proxies,
) -> SelectedNodesPlan {
    let mut plan = SelectedNodesPlan {
        selected: Vec::with_capacity(selected.len()),
        activations: Vec::new(),
        repaired_count: 0,
    };
    let mut seen_groups = HashSet::new();
    let mut unique_selected = selected
        .iter()
        .rev()
        .filter(|item| item.name.as_ref().is_some_and(|name| seen_groups.insert(name.clone())))
        .collect::<Vec<_>>();
    unique_selected.reverse();
    plan.repaired_count += selected.len() - unique_selected.len();

    for selected_item in unique_selected {
        let (Some(group_name), Some(node)) = (&selected_item.name, &selected_item.now) else {
            plan.repaired_count += 1;
            continue;
        };
        let Some(group) = proxies.proxies.get(group_name.as_str()) else {
            if previous.is_some_and(|snapshot| !snapshot.proxies.contains_key(group_name.as_str())) {
                plan.repaired_count += 1;
            } else {
                plan.selected.push(selected_item.clone());
            }
            continue;
        };
        let Some(available_nodes) = group.all.as_deref().filter(|nodes| !nodes.is_empty()) else {
            plan.selected.push(selected_item.clone());
            continue;
        };
        if !is_selectable_group(&group.proxy_type) {
            let preferred_node = group
                .now
                .as_deref()
                .filter(|current| node_is_available(available_nodes, current))
                .or_else(|| node_is_available(available_nodes, node).then_some(node.as_str()));
            if let Some(preferred_node) = preferred_node {
                if preferred_node != node.as_str() {
                    plan.repaired_count += 1;
                }
                plan.selected.push(PrfSelected {
                    name: Some(group_name.clone()),
                    now: Some(preferred_node.into()),
                });
            } else {
                plan.repaired_count += 1;
            }
            continue;
        }

        if node_is_available(available_nodes, node) {
            plan.selected.push(selected_item.clone());
            let smart_group = is_smart_group(&group.proxy_type);
            if group.now.as_deref() != Some(node.as_str()) || (smart_group && group.fixed.is_some()) {
                plan.activations.push(SelectedNodeActivation {
                    group_name: group_name.clone(),
                    node: node.clone(),
                    unfix_after_select: smart_group,
                });
            }
            continue;
        }

        let missing_was_confirmed = previous
            .and_then(|snapshot| snapshot.proxies.get(group_name.as_str()))
            .and_then(|group| group.all.as_deref())
            .filter(|nodes| !nodes.is_empty())
            .is_some_and(|nodes| !node_is_available(nodes, node));
        if !missing_was_confirmed {
            plan.selected.push(selected_item.clone());
            continue;
        }

        plan.repaired_count += 1;
        if let Some(current_node) = group
            .now
            .as_deref()
            .filter(|current| node_is_available(available_nodes, current))
        {
            plan.selected.push(PrfSelected {
                name: Some(group_name.clone()),
                now: Some(current_node.into()),
            });
        }
    }

    plan
}

fn is_activation_current(generation: u64) -> bool {
    ACTIVATE_SELECTED_GENERATION.load(Ordering::Acquire) == generation
}

async fn fetch_proxies_with_timeout() -> Result<Proxies> {
    tokio::time::timeout(MIHOMO_OPERATION_TIMEOUT, async {
        loop {
            match handle::Handle::mihomo().await.get_proxies().await {
                Ok(proxies) => return proxies,
                Err(err) => {
                    logging!(debug, Type::Config, "mihomo proxies are not ready yet: {err}");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
    })
    .await
    .context("timed out while waiting for mihomo proxies")
}

async fn select_node_with_timeout(group_name: &String, node: &String) -> Result<()> {
    tokio::time::timeout(MIHOMO_OPERATION_TIMEOUT, async {
        handle::Handle::mihomo()
            .await
            .select_node_for_group(group_name, node)
            .await
    })
    .await
    .with_context(|| format!("timed out while selecting node [{node}] for group [{group_name}]"))?
    .with_context(|| format!("failed to select node [{node}] for group [{group_name}]"))
}

async fn unfix_proxy_with_timeout(group_name: &String) -> Result<()> {
    tokio::time::timeout(MIHOMO_OPERATION_TIMEOUT, async {
        handle::Handle::mihomo().await.unfixed_proxy(group_name).await
    })
    .await
    .with_context(|| format!("timed out while unfixing Smart group [{group_name}]"))?
    .with_context(|| format!("failed to unfix Smart group [{group_name}]"))
}

fn remaining_activations(
    activations: &[SelectedNodeActivation],
    completed: &HashSet<SelectedNodeActivation>,
) -> Vec<SelectedNodeActivation> {
    activations
        .iter()
        .filter(|activation| !completed.contains(*activation))
        .cloned()
        .collect()
}

async fn apply_activations(
    activations: &[SelectedNodeActivation],
    completed: &mut HashSet<SelectedNodeActivation>,
    generation: u64,
) -> Option<usize> {
    let mut activated_count = 0;
    for activation in remaining_activations(activations, completed) {
        if !is_activation_current(generation) {
            return None;
        }
        let result = select_node_with_timeout(&activation.group_name, &activation.node).await;
        let result = if result.is_ok() && activation.unfix_after_select {
            unfix_proxy_with_timeout(&activation.group_name).await
        } else {
            result
        };
        match result {
            Ok(()) => {
                if !is_activation_current(generation) {
                    return None;
                }
                logging!(
                    info,
                    Type::Config,
                    "Selected node for proxy: {}, node: {}",
                    activation.group_name,
                    activation.node
                );
                completed.insert(activation);
                activated_count += 1;
            }
            Err(err) => logging!(error, Type::Config, "{err:#}"),
        }
        if !is_activation_current(generation) {
            return None;
        }
    }
    Some(activated_count)
}

async fn update_tray_after_activation(generation: u64) {
    if !is_activation_current(generation) {
        return;
    }
    if let Err(err) = Tray::global().update_tooltip().await {
        logging!(
            warn,
            Type::Config,
            "failed to update tray tooltip after profile switch: {err:#}"
        );
    }

    if !is_activation_current(generation) {
        return;
    }
    if let Err(err) = Tray::global().update_menu().await {
        logging!(
            warn,
            Type::Config,
            "failed to update tray menu after profile switch: {err:#}"
        );
    }
}

async fn persist_reconciled_selected(
    profile_uid: &String,
    original_selected: &[PrfSelected],
    selected: Vec<PrfSelected>,
    generation: u64,
) -> Result<()> {
    let _profile_transaction = lock_profile_transaction().await;
    if !is_activation_current(generation) {
        return Ok(());
    }

    let profiles = Config::profiles().await;
    let profile_uid = profile_uid.clone();
    let original_selected = original_selected.to_vec();
    let updated = profiles
        .with_data_modify(move |mut profiles| async move {
            if !is_activation_current(generation) || profiles.current.as_ref() != Some(&profile_uid) {
                return Ok((profiles, false));
            }

            let profile = profiles
                .items
                .as_mut()
                .and_then(|items| items.iter_mut().find(|item| item.uid.as_ref() == Some(&profile_uid)))
                .with_context(|| format!("failed to find the profile item \"uid:{profile_uid}\""))?;
            if profile.selected.as_deref().unwrap_or(&[]) != original_selected.as_slice() {
                return Ok((profiles, false));
            }

            profile.selected = (!selected.is_empty()).then_some(selected);
            profiles.save_file().await?;
            Ok((profiles, true))
        })
        .await?;

    if updated {
        handle::Handle::refresh_profiles();
    }
    Ok(())
}

async fn activate_selected_nodes_worker(
    profile_uid: String,
    selected: Vec<PrfSelected>,
    generation: u64,
) -> Result<()> {
    let first_snapshot = fetch_proxies_with_timeout().await?;
    if !is_activation_current(generation) {
        return Ok(());
    }

    let needs_confirmation = selected_nodes_need_confirmation(&selected, &first_snapshot);
    let immediate_plan = reconcile_selected_nodes(&selected, None, &first_snapshot);
    logging!(
        debug,
        Type::Config,
        "immediate selected nodes activation plan: {immediate_plan:?}"
    );

    let mut completed_activations = HashSet::new();
    if apply_activations(&immediate_plan.activations, &mut completed_activations, generation)
        .await
        .is_none()
    {
        return Ok(());
    }

    if is_activation_current(generation) {
        handle::Handle::refresh_clash();
    }

    let plan = if needs_confirmation {
        tokio::time::sleep(SELECTED_NODES_RECHECK_DELAY).await;
        if !is_activation_current(generation) {
            return Ok(());
        }
        let second_snapshot = fetch_proxies_with_timeout().await?;
        if !is_activation_current(generation) {
            return Ok(());
        }
        let confirmed_plan = reconcile_selected_nodes(&selected, Some(&first_snapshot), &second_snapshot);
        logging!(
            debug,
            Type::Config,
            "confirmed selected nodes activation plan: {confirmed_plan:?}"
        );
        let Some(confirmed_activated_count) =
            apply_activations(&confirmed_plan.activations, &mut completed_activations, generation).await
        else {
            return Ok(());
        };
        if confirmed_activated_count > 0 && is_activation_current(generation) {
            handle::Handle::refresh_clash();
        }
        confirmed_plan
    } else {
        immediate_plan
    };
    if !is_activation_current(generation) {
        return Ok(());
    }

    if plan.repaired_count > 0 {
        logging!(
            info,
            Type::Config,
            "repairing {} invalid selected node record(s) for profile {profile_uid}",
            plan.repaired_count
        );
        persist_reconciled_selected(&profile_uid, &selected, plan.selected, generation).await?;
    }

    Ok(())
}

pub fn activate_selected_nodes() {
    logging!(info, Type::Config, "starting activating selected nodes");
    let mut active_task = ACTIVATE_SELECTED_TASK.lock();
    let generation = ACTIVATE_SELECTED_GENERATION.fetch_add(1, Ordering::AcqRel) + 1;
    let previous_task = active_task.take();

    let handle = tokio::spawn(async move {
        if let Some(previous_task) = previous_task {
            let _ = previous_task.await;
        }
        if !is_activation_current(generation) {
            return;
        }

        let result = async {
            let profiles = Config::profiles().await.latest_arc();
            let current = profiles.get_current().context("no current profile running")?.clone();
            let selected = profiles
                .get_item(&current)
                .context("failed to get current profile")?
                .selected
                .clone()
                .unwrap_or_default();

            if selected.is_empty() {
                if is_activation_current(generation) {
                    handle::Handle::refresh_clash();
                }
                return Ok(());
            }
            activate_selected_nodes_worker(current, selected, generation).await
        }
        .await;

        if is_activation_current(generation) {
            if let Err(err) = result {
                logging!(error, Type::Config, "failed to activate selected nodes: {err:#}");
                handle::Handle::refresh_clash();
            }
            update_tray_after_activation(generation).await;
            logging!(info, Type::Config, "activating selected nodes done!");
        }
    });
    *active_task = Some(handle);
    drop(active_task);
}

#[cfg(test)]
mod profile_integrity_tests {
    use super::*;

    fn item(uid: &str, itype: &str, file: &str) -> PrfItem {
        PrfItem {
            uid: Some(uid.into()),
            itype: Some(itype.into()),
            file: Some(file.into()),
            ..PrfItem::default()
        }
    }

    fn primary(uid: &str, file: &str, option: PrfOption) -> PrfItem {
        PrfItem {
            option: Some(option),
            ..item(uid, "remote", file)
        }
    }

    fn profiles(current: &str, items: Vec<PrfItem>) -> IProfiles {
        IProfiles {
            current: Some(current.into()),
            items: Some(items),
        }
    }

    fn item_uids(profiles: &IProfiles) -> HashSet<&str> {
        profiles
            .items
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter_map(|item| item.uid.as_deref())
            .collect()
    }

    fn delete_files(outcome: &ProfileDeleteOutcome) -> HashSet<&str> {
        outcome.delete_files.iter().map(String::as_str).collect()
    }

    #[test]
    fn profile_file_name_accepts_only_one_normal_component() {
        for file in ["R123.yaml", "custom profile.yaml", "Script.js"] {
            assert!(validate_profile_file_name(file).is_ok(), "{file:?} should be accepted");
        }

        for file in [
            "",
            ".",
            "..",
            "../profile.yaml",
            "..\\profile.yaml",
            "nested/profile.yaml",
            "nested\\profile.yaml",
            "/absolute/profile.yaml",
            "C:\\absolute\\profile.yaml",
            "C:/absolute/profile.yaml",
            "\\\\server\\share\\profile.yaml",
        ] {
            assert!(validate_profile_file_name(file).is_err(), "{file:?} should be rejected");
        }
    }

    #[test]
    fn loaded_profiles_fill_missing_uid_and_reject_unsafe_files() -> Result<()> {
        let loaded = serde_yaml_ng::from_str::<IProfiles>("items:\n  - type: local\n    file: local.yaml\n")?
            .normalize_loaded()?;
        assert!(loaded.items.as_deref().unwrap_or_default()[0].uid.is_some());

        let unsafe_profiles =
            serde_yaml_ng::from_str::<IProfiles>("items:\n  - uid: bad\n    type: local\n    file: ../outside.yaml\n")?;
        assert!(unsafe_profiles.normalize_loaded().is_err());
        assert!(serde_yaml_ng::from_str::<IProfiles>("items: [").is_err());
        Ok(())
    }

    #[tokio::test]
    async fn save_file_rejects_an_unsafe_file_before_filesystem_access() -> Result<()> {
        let profiles = profiles("bad", vec![item("bad", "local", "../outside.yaml")]);
        let Err(err) = profiles.save_file().await else {
            bail!("unsafe profile metadata must not be saved");
        };
        assert!(err.to_string().contains("invalid profile file"));
        Ok(())
    }

    #[tokio::test]
    async fn append_rejects_an_unsafe_file_without_mutating_the_item() -> Result<()> {
        let mut profiles = IProfiles::default();
        let mut unsafe_item = item("bad", "local", "../outside.yaml");
        unsafe_item.file_data = Some("proxies: []".into());

        let Err(err) = profiles.append_item(&mut unsafe_item).await else {
            bail!("unsafe profile metadata must not be appended");
        };
        assert!(err.to_string().contains("single filename"));
        assert!(unsafe_item.file_data.is_some());
        assert!(profiles.items.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn reorder_with_a_missing_uid_preserves_the_profile_list() -> Result<()> {
        let mut profiles = profiles(
            "first",
            vec![
                item("first", "local", "first.yaml"),
                item("second", "local", "second.yaml"),
            ],
        );
        let before = serde_yaml_ng::to_string(&profiles.items)?;

        assert!(profiles.reorder(&"missing".into(), &"second".into()).await.is_err());
        assert_eq!(serde_yaml_ng::to_string(&profiles.items)?, before);
        assert!(profiles.reorder(&"first".into(), &"missing".into()).await.is_err());
        assert_eq!(serde_yaml_ng::to_string(&profiles.items)?, before);
        Ok(())
    }

    #[test]
    fn deleting_profile_preserves_a_shared_dependency() -> Result<()> {
        let shared = PrfOption {
            rules: Some("shared-rules".into()),
            ..PrfOption::default()
        };
        let mut profiles = profiles(
            "first",
            vec![
                primary("first", "first.yaml", shared.clone()),
                primary("second", "second.yaml", shared),
                item("shared-rules", "rules", "shared-rules.yaml"),
            ],
        );

        let outcome = profiles.delete_item_from_state(&"first".into())?;
        assert_eq!(profiles.current.as_deref(), Some("second"));
        assert_eq!(item_uids(&profiles), HashSet::from(["second", "shared-rules"]));
        assert_eq!(delete_files(&outcome), HashSet::from(["first.yaml"]));
        Ok(())
    }

    #[test]
    fn deleting_profile_removes_its_unique_dependency() -> Result<()> {
        let mut profiles = profiles(
            "second",
            vec![
                primary(
                    "first",
                    "first.yaml",
                    PrfOption {
                        rules: Some("unique-rules".into()),
                        ..PrfOption::default()
                    },
                ),
                primary("second", "second.yaml", PrfOption::default()),
                item("unique-rules", "rules", "unique-rules.yaml"),
            ],
        );

        let outcome = profiles.delete_item_from_state(&"first".into())?;
        assert_eq!(item_uids(&profiles), HashSet::from(["second"]));
        assert_eq!(
            delete_files(&outcome),
            HashSet::from(["first.yaml", "unique-rules.yaml"])
        );
        Ok(())
    }

    #[test]
    fn deleting_profile_always_preserves_global_merge_and_script() -> Result<()> {
        let mut profiles = profiles(
            "profile",
            vec![
                primary(
                    "profile",
                    "profile.yaml",
                    PrfOption {
                        merge: Some("Merge".into()),
                        script: Some("Script".into()),
                        ..PrfOption::default()
                    },
                ),
                item("Merge", "merge", "Merge.yaml"),
                item("Script", "script", "Script.js"),
            ],
        );

        let outcome = profiles.delete_item_from_state(&"profile".into())?;
        assert_eq!(item_uids(&profiles), HashSet::from(["Merge", "Script"]));
        assert_eq!(delete_files(&outcome), HashSet::from(["profile.yaml"]));
        assert!(profiles.delete_item_from_state(&"Merge".into()).is_err());
        assert_eq!(item_uids(&profiles), HashSet::from(["Merge", "Script"]));
        Ok(())
    }

    #[test]
    fn deleting_item_does_not_remove_a_file_still_used_by_name() -> Result<()> {
        let mut profiles = profiles(
            "first",
            vec![
                primary("first", "shared.yaml", PrfOption::default()),
                primary("second", "shared.yaml", PrfOption::default()),
            ],
        );

        let outcome = profiles.delete_item_from_state(&"first".into())?;
        assert!(outcome.delete_files.is_empty());
        assert_eq!(item_uids(&profiles), HashSet::from(["second"]));
        Ok(())
    }
}

#[cfg(test)]
mod selected_nodes_tests {
    use super::*;
    use std::collections::HashMap;
    use tauri_plugin_mihomo::models::Proxy;

    fn selected(group: &str, node: &str) -> PrfSelected {
        PrfSelected {
            name: Some(group.into()),
            now: Some(node.into()),
        }
    }

    fn group(proxy_type: ProxyType, current: &str, nodes: &[&str]) -> Proxies {
        group_with_fixed(proxy_type, current, nodes, None)
    }

    fn group_with_fixed(proxy_type: ProxyType, current: &str, nodes: &[&str], fixed: Option<&str>) -> Proxies {
        Proxies {
            proxies: HashMap::from([(
                "group".to_owned(),
                Proxy {
                    name: "group".to_owned(),
                    all: Some(nodes.iter().map(|node| (*node).to_owned()).collect()),
                    fixed: fixed.map(str::to_owned),
                    now: Some(current.to_owned()),
                    proxy_type,
                    ..Proxy::default()
                },
            )]),
        }
    }

    #[test]
    fn valid_selector_selection_is_activated() {
        let saved = vec![selected("group", "saved")];
        let plan = reconcile_selected_nodes(
            &saved,
            None,
            &group(ProxyType::Selector, "current", &["current", "saved"]),
        );
        assert_eq!(plan.selected, saved);
        assert_eq!(
            plan.activations,
            vec![SelectedNodeActivation {
                group_name: "group".into(),
                node: "saved".into(),
                unfix_after_select: false,
            }]
        );
    }

    #[test]
    fn smart_unknown_group_is_selectable_and_activates_saved_node() {
        let saved = vec![selected("group", "saved")];
        let plan = reconcile_selected_nodes(
            &saved,
            None,
            &group(ProxyType::Unknown("Smart".into()), "current", &["current", "saved"]),
        );
        assert_eq!(plan.selected, saved);
        assert_eq!(
            plan.activations,
            vec![SelectedNodeActivation {
                group_name: "group".into(),
                node: "saved".into(),
                unfix_after_select: true,
            }]
        );
        assert_eq!(plan.repaired_count, 0);
    }

    #[test]
    fn fixed_smart_group_is_unfixed_when_saved_node_is_already_current() {
        let saved = vec![selected("group", "saved")];
        let plan = reconcile_selected_nodes(
            &saved,
            None,
            &group_with_fixed(
                ProxyType::Unknown("Smart".into()),
                "saved",
                &["current", "saved"],
                Some("saved"),
            ),
        );
        assert_eq!(plan.selected, saved);
        assert_eq!(
            plan.activations,
            vec![SelectedNodeActivation {
                group_name: "group".into(),
                node: "saved".into(),
                unfix_after_select: true,
            }]
        );
        assert_eq!(plan.repaired_count, 0);
    }

    #[test]
    fn direct_group_is_not_activated() {
        let plan = reconcile_selected_nodes(
            &[selected("group", "saved")],
            None,
            &group(ProxyType::Direct, "current", &["current", "saved"]),
        );
        assert_eq!(plan.selected, vec![selected("group", "current")]);
        assert!(plan.activations.is_empty());
        assert_eq!(plan.repaired_count, 1);
    }
}
