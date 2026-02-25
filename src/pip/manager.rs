use crate::config::Config;
use crate::python::version::PythonVersionManager;
use crate::utils::executor::CommandExecutor;
use crate::utils::progress::moon_spinner_style;
use anyhow::{Context, Result};
use indicatif::ProgressBar;
use std::time::Duration;

/// Pip 包管理器
pub struct PipManager {
    executor: CommandExecutor,
}

fn sanitize_terminal_text(raw: &str) -> String {
    raw.chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect()
}

impl PipManager {
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        config.ensure_dirs()?;

        Ok(Self {
            executor: CommandExecutor::new(),
        })
    }

    /// 安装包
    pub async fn install(&self, package: &str) -> Result<()> {
        let python_exe = self.get_python_exe()?;

        let pb = ProgressBar::new_spinner();
        pb.set_style(moon_spinner_style());
        pb.enable_steady_tick(Duration::from_millis(120));
        pb.set_message(format!(
            "📦 正在安装 {}...",
            sanitize_terminal_text(package)
        ));

        let result = self
            .executor
            .execute(&python_exe, &["-m", "pip", "install", package])
            .await;

        pb.finish_and_clear();
        result
    }

    /// 卸载包
    pub async fn uninstall(&self, package: &str) -> Result<()> {
        let python_exe = self.get_python_exe()?;

        let pb = ProgressBar::new_spinner();
        pb.set_style(moon_spinner_style());
        pb.enable_steady_tick(Duration::from_millis(120));
        pb.set_message(format!(
            "🗑️ 正在卸载 {}...",
            sanitize_terminal_text(package)
        ));

        let result = self
            .executor
            .execute(&python_exe, &["-m", "pip", "uninstall", "-y", package])
            .await;

        pb.finish_and_clear();
        result
    }

    /// 升级包
    pub async fn upgrade(&self, package: &str) -> Result<()> {
        let python_exe = self.get_python_exe()?;

        let pb = ProgressBar::new_spinner();
        pb.set_style(moon_spinner_style());
        pb.enable_steady_tick(Duration::from_millis(120));
        pb.set_message(format!(
            "⬆️ 正在升级 {}...",
            sanitize_terminal_text(package)
        ));

        let result = self
            .executor
            .execute(&python_exe, &["-m", "pip", "install", "--upgrade", package])
            .await;

        pb.finish_and_clear();
        result
    }

    /// 列出已安装的包
    pub async fn list(&self) -> Result<Vec<String>> {
        let python_exe = self.get_python_exe()?;

        let output = self
            .executor
            .execute_with_output(&python_exe, &["-m", "pip", "list", "--format=json"])?;

        let packages: Vec<serde_json::Value> = serde_json::from_str(&output)?;

        let mut result = Vec::new();
        for pkg in packages {
            if let (Some(name), Some(version)) = (pkg.get("name"), pkg.get("version")) {
                result.push(format!(
                    "{}=={}",
                    name.as_str().unwrap(),
                    version.as_str().unwrap()
                ));
            }
        }

        result.sort();
        Ok(result)
    }

    /// 获取当前 Python 可执行文件路径
    fn get_python_exe(&self) -> Result<std::path::PathBuf> {
        let version_manager = PythonVersionManager::new()?;
        let current_version = version_manager
            .get_current_version()?
            .context("还没有选择 Python 版本，请先执行: meetai runtime use python <version>")?;

        let python_path = version_manager.get_python_path(&current_version)?;

        let python_exe = if cfg!(windows) {
            python_path.join("python.exe")
        } else {
            python_path.join("bin/python")
        };

        Ok(python_exe)
    }
}
