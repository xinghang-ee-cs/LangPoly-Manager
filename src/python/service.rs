use crate::python::{PythonInstaller, PythonVersionManager};
use crate::runtime::common::{
    EnsureShimsResult, GenericRuntimeService, RuntimeInstaller, RuntimeUninstaller, UsePathStatus,
    VersionManager,
};
use anyhow::{Context, Result};
use std::sync::Arc;

/// Python 领域服务，现在是一个薄包装器，委托给 [`GenericRuntimeService`]。
pub struct PythonService {
    runtime: GenericRuntimeService,
}

impl PythonService {
    /// 构建 `PythonService`，初始化版本管理器与安装器。
    pub fn new() -> Result<Self> {
        let version_manager: Arc<dyn VersionManager> = Arc::new(PythonVersionManager::new()?);
        let installer_impl = Arc::new(PythonInstaller::new()?);
        let installer: Arc<dyn RuntimeInstaller> = installer_impl.clone();
        let uninstaller: Arc<dyn RuntimeUninstaller> = installer_impl;
        Ok(Self {
            runtime: GenericRuntimeService::new(version_manager, installer, uninstaller),
        })
    }

    /// 列出已安装的 Python 版本。
    pub fn list_installed(&self) -> Result<Vec<String>> {
        self.runtime.list_installed()
    }

    /// 安装指定 Python 版本。
    pub async fn install(&self, version: &str) -> Result<String> {
        self.runtime.install(version).await
    }

    /// 卸载指定 Python 版本。
    pub async fn uninstall(&self, version: &str) -> Result<()> {
        self.runtime.uninstall(version).await
    }

    /// 设置当前激活版本。
    pub fn set_current_version(&self, version: &str) -> Result<()> {
        self.runtime.set_current_version(version)
    }

    /// 获取当前激活版本。
    pub fn get_current_version(&self) -> Result<Option<String>> {
        self.runtime.get_current_version()
    }

    /// 检测 `python use` 后的 PATH 状态。
    pub fn detect_use_path_status(&self, version: &str) -> Result<UsePathStatus> {
        self.runtime.detect_use_path_status(version)
    }

    /// 确保 shims 目录在 PATH 中。
    pub fn ensure_shims_in_path(&self) -> Result<EnsureShimsResult> {
        self.runtime.ensure_shims_in_path()
    }

    /// 激活版本（设置版本 + PATH 引导）。
    pub fn activate_version(&self, version: &str) -> Result<()> {
        self.runtime.activate_version(version)
    }
}

/// `python use` 命令的调用表面（用于错误消息定制）。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PythonCommandSurface {
    Python,
    Runtime,
}

/// 安装失败时的表面定制错误消息。
fn build_install_failure_message(surface: PythonCommandSurface, version: &str) -> String {
    match surface {
        PythonCommandSurface::Python => {
            if cfg!(windows) {
                format!(
                    "Python 安装失败（请求版本: {}）。\n下一步你可以执行：\n  meetai python list\n  meetai python install latest",
                    version
                )
            } else {
                format!(
                    "Python 安装失败（请求版本: {}）。\n当前平台暂不支持自动安装。\n下一步你可以执行：\n  meetai python list\n  meetai runtime list python\n  meetai runtime use python <version>",
                    version
                )
            }
        }
        PythonCommandSurface::Runtime => {
            if cfg!(windows) {
                format!(
                    "Python 安装失败（请求版本: {}）。\n下一步你可以执行：\n  meetai runtime list python\n  meetai runtime install python latest",
                    version
                )
            } else {
                format!(
                    "Python 安装失败（请求版本: {}）。\n当前平台暂不支持自动安装。\n下一步你可以执行：\n  meetai runtime list python\n  meetai runtime use python <version>",
                    version
                )
            }
        }
    }
}

/// 切换版本失败时的表面定制错误消息。
fn build_use_failure_message(surface: PythonCommandSurface, version: &str) -> String {
    match surface {
        PythonCommandSurface::Python => format!(
            "切换 Python 版本失败（目标版本: {}）。\n下一步你可以执行：\n  meetai python list\n  meetai runtime list python",
            version
        ),
        PythonCommandSurface::Runtime => format!(
            "Python 版本切换失败（目标版本: {}）。\n下一步你可以执行：\n  meetai runtime list python\n  meetai python list",
            version
        ),
    }
}

/// 卸载失败时的表面定制错误消息。
fn build_uninstall_failure_message(surface: PythonCommandSurface, version: &str) -> String {
    match surface {
        PythonCommandSurface::Python => format!(
            "卸载 Python 失败（目标版本: {}）。\n下一步你可以执行：\n  meetai python list\n  meetai runtime uninstall python {}",
            version, version
        ),
        PythonCommandSurface::Runtime => format!(
            "Python 卸载失败（目标版本: {}）。\n下一步你可以执行：\n  meetai runtime list python\n  meetai runtime uninstall python {}",
            version, version
        ),
    }
}

/// `meetai python install` 的统一入口（处理表面差异）。
pub(crate) async fn install_python_for_surface(
    version: &str,
    surface: PythonCommandSurface,
) -> Result<()> {
    let service = PythonService::new()?;
    let installed_version = service
        .install(version)
        .await
        .with_context(|| build_install_failure_message(surface, version))?;

    println!("Python {} 已准备就绪。", installed_version);
    println!("下一步你可以执行：");
    println!(
        "  meetai runtime use python {}   # 切换到该版本",
        installed_version
    );
    match surface {
        PythonCommandSurface::Python => {
            println!("  meetai python list      # 查看所有已安装版本");
        }
        PythonCommandSurface::Runtime => {
            println!("  meetai runtime list python      # 查看所有已安装版本");
        }
    }

    Ok(())
}

/// `meetai python use` 的统一入口（处理表面差异）。
pub(crate) fn use_python_for_surface(version: &str, surface: PythonCommandSurface) -> Result<()> {
    let service = PythonService::new()?;
    service
        .activate_version(version)
        .with_context(|| build_use_failure_message(surface, version))?;
    println!("✅ 已切换到 Python {}", version);
    match surface {
        PythonCommandSurface::Python => {
            println!("下一步你可以执行：");
            println!("  meetai python list   # 查看所有已安装版本");
        }
        PythonCommandSurface::Runtime => {
            println!("下一步你可以执行：");
            println!("  meetai runtime list python   # 查看所有已安装版本");
        }
    }
    Ok(())
}

/// `meetai python uninstall` 的统一入口（处理表面差异）。
pub(crate) async fn uninstall_python_for_surface(
    version: &str,
    surface: PythonCommandSurface,
) -> Result<()> {
    let service = PythonService::new()?;
    service
        .uninstall(version)
        .await
        .with_context(|| build_uninstall_failure_message(surface, version))?;
    println!("✅ Python {} 已卸载", version);
    println!("下一步你可以执行：");
    match surface {
        PythonCommandSurface::Python => {
            println!("  meetai python list                 # 查看剩余版本");
            if cfg!(windows) {
                println!("  meetai python install latest       # 安装最新版本");
            } else {
                println!("  meetai runtime list python         # 查看 MeetAI 已管理版本");
            }
        }
        PythonCommandSurface::Runtime => {
            println!("  meetai runtime list python              # 查看剩余版本");
            if cfg!(windows) {
                println!("  meetai runtime install python latest    # 安装最新版本");
            } else {
                println!("  meetai runtime use python <version>     # 切换到已管理版本");
            }
        }
    }
    Ok(())
}
