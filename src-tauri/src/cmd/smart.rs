use super::{CmdResult, CommandFailure};
use crate::core::handle::Handle;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::Method;
use tauri_plugin_mihomo::models::ErrorResponse;

/// Smart 核心专属接口：权重排行与缓存清理。
/// 通过 mihomo 插件的公开传输层发请求，HTTP / 本地 socket 两种模式均兼容。

#[tauri::command]
pub async fn get_smart_weights(group_name: String) -> CmdResult<serde_json::Value> {
    let group_name_encode = utf8_percent_encode(&group_name, NON_ALPHANUMERIC);
    let mihomo = Handle::mihomo();
    let response = mihomo
        .load_ctx()
        .build_request(Method::GET, &format!("/group/{group_name_encode}/weights"))
        .map_err(CommandFailure::plain)?
        .send()
        .await
        .map_err(|e| CommandFailure::plain(e.to_string()))?;
    if !response.status().is_success() {
        let err_msg = response.json::<ErrorResponse>().await.map_or_else(
            |e| format!("get smart weights for group[{group_name}] failed, {e}"),
            |err_res| err_res.message,
        );
        return Err(CommandFailure::plain(err_msg));
    }
    response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| CommandFailure::plain(e.to_string()))
}

#[tauri::command]
pub async fn flush_smart_cache() -> CmdResult<()> {
    let mihomo = Handle::mihomo();
    let response = mihomo
        .load_ctx()
        .build_request(Method::POST, "/cache/smart/flush")
        .map_err(CommandFailure::plain)?
        .send()
        .await
        .map_err(|e| CommandFailure::plain(e.to_string()))?;
    if !response.status().is_success() {
        let err_msg = response
            .json::<ErrorResponse>()
            .await
            .map_or_else(|e| format!("flush smart cache failed, {e}"), |err_res| err_res.message);
        return Err(CommandFailure::plain(err_msg));
    }
    Ok(())
}
