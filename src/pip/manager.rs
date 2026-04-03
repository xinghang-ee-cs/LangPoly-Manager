//! Pip 包管理器实现。
//!
//! 本模块提供 Python 包管理功能，封装 `pip` 命令的安装、卸载、升级和列表操作。
//! 通过当前激活的 Python 环境中的 pip 可执行文件执行操作。
//!
//! 核心类型：
//! - `PipManager`: Pip 包管理器，负责包的完整生命周期管理
//!
//! 主要功能：
//! 1. **安装包** (`install`): 从 PyPI 安装指定包及依赖
//!    - 支持精确版本（如 `"requests==2.28.0"`）
//!    - 支持版本范围（如 `"django>=4.0"`）
//!    - 支持 extras（如 `"fastapi[all]"`）
//! 2. **卸载包** (`uninstall`): 移除已安装的包
//!    - 自动确认，无需交互
//!    - 同时移除依赖（如果不再被其他包需要）
//! 3. **升级包** (`upgrade`): 更新包到最新版本
//!    - 使用 `--upgrade` 标志
//!    - 同时升级依赖
//! 4. **列出包** (`list`): 返回所有已安装包的名称列表
//!
//! 执行环境：
//! - 使用当前激活的 Python 环境中的 `pip` 可执行文件
//! - 通过 `PythonVersionManager::current_python_executable()` 获取 Python 解释器路径
//! - Pip 命令格式：`<python_exe> -m pip <subcommand> <args>`
//!
//! 进度显示：
//! - 安装/卸载/升级操作显示 "🌙 月亮" 进度指示器
//! - 使用 `indicatif::ProgressBar` 实现不确定进度的动画
//! - 列表操作无进度显示（快速完成）
//!
//! 错误处理：
//! - Python 可执行文件不存在：返回 `anyhow::Error`
//! - pip 命令执行失败：返回 `anyhow::Error`，包含命令 stderr 输出
//! - 包不存在（卸载/升级）：返回 `anyhow::Error`，提示包名
//!
//! 与 PythonService 集成：
//! - 通过 `PythonService::handle_pip_command` 暴露给 CLI
//! - 支持 `pip install`、`pip uninstall`、`pip upgrade`、`pip list` 子命令
//!
//! 测试：
//! - 模块内 `mod tests` 包含命令执行和输出解析测试
//! - 集成测试验证 pip 与 Python 环境集成

use crate::config::Config;
use crate::utils::executor::CommandExecutor;
use crate::utils::progress::moon_spinner_style;
use anyhow::Result;
use indicatif::ProgressBar;
use std::time::Duration;

/// Pip 包管理器
pub struct PipManager {
    executor: CommandExecutor,
}

impl PipManager {
    /// 创建 Pip 包管理器，并确保运行所需目录存在。
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
            super::sanitize_terminal_text(package)
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
            super::sanitize_terminal_text(package)
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
            super::sanitize_terminal_text(package)
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
            .execute_with_output_async(&python_exe, &["-m", "pip", "list", "--format=json"])
            .await?;

        let packages: Vec<serde_json::Value> = serde_json::from_str(&output)?;

        let mut result = Vec::new();
        for pkg in packages {
            let Some(name) = pkg.get("name").and_then(|value| value.as_str()) else {
                continue;
            };
            let Some(version) = pkg.get("version").and_then(|value| value.as_str()) else {
                continue;
            };
            result.push(format!("{name}=={version}"));
        }

        result.sort();
        Ok(result)
    }

    /// 获取当前 Python 可执行文件路径
    fn get_python_exe(&self) -> Result<std::path::PathBuf> {
        super::resolve_current_python_executable(
            "还没有选择 Python 版本，请先执行: meetai runtime use python <version>",
        )
    }
}
