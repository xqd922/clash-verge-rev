use anyhow::Result;
use percent_encoding::percent_decode_str;
use smartstring::alias::String;
use tauri::Url;

use crate::{
    config::{Config, PrfItem, profiles},
    core::{CoreManager, handle, manager::ConfigUpdatePermit, timer::Timer},
    utils::help,
};
use clash_verge_logging::{Type, logging, logging_error};

pub(super) async fn resolve_scheme(param: &str) -> Result<()> {
    let param_str = if param.starts_with("[") && param.len() > 4 {
        param
            .get(2..param.len() - 2)
            .ok_or_else(|| anyhow::anyhow!("Invalid string slice boundaries"))?
    } else {
        param
    };
    let masked_deep_link = help::mask_url(param_str);

    logging!(debug, Type::Config, "received deep link: {masked_deep_link}");

    let link_parsed = Url::parse(param_str)
        .map_err(|e| anyhow::anyhow!("failed to parse deep link: {e:?}, param: {masked_deep_link}"))?;

    let Some((url, name)) = extract_subscription_info(&link_parsed) else {
        logging!(
            warn,
            Type::Config,
            "missing url parameter in deep link: {masked_deep_link}"
        );
        return Ok(());
    };

    import_subscription(&url, name.as_ref()).await;
    Ok(())
}

fn extract_subscription_info(link_parsed: &Url) -> Option<(std::string::String, Option<String>)> {
    if !matches!(link_parsed.scheme(), "clash" | "clash-verge") {
        return None;
    }

    let name = link_parsed
        .query_pairs()
        .find(|(key, _)| key == "name")
        .map(|(_, value)| value.into_owned().into());
    let url = extract_subscription_url(link_parsed)?;
    Some((url, name))
}

fn extract_subscription_url(link_parsed: &Url) -> Option<std::string::String> {
    let query = link_parsed.query()?;
    let prefix = "url=";
    let pos = query.find(prefix)?;
    let raw_url = query[pos + prefix.len()..].trim();
    Some(decode_subscription_url(raw_url))
}

fn decode_subscription_url(raw_url: &str) -> std::string::String {
    // Avoid double-decoding nested subscription URLs; decode only when needed.
    if Url::parse(raw_url).is_ok() {
        return raw_url.to_string();
    }

    let mut candidate = raw_url.to_string();
    for _ in 0..2 {
        let next = percent_decode_str(&candidate).decode_utf8_lossy().to_string();
        if next == candidate {
            break;
        }
        candidate = next;
        if Url::parse(&candidate).is_ok() {
            break;
        }
    }
    candidate
}

async fn import_subscription(url: &str, name: Option<&String>) {
    let profile_transaction = profiles::lock_profile_transaction().await;
    let previous_profiles = (*Config::profiles().await.data_arc()).clone();
    let had_current_profile = {
        let profiles = Config::profiles().await;
        profiles.latest_arc().current.is_some()
    };
    let config_permit = if had_current_profile {
        None
    } else {
        let Some(permit) = CoreManager::global().try_acquire_config_update() else {
            handle::Handle::notice_message("import_sub_url::error", "configuration update is already running");
            return;
        };
        Some(permit)
    };

    let mut item = match PrfItem::from_url(url, name, None, None).await {
        Ok(item) => item,
        Err(err) => {
            rollback_deep_link_import(previous_profiles, err).await;
            return;
        }
    };

    let uid = item.uid.clone().unwrap_or_default();
    if let Err(e) = profiles::profiles_append_item_safe(&mut item).await {
        logging!(error, Type::Config, "failed to import subscription url: {:?}", e);
        rollback_deep_link_import(previous_profiles, e).await;
        return;
    }

    if let Err(e) = profiles::profiles_save_file_safe().await {
        logging!(error, Type::Config, "failed to save imported subscription: {}", e);
        rollback_deep_link_import(previous_profiles, e).await;
        return;
    }

    let should_update_core =
        !uid.is_empty() && !had_current_profile && Config::profiles().await.latest_arc().is_current_profile_index(&uid);
    if should_update_core {
        let Some(config_permit) = config_permit.as_ref() else {
            rollback_deep_link_import(
                previous_profiles,
                "missing configuration update permit for imported active profile",
            )
            .await;
            return;
        };
        if let Err(err) = refresh_core_config(config_permit).await {
            rollback_deep_link_import(previous_profiles, err).await;
            return;
        }
    }

    drop(config_permit);
    drop(profile_transaction);
    logging_error!(Type::Timer, Timer::global().refresh().await);
    handle::Handle::notice_message(
        "import_sub_url::ok",
        "", // 空 msg 传入，我们不希望导致 后端-前端-后端 死循环，这里只做提醒。
    );

    handle::Handle::refresh_verge();
    handle::Handle::notify_profile_changed(&uid);
}

async fn rollback_deep_link_import(snapshot: crate::config::IProfiles, primary_error: impl std::fmt::Display) {
    let primary_error = primary_error.to_string();
    let message = match profiles::profiles_restore_snapshot_safe(snapshot).await {
        Ok(()) => format!("{primary_error}; active profile state was rolled back"),
        Err(rollback_err) => format!("{primary_error}; profile rollback failed: {rollback_err:#}"),
    };
    logging!(error, Type::Config, "deep-link import failed: {message}");
    handle::Handle::notice_message("import_sub_url::error", message);
}

async fn refresh_core_config(config_permit: &ConfigUpdatePermit<'_>) -> Result<()> {
    logging!(
        info,
        Type::Config,
        "Deep link import set current profile; refreshing core config"
    );
    match CoreManager::global()
        .update_config_forced_with_permit(config_permit)
        .await
    {
        Ok(outcome) if outcome.is_valid() => {
            handle::Handle::refresh_clash();
            Ok(())
        }
        Ok(outcome) => Err(anyhow::anyhow!("Apply config failed: {outcome}")),
        Err(err) => Err(err.context("Apply imported profile config")),
    }
}
