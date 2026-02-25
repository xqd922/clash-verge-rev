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

        self.wait_for_core_ready().await;
        Ok(())
    }

    pub async fn stop_core(&self) -> Result<()> {
        CLASH_LOGGER.clear_logs().await;
        defer! {
            self.after_core_process();
        }

        match *self.get_running_mode() {
            RunningMode::Service => self.stop_core_by_service().await,
            RunningMode::Sidecar => {
                self.stop_core_by_sidecar();
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

        Config::verge().await.edit_draft(|d| {
            d.clash_core = Some(clash_core.to_owned());
        });
        Config::verge().await.apply();

        let verge_data = Config::verge().await.latest_arc();
        verge_data.save_file().await.map_err(|e| e.to_string())?;

        // Only generate config for the new core, don't try to reload on the
        // currently running (old) core — it may reject config types that only
        // the new core supports. The caller will restart_core() afterwards.
        Config::generate().await.map_err(|e| e.to_string())?;
        Config::runtime().await.apply();
        Ok(())
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
    async fn wait_for_core_ready(&self) {
        let ipc = match dirs::ipc_path() {
            Ok(p) => p,
            Err(_) => return,
        };
        let path_str = match dirs::path_to_str(&ipc) {
            Ok(s) => s.to_owned(),
            Err(_) => return,
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
                return;
            }

            if start.elapsed() >= max_wait {
                logging!(
                    warn,
                    Type::Core,
                    "Core IPC not ready after {}ms, proceeding anyway",
                    start.elapsed().as_millis()
                );
                return;
            }

            tokio::time::sleep(interval).await;
        }
    }

    #[cfg(target_os = "windows")]
    async fn wait_for_service_if_needed(&self) {
        use crate::{config::Config, constants::timing};
        use backoff::{Error as BackoffError, ExponentialBackoff};

        let needs_service = Config::verge().await.latest_arc().enable_tun_mode.unwrap_or(false);

        if !needs_service {
            return;
        }

        let backoff = ExponentialBackoff {
            initial_interval: timing::SERVICE_WAIT_INTERVAL,
            max_interval: timing::SERVICE_WAIT_INTERVAL,
            max_elapsed_time: Some(timing::SERVICE_WAIT_MAX),
            multiplier: 1.0,
            randomization_factor: 0.0,
            ..Default::default()
        };

        let operation = || async {
            let mut manager = SERVICE_MANAGER.lock().await;

            if matches!(manager.current(), ServiceStatus::Ready) {
                return Ok(());
            }

            manager.init().await.map_err(BackoffError::transient)?;
            let _ = manager.refresh().await;

            if matches!(manager.current(), ServiceStatus::Ready) {
                Ok(())
            } else {
                Err(BackoffError::transient(anyhow::anyhow!("Service not ready")))
            }
        };

        let _ = backoff::future::retry(backoff, operation).await;
    }
}
