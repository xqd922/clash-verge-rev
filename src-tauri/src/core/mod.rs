pub mod autostart;
pub mod backup;
pub mod handle;
pub mod hotkey;
pub mod logger;
pub mod manager;
mod notification;
pub mod owner_identity;
pub mod runtime_bundle;
pub mod service;
pub mod sysopt;
pub mod timer;
pub mod tray;
pub mod updater;
pub mod validate;
pub mod win_uwp;

pub use self::{manager::CoreManager, timer::Timer, updater::SilentUpdater};
