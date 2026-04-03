//! Python 运行时模块。
//!
//! 本模块提供 Python 版本管理、虚拟环境管理和包管理的完整功能。
//! 是 MeetAI 工具中 Python 支持的主要入口点。
//!
//! 子模块：
//! - `version`: `PythonVersion` 和 `PythonVersionManager` 实现版本管理
//! - `installer`: `PythonInstaller` 实现下载、安装、验证
//! - `service`: `PythonService` 实现业务逻辑和命令分发
//! - `environment`: `VenvManager` 实现虚拟环境管理
//!
//! 主要函数：
//! - `handle_python_command`: 处理 `meetai python <subcommand>` 命令
//! - `handle_venv_command`: 处理 `meetai venv <subcommand>` 命令
//!
//! 公开类型：
//! - `PythonService`: 主服务，供 CLI 调用
//! - `PythonVersionManager`: 版本管理器
//! - `VenvManager`: 虚拟环境管理器
//! - `PythonInstaller`: 安装器（通常内部使用）
//!
//! 命令路由：
//! | PythonAction | 处理函数 |
//! |--------------|----------|
//! | `Install(version)` | `PythonService::install()` |
//! | `Use(version)` | `PythonService::set_current_version()` |
//! | `Uninstall(version)` | `PythonService::uninstall()` |
//! | `List` | `PythonService::list_installed()` |
//!
//! | VenvAction | 处理函数 |
//! |-----------|----------|
//! | `Create { name, target_dir }` | `VenvManager::create()` |
//! | `Activate { name }` | `VenvManager::activate()` |
//! | `List` | `VenvManager::list()` |
//!
//! 设计特点：
//! - 版本号使用自定义 `PythonVersion` 类型，支持比较和显示
//! - 通过 shims 目录实现版本切换，无需管理员权限
//! - Windows 平台支持完整自动安装（python.org 安装包）
//! - macOS/Linux 平台需用户手动安装，仅管理已有版本
//! - 虚拟环境支持跨平台（PowerShell / shell 激活脚本）
//!
//! 与其它模块集成：
//! - `crate::runtime::handle_runtime_command`: 统一 runtime 命令入口
//! - `crate::pip::handle_pip_command`: pip 命令依赖 Python 环境
//! - `crate::quick_install::QuickInstaller`: 一键安装包含 Python
//!
//! 测试：
//! - 模块内 `mod tests` 包含命令处理、错误消息测试

pub mod environment;
pub mod installer;
pub mod service;
pub mod version;

pub use environment::VenvManager;
pub use installer::PythonInstaller;
pub use service::PythonService;
pub(crate) use service::{
    install_python_for_surface, uninstall_python_for_surface, use_python_for_surface,
    PythonCommandSurface,
};
pub use version::PythonVersionManager;

use crate::cli::{PythonAction, PythonArgs, VenvAction, VenvArgs};
use crate::utils::progress::moon_spinner_style;
use crate::utils::validator::Validator;
use anyhow::Result;
use indicatif::ProgressBar;
use std::time::Duration;

/// 处理 Python 相关命令
pub async fn handle_python_command(args: PythonArgs) -> Result<()> {
    let validator = Validator::new();

    match args.action {
        PythonAction::List => {
            let service = PythonService::new()?;
            let versions = service.list_installed()?;
            if versions.is_empty() {
                println!("当前还没有安装任何 Python 版本。");
                println!("下一步你可以执行：");
                if cfg!(windows) {
                    println!("  meetai python install latest   # 安装最新稳定版");
                    println!("  meetai python install 3.13.2   # 安装指定版本");
                } else {
                    println!("  当前平台暂不支持自动安装。");
                    println!("  meetai runtime list python            # 查看 MeetAI 已管理版本");
                    println!("  meetai runtime use python <version>   # 切换到已管理版本");
                }
            } else {
                println!("已安装的 Python 版本（共 {} 个）：", versions.len());
                for version in versions {
                    println!("  - {}", version);
                }
                println!("下一步你可以执行：");
                println!("  meetai runtime use python <version>    # 切换当前版本");
                println!("  meetai runtime list python     # 统一入口查看");
            }
        }
        PythonAction::Install { version } => {
            validator.validate_python_install_version(&version)?;
            install_python_for_surface(&version, PythonCommandSurface::Python).await?;
        }
        PythonAction::Use { version } => {
            validator.validate_python_selected_version(&version)?;
            use_python_for_surface(&version, PythonCommandSurface::Python)?;
        }
        PythonAction::Uninstall { version } => {
            validator.validate_python_selected_version(&version)?;
            uninstall_python_for_surface(&version, PythonCommandSurface::Python).await?;
        }
    }
    Ok(())
}

/// 处理虚拟环境相关命令
pub async fn handle_venv_command(args: VenvArgs) -> Result<()> {
    match args.action {
        VenvAction::Create { name, target_dir } => {
            let manager = VenvManager::new()?;
            let pb = ProgressBar::new_spinner();
            pb.set_style(moon_spinner_style());
            pb.enable_steady_tick(Duration::from_millis(120));
            pb.set_message(format!("🐍 正在创建虚拟环境 {}...", name));
            manager.create(&name, &target_dir).await?;
            pb.finish_and_clear();
            println!("✅ 虚拟环境 {} 创建成功", name);
            println!("下一步你可以执行：");
            println!("  meetai venv activate {}   # 查看激活命令", name);
        }
        VenvAction::Activate { name } => {
            let manager = VenvManager::new()?;
            manager.activate(&name)?;
        }
        VenvAction::List => {
            let manager = VenvManager::new()?;
            let envs = manager.list()?;
            if envs.is_empty() {
                println!("当前还没有创建任何虚拟环境。");
                println!("  meetai venv create <名称>   # 创建一个新的虚拟环境");
            } else {
                println!("虚拟环境列表（共 {} 个）：", envs.len());
                for env in envs {
                    println!("  - {}", env);
                }
                println!("  meetai venv activate <名称>   # 获取激活命令");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn python_install_rejects_path_like_version() {
        let err = handle_python_command(PythonArgs {
            action: PythonAction::Install {
                version: r"..\3.13.2".to_string(),
            },
        })
        .await
        .expect_err("path-like version should be rejected before install");

        assert!(
            err.to_string().contains("Python 版本号格式不正确"),
            "unexpected error: {err:#}"
        );
    }

    #[tokio::test]
    async fn python_uninstall_rejects_latest() {
        let err = handle_python_command(PythonArgs {
            action: PythonAction::Uninstall {
                version: "latest".to_string(),
            },
        })
        .await
        .expect_err("latest should not be accepted for uninstall");

        assert!(
            err.to_string().contains("Python 版本号格式不正确"),
            "unexpected error: {err:#}"
        );
    }
}
