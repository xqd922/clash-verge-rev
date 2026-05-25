use super::{CoreManager, RunningMode};
use crate::config::{Config, IVerge};
use crate::core::handle::Handle;
use crate::core::manager::CLASH_LOGGER;
use crate::core::service::{SERVICE_MANAGER, ServiceStatus};
use crate::utils::dirs;
use anyhow::Result;
use clash_verge_draft::{Draft, SharedDraft};
use clash_verge_logging::{Type, logging};
use scopeguard::defer;
use smartstring::alias::String;
use std::{
    future::Future,
    time::{Duration, Instant},
};
use tauri_plugin_clash_verge_sysinfo;

async fn prepare_core_change<Generate, GenerateFuture, Save, SaveFuture>(
    verge: Draft<IVerge>,
    clash_core: &String,
    generate: Generate,
    save: Save,
) -> std::result::Result<(), String>
where
    Generate: FnOnce() -> GenerateFuture,
    GenerateFuture: Future<Output = std::result::Result<(), String>>,
    Save: FnOnce(SharedDraft<IVerge>) -> SaveFuture,
    SaveFuture: Future<Output = std::result::Result<(), String>>,
{
    verge.edit_draft(|d| {
        d.clash_core = Some(clash_core.to_owned());
    });

    let result = async {
        generate().await?;
        save(verge.latest_arc()).await?;
        Ok(())
    }
    .await;

    match result {
        Ok(()) => {
            verge.apply();
            Ok(())
        }
        Err(err) => {
            verge.discard();
            Err(err)
        }
    }
}

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

        // Only generate config for the new core, don't try to reload on the
        // currently running (old) core — it may reject config types that only
        // the new core supports. The caller will restart_core() afterwards.
        let verge = Config::verge().await;
        let result = prepare_core_change(
            verge,
            clash_core,
            || async { Config::generate().await.map_err(|e| e.to_string().into()) },
            |verge_data| async move { verge_data.save_file().await.map_err(|e| e.to_string().into()) },
        )
        .await;

        if result.is_err() {
            Config::runtime().await.discard();
        }

        result?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use clash_verge_draft::Draft;
    use parking_lot::Mutex;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    #[tokio::test]
    async fn prepare_core_change_discards_draft_when_generate_fails() {
        let verge = Draft::new(IVerge {
            clash_core: Some("verge-mihomo".into()),
            ..IVerge::default()
        });
        let save_called = Arc::new(AtomicBool::new(false));
        let save_called_for_closure = Arc::clone(&save_called);

        let result = prepare_core_change(
            verge.clone(),
            &"verge-mihomo-smart".into(),
            || async { Err("generate failed".into()) },
            move |_| {
                let save_called = Arc::clone(&save_called_for_closure);
                async move {
                    save_called.store(true, Ordering::SeqCst);
                    Ok(())
                }
            },
        )
        .await;

        assert_eq!(result, Err("generate failed".into()));
        assert!(!save_called.load(Ordering::SeqCst));
        assert_eq!(verge.data_arc().clash_core.as_deref(), Some("verge-mihomo"));
        assert_eq!(verge.latest_arc().clash_core.as_deref(), Some("verge-mihomo"));
    }

    #[tokio::test]
    async fn prepare_core_change_discards_draft_when_save_fails() {
        let verge = Draft::new(IVerge {
            clash_core: Some("verge-mihomo".into()),
            ..IVerge::default()
        });

        let result = prepare_core_change(
            verge.clone(),
            &"verge-mihomo-smart".into(),
            || async { Ok(()) },
            |_| async { Err("save failed".into()) },
        )
        .await;

        assert_eq!(result, Err("save failed".into()));
        assert_eq!(verge.data_arc().clash_core.as_deref(), Some("verge-mihomo"));
        assert_eq!(verge.latest_arc().clash_core.as_deref(), Some("verge-mihomo"));
    }

    #[tokio::test]
    async fn prepare_core_change_applies_draft_after_generate_and_save_succeed() {
        let verge = Draft::new(IVerge {
            clash_core: Some("verge-mihomo".into()),
            ..IVerge::default()
        });
        let saved_core = Arc::new(Mutex::new(None));
        let saved_core_for_closure = Arc::clone(&saved_core);

        let result = prepare_core_change(
            verge.clone(),
            &"verge-mihomo-smart".into(),
            || async { Ok(()) },
            move |config| {
                let saved_core = Arc::clone(&saved_core_for_closure);
                async move {
                    *saved_core.lock() = config.clash_core.clone();
                    Ok(())
                }
            },
        )
        .await;

        assert_eq!(result, Ok(()));
        assert_eq!(verge.data_arc().clash_core.as_deref(), Some("verge-mihomo-smart"));
        assert_eq!(saved_core.lock().as_deref(), Some("verge-mihomo-smart"));
    }
}
