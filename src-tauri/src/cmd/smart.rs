use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow, bail};
use clash_verge_logging::{Type, logging};
use tokio::process::Command;
use tokio::time::timeout;

use crate::{
    cmd::{CmdResult, StringifyErr as _},
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

#[tauri::command]
pub async fn train_smart_model() -> CmdResult<String> {
    train_smart_model_inner().await.stringify_err()
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

    let (python, python_args) = detect_python().await?;

    logging!(info, Type::Core, "smart model training started with {rows} rows");
    run_training(&python, &python_args, &workspace).await?;

    let new_model = workspace.join("Model.bin");
    validate_model(&new_model)?;
    replace_model(&home_dir, &new_model)?;

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

/// 运行子进程并判断是否成功退出
async fn run_success(program: &str, prefix: &[&str], extra: &[&str]) -> bool {
    Command::new(program)
        .args(prefix.iter().chain(extra.iter()))
        .output()
        .await
        .is_ok_and(|output| output.status.success())
}

async fn run_training(python: &str, python_args: &[String], workspace: &Path) -> Result<()> {
    let future = Command::new(python)
        .args(python_args)
        .arg("train_flexible.py")
        .current_dir(workspace)
        .output();
    let output = timeout(TRAIN_TIMEOUT, future)
        .await
        .map_err(|_| anyhow!("训练超时（超过 30 分钟）"))?
        .context("启动训练进程失败")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut tail: Vec<&str> = stderr.lines().rev().take(15).collect();
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
