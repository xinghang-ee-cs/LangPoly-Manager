use crate::config::Config;
use crate::utils::executor::CommandExecutor;
use anyhow::{Context, Result};
use semver::Version;
use std::path::PathBuf;

/// Pip 版本管理器
pub struct PipVersionManager {
    executor: CommandExecutor,
}

impl PipVersionManager {
    /// 创建 Pip 版本管理器，并确保运行所需目录存在。
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        config.ensure_dirs()?;

        Ok(Self {
            executor: CommandExecutor::new(),
        })
    }

    /// 获取当前 Pip 版本
    pub fn get_version(&self) -> Result<Version> {
        let python_exe = self.current_python_executable()?;

        // 执行 pip --version
        let output = self
            .executor
            .execute_with_output(&python_exe, &["-m", "pip", "--version"])?;

        // 解析版本号
        let version_str = output
            .split_whitespace()
            .nth(1)
            .context("Failed to parse pip version")?;

        Version::parse(version_str).context("Failed to parse version string")
    }

    /// 安装指定版本的 Pip
    pub async fn install(&self, version: &str) -> Result<()> {
        let python_exe = self.current_python_executable()?;

        // 使用 pip 安装指定版本
        let pip_spec = format!("pip=={}", version);
        self.executor
            .execute(&python_exe, &["-m", "pip", "install", &pip_spec])
            .await?;

        Ok(())
    }

    /// 升级 Pip
    pub async fn upgrade(&self) -> Result<()> {
        let python_exe = self.current_python_executable()?;

        // 升级 pip
        self.executor
            .execute(&python_exe, &["-m", "pip", "install", "--upgrade", "pip"])
            .await?;

        Ok(())
    }

    fn current_python_executable(&self) -> Result<PathBuf> {
        super::resolve_current_python_executable("No Python version selected")
    }
}
