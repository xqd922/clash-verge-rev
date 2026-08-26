use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow, bail};
use clash_verge_logging::{Type, logging, logging_error};
use tokio::{
    io::{AsyncBufReadExt as _, BufReader},
    process::Command,
    sync::Mutex,
    time::timeout,
};

use crate::{
    cmd::{CmdResult, StringifyErr as _},
    config::Config,
    core::{CoreManager, handle::Handle},
    utils::dirs,
};

/// 训练超时上限
const TRAIN_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// 官方通用模型约 7.8MB，低于该值视为无效产物
const MIN_MODEL_SIZE_BYTES: u64 = 1024 * 1024;
/// 建议的最低数据量（行），低于该值仅提示不阻断
const MIN_RECOMMENDED_ROWS: usize = 2000;
/// 需要复制到工作目录的训练脚本
const TRAINER_FILES: [&str; 4] = ["transform.go", "go_parser.py", "train_flexible.py", "requirements.txt"];

/// 训练互斥锁：手动触发与后台定时训练共用，避免并发执行
static TRAIN_LOCK: Mutex<()> = Mutex::const_new(());

#[tauri::command]
pub async fn train_smart_model() -> CmdResult<String> {
    run_smart_training_exclusive().await.stringify_err()
}

/// 手动命令与后台定时任务共用的训练入口：串行化执行并在成功后记录时间戳
pub(crate) async fn run_smart_training_exclusive() -> Result<String> {
    let _guard = TRAIN_LOCK.lock().await;
    let message = train_smart_model_inner().await?;
    mark_smart_trained().await;
    Ok(message)
}

/// 记录本次成功训练时间，供自动训练判期使用（刚手动训完则顺延下个周期）
async fn mark_smart_trained() {
    Config::verge()
        .await
        .edit_draft(|d| d.smart_auto_train_last_at = Some(chrono::Local::now().timestamp()));
    Config::verge().await.apply();
    let data = Config::verge().await.data_arc();
    if let Err(e) = data.save_file().await {
        logging_error!(Type::Core, "Failed to save smart_auto_train_last_at: {:#?}", e);
    }
}

async fn train_smart_model_inner() -> Result<String> {
    let home_dir = dirs::app_home_dir()?;
    let csv_path = home_dir.join("smart_weight_data.csv");
    if !csv_path.exists() {
        bail!("尚未收集到训练数据（smart_weight_data.csv 不存在），请先使用 Smart 内核运行一段时间");
    }
    let rows = count_csv_rows(&csv_path)?;

    // 从打包资源复制脚本并快照一份数据，避免内核持续写入导致读到半行
    let resource_dir = dirs::app_resources_dir()?.join("smart-trainer");
    if !resource_dir.is_dir() {
        bail!("训练脚本资源缺失（smart-trainer）");
    }
    let workspace = trainer_workspace()?;
    for file in TRAINER_FILES {
        std::fs::copy(resource_dir.join(file), workspace.join(file))
            .with_context(|| format!("复制训练脚本失败: {file}"))?;
    }
    std::fs::copy(&csv_path, workspace.join("smart_weight_data.csv")).context("复制训练数据失败")?;
    Handle::smart_train_progress(format!("已快照 {rows} 行训练数据，正在检查 Python 环境"));

    let (python, python_args) = detect_python().await?;
    Handle::smart_train_progress(format!(
        "Python 环境就绪：{python}{}",
        if python_args.is_empty() {
            String::new()
        } else {
            format!(" {}", python_args.join(" "))
        }
    ));

    logging!(info, Type::Core, "smart model training started with {rows} rows");
    run_training(&python, &python_args, &workspace).await?;

    let new_model = workspace.join("Model.bin");
    validate_model(&new_model)?;
    replace_model(&home_dir, &new_model)?;
    Handle::smart_train_progress("新模型已写入，正在重启内核加载");

    // 自动重启内核加载新模型；失败不吞掉训练成果，仅提示手动重启
    let restarted = CoreManager::global().restart_core().await.is_ok();
    if restarted {
        Handle::refresh_clash();
    }

    let hint = if restarted {
        "已自动重启内核生效".to_string()
    } else {
        "内核自动重启失败，请手动重启内核生效".to_string()
    };
    let low_data = if rows < MIN_RECOMMENDED_ROWS {
        "（数据量偏少，建议积累更多后重新训练）"
    } else {
        ""
    };
    Ok(format!(
        "训练完成：共 {rows} 行数据{low_data}；已替换 Model.bin（旧模型备份为 Model.bin.bak），{hint}"
    ))
}

fn trainer_workspace() -> Result<PathBuf> {
    let dir = dirs::app_home_dir()?.join("smart-trainer");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 统计 CSV 行数（含表头），只需按字节计数，无需解析
fn count_csv_rows(path: &Path) -> Result<usize> {
    let content = std::fs::read(path).context("读取训练数据失败")?;
    Ok(content.iter().filter(|&&b| b == b'\n').count())
}

/// 训练依赖的导入自检语句
const DEP_CHECK: &str = "import lightgbm, pandas, sklearn, joblib";

/// 依次探测候选解释器：能运行且已装齐训练依赖的才可用，
/// 避免被 PATH 靠前但不可用的解释器（如未装 pip 的环境）卡住
async fn detect_python() -> Result<(String, Vec<String>)> {
    let requirements = trainer_workspace()?.join("requirements.txt");
    let candidates: [(&str, &[&str]); 2] = [("python", &[]), ("py", &["-3"])];
    let mut problems = Vec::new();
    for (program, prefix) in candidates {
        if !run_success(program, prefix, &["--version"]).await {
            problems.push(format!("{program}: 未检测到"));
            continue;
        }
        if run_success(program, prefix, &["-c", DEP_CHECK]).await {
            return Ok((program.into(), prefix.iter().map(|s| s.to_string()).collect()));
        }
        problems.push(format!(
            "{program}{}: 缺少训练依赖，请先执行 \"{program} {}\" -m pip install -r \"{}\"",
            prefix.join(" "),
            prefix.join(" "),
            requirements.display()
        ));
    }
    bail!("没有可用的 Python 训练环境（Python 3.11+）：\n{}", problems.join("\n"))
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 运行子进程并判断是否成功退出（不弹出控制台窗口）
async fn run_success(program: &str, prefix: &[&str], extra: &[&str]) -> bool {
    let mut cmd = Command::new(program);
    cmd.args(prefix.iter().chain(extra.iter()));
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.output().await.is_ok_and(|output| output.status.success())
}

async fn run_training(python: &str, python_args: &[String], workspace: &Path) -> Result<()> {
    let mut cmd = Command::new(python);
    cmd.args(python_args)
        .arg("train_flexible.py")
        .current_dir(workspace)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd.spawn().context("启动训练进程失败")?;
    let stdout = child.stdout.take().context("无法读取训练输出")?;
    let stderr = child.stderr.take().context("无法读取训练错误输出")?;

    // 并发收集 stderr，失败时截取尾部定位原因
    let stderr_task = tokio::spawn(async move {
        let mut text = String::new();
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            text.push_str(&line);
            text.push('\n');
        }
        text
    });

    // 逐行把训练日志转发到前端作为实时进度
    let drain_stdout = async {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim_end();
            if !line.is_empty() {
                Handle::smart_train_progress(line);
            }
        }
    };
    timeout(TRAIN_TIMEOUT, drain_stdout)
        .await
        .map_err(|_| anyhow!("训练超时（超过 30 分钟）"))?;

    let status = timeout(Duration::from_secs(60), child.wait())
        .await
        .map_err(|_| anyhow!("训练进程在输出结束后未退出"))?
        .context("等待训练进程退出失败")?;

    if !status.success() {
        let stderr_text = stderr_task.await.unwrap_or_default();
        let mut tail: Vec<&str> = stderr_text.lines().rev().take(15).collect();
        tail.reverse();
        bail!("训练脚本执行失败：\n{}", tail.join("\n"));
    }
    Ok(())
}

fn validate_model(model_path: &Path) -> Result<()> {
    let meta = model_path
        .metadata()
        .with_context(|| format!("训练产物不存在（{}），训练可能未成功", model_path.display()))?;
    if meta.len() < MIN_MODEL_SIZE_BYTES {
        bail!("训练产物过小（{} 字节），疑似无效模型", meta.len());
    }
    Ok(())
}

fn replace_model(home_dir: &Path, new_model: &Path) -> Result<()> {
    let target = home_dir.join("Model.bin");
    if target.exists() {
        let backup = home_dir.join("Model.bin.bak");
        std::fs::copy(&target, &backup).context("备份旧模型失败")?;
    }
    std::fs::copy(new_model, &target).context("替换 Model.bin 失败")?;
    Ok(())
}
