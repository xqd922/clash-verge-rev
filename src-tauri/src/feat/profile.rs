use crate::{
    cmd,
    config::{
        Config, IProfiles, PrfItem, PrfOption,
        profiles::{self, profile_file_path, profiles_draft_update_item_safe, profiles_restore_snapshot_safe},
    },
    core::{CoreManager, handle, tray, validate::ValidationOutcome},
    utils::help::{mask_err, mask_url},
};
use anyhow::{Result, anyhow, bail};
use clash_verge_logging::{Type, logging, logging_error};
use smartstring::alias::String;
use tauri::Emitter as _;

/// Toggle proxy profile
pub async fn toggle_proxy_profile(profile_index: String) {
    logging_error!(
        Type::Config,
        cmd::patch_profiles_config_by_profile_index(profile_index).await
    );
}

pub async fn switch_proxy_node(group_name: &str, proxy_name: &str) {
    match handle::Handle::mihomo()
        .await
        .select_node_for_group(group_name, proxy_name)
        .await
    {
        Ok(_) => {
            logging!(info, Type::Tray, "切换代理成功: {} -> {}", group_name, proxy_name);
            let _ = handle::Handle::app_handle().emit("verge://refresh-proxy-config", ());
            let _ = tray::Tray::global().update_menu().await;
            return;
        }
        Err(err) => {
            logging!(
                error,
                Type::Tray,
                "切换代理失败: {} -> {}, 错误: {:?}",
                group_name,
                proxy_name,
                err
            );
        }
    }

    match handle::Handle::mihomo()
        .await
        .select_node_for_group(group_name, proxy_name)
        .await
    {
        Ok(_) => {
            logging!(info, Type::Tray, "代理切换回退成功: {} -> {}", group_name, proxy_name);
            let _ = tray::Tray::global().update_menu().await;
        }
        Err(err) => {
            logging!(
                error,
                Type::Tray,
                "代理切换最终失败: {} -> {}, 错误: {:?}",
                group_name,
                proxy_name,
                err
            );
        }
    }
}

async fn should_update_profile(uid: &String, ignore_auto_update: bool) -> Result<Option<(String, Option<PrfOption>)>> {
    let profiles = Config::profiles().await;
    let profiles = profiles.latest_arc();
    let item = profiles.get_item(uid)?;
    let is_remote = item.itype.as_ref().is_some_and(|s| s == "remote");

    if !is_remote {
        logging!(info, Type::Config, "[订阅更新] {uid} 不是远程订阅，跳过更新");
        Ok(None)
    } else if item.url.is_none() {
        logging!(warn, Type::Config, "Warning: [订阅更新] {uid} 缺少URL，无法更新");
        bail!("failed to get the profile item url");
    } else if !ignore_auto_update && !item.option.as_ref().and_then(|o| o.allow_auto_update).unwrap_or(true) {
        logging!(info, Type::Config, "[订阅更新] {} 禁止自动更新，跳过更新", uid);
        Ok(None)
    } else {
        logging!(
            info,
            Type::Config,
            "[订阅更新] {} 是远程订阅，URL: {}",
            uid,
            mask_url(
                item.url
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Profile URL is None"))?
            )
        );
        Ok(Some((
            item.url.clone().ok_or_else(|| anyhow::anyhow!("Profile URL is None"))?,
            item.option.clone(),
        )))
    }
}

async fn perform_profile_update(
    uid: &String,
    url: &String,
    opt: Option<&PrfOption>,
    option: Option<&PrfOption>,
    is_mannual_trigger: bool,
) -> Result<bool> {
    logging!(info, Type::Config, "[订阅更新] 开始下载新的订阅内容");
    let mut merged_opt = PrfOption::merge(opt, option);
    let is_current = {
        let profiles = Config::profiles().await;
        profiles.latest_arc().is_current_profile_index(uid)
    };
    let profiles = Config::profiles().await;
    let profiles_arc = profiles.latest_arc();
    let profile_name = profiles_arc
        .get_name_by_uid(uid)
        .cloned()
        .unwrap_or_else(|| String::from("UnKnown Profile"));

    let mut last_err;

    match PrfItem::from_url(url, None, None, merged_opt.as_ref()).await {
        Ok(mut item) => {
            logging!(info, Type::Config, "[订阅更新] 更新订阅配置成功");
            profiles_draft_update_item_safe(uid, &mut item).await?;
            return Ok(is_current);
        }
        Err(err) => {
            logging!(
                warn,
                Type::Config,
                "Warning: [订阅更新] 正常更新失败: {}，尝试使用Clash代理更新",
                mask_err(&err.to_string())
            );
            last_err = err;
        }
    }

    merged_opt.get_or_insert_with(PrfOption::default).self_proxy = Some(true);
    merged_opt.get_or_insert_with(PrfOption::default).with_proxy = Some(false);

    match PrfItem::from_url(url, None, None, merged_opt.as_ref()).await {
        Ok(mut item) => {
            logging!(info, Type::Config, "[订阅更新] 使用 Clash代理 更新订阅配置成功");
            profiles_draft_update_item_safe(uid, &mut item).await?;
            handle::Handle::notice_message("update_with_clash_proxy", profile_name);
            drop(last_err);
            return Ok(is_current);
        }
        Err(err) => {
            logging!(
                warn,
                Type::Config,
                "Warning: [订阅更新] Clash代理更新失败: {}，尝试使用系统代理更新",
                mask_err(&err.to_string())
            );
            last_err = err;
        }
    }

    merged_opt.get_or_insert_with(PrfOption::default).self_proxy = Some(false);
    merged_opt.get_or_insert_with(PrfOption::default).with_proxy = Some(true);

    match PrfItem::from_url(url, None, None, merged_opt.as_ref()).await {
        Ok(mut item) => {
            logging!(info, Type::Config, "[订阅更新] 使用 系统代理 更新订阅配置成功");
            profiles_draft_update_item_safe(uid, &mut item).await?;
            handle::Handle::notice_message("update_with_clash_proxy", profile_name);
            drop(last_err);
            return Ok(is_current);
        }
        Err(err) => {
            logging!(
                warn,
                Type::Config,
                "Warning: [订阅更新] 系统代理更新失败: {}，所有重试均已失败",
                mask_err(&err.to_string())
            );
            last_err = err;
        }
    }

    if is_mannual_trigger {
        handle::Handle::notice_message("update_failed_even_with_clash", format!("{profile_name} - {last_err}"));
    }
    Err(last_err.context("all subscription update attempts failed"))
}

struct ProfileUpdateSnapshot {
    profiles: IProfiles,
    file: Option<(std::path::PathBuf, Option<Vec<u8>>)>,
}

impl ProfileUpdateSnapshot {
    async fn capture(uid: &String) -> Result<Self> {
        let profiles = (*Config::profiles().await.data_arc()).clone();
        let file = profiles.get_item(uid)?.file.clone();
        let file = match file {
            Some(file) => {
                let path = profile_file_path(file.as_str())?;
                let content = if tokio::fs::try_exists(&path).await? {
                    Some(tokio::fs::read(&path).await?)
                } else {
                    None
                };
                Some((path, content))
            }
            None => None,
        };
        Ok(Self { profiles, file })
    }

    async fn rollback(self, primary_error: impl std::fmt::Display) -> anyhow::Error {
        let primary_error = primary_error.to_string();
        let file_restore = match self.file {
            Some((path, Some(content))) => tokio::fs::write(path, content).await.map_err(anyhow::Error::from),
            Some((path, None)) => match tokio::fs::remove_file(path).await {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(err.into()),
            },
            None => Ok(()),
        };
        let metadata_restore = profiles_restore_snapshot_safe(self.profiles).await;

        match (file_restore, metadata_restore) {
            (Ok(()), Ok(())) => anyhow!("{primary_error}; active profile update state was rolled back"),
            (Err(file_err), Ok(())) => {
                anyhow!("{primary_error}; subscription file rollback failed: {file_err:#}")
            }
            (Ok(()), Err(metadata_err)) => {
                anyhow!("{primary_error}; profile metadata rollback failed: {metadata_err:#}")
            }
            (Err(file_err), Err(metadata_err)) => anyhow!(
                "{primary_error}; subscription file rollback failed: {file_err:#}; profile metadata rollback failed: {metadata_err:#}"
            ),
        }
    }
}

pub async fn update_profile(
    uid: &String,
    option: Option<&PrfOption>,
    auto_refresh: bool,
    ignore_auto_update: bool,
    is_mannual_trigger: bool,
) -> Result<()> {
    let _profile_transaction = profiles::lock_profile_transaction().await;
    logging!(info, Type::Config, "[订阅更新] 开始更新订阅 {}", uid);
    let url_opt = should_update_profile(uid, ignore_auto_update).await?;
    let update_snapshot = ProfileUpdateSnapshot::capture(uid).await?;
    let is_current = Config::profiles().await.latest_arc().is_current_profile_index(uid);
    let config_permit = if auto_refresh && is_current {
        match CoreManager::global().try_acquire_config_update() {
            Some(permit) => Some(permit),
            None if is_mannual_trigger => bail!("configuration update is already running"),
            None => {
                logging!(
                    info,
                    Type::Config,
                    "[订阅更新] 配置更新正在进行，本次自动更新未产生任何修改"
                );
                return Ok(());
            }
        }
    } else {
        None
    };

    let should_refresh = match url_opt {
        Some((url, opt)) => match perform_profile_update(uid, &url, opt.as_ref(), option, is_mannual_trigger).await {
            Ok(updated_current) => updated_current && auto_refresh,
            Err(err) => return Err(update_snapshot.rollback(err).await),
        },
        None => auto_refresh && is_current,
    };

    if should_refresh {
        let Some(config_permit) = config_permit.as_ref() else {
            bail!("missing configuration update permit for active profile refresh");
        };
        logging!(info, Type::Config, "[订阅更新] 更新内核配置");
        match CoreManager::global()
            .update_config_forced_with_permit(config_permit)
            .await
        {
            Ok(outcome) if outcome.is_valid() => {
                logging!(info, Type::Config, "[订阅更新] 更新成功");
                handle::Handle::refresh_clash();
            }
            Ok(outcome) => {
                let message = outcome.to_string();
                logging!(error, Type::Config, "[订阅更新] 更新失败: {}", message);
                handle::Handle::notice_message("update_failed", message);
                return Err(update_snapshot.rollback(outcome).await);
            }
            Err(err) => {
                logging!(error, Type::Config, "[订阅更新] 更新失败: {}", err);
                handle::Handle::notice_message("update_failed", format!("{err}"));
                logging!(error, Type::Config, "{err}");
                return Err(update_snapshot.rollback(err).await);
            }
        }
    }

    Ok(())
}

/// 增强配置
pub async fn enhance_profiles() -> Result<ValidationOutcome> {
    CoreManager::global().update_config_forced().await
}
