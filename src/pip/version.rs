//! Pip 版本查询与执行模块。
//!
//! 本模块提供 pip 可执行文件的定位和版本查询功能。
//! 注意：本模块**不管理多个 pip 版本**，而是使用当前 Python 环境中的 pip。
//!
//! 核心类型：
//! - `PipVersionManager`: Pip 版本管理器，负责查找 pip 可执行文件并查询版本
//!
//! 主要功能：
//! 1. **定位 pip 可执行文件** (`get_pip_exe`): 根据 Python 解释器路径构造 pip 命令
//!    - Windows: `<python_dir>\python.exe -m pip`
//!    - Unix: `<python_dir>/python -m pip`
//! 2. **查询版本** (`get_version_string`): 执行 `pip --version` 返回版本信息
//!
//! 设计说明：
//! - Pip 版本与 Python 版本**强绑定**，每个 Python 环境自带对应的 pip
//! - 不提供 pip 的安装/卸载/切换功能（这些由 Python 版本管理器间接提供）
//! - 仅作为 `PipManager` 的辅助类型，用于获取 pip 命令路径
//!
//! 错误处理：
//! - Python 可执行文件不存在：返回 `anyhow::Error`
//! - pip 命令执行失败：返回 `anyhow::Error`，包含命令 stderr
//!
//! 与 PipManager 的关系：
//! - `PipManager` 使用 `PipVersionManager` 获取 pip 命令路径
//! - `PipManager` 实现包管理操作（install/uninstall/upgrade/list）
//! - `PipVersionManager` 仅负责版本查询，不涉及包操作
//!
//! 测试：
//! - 模块内 `mod tests` 包含版本字符串解析和命令构造测试

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
