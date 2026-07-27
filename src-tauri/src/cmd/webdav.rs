use super::CmdResult;
use crate::{
    cmd::StringifyErr as _,
    config::{Config, IVerge},
    core::{self, CoreManager},
    feat,
};
use reqwest_dav::list_cmd::ListFile;
use smartstring::alias::String;

/// 保存 WebDAV 配置
#[tauri::command]
pub async fn save_webdav_config(url: String, username: String, password: String) -> CmdResult<()> {
    let Some(_config_permit) = CoreManager::global().try_acquire_config_update() else {
        return Err("A configuration update is already running".into());
    };

    let patch = IVerge {
        webdav_url: Some(url),
        webdav_username: Some(username),
        webdav_password: Some(password),
        ..IVerge::default()
    };
    let verge = Config::verge().await;
    let committed_verge = verge.data_arc();
    verge.edit_draft(|draft| draft.patch_config(&patch));

    if let Err(err) = verge.latest_arc().save_file().await {
        verge.discard();
        return match committed_verge.save_file().await {
            Ok(()) => Err(err.to_string().into()),
            Err(rollback_err) => Err(format!("{err}; failed to restore Verge config file: {rollback_err}").into()),
        };
    }

    verge.apply();
    core::backup::WebDavClient::global().reset();
    Ok(())
}

/// 创建 WebDAV 备份并上传
#[tauri::command]
pub async fn create_webdav_backup() -> CmdResult<()> {
    feat::create_backup_and_upload_webdav().await.stringify_err()
}

/// 列出 WebDAV 上的备份文件
#[tauri::command]
pub async fn list_webdav_backup() -> CmdResult<Vec<ListFile>> {
    feat::list_wevdav_backup().await.stringify_err()
}

/// 删除 WebDAV 上的备份文件
#[tauri::command]
pub async fn delete_webdav_backup(filename: String) -> CmdResult<()> {
    feat::delete_webdav_backup(filename).await.stringify_err()
}

/// 从 WebDAV 恢复备份文件
#[tauri::command]
pub async fn restore_webdav_backup(filename: String) -> CmdResult<()> {
    feat::restore_webdav_backup(filename).await.stringify_err()
}
