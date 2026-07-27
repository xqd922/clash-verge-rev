mod config;
mod lifecycle;
mod state;

use anyhow::Result;
use arc_swap::{ArcSwap, ArcSwapOption};
use clash_verge_logger::AsyncLogger;
use once_cell::sync::Lazy;
#[cfg(target_os = "windows")]
use std::os::windows::io::OwnedHandle;
use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
    time::Instant,
};
use tauri_plugin_shell::process::CommandChild;

use crate::singleton;

pub(crate) static CLASH_LOGGER: Lazy<Arc<AsyncLogger>> = Lazy::new(|| Arc::new(AsyncLogger::new()));

#[derive(Debug, serde::Serialize, PartialEq, Eq)]
pub enum RunningMode {
    Service,
    Sidecar,
    NotRunning,
}

impl fmt::Display for RunningMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Service => write!(f, "Service"),
            Self::Sidecar => write!(f, "Sidecar"),
            Self::NotRunning => write!(f, "NotRunning"),
        }
    }
}

#[derive(Debug)]
pub struct CoreManager {
    state: ArcSwap<State>,
    last_update: ArcSwapOption<Instant>,
    #[cfg(target_os = "windows")]
    job_handle: ArcSwapOption<OwnedHandle>,
    config_update_in_progress: AtomicBool,
    lifecycle_lock: tokio::sync::Mutex<()>,
    sidecar_pid: AtomicU32,
    sidecar_generation: AtomicU64,
    #[cfg(target_os = "windows")]
    handoff_watcher_running: AtomicBool,
}

#[must_use]
pub(crate) struct ConfigUpdatePermit<'a> {
    manager: &'a CoreManager,
}

impl Drop for ConfigUpdatePermit<'_> {
    fn drop(&mut self) {
        self.manager.config_update_in_progress.store(false, Ordering::Release);
    }
}

#[derive(Debug)]
struct State {
    running_mode: ArcSwap<RunningMode>,
    child_sidecar: ArcSwapOption<CommandChild>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            running_mode: ArcSwap::new(Arc::new(RunningMode::NotRunning)),
            child_sidecar: ArcSwapOption::new(None),
        }
    }
}

impl Default for CoreManager {
    fn default() -> Self {
        Self {
            state: ArcSwap::new(Arc::new(State::default())),
            last_update: ArcSwapOption::new(None),
            #[cfg(target_os = "windows")]
            job_handle: ArcSwapOption::new(None),
            config_update_in_progress: AtomicBool::new(false),
            lifecycle_lock: tokio::sync::Mutex::new(()),
            sidecar_pid: AtomicU32::new(0),
            sidecar_generation: AtomicU64::new(0),
            #[cfg(target_os = "windows")]
            handoff_watcher_running: AtomicBool::new(false),
        }
    }
}

impl CoreManager {
    fn new() -> Self {
        Self::default()
    }

    pub fn get_running_mode(&self) -> Arc<RunningMode> {
        Arc::clone(&self.state.load().running_mode.load())
    }

    pub fn take_child_sidecar(&self) -> Option<(CommandChild, u64)> {
        let generation = self.sidecar_generation.load(Ordering::Acquire);
        self.state
            .load()
            .child_sidecar
            .swap(None)
            .and_then(|arc| Arc::try_unwrap(arc).ok())
            .map(|child| (child, generation))
    }

    pub fn get_last_update(&self) -> Option<Arc<Instant>> {
        self.last_update.load_full()
    }

    pub fn set_running_mode(&self, mode: RunningMode) {
        let state = self.state.load();
        state.running_mode.store(Arc::new(mode));
    }

    pub fn set_running_child_sidecar(&self, child: CommandChild) -> u64 {
        let generation = self.sidecar_generation.fetch_add(1, Ordering::AcqRel) + 1;
        self.sidecar_pid.store(child.pid(), Ordering::Release);
        let state = self.state.load();
        state.child_sidecar.store(Some(Arc::new(child)));
        generation
    }

    fn is_current_sidecar(&self, pid: u32, generation: u64) -> bool {
        self.sidecar_generation.load(Ordering::Acquire) == generation
            && self.sidecar_pid.load(Ordering::Acquire) == pid
            && matches!(*self.get_running_mode(), RunningMode::Sidecar)
    }

    fn clear_sidecar_identity(&self, pid: u32, generation: u64) {
        if self.sidecar_generation.load(Ordering::Acquire) == generation
            && self.sidecar_pid.load(Ordering::Acquire) == pid
        {
            self.sidecar_pid.store(0, Ordering::Release);
        }
    }

    pub fn set_last_update(&self, time: Instant) {
        self.last_update.store(Some(Arc::new(time)));
    }

    #[cfg(target_os = "windows")]
    fn set_job_handle(&self, handle: Option<OwnedHandle>) {
        self.job_handle.store(handle.map(Arc::new));
    }

    pub(crate) fn try_acquire_config_update(&self) -> Option<ConfigUpdatePermit<'_>> {
        if self.config_update_in_progress.swap(true, Ordering::AcqRel) {
            None
        } else {
            Some(ConfigUpdatePermit { manager: self })
        }
    }

    pub(crate) async fn acquire_config_update(&self) -> ConfigUpdatePermit<'_> {
        loop {
            if let Some(permit) = self.try_acquire_config_update() {
                return permit;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    pub async fn init(&self) -> Result<()> {
        self.start_core().await?;
        Ok(())
    }
}

singleton!(CoreManager, CORE_MANAGER);

#[cfg(test)]
mod tests {
    use super::CoreManager;

    #[test]
    fn config_update_permit_is_exclusive_and_drop_releases_it() -> anyhow::Result<()> {
        let manager = CoreManager::default();
        let Some(permit) = manager.try_acquire_config_update() else {
            anyhow::bail!("first config update permit should be available");
        };
        assert!(manager.try_acquire_config_update().is_none());
        assert!(
            manager.try_acquire_config_update().is_none(),
            "a failed acquisition must not release the live permit"
        );

        drop(permit);

        assert!(manager.try_acquire_config_update().is_some());
        Ok(())
    }
}
