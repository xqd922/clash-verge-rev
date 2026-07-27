use super::{ConfigUpdatePermit, CoreManager, RunningMode};
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
#[cfg(target_os = "windows")]
use tauri_plugin_clash_verge_sysinfo::is_current_app_handle_admin;

#[cfg(any(target_os = "windows", test))]
const fn should_wait_for_service(tun_enabled: bool, service_ready: bool, is_admin: bool) -> bool {
    tun_enabled && !service_ready && !is_admin
}

#[cfg(target_os = "windows")]
enum HandoffOutcome {
    NotReady,
    Done,
    Failed,
}

const CORE_READY_MAX_WAIT: Duration = Duration::from_secs(20);
const SMART_CORE_READY_MAX_WAIT: Duration = Duration::from_secs(180);
const CORE_READY_POLL_INTERVAL: Duration = Duration::from_millis(100);
const CORE_STARTUP_LOG_TAIL_LINES: usize = 8;
const SMART_CORE_NAME: &str = "verge-mihomo-smart";
const SMART_MODEL_FILE: &str = "Model.bin";
const SMART_MODEL_MIN_BYTES: u64 = 4 * 1024 * 1024;

#[cfg(any(target_os = "windows", test))]
const fn sidecar_fallback_is_safe(service_was_running: bool, service_stop_succeeded: bool) -> bool {
    !service_was_running || service_stop_succeeded
}

impl CoreManager {
    pub async fn start_core(&self) -> Result<()> {
        let Some(config_permit) = self.try_acquire_config_update() else {
            anyhow::bail!("configuration update is already running");
        };
        self.start_core_with_permit(&config_permit).await
    }

    pub(crate) async fn start_core_with_permit(&self, _permit: &ConfigUpdatePermit<'_>) -> Result<()> {
        let _life = self.lifecycle_lock.lock().await;
        self.start_core_inner().await
    }

    async fn start_core_inner(&self) -> Result<()> {
        if Handle::global().is_exiting() {
            return Ok(());
        }

        if !matches!(*self.get_running_mode(), RunningMode::NotRunning) {
            logging!(
                info,
                Type::Core,
                "start_core called while a core is running; treated as no-op"
            );
            return Ok(());
        }

        self.prepare_startup().await;
        self.clear_core_ipc_pool();
        defer! {
            self.after_core_process();
        }

        if Handle::global().is_exiting() {
            self.set_running_mode(RunningMode::NotRunning);
            return Ok(());
        }

        let result = match *self.get_running_mode() {
            RunningMode::Service => self.start_core_by_service().await,
            RunningMode::NotRunning | RunningMode::Sidecar => self.start_core_by_sidecar().await,
        };
        if let Err(err) = result {
            if !matches!(*self.get_running_mode(), RunningMode::Service) {
                self.set_running_mode(RunningMode::NotRunning);
            }
            return Err(err);
        }

        self.clear_core_ipc_pool();
        if let Err(err) = self.wait_for_core_ready().await {
            let stop_result = match *self.get_running_mode() {
                RunningMode::Service => self.stop_core_by_service().await,
                RunningMode::Sidecar => self.stop_core_by_sidecar().await,
                RunningMode::NotRunning => Ok(()),
            };
            return match stop_result {
                Ok(()) => Err(err),
                Err(stop_err) => Err(anyhow::anyhow!(
                    "core did not become ready: {err}; failed to stop service core: {stop_err}"
                )),
            };
        }

        #[cfg(target_os = "windows")]
        if matches!(*self.get_running_mode(), RunningMode::Sidecar) {
            self.spawn_service_handoff_watcher().await;
        }

        Ok(())
    }

    pub async fn stop_core(&self) -> Result<()> {
        let Some(config_permit) = self.try_acquire_config_update() else {
            anyhow::bail!("configuration update is already running");
        };
        self.stop_core_with_permit(&config_permit).await
    }

    pub(crate) async fn stop_core_with_permit(&self, _permit: &ConfigUpdatePermit<'_>) -> Result<()> {
        let _life = self.lifecycle_lock.lock().await;
        self.stop_core_inner().await
    }

    async fn stop_core_inner(&self) -> Result<()> {
        CLASH_LOGGER.clear_logs().await;
        self.clear_core_ipc_pool();
        defer! {
            self.after_core_process();
        }

        let result = match *self.get_running_mode() {
            RunningMode::Service => self.stop_core_by_service().await,
            RunningMode::Sidecar => self.stop_core_by_sidecar().await,
            RunningMode::NotRunning => Ok(()),
        };
        self.clear_core_ipc_pool();
        result
    }

    pub async fn restart_core(&self) -> Result<()> {
        let Some(permit) = self.try_acquire_config_update() else {
            anyhow::bail!("configuration update is already running");
        };
        self.restart_core_with_permit(&permit).await
    }

    pub(crate) async fn restart_core_with_permit(&self, _permit: &ConfigUpdatePermit<'_>) -> Result<()> {
        let _life = self.lifecycle_lock.lock().await;
        logging!(info, Type::Core, "Restarting core");
        self.stop_core_inner().await?;
        self.start_core_inner().await
    }

    pub async fn handle_service_operation(&'static self, status: ServiceStatus) -> Result<()> {
        tokio::spawn(async move { self.handle_service_operation_inner(status).await })
            .await
            .map_err(|err| anyhow::anyhow!("service operation task failed: {err}"))?
    }

    async fn handle_service_operation_inner(&self, status: ServiceStatus) -> Result<()> {
        let Some(_config_permit) = self.try_acquire_config_update() else {
            anyhow::bail!("configuration update is already running");
        };
        let _life = self.lifecycle_lock.lock().await;
        let restart_previous_core = matches!(*self.get_running_mode(), RunningMode::Service);

        if restart_previous_core {
            self.stop_core_inner()
                .await
                .map_err(|err| anyhow::anyhow!("failed to stop the service core before service operation: {err}"))?;
        }

        let operation_result = SERVICE_MANAGER.handle_service_status(status).await;
        if !restart_previous_core {
            return operation_result;
        }

        let restart_result = self.start_core_inner().await;
        match (operation_result, restart_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(operation_err), Ok(())) => Err(operation_err),
            (Ok(()), Err(restart_err)) => Err(anyhow::anyhow!(
                "service operation completed, but restarting the core failed: {restart_err}"
            )),
            (Err(operation_err), Err(restart_err)) => Err(anyhow::anyhow!(
                "service operation failed: {operation_err}; restarting the previous core also failed: {restart_err}"
            )),
        }
    }

    pub async fn change_core(&self, clash_core: &String) -> Result<(), String> {
        if !IVerge::VALID_CLASH_CORES.contains(&clash_core.as_str()) {
            return Err(format!("Invalid clash core: {}", clash_core).into());
        }
        let Some(config_permit) = self.try_acquire_config_update() else {
            return Err("A configuration update is already running".into());
        };

        let verge = Config::verge().await;
        let runtime = Config::runtime().await;
        let committed_verge = verge.data_arc();
        let current_core = committed_verge.get_valid_clash_core();

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

        match self.restart_core_with_permit(&config_permit).await {
            Ok(()) => {
                if let Err(err) = verge.latest_arc().save_file().await {
                    let switch_error = err.to_string();
                    runtime.discard();
                    verge.discard();

                    let disk_rollback_result = committed_verge.save_file().await;
                    let rollback_result = self.restart_core_with_permit(&config_permit).await;
                    match (disk_rollback_result, rollback_result) {
                        (Ok(()), Ok(())) => {}
                        (Err(disk_err), Ok(())) => {
                            return Err(format!(
                                "Core switch config save failed: {switch_error}; core rolled back to {current_core}, but restoring verge.yaml failed: {disk_err}"
                            )
                            .into());
                        }
                        (Ok(()), Err(rollback_err)) => {
                            return Err(format!(
                                "Core switch config save failed: {switch_error}; rollback to {current_core} also failed: {rollback_err}"
                            )
                            .into());
                        }
                        (Err(disk_err), Err(rollback_err)) => {
                            return Err(format!(
                                "Core switch config save failed: {switch_error}; restoring verge.yaml failed: {disk_err}; rollback to {current_core} also failed: {rollback_err}"
                            )
                            .into());
                        }
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

                let rollback_result = self.restart_core_with_permit(&config_permit).await;
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

    async fn prepare_startup(&self) {
        self.ensure_smart_model_resource().await;

        // Portable mode must always use sidecar to avoid conflicts with
        // a service installed by a non-portable installation (the service
        // would start mihomo with the non-portable home directory).
        if *dirs::PORTABLE_FLAG.get().unwrap_or(&false) {
            logging!(info, Type::Core, "Portable mode: using sidecar");
            self.set_running_mode(RunningMode::Sidecar);
            return;
        }

        #[cfg(target_os = "windows")]
        self.wait_for_service_if_needed().await;

        let mode = match SERVICE_MANAGER.current().await {
            ServiceStatus::Ready => RunningMode::Service,
            _ => RunningMode::Sidecar,
        };

        self.set_running_mode(mode);
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
        let Some(src_metadata) = smart_model_source_metadata(&src_path).await else {
            return;
        };

        let dest_path = match dirs::app_home_dir() {
            Ok(path) => path.join(SMART_MODEL_FILE),
            Err(err) => {
                logging!(warn, Type::Core, "Failed to resolve app home dir: {}", err);
                return;
            }
        };

        let dest_metadata = tokio::fs::metadata(&dest_path).await.ok();
        let should_copy = smart_model_copy_required(
            src_metadata.len(),
            dest_metadata
                .as_ref()
                .filter(|metadata| metadata.is_file())
                .map(std::fs::Metadata::len),
            resource_is_newer(&src_path, &dest_path).await,
        );

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

        let temp_path = dest_path.with_file_name(format!("{SMART_MODEL_FILE}.tmp-{}", std::process::id()));
        let copy_result = tokio::fs::copy(&src_path, &temp_path).await;
        let copied_bytes = match copy_result {
            Ok(copied_bytes) if copied_bytes == src_metadata.len() => copied_bytes,
            Ok(copied_bytes) => {
                let _ = tokio::fs::remove_file(&temp_path).await;
                logging!(
                    warn,
                    Type::Core,
                    "Incomplete Smart model copy (expected {} bytes, copied {}): {}",
                    src_metadata.len(),
                    copied_bytes,
                    temp_path.display()
                );
                return;
            }
            Err(err) => {
                let _ = tokio::fs::remove_file(&temp_path).await;
                logging!(
                    warn,
                    Type::Core,
                    "Failed to stage bundled Smart model from {} to {}: {}",
                    src_path.display(),
                    temp_path.display(),
                    err
                );
                return;
            }
        };

        match replace_file_atomically(&temp_path, &dest_path).await {
            Ok(()) => logging!(
                info,
                Type::Core,
                "Smart model prepared from bundled resource ({} bytes): {}",
                copied_bytes,
                dest_path.display()
            ),
            Err(err) => {
                let _ = tokio::fs::remove_file(&temp_path).await;
                logging!(
                    warn,
                    Type::Core,
                    "Failed to replace Smart model with staged file {} -> {}: {}",
                    temp_path.display(),
                    dest_path.display(),
                    err
                )
            }
        }
    }

    pub(super) fn after_core_process(&self) {
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

    pub(super) fn clear_core_ipc_pool(&self) {
        if let Ok(pool) = tauri_plugin_mihomo::IpcConnectionPool::global() {
            pool.clear_pool();
        }
    }

    #[cfg(target_os = "windows")]
    async fn wait_for_service_if_needed(&self) {
        use crate::{config::Config, constants::timing, core::service};
        use backon::{ConstantBuilder, Retryable as _};

        let tun_enabled = Config::verge().await.latest_arc().enable_tun_mode.unwrap_or(false);
        let service_ready = matches!(SERVICE_MANAGER.current().await, ServiceStatus::Ready);
        let is_admin = is_current_app_handle_admin(Handle::app_handle());

        if !should_wait_for_service(tun_enabled, service_ready, is_admin) {
            if tun_enabled && !service_ready && is_admin {
                logging!(
                    info,
                    Type::Core,
                    "service unavailable while app is elevated; starting sidecar immediately"
                );
            }
            return;
        }

        let max_times = timing::SERVICE_WAIT_MAX.as_millis() / timing::SERVICE_WAIT_INTERVAL.as_millis();
        let backoff = ConstantBuilder::default()
            .with_delay(timing::SERVICE_WAIT_INTERVAL)
            .with_max_times(max_times as usize);

        let _ = (|| async {
            if matches!(SERVICE_MANAGER.current().await, ServiceStatus::Ready) {
                return Ok(());
            }

            // If the service IPC path is not ready yet, treat it as transient and retry.
            // Running init/refresh too early can mark service state unavailable and break later config reloads.
            if !service::is_service_ipc_path_exists() {
                return Err(anyhow::anyhow!("Service IPC not ready"));
            }

            SERVICE_MANAGER.init().await?;
            let _ = SERVICE_MANAGER.refresh().await;

            if matches!(SERVICE_MANAGER.current().await, ServiceStatus::Ready) {
                Ok(())
            } else {
                Err(anyhow::anyhow!("Service not ready"))
            }
        })
        .retry(backoff)
        .await;
    }

    #[cfg(target_os = "windows")]
    async fn spawn_service_handoff_watcher(&self) {
        use crate::constants::timing;
        use crate::process::AsyncHandler;
        use std::sync::atomic::Ordering;

        if *dirs::PORTABLE_FLAG.get().unwrap_or(&false) {
            return;
        }

        let needs_service = Config::verge().await.latest_arc().enable_tun_mode.unwrap_or(false);
        if !needs_service || self.handoff_watcher_running.swap(true, Ordering::AcqRel) {
            return;
        }

        logging!(
            info,
            Type::Core,
            "service not ready at startup; sidecar active, watching for handoff"
        );

        AsyncHandler::spawn(|| async move {
            let manager = Self::global();
            let started = Instant::now();
            loop {
                if started.elapsed() >= timing::SERVICE_HANDOFF_WINDOW {
                    logging!(
                        info,
                        Type::Core,
                        "service handoff window elapsed; staying in sidecar mode"
                    );
                    break;
                }
                tokio::time::sleep(timing::SERVICE_HANDOFF_INTERVAL).await;

                if !matches!(*manager.get_running_mode(), RunningMode::Sidecar) {
                    break;
                }
                match manager.try_handoff_sidecar_to_service().await {
                    HandoffOutcome::Done => break,
                    HandoffOutcome::Failed => {
                        logging!(warn, Type::Core, "handoff attempt failed; automatic handoff stopped");
                        break;
                    }
                    HandoffOutcome::NotReady => {}
                }
            }
            manager.handoff_watcher_running.store(false, Ordering::Release);
        });
    }

    #[cfg(target_os = "windows")]
    async fn try_handoff_sidecar_to_service(&self) -> HandoffOutcome {
        use crate::core::service;

        if !service::is_service_ipc_path_exists() || SERVICE_MANAGER.init().await.is_err() {
            return HandoffOutcome::NotReady;
        }
        let _ = SERVICE_MANAGER.refresh().await;
        if !matches!(SERVICE_MANAGER.current().await, ServiceStatus::Ready) {
            return HandoffOutcome::NotReady;
        }

        let Some(_config_permit) = self.try_acquire_config_update() else {
            return HandoffOutcome::NotReady;
        };

        let _life = self.lifecycle_lock.lock().await;
        if !matches!(*self.get_running_mode(), RunningMode::Sidecar)
            || !Config::verge().await.latest_arc().enable_tun_mode.unwrap_or(false)
            || *dirs::PORTABLE_FLAG.get().unwrap_or(&false)
        {
            return HandoffOutcome::Done;
        }

        logging!(
            info,
            Type::Core,
            "service became ready; handing off from sidecar to service"
        );
        let config_file = match Config::generate_file(crate::config::ConfigType::Run).await {
            Ok(path) => path,
            Err(err) => {
                logging!(
                    error,
                    Type::Core,
                    "failed to prepare config for service handoff: {}",
                    err
                );
                return HandoffOutcome::Failed;
            }
        };
        if let Err(stop_err) = self.stop_core_by_sidecar().await {
            logging!(
                error,
                Type::Core,
                "failed to stop sidecar for service handoff: {}",
                stop_err
            );
            return HandoffOutcome::Failed;
        }
        self.clear_core_ipc_pool();

        let service_result = match self.start_core_by_service_with_config(&config_file).await {
            Ok(()) => self.wait_for_core_ready().await,
            Err(err) => Err(err),
        };
        if service_result.is_ok() {
            self.after_core_process();
            logging!(info, Type::Core, "handoff to service mode succeeded");
            return HandoffOutcome::Done;
        }

        let Err(err) = service_result else {
            return HandoffOutcome::Done;
        };
        logging!(
            error,
            Type::Core,
            "handoff to service failed: {}; attempting sidecar rollback",
            err
        );
        self.rollback_failed_service_handoff().await
    }

    #[cfg(target_os = "windows")]
    async fn rollback_failed_service_handoff(&self) -> HandoffOutcome {
        let service_was_running = matches!(*self.get_running_mode(), RunningMode::Service);
        let service_stop_succeeded = if service_was_running {
            match self.stop_core_by_service().await {
                Ok(()) => true,
                Err(stop_err) => {
                    logging!(
                        error,
                        Type::Core,
                        "cannot safely restart sidecar because the service core could not be stopped: {}",
                        stop_err
                    );
                    false
                }
            }
        } else {
            true
        };
        if !sidecar_fallback_is_safe(service_was_running, service_stop_succeeded) {
            self.clear_core_ipc_pool();
            self.after_core_process();
            return HandoffOutcome::Failed;
        }
        self.clear_core_ipc_pool();
        if let Err(sidecar_err) = self.start_core_by_sidecar().await {
            logging!(
                error,
                Type::Core,
                "failed to restart sidecar after handoff failure: {}",
                sidecar_err
            );
        } else if let Err(ready_err) = self.wait_for_core_ready().await {
            logging!(
                error,
                Type::Core,
                "sidecar did not become ready after handoff failure: {}",
                ready_err
            );
            if let Err(stop_err) = self.stop_core_by_sidecar().await {
                logging!(
                    error,
                    Type::Core,
                    "failed to stop unready sidecar after handoff rollback: {}",
                    stop_err
                );
            }
            self.clear_core_ipc_pool();
        }
        self.after_core_process();
        HandoffOutcome::Failed
    }
}

async fn smart_model_source_metadata(src_path: &std::path::Path) -> Option<std::fs::Metadata> {
    match tokio::fs::metadata(src_path).await {
        Ok(metadata) if metadata.is_file() && metadata.len() >= SMART_MODEL_MIN_BYTES => Some(metadata),
        Ok(metadata) => {
            logging!(
                warn,
                Type::Core,
                "Bundled Smart model is invalid ({} bytes): {}",
                metadata.len(),
                src_path.display()
            );
            None
        }
        Err(err) => {
            logging!(
                warn,
                Type::Core,
                "Bundled Smart model not found at {}: {}",
                src_path.display(),
                err
            );
            None
        }
    }
}

async fn resource_is_newer(src_path: &std::path::Path, dest_path: &std::path::Path) -> bool {
    let src_modified = tokio::fs::metadata(src_path).await.and_then(|m| m.modified());
    let dest_modified = tokio::fs::metadata(dest_path).await.and_then(|m| m.modified());

    matches!((src_modified, dest_modified), (Ok(src), Ok(dest)) if src > dest)
}

const fn smart_model_copy_required(src_len: u64, dest_len: Option<u64>, source_is_newer: bool) -> bool {
    match dest_len {
        Some(dest_len) => dest_len != src_len || source_is_newer,
        None => true,
    }
}

#[cfg(target_os = "windows")]
async fn replace_file_atomically(staged_path: &std::path::Path, dest_path: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt as _;
    use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW};

    let staged_path = staged_path.to_owned();
    let dest_path = dest_path.to_owned();
    tokio::task::spawn_blocking(move || {
        let staged_wide = staged_path.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();
        let dest_wide = dest_path.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();
        // SAFETY: both buffers are NUL-terminated and remain alive for the duration of the call.
        let result = unsafe {
            MoveFileExW(
                staged_wide.as_ptr(),
                dest_wide.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        if result == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    })
    .await
    .map_err(std::io::Error::other)?
}

#[cfg(not(target_os = "windows"))]
async fn replace_file_atomically(staged_path: &std::path::Path, dest_path: &std::path::Path) -> std::io::Result<()> {
    tokio::fs::rename(staged_path, dest_path).await
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
    use super::{apply_core_change_to_draft, should_wait_for_service};
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
    #[allow(clippy::assertions_on_constants)]
    fn smart_model_min_size_rejects_truncated_download() {
        assert!(super::SMART_MODEL_MIN_BYTES > 2 * 1024 * 1024);
    }

    #[test]
    fn smart_model_copy_detects_large_but_truncated_destination() {
        let source_len = super::SMART_MODEL_MIN_BYTES * 2;
        let truncated_len = super::SMART_MODEL_MIN_BYTES + 1;

        assert!(super::smart_model_copy_required(source_len, Some(truncated_len), false));
        assert!(!super::smart_model_copy_required(source_len, Some(source_len), false));
        assert!(super::smart_model_copy_required(source_len, Some(source_len), true));
    }

    #[tokio::test]
    async fn atomic_model_replace_overwrites_existing_destination() -> std::io::Result<()> {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let test_dir = std::env::temp_dir().join(format!("clash-verge-model-replace-{}-{unique}", std::process::id()));
        let staged_path = test_dir.join("Model.bin.tmp");
        let dest_path = test_dir.join("Model.bin");

        tokio::fs::create_dir_all(&test_dir).await?;
        tokio::fs::write(&dest_path, b"old model").await?;
        tokio::fs::write(&staged_path, b"new model").await?;
        super::replace_file_atomically(&staged_path, &dest_path).await?;

        assert_eq!(tokio::fs::read(&dest_path).await?, b"new model");
        assert!(!staged_path.exists());
        tokio::fs::remove_dir_all(&test_dir).await?;
        Ok(())
    }

    #[test]
    fn service_wait_is_only_required_for_non_admin_tun() {
        assert!(should_wait_for_service(true, false, false));
        assert!(!should_wait_for_service(true, false, true));
        assert!(!should_wait_for_service(true, true, false));
        assert!(!should_wait_for_service(false, false, false));
    }

    #[test]
    fn lost_service_start_response_requires_confirmed_stop_before_sidecar() {
        assert!(super::sidecar_fallback_is_safe(true, true));
        assert!(!super::sidecar_fallback_is_safe(true, false));
        assert!(super::sidecar_fallback_is_safe(false, false));
    }
}
