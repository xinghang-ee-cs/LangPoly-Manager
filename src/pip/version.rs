use crate::config::Config;
use crate::python::version::PythonVersionManager;
use crate::utils::executor::CommandExecutor;
use anyhow::{Context, Result};
use semver::Version;

/// Pip 版本管理器
pub struct PipVersionManager {
    executor: CommandExecutor,
}

impl PipVersionManager {
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        config.ensure_dirs()?;

        Ok(Self {
            executor: CommandExecutor::new(),
        })
    }

    /// 获取当前 Pip 版本
    pub fn get_version(&self) -> Result<Version> {
        // 获取当前 Python 版本
        let version_manager = PythonVersionManager::new()?;
        let current_version = version_manager
            .get_current_version()?
            .context("No Python version selected")?;

        // 获取 Python 路径
        let python_path = version_manager.get_python_path(&current_version)?;
        let python_exe = if cfg!(windows) {
            python_path.join("python.exe")
        } else {
            python_path.join("bin/python")
        };

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
        // 获取当前 Python 版本
        let version_manager = PythonVersionManager::new()?;
        let current_version = version_manager
            .get_current_version()?
            .context("No Python version selected")?;

        // 获取 Python 路径
        let python_path = version_manager.get_python_path(&current_version)?;
        let python_exe = if cfg!(windows) {
            python_path.join("python.exe")
        } else {
            python_path.join("bin/python")
        };

        // 使用 pip 安装指定版本
        let pip_spec = format!("pip=={}", version);
        self.executor
            .execute(&python_exe, &["-m", "pip", "install", &pip_spec])
            .await?;

        Ok(())
    }

    /// 升级 Pip
    pub async fn upgrade(&self) -> Result<()> {
        // 获取当前 Python 版本
        let version_manager = PythonVersionManager::new()?;
        let current_version = version_manager
            .get_current_version()?
            .context("No Python version selected")?;

        // 获取 Python 路径
        let python_path = version_manager.get_python_path(&current_version)?;
        let python_exe = if cfg!(windows) {
            python_path.join("python.exe")
        } else {
            python_path.join("bin/python")
        };

        // 升级 pip
        self.executor
            .execute(&python_exe, &["-m", "pip", "install", "--upgrade", "pip"])
            .await?;

        Ok(())
    }
}
