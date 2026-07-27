use super::{ConfigUpdatePermit, CoreManager};
use crate::{
    config::{Config, ConfigType, runtime::IRuntime},
    constants::timing,
    core::{
        handle,
        validate::{CoreConfigValidator, ValidationOutcome, ValidationSkipReason},
    },
    utils::{dirs, help},
};
use anyhow::{Result, anyhow};
use clash_verge_logging::{Type, logging};
use smartstring::alias::String;
use std::{collections::HashSet, path::PathBuf, time::Instant};
use tauri_plugin_mihomo::Error as MihomoError;

impl CoreManager {
    pub async fn use_default_config(&self, error_key: &str, error_msg: &str) -> Result<()> {
        use crate::constants::files::RUNTIME_CONFIG;

        let runtime_path = dirs::app_home_dir()?.join(RUNTIME_CONFIG);
        let clash_config = &Config::clash().await.latest_arc().0;

        Config::runtime().await.edit_draft(|d| {
            *d = IRuntime {
                config: Some(clash_config.to_owned()),
                exists_keys: HashSet::new(),
                chain_logs: Default::default(),
            }
        });

        help::save_yaml(&runtime_path, &clash_config, Some("# Clash Verge Runtime")).await?;
        handle::Handle::notice_message(error_key, error_msg);
        Ok(())
    }

    pub async fn update_config_forced(&self) -> Result<ValidationOutcome> {
        self.update_config_with_force(true).await
    }

    pub async fn update_config_with_force(&self, force: bool) -> Result<ValidationOutcome> {
        let Some(permit) = self.try_acquire_config_update() else {
            logging!(info, Type::Core, "Configuration update is already running");
            return Ok(ValidationOutcome::Busy);
        };
        self.update_config_with_force_permit(force, &permit).await
    }

    pub(crate) async fn update_config_forced_with_permit(
        &self,
        permit: &ConfigUpdatePermit<'_>,
    ) -> Result<ValidationOutcome> {
        self.update_config_with_force_permit(true, permit).await
    }

    async fn update_config_with_force_permit(
        &self,
        force: bool,
        permit: &ConfigUpdatePermit<'_>,
    ) -> Result<ValidationOutcome> {
        if handle::Handle::global().is_exiting() {
            return Ok(ValidationOutcome::Skipped {
                reason: ValidationSkipReason::Exiting,
            });
        }

        if !force && !self.should_update_config() {
            logging!(debug, Type::Core, "Skipping config update due to debounce");
            return Ok(ValidationOutcome::Skipped {
                reason: ValidationSkipReason::Debounced,
            });
        }

        if force {
            self.set_last_update(Instant::now());
        }

        self.perform_config_update(permit).await
    }

    pub async fn update_config_checked(&self) -> Result<()> {
        let outcome = self.update_config_forced().await?;
        if outcome.is_valid() {
            Ok(())
        } else {
            Err(anyhow!("{outcome}"))
        }
    }

    fn should_update_config(&self) -> bool {
        let now = Instant::now();
        let last = self.get_last_update();

        if let Some(last_time) = last
            && now.duration_since(*last_time) < timing::CONFIG_UPDATE_DEBOUNCE
        {
            return false;
        }

        self.set_last_update(now);
        true
    }

    async fn perform_config_update(&self, permit: &ConfigUpdatePermit<'_>) -> Result<ValidationOutcome> {
        if let Err(err) = Config::generate().await {
            let message: String = err.to_string().into();
            Config::runtime().await.discard();
            return Ok(ValidationOutcome::invalid_from_message(message));
        }

        // 乐观更新：跳过 mihomo -t 子进程验证，直接生成配置文件并热重载
        // 验证由增强管线保证，如果配置无效则热重载会失败并触发核心重启
        let run_path = match Config::generate_file(ConfigType::Run).await {
            Ok(path) => path,
            Err(err) => {
                Config::runtime().await.discard();
                return Err(err);
            }
        };
        if let Err(err) = self.apply_config(run_path, permit).await {
            Config::runtime().await.discard();
            return Err(err);
        }
        Ok(ValidationOutcome::Valid)
    }

    pub(crate) async fn update_runtime_config<F>(&self, f: F) -> Result<ValidationOutcome>
    where
        F: FnOnce(&mut IRuntime),
    {
        let Some(permit) = self.try_acquire_config_update() else {
            logging!(info, Type::Core, "Configuration update is already running");
            return Ok(ValidationOutcome::Busy);
        };

        self.update_runtime_config_with_permit(&permit, f).await
    }

    pub(crate) async fn update_runtime_config_with_permit<F>(
        &self,
        permit: &ConfigUpdatePermit<'_>,
        f: F,
    ) -> Result<ValidationOutcome>
    where
        F: FnOnce(&mut IRuntime),
    {
        debug_assert!(std::ptr::eq(permit.manager, self));

        Config::runtime().await.edit_draft(f);
        self.apply_generate_config_inner(permit).await
    }

    async fn apply_generate_config_inner(&self, permit: &ConfigUpdatePermit<'_>) -> Result<ValidationOutcome> {
        match CoreConfigValidator::global().validate_config_outcome().await {
            Ok(outcome) if outcome.is_valid() => {
                let result = async {
                    let run_path = Config::generate_file(ConfigType::Run).await?;
                    self.apply_config(run_path, permit).await?;
                    Ok(ValidationOutcome::Valid)
                }
                .await;
                if result.is_err() {
                    Config::runtime().await.discard();
                }
                result
            }
            Ok(outcome) => {
                Config::runtime().await.discard();
                Ok(outcome)
            }
            Err(e) => {
                Config::runtime().await.discard();
                Err(e)
            }
        }
    }

    async fn apply_config(&self, path: PathBuf, permit: &ConfigUpdatePermit<'_>) -> Result<()> {
        let path = dirs::path_to_str(&path)?;
        match self.reload_config(path).await {
            Ok(_) => {
                Config::runtime().await.apply();
                logging!(info, Type::Core, "Configuration applied");
                Ok(())
            }
            Err(err) => {
                logging!(
                    warn,
                    Type::Core,
                    "Failed to apply configuration by mihomo api, restart core to apply it, error msg: {err}"
                );
                match self.restart_core_with_permit(permit).await {
                    Ok(_) => {
                        Config::runtime().await.apply();
                        logging!(info, Type::Core, "Configuration applied after restart");
                        Ok(())
                    }
                    Err(err) => {
                        logging!(error, Type::Core, "Failed to restart core: {}", err);
                        Config::runtime().await.discard();
                        match self.restore_committed_runtime(permit).await {
                            Ok(()) => Err(anyhow!("Failed to apply config: {}", err)),
                            Err(rollback_err) => Err(anyhow!(
                                "Failed to apply config: {}; failed to restore previous runtime config: {}",
                                err,
                                rollback_err
                            )),
                        }
                    }
                }
            }
        }
    }

    async fn reload_config(&self, path: &str) -> Result<(), MihomoError> {
        handle::Handle::mihomo().await.reload_config(true, path).await
    }

    async fn restore_committed_runtime(&self, permit: &ConfigUpdatePermit<'_>) -> Result<()> {
        let rollback_path = Config::generate_file(ConfigType::Run).await?;
        self.restart_core_with_permit(permit).await?;
        logging!(
            warn,
            Type::Core,
            "Restored previous runtime config after failed restart"
        );
        logging!(
            debug,
            Type::Core,
            "Restored runtime config file: {}",
            rollback_path.display()
        );
        Ok(())
    }
}
