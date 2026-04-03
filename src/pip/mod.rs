//! Pip 包管理模块。
//!
//! 本模块提供 Pip 命令的分发和包管理功能，封装 `pip install/uninstall/upgrade/list` 操作。
//! 通过 `PipManager` 类型暴露给 CLI 和内部调用者。
//!
//! 子模块：
//! - `manager`: `PipManager` 实现，负责包的完整生命周期
//! - `version`: `PipVersionManager` 实现，负责 pip 版本查询
//!
//! 主要函数：
//! - `handle_pip_command`: 处理 `meetai pip <subcommand>` 命令
//! - `sanitize_terminal_text`: 清理终端控制字符，防止日志污染
//! - `resolve_current_python_executable`: 解析当前 Python 可执行文件路径
//!
//! 命令路由：
//! | PipAction | 处理函数 |
//! |-----------|----------|
//! | `Install(package)` | `PipManager::install()` |
//! | `Uninstall(package)` | `PipManager::uninstall()` |
//! | `Upgrade(package)` | `PipManager::upgrade()` |
//! | `List` | `PipManager::list()` |
//!
//! 设计说明：
//! - Pip 版本与 Python 环境**强绑定**，不独立管理多个 pip 版本
//! - 所有 pip 操作使用**当前激活**的 Python 环境
//! - 命令格式：`<python_exe> -m pip <subcommand> <args>`
//! - 输出包含进度条（安装/卸载/升级）或简单列表（list）
//!
//! 错误处理：
//! - Python 环境未激活：返回 `anyhow::Error`，提示运行 `meetai python use <version>`
//! - 包不存在（卸载/升级）：返回 `anyhow::Error`，包含包名
//! - 网络错误（安装）：返回 `anyhow::Error`，包含 stderr 输出
//!
//! 与 PythonService 集成：
//! - `PythonService::handle_pip_command` 调用 `handle_pip_command`
//! - 支持 `meetai pip install <package>` 等子命令
//!
//! 测试：
//! - 模块内 `mod tests` 包含命令解析和文本清理测试

pub mod manager;
pub mod version;

pub use manager::PipManager;

use crate::cli::{PipAction, PipArgs};
use crate::python::version::PythonVersionManager;
use crate::utils::validator::Validator;
use anyhow::Result;
use std::path::PathBuf;

pub(super) fn sanitize_terminal_text(raw: &str) -> String {
    raw.chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect()
}

pub(super) fn resolve_current_python_executable(
    missing_selection_message: &'static str,
) -> Result<PathBuf> {
    let version_manager = PythonVersionManager::new()?;
    version_manager.current_python_executable(missing_selection_message)
}

/// 处理 Pip 相关命令
pub async fn handle_pip_command(args: PipArgs) -> Result<()> {
    let validator = Validator::new();

    match args.action {
        PipAction::Install { package, version } => {
            validator.validate_pip_package_name(&package)?;
            let manager = PipManager::new()?;
            let package_spec = if let Some(v) = version {
                validator.validate_pip_pin_version(&v)?;
                format!("{}=={}", package, v)
            } else {
                package
            };
            let display_spec = sanitize_terminal_text(&package_spec);
            manager.install(&package_spec).await?;
            println!("✅ {} 安装成功", display_spec);
            println!("  - 查看已安装的包: meetai pip list");
        }
        PipAction::Uninstall { package } => {
            validator.validate_pip_package_name(&package)?;
            let manager = PipManager::new()?;
            manager.uninstall(&package).await?;
            println!("✅ {} 已卸载", sanitize_terminal_text(&package));
            println!("  - 查看已安装的包: meetai pip list");
        }
        PipAction::Upgrade { package } => {
            validator.validate_pip_package_name(&package)?;
            let manager = PipManager::new()?;
            manager.upgrade(&package).await?;
            println!("✅ {} 已升级到最新版本", sanitize_terminal_text(&package));
            println!("  - 查看版本: meetai pip list");
        }
        PipAction::List => {
            let manager = PipManager::new()?;
            let packages = manager.list().await?;
            if packages.is_empty() {
                println!("还没有安装任何包，来安装第一个吧！");
                println!("  meetai pip install <包名>   # 安装第一个包");
            } else {
                println!("📦 已安装的包（共 {} 个）:", packages.len());
                for pkg in packages {
                    println!("  - {}", pkg);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_terminal_text_replaces_control_characters() {
        let sanitized = sanitize_terminal_text("pip\nlist\t\x00ok");
        assert_eq!(sanitized, "pip list  ok");
    }
}
