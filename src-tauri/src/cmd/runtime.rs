use super::CmdResult;
use crate::{cmd::StringifyErr as _, config::Config, utils::yaml_emitter};
use anyhow::{Context as _, anyhow};
use serde_yaml_ng::Mapping;
use smartstring::alias::String;
use std::collections::{HashMap, HashSet};

#[tauri::command]
pub async fn get_runtime_config() -> CmdResult<Option<Mapping>> {
    Ok(Config::runtime().await.latest_arc().config.clone())
}

#[tauri::command]
pub async fn get_runtime_yaml() -> CmdResult<String> {
    let runtime = Config::runtime().await;
    let runtime = runtime.latest_arc();

    let config = runtime.config.as_ref();
    config
        .ok_or_else(|| anyhow!("failed to parse config to yaml file"))
        .and_then(|config| {
            yaml_emitter::to_mihomo_config_string(config)
                .context("failed to convert config to yaml")
                .map(|s| s.into())
        })
        .stringify_err()
}

#[tauri::command]
pub async fn get_runtime_exists() -> CmdResult<HashSet<String>> {
    Ok(Config::runtime().await.latest_arc().exists_keys.clone())
}

#[tauri::command]
pub async fn get_runtime_logs() -> CmdResult<HashMap<String, Vec<(String, String)>>> {
    Ok(Config::runtime().await.latest_arc().chain_logs.clone())
}
