use super::{CoreManager, RunningMode};
use crate::{
    AsyncHandler,
    config::{Config, IClashTemp},
    core::{handle, logger::Logger, manager::CLASH_LOGGER, service},
    logging,
    utils::dirs,
};
use anyhow::Result;
use clash_verge_logging::Type;
use compact_str::CompactString;
use log::Level;
use std::time::{Duration, Instant};
use tauri_plugin_shell::ShellExt as _;

#[cfg(target_os = "windows")]
use {
    std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle},
    windows_sys::Win32::{
        Foundation::HANDLE,
        System::{
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
            },
            Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA, PROCESS_TERMINATE},
        },
    },
};

impl CoreManager {
    pub async fn get_clash_logs(&self) -> Result<Vec<CompactString>> {
        match *self.get_running_mode() {
            RunningMode::Service => service::get_clash_logs_by_service().await,
            RunningMode::Sidecar => Ok(CLASH_LOGGER.get_logs().await),
            RunningMode::NotRunning => Ok(Vec::new()),
        }
    }

    pub(super) async fn start_core_by_sidecar(&self) -> Result<()> {
        logging!(info, Type::Core, "Starting core in sidecar mode");

        let config_file = Config::generate_file(crate::config::ConfigType::Run).await?;
        let app_handle = handle::Handle::app_handle();
        let clash_core = Config::verge().await.latest_arc().get_valid_clash_core();
        let config_dir = dirs::app_home_dir()?;

        #[cfg(unix)]
        let previous_mask = unsafe { tauri_plugin_clash_verge_sysinfo::libc::umask(0o007) };
        let (mut rx, child) = app_handle
            .shell()
            .sidecar(clash_core.as_str())?
            .args([
                "-d",
                dirs::path_to_str(&config_dir)?,
                "-f",
                dirs::path_to_str(&config_file)?,
                if cfg!(windows) {
                    "-ext-ctl-pipe"
                } else {
                    "-ext-ctl-unix"
                },
                &IClashTemp::guard_external_controller_ipc(),
            ])
            .spawn()?;
        #[cfg(target_os = "windows")]
        {
            let job = match create_and_assign_sidecar_job(child.pid()) {
                Ok(job) => job,
                Err(job_error) => {
                    let pid = child.pid();
                    let error = match child.kill() {
                        Ok(()) => job_error,
                        Err(kill_error) => anyhow::anyhow!(
                            "failed to configure Job Object for sidecar PID {pid}: {job_error:#}; \
                             failed to terminate child: {kill_error:#}"
                        ),
                    };
                    logging!(error, Type::Core, "Failed to start sidecar: {error:#}");
                    return Err(error);
                }
            };
            self.set_job_handle(Some(job));
        }
        #[cfg(unix)]
        unsafe {
            tauri_plugin_clash_verge_sysinfo::libc::umask(previous_mask)
        };

        let pid = child.pid();
        logging!(trace, Type::Core, "Sidecar started with PID: {}", pid);

        let generation = self.set_running_child_sidecar(child);
        self.set_running_mode(RunningMode::Sidecar);

        AsyncHandler::spawn(move || async move {
            while let Some(event) = rx.recv().await {
                match event {
                    tauri_plugin_shell::process::CommandEvent::Stdout(line)
                    | tauri_plugin_shell::process::CommandEvent::Stderr(line) => {
                        let message = CompactString::from(&*String::from_utf8_lossy(&line));
                        Logger::global().writer_sidecar_log(Level::Error, &message);
                        CLASH_LOGGER.append_log(message).await;
                    }
                    tauri_plugin_shell::process::CommandEvent::Terminated(term) => {
                        let message = if let Some(code) = term.code {
                            CompactString::from(format!("Process terminated with code: {}", code))
                        } else if let Some(signal) = term.signal {
                            CompactString::from(format!("Process terminated by signal: {}", signal))
                        } else {
                            CompactString::from("Process terminated")
                        };
                        Logger::global().writer_sidecar_log(Level::Info, &message);
                        CLASH_LOGGER.clear_logs().await;
                        Self::global().handle_sidecar_terminated(pid, generation).await;
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }

    async fn handle_sidecar_terminated(&self, pid: u32, generation: u64) {
        let _life = self.lifecycle_lock.lock().await;
        if !self.is_current_sidecar(pid, generation) {
            logging!(trace, Type::Core, "Ignoring stale sidecar termination (PID: {pid})");
            return;
        }

        let _ = self.take_child_sidecar();
        self.clear_sidecar_identity(pid, generation);
        #[cfg(target_os = "windows")]
        self.set_job_handle(None);
        self.set_running_mode(RunningMode::NotRunning);
        self.clear_core_ipc_pool();
        self.after_core_process();
        logging!(
            info,
            Type::Core,
            "Sidecar state cleared after process exit (PID: {pid})"
        );
    }

    pub(super) async fn stop_core_by_sidecar(&self) -> Result<()> {
        logging!(info, Type::Core, "Stopping sidecar");
        let Some((child, generation)) = self.take_child_sidecar() else {
            if matches!(*self.get_running_mode(), RunningMode::Sidecar) {
                anyhow::bail!("sidecar process handle is unavailable while it may still be running");
            }
            self.set_running_mode(RunningMode::NotRunning);
            return Ok(());
        };

        let pid = child.pid();
        let kill_result = child.kill();
        #[cfg(target_os = "windows")]
        {
            self.set_job_handle(None);
            logging!(
                trace,
                Type::Core,
                "Closed job handle for sidecar process (PID: {})",
                pid
            );
        }

        match self.wait_for_sidecar_exit(pid).await {
            Ok(()) => {
                if let Err(err) = kill_result {
                    logging!(
                        warn,
                        Type::Core,
                        "Sidecar kill returned an error after process exit (PID: {pid}): {err}"
                    );
                }
                self.clear_sidecar_identity(pid, generation);
                self.set_running_mode(RunningMode::NotRunning);
                Ok(())
            }
            Err(wait_err) => {
                self.set_running_mode(RunningMode::Sidecar);
                match kill_result {
                    Ok(()) => Err(wait_err),
                    Err(kill_err) => Err(anyhow::anyhow!(
                        "failed to kill sidecar PID {pid}: {kill_err}; process exit was not observed: {wait_err}"
                    )),
                }
            }
        }
    }

    #[cfg(unix)]
    async fn wait_for_sidecar_exit(&self, pid: u32) -> Result<()> {
        let max_wait = Duration::from_secs(5);
        let interval = Duration::from_millis(40);
        let start = Instant::now();

        loop {
            if !sidecar_process_exists(pid) {
                logging!(
                    trace,
                    Type::Core,
                    "Sidecar exited after {}ms (PID: {})",
                    start.elapsed().as_millis(),
                    pid
                );
                return Ok(());
            }

            if start.elapsed() >= max_wait {
                anyhow::bail!("sidecar PID {pid} still exists after {}ms", start.elapsed().as_millis());
            }

            tokio::time::sleep(interval).await;
        }
    }

    #[cfg(target_os = "windows")]
    async fn wait_for_sidecar_exit(&self, pid: u32) -> Result<()> {
        let ipc = dirs::ipc_path()?;
        let path_str = dirs::path_to_str(&ipc)?.to_owned();
        let max_wait = Duration::from_secs(5);
        let interval = Duration::from_millis(40);
        let start = Instant::now();

        loop {
            let path = path_str.clone();
            let still_open = tokio::task::spawn_blocking(move || std::fs::File::open(path).is_ok())
                .await
                .map_err(|err| anyhow::anyhow!("failed to check sidecar IPC release: {err}"))?;
            if !still_open {
                return Ok(());
            }
            if start.elapsed() >= max_wait {
                anyhow::bail!(
                    "sidecar IPC is still open after {}ms (PID: {pid})",
                    start.elapsed().as_millis()
                );
            }
            tokio::time::sleep(interval).await;
        }
    }

    pub(super) async fn start_core_by_service(&self) -> Result<()> {
        logging!(info, Type::Core, "Starting core in service mode");
        let config_file = match Config::generate_file(crate::config::ConfigType::Run).await {
            Ok(path) => path,
            Err(err) => {
                self.set_running_mode(RunningMode::NotRunning);
                return Err(err);
            }
        };
        self.start_core_by_service_with_config(&config_file).await
    }

    pub(super) async fn start_core_by_service_with_config(&self, config_file: &std::path::PathBuf) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            use crate::constants::timing;
            let mut last_err = None;
            for attempt in 0..timing::SERVICE_START_RETRIES {
                match service::run_core_by_service(config_file).await {
                    Ok(()) => {
                        self.set_running_mode(RunningMode::Service);
                        return Ok(());
                    }
                    Err(start_err) => {
                        logging!(
                            warn,
                            Type::Core,
                            "service start attempt {}/{} failed: {}",
                            attempt + 1,
                            timing::SERVICE_START_RETRIES,
                            start_err
                        );
                        let compensated_err = self.compensate_failed_service_start(start_err).await;
                        if matches!(*self.get_running_mode(), RunningMode::Service) {
                            return Err(compensated_err);
                        }
                        last_err = Some(compensated_err);
                        tokio::time::sleep(timing::SERVICE_START_RETRY_DELAY).await;
                    }
                }
            }
            Err(last_err.unwrap_or_else(|| anyhow::anyhow!("service start failed")))
        }

        #[cfg(not(target_os = "windows"))]
        {
            match service::run_core_by_service(config_file).await {
                Ok(()) => {
                    self.set_running_mode(RunningMode::Service);
                    Ok(())
                }
                Err(start_err) => Err(self.compensate_failed_service_start(start_err).await),
            }
        }
    }

    pub(super) async fn stop_core_by_service(&self) -> Result<()> {
        logging!(info, Type::Core, "Stopping service");
        match service::stop_core_by_service().await {
            Ok(()) => {
                self.set_running_mode(RunningMode::NotRunning);
                Ok(())
            }
            Err(err) => {
                // A failed stop is ambiguous: keep the conservative mode so no sidecar can be started.
                self.set_running_mode(RunningMode::Service);
                Err(err)
            }
        }
    }

    async fn compensate_failed_service_start(&self, start_err: anyhow::Error) -> anyhow::Error {
        // StartClash may have been executed even when its response was lost.
        self.set_running_mode(RunningMode::Service);
        match self.stop_core_by_service().await {
            Ok(()) => anyhow::anyhow!("service start failed and the possible core was stopped: {start_err}"),
            Err(stop_err) => anyhow::anyhow!(
                "service start failed: {start_err}; failed to stop a possibly running service core: {stop_err}"
            ),
        }
    }
}

#[cfg(unix)]
fn sidecar_process_exists(pid: u32) -> bool {
    use tauri_plugin_clash_verge_sysinfo::libc;

    // SAFETY: signal 0 does not modify the target process; it only probes PID existence.
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

#[cfg(target_os = "windows")]
fn create_and_assign_sidecar_job(child_pid: u32) -> Result<OwnedHandle> {
    unsafe {
        let raw_job: HANDLE = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if raw_job.is_null() {
            return Err(last_win32_error("CreateJobObjectW failed"));
        }
        let job = OwnedHandle::from_raw_handle(raw_job);
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        let set_info_result = SetInformationJobObject(
            job.as_raw_handle() as HANDLE,
            JobObjectExtendedLimitInformation,
            &mut info as *mut _ as *mut _,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        if set_info_result == 0 {
            return Err(last_win32_error("SetInformationJobObject failed"));
        }

        let raw_process_handle = OpenProcess(
            PROCESS_SET_QUOTA | PROCESS_TERMINATE | PROCESS_QUERY_INFORMATION,
            0,
            child_pid,
        );
        if raw_process_handle.is_null() {
            return Err(last_win32_error("OpenProcess failed"));
        }
        let process_handle = OwnedHandle::from_raw_handle(raw_process_handle);

        let assign_result = AssignProcessToJobObject(job.as_raw_handle(), process_handle.as_raw_handle());
        if assign_result == 0 {
            return Err(last_win32_error("AssignProcessToJobObject failed"));
        }

        Ok(job)
    }
}

#[cfg(target_os = "windows")]
fn last_win32_error(operation: &'static str) -> anyhow::Error {
    anyhow::Error::new(std::io::Error::last_os_error()).context(operation)
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::create_and_assign_sidecar_job;
    use anyhow::Result;
    use std::{
        process::{Child, Command, Stdio},
        thread::sleep,
        time::{Duration, Instant},
    };

    fn spawn_long_lived() -> Result<Child> {
        Ok(Command::new("ping")
            .args(["-n", "999", "127.0.0.1"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?)
    }

    fn wait_until_exited(child: &mut Child, timeout: Duration) -> Result<bool> {
        let deadline = Instant::now() + timeout;
        loop {
            if child.try_wait()?.is_some() {
                return Ok(true);
            }
            if Instant::now() >= deadline {
                return Ok(false);
            }
            sleep(Duration::from_millis(50));
        }
    }

    #[test]
    fn job_kills_child_on_handle_drop() -> Result<()> {
        let mut child = spawn_long_lived()?;
        let job = create_and_assign_sidecar_job(child.id())?;
        assert!(child.try_wait()?.is_none());
        drop(job);
        assert!(wait_until_exited(&mut child, Duration::from_secs(5))?);
        Ok(())
    }

    #[test]
    fn returns_err_for_invalid_pid() {
        assert!(create_and_assign_sidecar_job(0xFFFF_FFFC).is_err());
    }
}
