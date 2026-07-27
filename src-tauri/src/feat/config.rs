use crate::{
    config::{Config, IVerge},
    core::{CoreManager, autostart, handle, hotkey, logger::Logger, manager::ConfigUpdatePermit, sysopt, tray},
    module::{auto_backup::AutoBackupManager, lightweight},
};
use anyhow::{Result, anyhow};
use bitflags::bitflags;
use clash_verge_draft::SharedDraft;
use clash_verge_logging::{Type, logging, logging_error};
use serde_yaml_ng::Mapping;

/// Patch Clash configuration
pub async fn patch_clash(patch: &Mapping) -> Result<()> {
    let manager = CoreManager::global();
    let Some(config_permit) = manager.try_acquire_config_update() else {
        return Err(anyhow!("A configuration update is already running"));
    };

    let clash = Config::clash().await;
    let runtime = Config::runtime().await;
    let committed_clash = clash.data_arc();
    let restart_required = patch.get("secret").is_some() || patch.get("external-controller").is_some();

    clash.edit_draft(|draft| draft.patch_config(patch));
    if restart_required {
        if let Err(err) = Config::generate().await {
            clash.discard();
            runtime.discard();
            return Err(err);
        }
    } else {
        runtime.edit_draft(|draft| draft.patch_config(patch));
    }

    // Persist the staged Clash config before changing the running core. From
    // this point onward every failure restores the committed file snapshot.
    if let Err(err) = clash.latest_arc().save_config().await {
        clash.discard();
        runtime.discard();
        return match committed_clash.save_config().await {
            Ok(()) => Err(err),
            Err(rollback_err) => Err(anyhow!("{err}; failed to restore Clash config file: {rollback_err}")),
        };
    }

    let update_result = if restart_required {
        manager.restart_core_with_permit(&config_permit).await
    } else {
        match manager.update_config_forced_with_permit(&config_permit).await {
            Ok(outcome) if outcome.is_valid() => Ok(()),
            Ok(outcome) => Err(anyhow!("{outcome}")),
            Err(err) => Err(err),
        }
    };

    if let Err(err) = update_result {
        clash.discard();
        runtime.discard();

        let mut rollback_errors = Vec::new();
        if let Err(rollback_err) = committed_clash.save_config().await {
            rollback_errors.push(format!("failed to restore Clash config file: {rollback_err}"));
        }
        if restart_required && let Err(rollback_err) = manager.restart_core_with_permit(&config_permit).await {
            rollback_errors.push(format!("failed to restart the previous core config: {rollback_err}"));
        }

        return if rollback_errors.is_empty() {
            Err(err)
        } else {
            Err(anyhow!("{err}; rollback failed: {}", rollback_errors.join("; ")))
        };
    }

    clash.apply();
    runtime.apply();
    if patch.get("mode").is_some() {
        tray::Tray::global().update_menu_and_icon().await;
    }
    handle::Handle::refresh_clash();
    Ok(())
}

// Define update flags as bitflags for better performance
bitflags! {
     #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
     struct UpdateFlags: u16 {
        const RESTART_CORE = 1 << 0;
        const CLASH_CONFIG = 1 << 1;
        const VERGE_CONFIG = 1 << 2;
        const LAUNCH = 1 << 3;
        const SYS_PROXY = 1 << 4;
        const SYSTRAY_ICON = 1 << 5;
        const HOTKEY = 1 << 6;
        const SYSTRAY_MENU = 1 << 7;
        const SYSTRAY_TOOLTIP = 1 << 8;
        const SYSTRAY_CLICK_BEHAVIOR = 1 << 9;
        const LIGHT_WEIGHT = 1 << 10;
        const LANGUAGE = 1 << 11;
        const LOG_LEVEL = 1 << 12;
        const LOG_FILE = 1 << 13;

        const GROUP_SYS_TRAY = Self::SYSTRAY_MENU.bits()
                             | Self::SYSTRAY_TOOLTIP.bits()
                             | Self::SYSTRAY_ICON.bits();
     }
}

fn determine_update_flags(patch: &IVerge) -> UpdateFlags {
    let tun_mode = patch.enable_tun_mode;
    let auto_launch = patch.enable_auto_launch;
    let silent_start = patch.enable_silent_start;
    let system_proxy = patch.enable_system_proxy;
    let pac = patch.proxy_auto_config;
    let pac_content = &patch.pac_file_content;
    let proxy_bypass = &patch.system_proxy_bypass;
    let language = &patch.language;
    let mixed_port = patch.verge_mixed_port;
    #[cfg(target_os = "macos")]
    let tray_icon = &patch.tray_icon;
    #[cfg(not(target_os = "macos"))]
    let tray_icon: Option<String> = None;
    let common_tray_icon = patch.common_tray_icon;
    let sysproxy_tray_icon = patch.sysproxy_tray_icon;
    let tun_tray_icon = patch.tun_tray_icon;
    #[cfg(not(target_os = "windows"))]
    let redir_enabled = patch.verge_redir_enabled;
    #[cfg(not(target_os = "windows"))]
    let redir_port = patch.verge_redir_port;
    #[cfg(target_os = "linux")]
    let tproxy_enabled = patch.verge_tproxy_enabled;
    #[cfg(target_os = "linux")]
    let tproxy_port = patch.verge_tproxy_port;
    let socks_enabled = patch.verge_socks_enabled;
    let socks_port = patch.verge_socks_port;
    let http_enabled = patch.verge_http_enabled;
    let http_port = patch.verge_port;
    #[cfg(target_os = "macos")]
    let enable_tray_speed = patch.enable_tray_speed;
    #[cfg(not(target_os = "macos"))]
    let enable_tray_speed: Option<bool> = None;
    // let enable_tray_icon = patch.enable_tray_icon;
    let enable_global_hotkey = patch.enable_global_hotkey;
    let tray_event = &patch.tray_event;
    let home_cards = patch.home_cards.as_ref();
    let enable_auto_light_weight = patch.enable_auto_light_weight_mode;
    let enable_external_controller = patch.enable_external_controller;
    let tray_proxy_groups_display_mode = &patch.tray_proxy_groups_display_mode;
    let tray_inline_outbound_modes = patch.tray_inline_outbound_modes;
    let enable_proxy_guard = patch.enable_proxy_guard;
    let proxy_guard_duration = patch.proxy_guard_duration;
    let log_level = &patch.app_log_level;
    let log_max_size = patch.app_log_max_size;
    let log_max_count = patch.app_log_max_count;
    let enable_builtin_enhanced = patch.enable_builtin_enhanced;

    #[cfg(target_os = "windows")]
    let restart_core_needed = socks_enabled.is_some()
        || http_enabled.is_some()
        || socks_port.is_some()
        || http_port.is_some()
        || mixed_port.is_some()
        || enable_external_controller.is_some()
        || enable_builtin_enhanced.is_some();
    #[cfg(not(target_os = "windows"))]
    let mut restart_core_needed = socks_enabled.is_some()
        || http_enabled.is_some()
        || socks_port.is_some()
        || http_port.is_some()
        || mixed_port.is_some()
        || enable_external_controller.is_some()
        || enable_builtin_enhanced.is_some();
    #[cfg(not(target_os = "windows"))]
    {
        restart_core_needed |= redir_enabled.is_some() || redir_port.is_some();
    }
    #[cfg(target_os = "linux")]
    {
        restart_core_needed |= tproxy_enabled.is_some() || tproxy_port.is_some();
        restart_core_needed |= tun_mode == Some(true);
    }

    let mut update_flags = UpdateFlags::empty();
    if restart_core_needed {
        update_flags.insert(UpdateFlags::RESTART_CORE);
    }
    if tun_mode.is_some() {
        update_flags.insert(UpdateFlags::CLASH_CONFIG | UpdateFlags::GROUP_SYS_TRAY);
    }
    if enable_global_hotkey.is_some() || home_cards.is_some() {
        update_flags.insert(UpdateFlags::VERGE_CONFIG);
    }
    if auto_launch.is_some() || silent_start.is_some() {
        update_flags.insert(UpdateFlags::LAUNCH);
    }
    if system_proxy.is_some() {
        update_flags.insert(UpdateFlags::SYS_PROXY | UpdateFlags::GROUP_SYS_TRAY);
    }
    if proxy_bypass.is_some()
        || pac_content.is_some()
        || pac.is_some()
        || enable_proxy_guard.is_some()
        || proxy_guard_duration.is_some()
    {
        update_flags.insert(UpdateFlags::SYS_PROXY);
    }
    if language.is_some() {
        update_flags.insert(UpdateFlags::LANGUAGE | UpdateFlags::SYSTRAY_MENU | UpdateFlags::SYSTRAY_TOOLTIP);
    }
    if common_tray_icon.is_some()
        || sysproxy_tray_icon.is_some()
        || tun_tray_icon.is_some()
        || tray_icon.is_some()
        || enable_tray_speed.is_some()
    {
        update_flags.insert(UpdateFlags::SYSTRAY_ICON);
    }
    if patch.hotkeys.is_some() {
        update_flags.insert(UpdateFlags::HOTKEY | UpdateFlags::SYSTRAY_MENU);
    }
    if tray_event.is_some() {
        update_flags.insert(UpdateFlags::SYSTRAY_CLICK_BEHAVIOR);
    }
    if enable_auto_light_weight.is_some() {
        update_flags.insert(UpdateFlags::LIGHT_WEIGHT);
    }
    if tray_proxy_groups_display_mode.is_some() {
        update_flags.insert(UpdateFlags::SYSTRAY_MENU);
    }
    if log_level.is_some() {
        update_flags.insert(UpdateFlags::LOG_LEVEL);
    }
    if log_max_size.is_some() || log_max_count.is_some() {
        update_flags.insert(UpdateFlags::LOG_FILE);
    }
    if tray_inline_outbound_modes.is_some() {
        update_flags.insert(UpdateFlags::SYSTRAY_MENU);
    }

    update_flags
}

#[derive(Default)]
struct AppliedSystemEffects {
    launch: bool,
    locale: bool,
    sys_proxy: bool,
    hotkeys: bool,
}

async fn rollback_system_effects(effects: &AppliedSystemEffects, committed_verge: &IVerge) -> Vec<String> {
    let mut rollback_errors = Vec::new();

    if effects.hotkeys {
        let hotkey = hotkey::Hotkey::global();
        if let Err(err) = hotkey.reset() {
            rollback_errors.push(format!("failed to reset hotkeys: {err}"));
        }
        // reset unregisters OS shortcuts but does not clear Hotkey's current
        // snapshot. Move it through empty so update re-registers every old key
        // and reports registration failures instead of silently ignoring them.
        if let Err(err) = hotkey.update(Vec::new()).await {
            rollback_errors.push(format!("failed to clear hotkey state: {err}"));
        }
        if let Err(err) = hotkey.update(committed_verge.hotkeys.clone().unwrap_or_default()).await {
            rollback_errors.push(format!("failed to restore hotkeys: {err}"));
        }
    }

    if effects.sys_proxy {
        let sysopt = sysopt::Sysopt::global();
        if let Err(err) = sysopt.update_sysproxy().await {
            rollback_errors.push(format!("failed to restore system proxy: {err}"));
        }
        sysopt.refresh_guard().await;
    }

    if effects.locale {
        clash_verge_i18n::sync_locale(committed_verge.language.as_deref());
    }

    if effects.launch
        && let Err(err) = autostart::update_launch().await
    {
        rollback_errors.push(format!("failed to restore auto-launch: {err}"));
    }

    rollback_errors
}

#[allow(clippy::cognitive_complexity)]
async fn process_terminated_flags(
    update_flags: UpdateFlags,
    patch: &IVerge,
    committed_verge: &IVerge,
    config_permit: &ConfigUpdatePermit<'_>,
) -> Result<()> {
    let mut effects = AppliedSystemEffects::default();
    let mut restart_attempted = false;

    let result: Result<()> = async {
        if update_flags.contains(UpdateFlags::LAUNCH) {
            effects.launch = true;
            autostart::update_launch().await?;
        }
        if update_flags.contains(UpdateFlags::LANGUAGE)
            && let Some(language) = &patch.language
        {
            effects.locale = true;
            clash_verge_i18n::set_locale(language.as_str());
        }
        if update_flags.contains(UpdateFlags::SYS_PROXY) {
            effects.sys_proxy = true;
            sysopt::Sysopt::global().update_sysproxy().await?;
            sysopt::Sysopt::global().refresh_guard().await;
        }
        if update_flags.contains(UpdateFlags::HOTKEY)
            && let Some(hotkeys) = &patch.hotkeys
        {
            effects.hotkeys = true;
            hotkey::Hotkey::global().update(hotkeys.to_owned()).await?;
        }

        // Apply the core change after reversible OS state. If it fails, the
        // error path below restores both the old core and every applied state.
        let manager = CoreManager::global();
        if update_flags.contains(UpdateFlags::RESTART_CORE) {
            Config::generate().await?;
            restart_attempted = true;
            manager.restart_core_with_permit(config_permit).await?;
        } else if update_flags.contains(UpdateFlags::CLASH_CONFIG) {
            match manager.update_config_forced_with_permit(config_permit).await {
                Ok(outcome) if outcome.is_valid() => {}
                Ok(outcome) => return Err(anyhow!("{outcome}")),
                Err(err) => return Err(err),
            }
        }

        // These operations only refresh process/UI state. They must not abort
        // an otherwise committed config transaction.
        if update_flags.contains(UpdateFlags::SYSTRAY_MENU) {
            logging_error!(Type::Setup, tray::Tray::global().update_menu().await);
        }
        if update_flags.contains(UpdateFlags::SYSTRAY_ICON) {
            logging_error!(
                Type::Setup,
                tray::Tray::global()
                    .update_icon(&Config::verge().await.latest_arc())
                    .await
            );
            #[cfg(target_os = "macos")]
            if patch.enable_tray_speed.is_some() {
                tray::Tray::global().update_speed_task(patch.enable_tray_speed.unwrap_or(false));
            }
        }
        if update_flags.contains(UpdateFlags::SYSTRAY_TOOLTIP) {
            logging_error!(Type::Setup, tray::Tray::global().update_tooltip().await);
        }
        if update_flags.contains(UpdateFlags::SYSTRAY_CLICK_BEHAVIOR) {
            logging_error!(Type::Setup, tray::Tray::global().update_click_behavior().await);
        }
        if update_flags.contains(UpdateFlags::LIGHT_WEIGHT) {
            if patch.enable_auto_light_weight_mode.unwrap_or(false) {
                lightweight::enable_auto_light_weight_mode().await;
            } else {
                lightweight::disable_auto_light_weight_mode();
            }
        }
        if update_flags.contains(UpdateFlags::LOG_LEVEL) {
            logging_error!(Type::Setup, Logger::global().update_log_level(patch.get_log_level()));
        }
        if update_flags.contains(UpdateFlags::LOG_FILE) {
            let log_max_size = patch.app_log_max_size.unwrap_or(128);
            let log_max_count = patch.app_log_max_count.unwrap_or(8);
            logging_error!(
                Type::Setup,
                Logger::global().update_log_config(log_max_size, log_max_count).await
            );
        }

        if update_flags.contains(UpdateFlags::CLASH_CONFIG) {
            handle::Handle::refresh_clash();
        }
        if update_flags.contains(UpdateFlags::VERGE_CONFIG) {
            handle::Handle::refresh_verge();
        }
        Ok(())
    }
    .await;

    let Err(err) = result else {
        return Ok(());
    };

    // Rollback helpers read Config::verge().latest_arc(), so expose the old
    // committed snapshots before replaying any OS state.
    Config::verge().await.discard();
    Config::runtime().await.discard();

    let mut rollback_errors = Vec::new();
    if restart_attempted && let Err(rollback_err) = CoreManager::global().restart_core_with_permit(config_permit).await
    {
        rollback_errors.push(format!("failed to restart the previous core config: {rollback_err}"));
    }
    rollback_errors.extend(rollback_system_effects(&effects, committed_verge).await);

    if rollback_errors.is_empty() {
        Err(err)
    } else {
        Err(anyhow!("{err}; rollback failed: {}", rollback_errors.join("; ")))
    }
}

pub async fn patch_verge(patch: &IVerge, not_save_file: bool) -> Result<()> {
    let manager = CoreManager::global();
    let Some(config_permit) = manager.try_acquire_config_update() else {
        return Err(anyhow!("A configuration update is already running"));
    };

    patch_verge_with_permit(patch, not_save_file, &config_permit).await
}

pub(crate) async fn patch_verge_with_permit(
    patch: &IVerge,
    not_save_file: bool,
    config_permit: &ConfigUpdatePermit<'_>,
) -> Result<()> {
    let verge = Config::verge().await;
    let runtime = Config::runtime().await;
    let committed_verge = verge.data_arc();

    verge.edit_draft(|draft| draft.patch_config(patch));

    let update_flags = determine_update_flags(patch);
    logging!(debug, Type::Setup, "Determined update flags: {:?}", update_flags);

    if !not_save_file {
        logging!(debug, Type::Setup, "Saving Verge configuration to file...");
        if let Err(err) = verge.latest_arc().save_file().await {
            verge.discard();
            runtime.discard();
            return match committed_verge.save_file().await {
                Ok(()) => Err(err),
                Err(rollback_err) => Err(anyhow!("{err}; failed to restore Verge config file: {rollback_err}")),
            };
        }
    }

    if let Err(err) = process_terminated_flags(update_flags, patch, &committed_verge, config_permit).await {
        verge.discard();
        runtime.discard();

        if !not_save_file && let Err(rollback_err) = committed_verge.save_file().await {
            return Err(anyhow!("{err}; failed to restore Verge config file: {rollback_err}"));
        }
        return Err(err);
    }

    verge.apply();
    runtime.apply();
    logging_error!(Type::Backup, AutoBackupManager::global().refresh_settings().await);
    Ok(())
}

pub async fn fetch_verge_config() -> Result<SharedDraft<IVerge>> {
    let draft = Config::verge().await;
    let data = draft.data_arc();
    Ok(data)
}
