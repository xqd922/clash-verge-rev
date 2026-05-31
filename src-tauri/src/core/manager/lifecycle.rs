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

impl CoreManager {
    pub async fn start_core(&self) -> Result<()> {
        self.prepare_startup().await?;
        defer! {
            self.after_core_process();
        }

        match *self.get_running_mode() {
            RunningMode::Service => self.start_core_by_service().await?,
            RunningMode::NotRunning | RunningMode::Sidecar => self.start_core_by_sidecar().await?,
        }

        self.wait_for_core_ready().await
    }

    pub async fn stop_core(&self) -> Result<()> {
        CLASH_LOGGER.clear_logs().await;
        defer! {
            self.after_core_process();
        }

        match *self.get_running_mode() {
            RunningMode::Service => self.stop_core_by_service().await,
            RunningMode::Sidecar => {
                self.stop_core_by_sidecar().await;
                Ok(())
            }
            RunningMode::NotRunning => Ok(()),
        }
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

    fn after_core_process(&self) {
        let app_handle = Handle::app_handle();
        tauri_plugin_clash_verge_sysinfo::set_app_core_mode(app_handle, self.get_running_mode().to_string());
    }

    /// Wait for the core IPC pipe/socket to become available after startup.
    /// This prevents race conditions where the frontend tries to connect
    /// before the core process has finished initializing (especially for
    /// Smart core which needs extra time for LightGBM model loading).
    async fn wait_for_core_ready(&self) -> Result<()> {
        let ipc = match dirs::ipc_path() {
            Ok(p) => p,
            Err(err) => return Err(err),
        };
        let path_str = match dirs::path_to_str(&ipc) {
            Ok(s) => s.to_owned(),
            Err(err) => return Err(err),
        };

        let max_wait = Duration::from_secs(10);
        let interval = Duration::from_millis(100);
        let start = Instant::now();

        loop {
            let p = path_str.clone();
            let connected = tokio::task::spawn_blocking(move || std::fs::File::open(p).is_ok())
                .await
                .unwrap_or(false);

            if connected {
                logging!(
                    info,
                    Type::Core,
                    "Core IPC ready after {}ms",
                    start.elapsed().as_millis()
                );
                return Ok(());
            }

            if start.elapsed() >= max_wait {
                logging!(
                    warn,
                    Type::Core,
                    "Core IPC not ready after {}ms, treating startup as failed",
                    start.elapsed().as_millis()
                );
                return Err(anyhow::anyhow!(
                    "Core IPC not ready after {}ms",
                    start.elapsed().as_millis()
                ));
            }

            tokio::time::sleep(interval).await;
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

fn apply_core_change_to_draft(d: &mut IVerge, clash_core: &str) {
    d.clash_core = Some(clash_core.into());
    d.enable_smart_convert = Some(clash_core == "verge-mihomo-smart");
}

#[cfg(test)]
mod tests {
    use super::apply_core_change_to_draft;
    use crate::config::IVerge;

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
}
