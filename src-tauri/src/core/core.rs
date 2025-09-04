use crate::config::*;
use crate::core::{clash_api, handle, service};
use crate::core::tray::Tray;
use crate::log_err;
use crate::utils::{dirs, help};
use anyhow::{bail, Result};
use once_cell::sync::OnceCell;
use serde_yaml::Mapping;
use std::{sync::Arc, time::Duration};
use tauri_plugin_shell::ShellExt;
use tokio::sync::Mutex;
use tokio::time::sleep;
use std::path::PathBuf;

#[derive(Debug)]
pub struct CoreManager {
    running: Arc<Mutex<bool>>,
}

impl CoreManager {
    pub fn global() -> &'static CoreManager {
        static CORE_MANAGER: OnceCell<CoreManager> = OnceCell::new();
        CORE_MANAGER.get_or_init(|| CoreManager {
            running: Arc::new(Mutex::new(false)),
        })
    }

    pub async fn init(&self) -> Result<()> {
        log::trace!("run core start");
        // 启动clash
        log_err!(Self::global().start_core().await);
        log::trace!("run core end");
        Ok(())
    }

    /// 停止核心运行
    pub async fn stop_core(&self) -> Result<()> {
        let mut running = self.running.lock().await;

        if !*running {
            log::debug!("core is not running");
            return Ok(());
        }

        // 关闭tun模式
        let mut disable = Mapping::new();
        let mut tun = Mapping::new();
        tun.insert("enable".into(), false.into());
        disable.insert("tun".into(), tun.into());
        log::debug!(target: "app", "disable tun mode");
        log_err!(clash_api::patch_configs(&disable).await);

        // 服务模式
        if service::check_service().await.is_ok() {
            log::info!(target: "app", "stop the core by service");
            service::stop_core_by_service().await?;
        }
        *running = false;
        Ok(())
    }

    /// 启动核心
    pub async fn start_core(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        if *running {
            log::info!("core is running");
            return Ok(());
        }

        let config_path = Config::generate_file(ConfigType::Run)?;

        // 服务模式
        if service::check_service().await.is_ok() {
            log::info!(target: "app", "try to run core in service mode");
            service::run_core_by_service(&config_path).await?;
        } else {
            // 侧载模式（无服务时直接启动核心进程）
            log::warn!(target: "app", "service unavailable, start core by sidecar");
            Self::run_core_by_sidecar(&config_path).await?;
        }
        // 流量订阅
        #[cfg(target_os = "macos")]
        log_err!(Tray::global().subscribe_traffic().await);

        *running = true;

        Ok(())
    }

    /// 无服务时，直接以子进程方式启动核心
    async fn run_core_by_sidecar(config_file: &PathBuf) -> Result<()> {
        let clash_core = { Config::verge().latest().clash_core.clone() };
        let clash_core = clash_core.unwrap_or("verge-mihomo".into());

        let app_handle = handle::Handle::global().app_handle().unwrap();
        let config_dir = crate::utils::dirs::app_home_dir()?;
        let config_dir = crate::utils::dirs::path_to_str(&config_dir)?;
        let config_file = crate::utils::dirs::path_to_str(config_file)?;

        log::info!(target:"app", "start core by sidecar: {} -d {} -f {}", clash_core, config_dir, config_file);

        let _child = app_handle
            .shell()
            .sidecar(clash_core)?
            .args(["-d", config_dir, "-f", config_file])
            .spawn()?;

        Ok(())
    }

    /// 重启内核
    pub async fn restart_core(&self) -> Result<()> {
        // 重新启动app
        self.stop_core().await?;
        self.start_core().await?;
        Ok(())
    }

    /// 使用默认配置
    pub async fn use_default_config(&self, msg_type: &str, msg_content: &str) -> Result<()> {
        let runtime_path = dirs::app_home_dir()?.join(RUNTIME_CONFIG);
        *Config::runtime().draft() = IRuntime {
            config: Some(Config::clash().latest().0.clone()),
            exists_keys: vec![],
            chain_logs: Default::default(),
        };
        help::save_yaml(
            &runtime_path,
            &Config::clash().latest().0,
            Some("# Clash Verge Runtime"),
        )?;
        handle::Handle::notice_message(msg_type, msg_content);
        Ok(())
    }

    /// 切换核心
    pub async fn change_core(&self, clash_core: Option<String>) -> Result<()> {
        let clash_core = clash_core.ok_or(anyhow::anyhow!("clash core is null"))?;
        const CLASH_CORES: [&str; 2] = ["verge-mihomo", "verge-mihomo-alpha"];

        if !CLASH_CORES.contains(&clash_core.as_str()) {
            bail!("invalid clash core name \"{clash_core}\"");
        }

        log::info!(target: "app", "change core to `{clash_core}`");
        
        // 1. 先更新内核配置（但不应用）
        Config::verge().draft().clash_core = Some(clash_core);
        
        // 2. 使用新内核验证配置
        println!("[切换内核] 使用新内核验证配置");
        match self.validate_config().await {
            Ok((true, _)) => {
                println!("[切换内核] 配置验证通过，开始切换内核");
                // 3. 验证通过后，应用内核配置并重启
                Config::verge().apply();
                log_err!(Config::verge().latest().save_file());
                
                match self.restart_core().await {
                    Ok(_) => {
                        println!("[切换内核] 内核切换成功");
                        Config::runtime().apply();
                        Ok(())
                    }
                    Err(err) => {
                        println!("[切换内核] 内核切换失败: {}", err);
                        Config::verge().discard();
                        Config::runtime().discard();
                        Err(err)
                    }
                }
            }
            Ok((false, error_msg)) => {
                println!("[切换内核] 配置验证失败: {}", error_msg);
                // 使用默认配置并继续切换内核
                self.use_default_config("config_validate::core_change", &error_msg).await?;
                Config::verge().apply();
                log_err!(Config::verge().latest().save_file());
                
                match self.restart_core().await {
                    Ok(_) => {
                        println!("[切换内核] 内核切换成功（使用默认配置）");
                        Ok(())
                    }
                    Err(err) => {
                        println!("[切换内核] 内核切换失败: {}", err);
                        Config::verge().discard();
                        Err(err)
                    }
                }
            }
            Err(err) => {
                println!("[切换内核] 验证过程发生错误: {}", err);
                Config::verge().discard();
                Err(err)
            }
        }
    }

    /// 内部验证配置文件的实现
    async fn validate_config_internal(&self, config_path: &str) -> Result<(bool, String)> {
        println!("[core配置验证] 开始验证配置文件: {}", config_path);
        
        let clash_core = { Config::verge().latest().clash_core.clone() };
        let clash_core = clash_core.unwrap_or("verge-mihomo".into());
        println!("[core配置验证] 使用内核: {}", clash_core);
        
        let app_handle = handle::Handle::global().app_handle().unwrap();
        let test_dir = dirs::app_home_dir()?.join("test");
        let test_dir = dirs::path_to_str(&test_dir)?;
        println!("[core配置验证] 测试目录: {}", test_dir);

        // 使用子进程运行clash验证配置
        println!("[core配置验证] 运行子进程验证配置");
        let output = app_handle
            .shell()
            .sidecar(clash_core)?
            .args(["-t", "-d", test_dir, "-f", config_path])
            .output()
            .await?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // 检查进程退出状态和错误输出
        let error_keywords = ["FATA", "fatal", "Parse config error", "level=fatal"];
        let has_error = !output.status.success() || error_keywords.iter().any(|&kw| stderr.contains(kw));
        
        println!("\n[core配置验证] -------- 验证结果 --------");
        println!("[core配置验证] 进程退出状态: {:?}", output.status);
    
        if !stderr.is_empty() {
            println!("[core配置验证] stderr输出:\n{}", stderr);
        }
        if !stdout.is_empty() {
            println!("[core配置验证] stdout输出:\n{}", stdout);
        }

        if has_error {
            println!("[core配置验证] 发现错误，开始处理错误信息");
            let error_msg = if !stdout.is_empty() {
                stdout.to_string()
            } else if !stderr.is_empty() {
                stderr.to_string()
            } else if let Some(code) = output.status.code() {
                format!("验证进程异常退出，退出码: {}", code)
            } else {
                "验证进程被终止".to_string()
            };

            println!("[core配置验证] -------- 验证结束 --------\n");
            Ok((false, error_msg))  // 返回错误消息给调用者处理
        } else {
            println!("[core配置验证] 验证成功");
            println!("[core配置验证] -------- 验证结束 --------\n");
            Ok((true, String::new()))
        }
    }

    /// 验证运行时配置
    pub async fn validate_config(&self) -> Result<(bool, String)> {
        let config_path = Config::generate_file(ConfigType::Check)?;
        let config_path = dirs::path_to_str(&config_path)?;
        self.validate_config_internal(config_path).await
    }

    /// 验证指定的配置文件
    pub async fn validate_config_file(&self, config_path: &str) -> Result<(bool, String)> {
        // 检查文件是否存在
        if !std::path::Path::new(config_path).exists() {
            let error_msg = format!("File not found: {}", config_path);
            //handle::Handle::notice_message("config_validate::file_not_found", &error_msg);
            return Ok((false, error_msg));
        }
        
        // 检查是否为脚本文件
        let is_script = if config_path.ends_with(".js") {
            true
        } else {
            match self.is_script_file(config_path) {
                Ok(result) => result,
                Err(err) => {
                    // 如果无法确定文件类型，尝试使用Clash内核验证
                    log::warn!(target: "app", "无法确定文件类型: {}, 错误: {}", config_path, err);
                    return self.validate_config_internal(config_path).await;
                }
            }
        };
        
        if is_script {
            log::info!(target: "app", "检测到脚本文件，使用JavaScript验证: {}", config_path);
            return self.validate_script_file(config_path).await;
        }
        
        // 对YAML配置文件使用Clash内核验证
        log::info!(target: "app", "使用Clash内核验证配置文件: {}", config_path);
        self.validate_config_internal(config_path).await
    }

    /// 检查文件是否为脚本文件
    fn is_script_file(&self, path: &str) -> Result<bool> {
        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(err) => {
                log::warn!(target: "app", "无法读取文件以检测类型: {}, 错误: {}", path, err);
                return Err(anyhow::anyhow!("Failed to read file to detect type: {}", err));
            }
        };
        
        // 检查文件前几行是否包含JavaScript特征
        let first_lines = content.lines().take(5).collect::<String>();
        Ok(first_lines.contains("function") || 
           first_lines.contains("//") || 
           first_lines.contains("/*") ||
           first_lines.contains("import") ||
           first_lines.contains("export") ||
           first_lines.contains("const ") ||
           first_lines.contains("let "))
    }

    /// 验证脚本文件语法
    async fn validate_script_file(&self, path: &str) -> Result<(bool, String)> {
        // 读取脚本内容
        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(err) => {
                let error_msg = format!("Failed to read script file: {}", err);
                //handle::Handle::notice_message("config_validate::script_error", &error_msg);
                return Ok((false, error_msg));
            }
        };
        
        log::debug!(target: "app", "验证脚本文件: {}", path);
        
        // 使用boa引擎进行基本语法检查
        use boa_engine::{Context, Source};
        
        let mut context = Context::default();
        let result = context.eval(Source::from_bytes(&content));
        
        match result {
            Ok(_) => {
                log::debug!(target: "app", "脚本语法验证通过: {}", path);
                
                // 检查脚本是否包含main函数
                if !content.contains("function main") && !content.contains("const main") && !content.contains("let main") {
                    let error_msg = "Script must contain a main function";
                    log::warn!(target: "app", "脚本缺少main函数: {}", path);
                    //handle::Handle::notice_message("config_validate::script_missing_main", error_msg);
                    return Ok((false, error_msg.to_string()));
                }
                
                Ok((true, String::new()))
            },
            Err(err) => {
                let error_msg = format!("Script syntax error: {}", err);
                log::warn!(target: "app", "脚本语法错误: {}", err);
                //handle::Handle::notice_message("config_validate::script_syntax_error", &error_msg);
                Ok((false, error_msg))
            }
        }
    }

    /// 更新proxies等配置
    pub async fn update_config(&self) -> Result<(bool, String)> {
        println!("[core配置更新] 开始更新配置");
        
        // 1. 先生成新的配置内容
        println!("[core配置更新] 生成新的配置内容");
        Config::generate().await?;
        
        // 2. 生成临时文件并进行验证
        println!("[core配置更新] 生成临时配置文件用于验证");
        let temp_config = Config::generate_file(ConfigType::Check)?;
        let temp_config = dirs::path_to_str(&temp_config)?;
        println!("[core配置更新] 临时配置文件路径: {}", temp_config);

        // 3. 验证配置
        match self.validate_config().await {
            Ok((true, _)) => {
                println!("[core配置更新] 配置验证通过");
                // 4. 验证通过后，生成正式的运行时配置
                println!("[core配置更新] 生成运行时配置");
                let run_path = Config::generate_file(ConfigType::Run)?;
                let run_path = dirs::path_to_str(&run_path)?;

                // 5. 等待核心就绪并应用新配置
                println!("[core配置更新] 检查核心就绪状态");
                let mut ready = false;
                for attempt in 1..=20 {
                    match clash_api::check_ready().await {
                        Ok(_) => { 
                            println!("[core配置更新] 核心就绪 (检查第{}次)", attempt);
                            ready = true; 
                            break; 
                        }
                        Err(e) => {
                            if attempt % 5 == 0 { println!("[core配置更新] 核心未就绪({attempt}/20): {}", e); }
                            sleep(Duration::from_millis(250)).await
                        },
                    }
                }
                if !ready {
                    println!("[core配置更新] 核心未就绪，尝试重启核心");
                    if let Err(e) = Self::global().restart_core().await {
                        println!("[core配置更新] 重启核心失败: {}", e);
                    } else {
                        // 重启后再等待就绪
                        for attempt in 1..=30 {
                            match clash_api::check_ready().await {
                                Ok(_) => { println!("[core配置更新] 核心重启后就绪 (检查第{}次)", attempt); break; }
                                Err(e) => {
                                    if attempt % 5 == 0 { println!("[core配置更新] 重启后仍未就绪({attempt}/30): {}", e); }
                                    sleep(Duration::from_millis(250)).await
                                }
                            }
                        }
                    }
                }

                println!("[core配置更新] 应用新配置");
                for i in 0..3 {
                    match clash_api::put_configs(run_path).await {
                        Ok(_) => {
                            println!("[core配置更新] 配置应用成功");
                            Config::runtime().apply();
                            return Ok((true, String::new()));
                        }
                        Err(err) => {
                            if i < 2 {
                                println!("[core配置更新] 第{}次重试应用配置", i + 1);
                                log::info!(target: "app", "{err}");
                                sleep(Duration::from_millis(100)).await;
                            } else {
                                println!("[core配置更新] 配置应用失败: {}", err);
                                Config::runtime().discard();
                                return Ok((false, err.to_string()));
                            }
                        }
                    }
                }
                Ok((true, String::new()))
            }
            Ok((false, error_msg)) => {
                println!("[core配置更新] 配置验证失败: {}", error_msg);
                Config::runtime().discard();
                Ok((false, error_msg))
            }
            Err(e) => {
                println!("[core配置更新] 验证过程发生错误: {}", e);
                Config::runtime().discard();
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    
    async fn create_test_script() -> Result<String> {
        let temp_dir = std::env::temp_dir();
        let script_path = temp_dir.join("test_script.js");
        let script_content = r#"
        // This is a test script
        function main(config) {
            console.log("Testing script");
            return config;
        }
        "#;
        
        fs::write(&script_path, script_content)?;
        Ok(script_path.to_string_lossy().to_string())
    }
    
    async fn create_invalid_script() -> Result<String> {
        let temp_dir = std::env::temp_dir();
        let script_path = temp_dir.join("invalid_script.js");
        let script_content = r#"
        // This is an invalid script
        function main(config {  // Missing closing parenthesis
            console.log("Testing script");
            return config;
        }
        "#;
        
        fs::write(&script_path, script_content)?;
        Ok(script_path.to_string_lossy().to_string())
    }
    
    async fn create_no_main_script() -> Result<String> {
        let temp_dir = std::env::temp_dir();
        let script_path = temp_dir.join("no_main_script.js");
        let script_content = r#"
        // This script has no main function
        function helper(config) {
            console.log("Testing script");
            return config;
        }
        "#;
        
        fs::write(&script_path, script_content)?;
        Ok(script_path.to_string_lossy().to_string())
    }
    
    #[tokio::test]
    async fn test_validate_script_file() -> Result<()> {
        let core_manager = CoreManager::global();
        
        // 测试有效脚本
        let script_path = create_test_script().await?;
        let result = core_manager.validate_config_file(&script_path).await?;
        assert!(result.0, "有效脚本应该通过验证");
        
        // 测试无效脚本
        let invalid_script_path = create_invalid_script().await?;
        let result = core_manager.validate_config_file(&invalid_script_path).await?;
        assert!(!result.0, "无效脚本不应该通过验证");
        assert!(result.1.contains("脚本语法错误"), "无效脚本应该返回语法错误");
        
        // 测试缺少main函数的脚本
        let no_main_script_path = create_no_main_script().await?;
        let result = core_manager.validate_config_file(&no_main_script_path).await?;
        assert!(!result.0, "缺少main函数的脚本不应该通过验证");
        assert!(result.1.contains("缺少main函数"), "应该提示缺少main函数");
        
        // 清理测试文件
        let _ = fs::remove_file(script_path);
        let _ = fs::remove_file(invalid_script_path);
        let _ = fs::remove_file(no_main_script_path);
        
        Ok(())
    }
}
