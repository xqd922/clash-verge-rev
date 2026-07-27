use crate::{
    config::{Config, ConfigType, IClashTemp},
    core::{CoreManager, handle, tray},
    feat::clean_async,
    process::AsyncHandler,
    utils,
};
use bytes::BytesMut;
use clash_verge_logging::{Type, logging};
use once_cell::sync::Lazy;
use serde_yaml_ng::{Mapping, Value};
use smartstring::alias::String;
use std::sync::Arc;

#[allow(clippy::expect_used)]
static TLS_CONFIG: Lazy<Arc<rustls::ClientConfig>> = Lazy::new(|| {
    let root_store = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .expect("Failed to set TLS versions")
        .with_root_certificates(root_store)
        .with_no_client_auth();
    Arc::new(config)
});

/// Restart the Clash core
pub async fn restart_clash_core() {
    match CoreManager::global().restart_core().await {
        Ok(_) => {
            handle::Handle::refresh_clash();
            handle::Handle::notice_message("set_config::ok", "ok");
        }
        Err(err) => {
            handle::Handle::notice_message("set_config::error", format!("{err}"));
            logging!(error, Type::Core, "{err}");
        }
    }
}

/// Restart the application
pub async fn restart_app() {
    logging!(debug, Type::System, "启动重启应用流程");
    // 设置退出标志
    handle::Handle::global().set_is_exiting();

    utils::server::shutdown_embedded_server();
    Config::apply_all_and_save_file().await;

    logging!(info, Type::System, "开始异步清理资源");
    let cleanup_result = clean_async().await;

    logging!(
        info,
        Type::System,
        "资源清理完成，退出代码: {}",
        if cleanup_result { 0 } else { 1 }
    );

    let app_handle = handle::Handle::app_handle();
    app_handle.restart();
}

fn after_change_clash_mode() {
    AsyncHandler::spawn(move || async {
        let mihomo = handle::Handle::mihomo().await;
        match mihomo.get_connections().await {
            Ok(connections) => {
                if let Some(connections_array) = connections.connections {
                    for connection in connections_array {
                        let _ = mihomo.close_connection(&connection.id).await;
                    }
                    drop(mihomo);
                }
            }
            Err(err) => {
                logging!(error, Type::Core, "Failed to get connections: {err}");
            }
        }
    });
}

async fn restore_clash_mode_files(committed_clash: &IClashTemp) -> Vec<std::string::String> {
    let mut rollback_errors = Vec::new();
    if let Err(rollback_err) = committed_clash.save_config().await {
        rollback_errors.push(format!("failed to restore Clash config file: {rollback_err}"));
    }
    if let Err(rollback_err) = Config::generate_file(ConfigType::Run).await {
        rollback_errors.push(format!("failed to restore runtime config file: {rollback_err}"));
    }
    rollback_errors
}

fn with_rollback_failures(err: anyhow::Error, rollback_errors: Vec<std::string::String>) -> anyhow::Error {
    if rollback_errors.is_empty() {
        err
    } else {
        anyhow::anyhow!("{err}; rollback failed: {}", rollback_errors.join("; "))
    }
}

/// Change Clash mode (rule/global/direct/script)
pub async fn change_clash_mode(mode: String) -> anyhow::Result<()> {
    let manager = CoreManager::global();
    let Some(_config_permit) = manager.try_acquire_config_update() else {
        return Err(anyhow::anyhow!("A configuration update is already running"));
    };

    let previous_mode = {
        let mihomo = handle::Handle::mihomo().await;
        mihomo.get_base_config().await?.mode.to_string()
    };

    let mut mapping = Mapping::new();
    mapping.insert(Value::from("mode"), Value::from(mode.as_str()));
    let json_value = serde_json::json!({ "mode": mode.as_str() });
    logging!(debug, Type::Core, "change clash mode to {mode}");

    let clash = Config::clash().await;
    let runtime = Config::runtime().await;
    let committed_clash = clash.data_arc();

    clash.edit_draft(|draft| draft.patch_config(&mapping));
    let runtime_has_config = runtime.edit_draft(|draft| {
        let has_config = draft.config.is_some();
        draft.patch_config(&mapping);
        has_config
    });
    if !runtime_has_config {
        clash.discard();
        runtime.discard();
        return Err(anyhow::anyhow!("Runtime config is not initialized"));
    }

    if let Err(err) = clash.latest_arc().save_config().await {
        clash.discard();
        runtime.discard();
        return match committed_clash.save_config().await {
            Ok(()) => Err(err),
            Err(rollback_err) => Err(anyhow::anyhow!(
                "{err}; failed to restore Clash config file: {rollback_err}"
            )),
        };
    }

    if let Err(err) = Config::generate_file(ConfigType::Run).await {
        clash.discard();
        runtime.discard();
        let rollback_errors = restore_clash_mode_files(&committed_clash).await;
        return Err(with_rollback_failures(err, rollback_errors));
    }

    if let Err(err) = handle::Handle::mihomo().await.patch_base_config(&json_value).await {
        clash.discard();
        runtime.discard();

        let mut rollback_errors = Vec::new();
        let rollback_json = serde_json::json!({ "mode": previous_mode });
        if let Err(rollback_err) = handle::Handle::mihomo().await.patch_base_config(&rollback_json).await {
            rollback_errors.push(format!("failed to restore core mode: {rollback_err}"));
        }
        rollback_errors.extend(restore_clash_mode_files(&committed_clash).await);

        logging!(error, Type::Core, "{err}");
        return Err(with_rollback_failures(anyhow::anyhow!("{err}"), rollback_errors));
    }

    clash.apply();
    runtime.apply();
    handle::Handle::refresh_clash();
    tray::Tray::global().update_menu_and_icon().await;

    let is_auto_close_connection = Config::verge().await.data_arc().auto_close_connection.unwrap_or(false);
    if is_auto_close_connection {
        after_change_clash_mode();
    }
    Ok(())
}

/// Test delay to a URL through proxy.
/// HTTPS: measures TLS handshake time. HTTP: measures HEAD round-trip time.
pub async fn test_delay(url: String) -> anyhow::Result<u32> {
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio::net::TcpStream;
    use tokio::time::Instant;

    let parsed = tauri::Url::parse(&url)?;
    let is_https = parsed.scheme() == "https";
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid URL: no host"))?
        .to_string();
    let port = parsed.port().unwrap_or(if is_https { 443 } else { 80 });

    let verge = Config::verge().await.latest_arc();
    let proxy_enabled = verge.enable_system_proxy.unwrap_or(false) || verge.enable_tun_mode.unwrap_or(false);
    let proxy_port = if proxy_enabled {
        Some(match verge.verge_mixed_port {
            Some(p) => p,
            None => Config::clash().await.data_arc().get_mixed_port(),
        })
    } else {
        None
    };

    tokio::time::timeout(Duration::from_secs(10), async {
        let start = Instant::now();
        let mut buf = BytesMut::with_capacity(1024);

        if is_https {
            let stream = match proxy_port {
                Some(pp) => {
                    let mut s = TcpStream::connect(format!("127.0.0.1:{pp}")).await?;
                    s.write_all(format!("CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\n\r\n").as_bytes())
                        .await?;
                    s.read_buf(&mut buf).await?;
                    if !buf.windows(3).any(|w| w == b"200") {
                        return Err(anyhow::anyhow!("Proxy CONNECT failed"));
                    }
                    s
                }
                None => TcpStream::connect(format!("{host}:{port}")).await?,
            };
            let connector = tokio_rustls::TlsConnector::from(Arc::clone(&TLS_CONFIG));
            let server_name = rustls::pki_types::ServerName::try_from(host.as_str())
                .map_err(|_| anyhow::anyhow!("Invalid DNS name: {host}"))?
                .to_owned();
            connector.connect(server_name, stream).await?;
        } else {
            let (mut stream, req) = match proxy_port {
                Some(pp) => (
                    TcpStream::connect(format!("127.0.0.1:{pp}")).await?,
                    format!("HEAD {url} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"),
                ),
                None => (
                    TcpStream::connect(format!("{host}:{port}")).await?,
                    format!("HEAD / HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"),
                ),
            };
            stream.write_all(req.as_bytes()).await?;
            let _ = stream.read(&mut buf).await?;
        }

        // frontend treats 0 as timeout
        Ok((start.elapsed().as_millis() as u32).max(1))
    })
    .await
    .unwrap_or(Ok(10000u32))
}
