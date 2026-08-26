use crate::{cmd::smart::run_smart_training_exclusive, config::Config, process::AsyncHandler};
use anyhow::Result;
use chrono::Local;
use clash_verge_logging::{Type, logging, logging_error};
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use tokio::sync::watch;

const DEFAULT_INTERVAL_DAYS: u64 = 7;
const MIN_INTERVAL_DAYS: u64 = 1;
const MAX_INTERVAL_DAYS: u64 = 90;

/// 到期检查的轮询周期：间隔以天为单位，每小时核对一次足够
const TICK_SECS: u64 = 3600;
const DAY_SECS: i64 = 86400;

#[derive(Clone, Copy, Debug)]
struct SmartTrainSettings {
    enabled: bool,
    interval_days: u64,
}

impl SmartTrainSettings {
    async fn from_config() -> Self {
        let verge = Config::verge().await;
        Self::from_verge(&verge.latest_arc())
    }

    fn from_verge(verge: &crate::config::IVerge) -> Self {
        Self {
            enabled: verge.enable_smart_auto_train.unwrap_or(false),
            interval_days: verge
                .smart_auto_train_interval_days
                .unwrap_or(DEFAULT_INTERVAL_DAYS)
                .clamp(MIN_INTERVAL_DAYS, MAX_INTERVAL_DAYS),
        }
    }
}

impl Default for SmartTrainSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_days: DEFAULT_INTERVAL_DAYS,
        }
    }
}

pub struct SmartTrainManager {
    settings: Arc<RwLock<SmartTrainSettings>>,
    settings_tx: watch::Sender<SmartTrainSettings>,
    runner_started: AtomicBool,
}

impl SmartTrainManager {
    pub fn global() -> &'static Self {
        static INSTANCE: OnceCell<SmartTrainManager> = OnceCell::new();
        INSTANCE.get_or_init(|| {
            let (tx, _rx) = watch::channel(SmartTrainSettings::default());
            Self {
                settings: Arc::new(RwLock::new(SmartTrainSettings::default())),
                settings_tx: tx,
                runner_started: AtomicBool::new(false),
            }
        })
    }

    pub async fn init(&self) -> Result<()> {
        self.reload(false).await
    }

    /// 配置变更后由 patch_verge 调用，同步设置并唤醒调度循环；
    /// 用户把开关从关切到开时视为主动请求，立即训练一次
    pub async fn refresh_settings(&self) -> Result<()> {
        self.reload(true).await
    }

    async fn reload(&self, fire_on_enable: bool) -> Result<()> {
        let settings = SmartTrainSettings::from_config().await;
        let previous = *self.settings.read();
        {
            *self.settings.write() = settings;
        }
        let _ = self.settings_tx.send(settings);
        if settings.enabled {
            self.ensure_runner();
            if fire_on_enable && !previous.enabled {
                Self::train_now("enabled by user");
            }
        }
        Ok(())
    }

    /// 后台立即触发一次训练（与手动/定时共用互斥入口），结果仅记录日志
    fn train_now(reason: &'static str) {
        AsyncHandler::spawn(move || async move {
            logging!(info, Type::Core, "smart model training triggered ({reason})");
            match run_smart_training_exclusive().await {
                Ok(message) => {
                    logging!(info, Type::Core, "smart model training finished: {message}");
                }
                Err(err) => {
                    logging_error!(Type::Core, "smart model training failed: {:#}", err);
                }
            }
        });
    }

    fn ensure_runner(&self) {
        if self.runner_started.swap(true, Ordering::SeqCst) {
            return;
        }

        let mut rx = self.settings_tx.subscribe();
        AsyncHandler::spawn(move || async move {
            Self::run_scheduler(&mut rx).await;
        });
    }

    /// 常驻调度循环：开关关闭时挂起等待配置变化；开启时每 TICK 核对一次是否到期
    async fn run_scheduler(rx: &mut watch::Receiver<SmartTrainSettings>) {
        let mut current = *rx.borrow();
        loop {
            if !current.enabled {
                if rx.changed().await.is_err() {
                    break;
                }
                current = *rx.borrow();
                continue;
            }

            let sleeper = tokio::time::sleep(Duration::from_secs(TICK_SECS));
            tokio::pin!(sleeper);

            tokio::select! {
                _ = &mut sleeper => Self::train_if_due(current).await,
                changed = rx.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    current = *rx.borrow();
                }
            }
        }
    }

    async fn train_if_due(settings: SmartTrainSettings) {
        let last_at = Config::verge().await.latest_arc().smart_auto_train_last_at.unwrap_or(0);
        let due_secs = (settings.interval_days as i64).saturating_mul(DAY_SECS);
        let now = Local::now().timestamp();
        if now.saturating_sub(last_at) < due_secs {
            return;
        }

        logging!(
            info,
            Type::Core,
            "smart model due for scheduled training (every {}d)",
            settings.interval_days
        );
        Self::train_now("due by schedule");
    }
}
