use super::{CoreManager, RunningMode};
use crate::config::{Config, IVerge};
use crate::core::handle::Handle;
use crate::core::manager::CLASH_LOGGER;
use crate::core::service::{SERVICE_MANAGER, ServiceStatus};
use crate::utils::dirs;
use anyhow::Result;
use clash_verge_logging::{Type, logging};
use scopeguard::defer;
use smartstring::alias::String;
use std::time::{Duration, Instant};
use tauri_plugin_clash_verge_sysinfo;

const CORE_READY_MAX_WAIT: Duration = Duration::from_secs(20);
const SMART_CORE_READY_MAX_WAIT: Duration = Duration::from_secs(180);
const CORE_READY_POLL_INTERVAL: Duration = Duration::from_millis(100);
const CORE_STARTUP_LOG_TAIL_LINES: usize = 8;
const SMART_CORE_NAME: &str = "verge-mihomo-smart";
const SMART_MODEL_FILE: &str = "Model.bin";
const SMART_MODEL_MIN_BYTES: u64 = 4 * 1024 * 1024;

impl CoreManager {
    pub async fn start_core(&self) -> Result<()> {
        self.prepare_startup().await?;
        self.clear_core_ipc_pool().await;
        defer! {
            self.after_core_process();
        }

        match *self.get_running_mode() {
            RunningMode::Service => self.start_core_by_service().await?,
            RunningMode::NotRunning | RunningMode::Sidecar => self.start_core_by_sidecar().await?,
        }

        self.clear_core_ipc_pool().await;
        self.wait_for_core_ready().await
    }

    pub async fn stop_core(&self) -> Result<()> {
        CLASH_LOGGER.clear_logs().await;
        self.clear_core_ipc_pool().await;
        defer! {
            self.after_core_process();
        }

        let result = match *self.get_running_mode() {
            RunningMode::Service => self.stop_core_by_service().await,
            RunningMode::Sidecar => {
                self.stop_core_by_sidecar().await;
                Ok(())
            }
            RunningMode::NotRunning => Ok(()),
        };
        self.clear_core_ipc_pool().await;
        result
    }

    pub async fn restart_core(&self) -> Result<()> {
        logging!(info, Type::Core, "Restarting core");
        self.stop_core().await?;
        self.start_core().await
    }

    pub async fn change_core(&self, clash_core: &String) -> Result<(), String> {
        if !IVerge::VALID_CLASH_CORES.contains(&clash_core.as_str()) {
            return Err(format!("Invalid clash core: {}", clash_core).into());
        }

        let verge = Config::verge().await;
        let runtime = Config::runtime().await;
        let current_core = verge.data_arc().get_valid_clash_core();

        if current_core.as_str() == clash_core.as_str() {
            return Ok(());
        }

        verge.edit_draft(|d| {
            apply_core_change_to_draft(d, clash_core.as_str());
        });

        // Generate against the target core while it is still only a draft.
        // The generated runtime and verge.yaml are committed only after the
        // new core starts successfully, so failed switches leave disk state on
        // the previous working core.
        if let Err(err) = Config::generate().await {
            verge.discard();
            runtime.discard();
            return Err(err.to_string().into());
        }

        match self.restart_core().await {
            Ok(()) => {
                if let Err(err) = verge.latest_arc().save_file().await {
                    let switch_error = err.to_string();
                    runtime.discard();
                    verge.discard();

                    let rollback_result = self.restart_core().await;
                    if let Err(rollback_err) = rollback_result {
                        return Err(format!(
                            "Core switch config save failed: {switch_error}; rollback to {current_core} also failed: {rollback_err}"
                        )
                        .into());
                    }

                    return Err(format!(
                        "Core switch config save failed and rolled back to {current_core}: {switch_error}"
                    )
                    .into());
                }

                runtime.apply();
                verge.apply();
                Ok(())
            }
            Err(err) => {
                let switch_error = err.to_string();
                runtime.discard();
                verge.discard();

                let rollback_result = self.restart_core().await;
                if let Err(rollback_err) = rollback_result {
                    return Err(format!(
                        "Core switch failed: {switch_error}; rollback to {current_core} also failed: {rollback_err}"
                    )
                    .into());
                }

                Err(format!("Core switch failed and rolled back to {current_core}: {switch_error}").into())
            }
        }
    }

    async fn prepare_startup(&self) -> Result<()> {
        self.ensure_smart_model_resource().await;

        // Portable mode must always use sidecar to avoid conflicts with
        // a service installed by a non-portable installation (the service
        // would start mihomo with the non-portable home directory).
        if *dirs::PORTABLE_FLAG.get().unwrap_or(&false) {
            logging!(info, Type::Core, "Portable mode: using sidecar");
            self.set_running_mode(RunningMode::Sidecar);
            return Ok(());
        }

        #[cfg(target_os = "windows")]
        self.wait_for_service_if_needed().await;

        let value = SERVICE_MANAGER.lock().await.current();
        let mode = match value {
            ServiceStatus::Ready => RunningMode::Service,
            _ => RunningMode::Sidecar,
        };

        self.set_running_mode(mode);
        Ok(())
    }

    async fn ensure_smart_model_resource(&self) {
        let clash_core = Config::verge().await.latest_arc().get_valid_clash_core();
        if clash_core != SMART_CORE_NAME {
            return;
        }

        let src_path = match dirs::app_resources_dir() {
            Ok(path) => path.join(SMART_MODEL_FILE),
            Err(err) => {
                logging!(warn, Type::Core, "Failed to resolve resource dir: {}", err);
                return;
            }
        };
        if !src_path.exists() {
            logging!(
                warn,
                Type::Core,
                "Bundled Smart model not found: {}",
                src_path.display()
            );
            return;
        }

        let dest_path = match dirs::app_home_dir() {
            Ok(path) => path.join(SMART_MODEL_FILE),
            Err(err) => {
                logging!(warn, Type::Core, "Failed to resolve app home dir: {}", err);
                return;
            }
        };

        let should_copy = match tokio::fs::metadata(&dest_path).await {
            Ok(metadata) => metadata.len() < SMART_MODEL_MIN_BYTES || resource_is_newer(&src_path, &dest_path).await,
            Err(_) => true,
        };

        if !should_copy {
            return;
        }

        if let Some(parent) = dest_path.parent()
            && let Err(err) = tokio::fs::create_dir_all(parent).await
        {
            logging!(
                warn,
                Type::Core,
                "Failed to create Smart model directory {}: {}",
                parent.display(),
                err
            );
            return;
        }

        match tokio::fs::copy(&src_path, &dest_path).await {
            Ok(_) => logging!(
                info,
                Type::Core,
                "Smart model prepared from bundled resource: {}",
                dest_path.display()
            ),
            Err(err) => logging!(
                warn,
                Type::Core,
                "Failed to prepare bundled Smart model from {} to {}: {}",
                src_path.display(),
                dest_path.display(),
                err
            ),
        }
    }

    fn after_core_process(&self) {
        let app_handle = Handle::app_handle();
        tauri_plugin_clash_verge_sysinfo::set_app_core_mode(app_handle, self.get_running_mode().to_string());
    }

    /// Wait for the core IPC pipe/socket to become available after startup.
    /// This prevents race conditions where the frontend tries to connect
    /// before the core process has finished initializing (especially for
    /// Smart core which needs extra time for LightGBM model loading).
    async fn wait_for_core_ready(&self) -> Result<()> {
        let clash_core = Config::verge().await.latest_arc().get_valid_clash_core();
        let max_wait = if clash_core == "verge-mihomo-smart" {
            SMART_CORE_READY_MAX_WAIT
        } else {
            CORE_READY_MAX_WAIT
        };
        let start = Instant::now();
        let mut last_error = None;

        loop {
            if start.elapsed() >= max_wait {
                let elapsed_ms = start.elapsed().as_millis();
                let reason = last_error.unwrap_or_else(|| "unknown error".to_string());
                let reason = append_core_startup_log_tail(reason).await;
                logging!(
                    warn,
                    Type::Core,
                    "Core IPC not ready after {}ms, treating startup as failed: {}",
                    elapsed_ms,
                    reason
                );
                return Err(anyhow::anyhow!("Core IPC not ready after {}ms: {}", elapsed_ms, reason));
            }

            match Handle::mihomo().await.get_version().await {
                Ok(_) => {
                    logging!(
                        info,
                        Type::Core,
                        "Core IPC ready after {}ms",
                        start.elapsed().as_millis()
                    );
                    return Ok(());
                }
                Err(err) => {
                    last_error = Some(err.to_string());
                }
            }

            tokio::time::sleep(CORE_READY_POLL_INTERVAL).await;
        }
    }

    async fn clear_core_ipc_pool(&self) {
        if let Ok(pool) = tauri_plugin_mihomo::IpcConnectionPool::global() {
            pool.clear_pool().await;
        }
    }

    #[cfg(target_os = "windows")]
    async fn wait_for_service_if_needed(&self) {
        use crate::{config::Config, constants::timing, core::service};
        use backon::{ConstantBuilder, Retryable as _};

        let needs_service = Config::verge().await.latest_arc().enable_tun_mode.unwrap_or(false);

        if !needs_service {
            return;
        }

        let max_times = timing::SERVICE_WAIT_MAX.as_millis() / timing::SERVICE_WAIT_INTERVAL.as_millis();
        let backoff = ConstantBuilder::default()
            .with_delay(timing::SERVICE_WAIT_INTERVAL)
            .with_max_times(max_times as usize);

        let _ = (|| async {
            let mut manager = SERVICE_MANAGER.lock().await;

            if matches!(manager.current(), ServiceStatus::Ready) {
                return Ok(());
            }

            // If the service IPC path is not ready yet, treat it as transient and retry.
            // Running init/refresh too early can mark service state unavailable and break later config reloads.
            if !service::is_service_ipc_path_exists() {
                return Err(anyhow::anyhow!("Service IPC not ready"));
            }

            manager.init().await?;
            let _ = manager.refresh().await;

            if matches!(manager.current(), ServiceStatus::Ready) {
                Ok(())
            } else {
                Err(anyhow::anyhow!("Service not ready"))
            }
        })
        .retry(backoff)
        .await;
    }
}

async fn resource_is_newer(src_path: &std::path::Path, dest_path: &std::path::Path) -> bool {
    let src_modified = tokio::fs::metadata(src_path).await.and_then(|m| m.modified());
    let dest_modified = tokio::fs::metadata(dest_path).await.and_then(|m| m.modified());

    matches!((src_modified, dest_modified), (Ok(src), Ok(dest)) if src > dest)
}

async fn append_core_startup_log_tail(reason: std::string::String) -> std::string::String {
    let logs = CLASH_LOGGER.get_logs().await;
    if logs.is_empty() {
        return reason;
    }

    let mut tail = logs
        .iter()
        .rev()
        .take(CORE_STARTUP_LOG_TAIL_LINES)
        .map(|line| line.as_str())
        .collect::<Vec<_>>();
    tail.reverse();

    format!("{}\nRecent core logs:\n{}", reason, tail.join("\n"))
}

fn apply_core_change_to_draft(d: &mut IVerge, clash_core: &str) {
    d.clash_core = Some(clash_core.into());
    d.enable_smart_convert = Some(clash_core == "verge-mihomo-smart");
}

#[cfg(test)]
mod tests {
    use super::apply_core_change_to_draft;
    use crate::config::IVerge;
    use std::time::Duration;

    #[test]
    fn switching_to_smart_core_enables_smart_conversion() {
        let mut verge = IVerge {
            clash_core: Some("verge-mihomo".into()),
            enable_smart_convert: Some(false),
            ..IVerge::default()
        };

        apply_core_change_to_draft(&mut verge, "verge-mihomo-smart");

        assert_eq!(verge.clash_core.as_deref(), Some("verge-mihomo-smart"));
        assert_eq!(verge.enable_smart_convert, Some(true));
    }

    #[test]
    fn switching_to_standard_core_disables_smart_conversion() {
        let mut verge = IVerge {
            clash_core: Some("verge-mihomo-smart".into()),
            enable_smart_convert: Some(true),
            ..IVerge::default()
        };

        apply_core_change_to_draft(&mut verge, "verge-mihomo");

        assert_eq!(verge.clash_core.as_deref(), Some("verge-mihomo"));
        assert_eq!(verge.enable_smart_convert, Some(false));
    }

    #[test]
    fn core_ready_wait_budget_allows_smart_core_startup() {
        assert!(super::CORE_READY_MAX_WAIT >= Duration::from_secs(20));
        assert!(super::SMART_CORE_READY_MAX_WAIT >= Duration::from_secs(180));
    }

    #[test]
    fn smart_model_min_size_rejects_truncated_download() {
        assert!(super::SMART_MODEL_MIN_BYTES > 2 * 1024 * 1024);
    }
}
